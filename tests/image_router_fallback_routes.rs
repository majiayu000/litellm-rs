#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures::{mock_provider_config, route_policy_bootstrap_providers};
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use actix_web::{HttpMessage, dev::Service};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::core::net::ProviderEndpointAccess;
    use litellm_rs::core::pricing_service::{LiteLLMModelInfo, PricingUsage};
    use litellm_rs::server::{HttpServer as GatewayHttpServer, state::AppState};
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone)]
    struct MockImageState {
        paths: Arc<Mutex<Vec<String>>>,
        bodies: Arc<Mutex<Vec<Bytes>>>,
    }

    struct MockImageServer {
        base_url: String,
        paths: Arc<Mutex<Vec<String>>>,
        bodies: Arc<Mutex<Vec<Bytes>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockImageServer {
        async fn start() -> Self {
            let paths = Arc::new(Mutex::new(Vec::new()));
            let bodies = Arc::new(Mutex::new(Vec::new()));
            let state = MockImageState {
                paths: Arc::clone(&paths),
                bodies: Arc::clone(&bodies),
            };
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
            let address = listener.local_addr().expect("mock address");
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .route(
                        "/v1/images/generations",
                        web::post().to(mock_image_generation),
                    )
                    .route("/v1/images/edits", web::post().to(mock_image_edit))
                    .route(
                        "/v2beta/stable-image/edit/inpaint",
                        web::post().to(mock_image_edit),
                    )
            })
            .listen(listener)
            .expect("mock server should listen")
            .run();
            let handle = server.handle();
            let task = tokio::spawn(server);
            wait_for_server(address).await;

            Self {
                base_url: format!("http://{address}/v1"),
                paths,
                bodies,
                handle,
                task,
            }
        }

        fn paths(&self) -> Vec<String> {
            self.paths.lock().unwrap().clone()
        }

        fn bodies(&self) -> Vec<Bytes> {
            self.bodies.lock().unwrap().clone()
        }

        async fn stop(self) {
            self.handle.stop(false).await;
            let result = self.task.await.expect("mock server task should join");
            if let Err(error) = result {
                panic!("mock server should stop cleanly: {error}");
            }
        }
    }

    async fn wait_for_server(address: std::net::SocketAddr) {
        for _ in 0..20 {
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("mock server did not accept connections at {address}");
    }

    async fn mock_image_edit(
        state: web::Data<MockImageState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        let path = request.path().to_string();
        state.paths.lock().unwrap().push(path.clone());
        state.bodies.lock().unwrap().push(body);
        if path.starts_with("/v2beta/") {
            HttpResponse::Ok()
                .content_type("image/png")
                .body(Bytes::from_static(b"\x89PNG\r\n\x1a\n"))
        } else {
            HttpResponse::Ok().json(json!({
                "created": 1710000000,
                "data": [{ "url": "https://images.example.test/edit.png" }]
            }))
        }
    }

    async fn mock_image_generation(
        state: web::Data<MockImageState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        state.paths.lock().unwrap().push(request.path().to_string());
        state.bodies.lock().unwrap().push(body);
        HttpResponse::Ok().json(json!({
            "created": 1710000002,
            "data": [{ "url": "https://images.example.test/generated.png" }]
        }))
    }

    async fn build_test_state(
        providers: Vec<ProviderConfig>,
    ) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.providers = providers;

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
    }

    async fn build_route_policy_test_state(providers: Vec<ProviderConfig>) -> AppState {
        build_route_policy_test_state_with_pricing(providers, None).await
    }

    async fn build_route_policy_test_state_with_pricing(
        mut providers: Vec<ProviderConfig>,
        pricing: Option<HashMap<String, LiteLLMModelInfo>>,
    ) -> AppState {
        let pricing_file = pricing.map(|pricing| {
            let mut file = tempfile::NamedTempFile::new().expect("pricing tempfile");
            serde_json::to_writer(file.as_file_mut(), &pricing).expect("serialize pricing fixture");
            file
        });
        for provider in &mut providers {
            provider.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        }
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.providers = route_policy_bootstrap_providers(&providers);
        if let Some(file) = pricing_file.as_ref() {
            config.gateway.pricing.source = Some(file.path().to_string_lossy().into_owned());
        }
        let state = GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone();
        let mut runtime_config = state.config().as_ref().clone();
        runtime_config.gateway.providers = providers;
        state.config.store(runtime_config);
        state
    }

    fn api_key_with_allowed_models(allowed_models: &[&str]) -> ApiKey {
        let mut metadata = Metadata::new();
        metadata.set_extra(
            "__core_keys",
            json!({
                "permissions": {
                    "allowed_models": allowed_models,
                    "allowed_endpoints": [],
                    "custom_permissions": []
                }
            }),
        );

        ApiKey {
            metadata,
            name: "image-test-key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "sk-image".to_string(),
            user_id: None,
            team_id: None,
            permissions: Vec::new(),
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        }
    }

    fn image_provider(
        name: &str,
        provider_type: &str,
        base_url: &str,
        models: Vec<String>,
    ) -> ProviderConfig {
        mock_provider_config(name, provider_type, "sk-test", base_url, models)
    }

    fn openai_image_provider(name: &str, base_url: &str, models: Vec<String>) -> ProviderConfig {
        mock_provider_config(name, "openai", "sk-test", base_url, models)
    }

    fn openai_image_provider_with_mapping(
        name: &str,
        base_url: &str,
        alias: &str,
        upstream_model: &str,
    ) -> ProviderConfig {
        let mut provider = openai_image_provider(name, base_url, vec![alias.to_string()]);
        provider.settings.insert(
            "model_mappings".to_string(),
            json!({ alias: upstream_model }),
        );
        provider.settings.insert(
            "model_identity_mappings".to_string(),
            json!({ alias: {
                "capability_catalog_model": "gpt-image-1-mini",
                "pricing_model": upstream_model,
            }}),
        );
        provider
    }

    fn flat_image_model_info(output_cost_per_image: f64) -> LiteLLMModelInfo {
        flat_image_model_info_for_provider("openai", output_cost_per_image)
    }

    fn flat_image_model_info_for_provider(
        provider: &str,
        output_cost_per_image: f64,
    ) -> LiteLLMModelInfo {
        let mut extra = HashMap::new();
        extra.insert(
            "output_cost_per_image".to_string(),
            serde_json::Value::from(output_cost_per_image),
        );
        LiteLLMModelInfo {
            max_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "image_generation".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra,
        }
    }

    fn add_raw_image_alias_pricing(
        state: &litellm_rs::server::state::AppState,
        alias: &str,
        catalog_model: &str,
    ) {
        let (_, info) = state
            .pricing
            .get_model_info_for_provider("openai", catalog_model)
            .unwrap_or_else(|| panic!("catalog pricing should exist for {catalog_model}"));
        // OpenAI model_mappings are chat-only. Image transport sends the selected alias,
        // so the fixture must price that exact wire identity instead of borrowing the
        // mapped chat target accidentally.
        state.pricing.add_custom_model(alias.to_string(), info);
    }

    fn token_priced_image_model_info(input_cost_per_token: f64) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: Some(input_cost_per_token),
            output_cost_per_token: Some(0.0),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "openai".to_string(),
            mode: "image_generation".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        }
    }

    fn image_edit_multipart_body(boundary: &str) -> Vec<u8> {
        image_edit_multipart_body_for_model(boundary, "gpt-image-1-mini", 1)
    }

    fn image_edit_multipart_body_for_model(boundary: &str, model: &str, quantity: u32) -> Vec<u8> {
        let mut body = Vec::new();
        add_text_field(&mut body, boundary, "model", model);
        add_text_field(&mut body, boundary, "prompt", "make it lighter");
        add_text_field(&mut body, boundary, "n", &quantity.to_string());
        add_file_field(&mut body, boundary, "image", "input.png", b"png-bytes");
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    fn add_text_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    fn add_file_field(
        body: &mut Vec<u8>,
        boundary: &str,
        name: &str,
        filename: &str,
        content: &[u8],
    ) {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }

    #[tokio::test]
    async fn image_generation_resolves_runtime_alias_before_authz_and_upstream() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "image-alias",
            "gpt-image-1-mini",
        )])
        .await;
        add_raw_image_alias_pricing(&state, "image-alias", "gpt-image-1-mini");
        state
            .unified_router
            .add_model_alias("public-image", "image-alias")
            .expect("runtime image alias should install");
        let api_key = api_key_with_allowed_models(&["public-image"]);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<ApiKey>(api_key.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "model": "public-image",
                    "prompt": "make an icon",
                    "size": "1024x1024"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(mock.paths(), vec!["/v1/images/generations".to_string()]);
        mock.stop().await;
    }

    #[tokio::test]
    async fn image_generation_trims_model_before_authz_and_upstream() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "image-alias",
            "gpt-image-1-mini",
        )])
        .await;
        add_raw_image_alias_pricing(&state, "image-alias", "gpt-image-1-mini");
        let api_key = api_key_with_allowed_models(&["image-alias"]);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<ApiKey>(api_key.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "model": " image-alias ",
                    "prompt": "make an icon",
                    "size": "1024x1024"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(mock.paths(), vec!["/v1/images/generations".to_string()]);
        mock.stop().await;
    }

    #[tokio::test]
    async fn image_generation_rejects_api_key_disallowed_model_before_upstream() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "image-alias",
            "gpt-image-1-mini",
        )])
        .await;
        let api_key = api_key_with_allowed_models(&["gpt-4o"]);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<ApiKey>(api_key.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "model": "image-alias",
                    "prompt": "make an icon",
                    "size": "1024x1024"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert!(
            mock.paths().is_empty(),
            "image generation model authorization must happen before upstream"
        );
        mock.stop().await;
    }

    #[tokio::test]
    async fn image_generation_rejects_missing_model_before_upstream() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "image-alias",
            "gpt-image-1-mini",
        )])
        .await;
        let api_key = api_key_with_allowed_models(&["image-alias"]);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<ApiKey>(api_key.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "prompt": "make an icon",
                    "size": "1024x1024"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(
            mock.paths().is_empty(),
            "missing image generation model must fail before upstream"
        );
        mock.stop().await;
    }

    #[tokio::test]
    async fn image_generation_rejects_exhausted_provider_budget_before_upstream() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "image-alias",
            "gpt-image-1-mini",
        )])
        .await;
        add_raw_image_alias_pricing(&state, "image-alias", "gpt-image-1-mini");
        state.budget_limits.providers.set_provider_limit(
            "openai-primary",
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .record_spend("openai-primary", "image-alias", 0.01);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "model": "image-alias",
                    "prompt": "make an icon",
                    "size": "1024x1024"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(
            mock.paths().is_empty(),
            "image generation budget rejection must happen before upstream"
        );
        mock.stop().await;
    }

    #[tokio::test]
    async fn image_generation_records_provider_spend_after_success() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "image-alias",
            "gpt-image-1-mini",
        )])
        .await;
        add_raw_image_alias_pricing(&state, "image-alias", "gpt-image-1-mini");
        state.budget_limits.providers.set_provider_limit(
            "openai-primary",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let mut expected_usage = PricingUsage::new(3, 0);
        expected_usage.image_tokens = Some(1024);
        let expected_cost = state
            .pricing
            .calculate_loaded_usage_cost_for_provider("openai", "image-alias", &expected_usage)
            .expect("image generation pricing should be available")
            .total_cost;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "model": "image-alias",
                    "prompt": "make an icon",
                    "size": "1024x1024"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(mock.paths(), vec!["/v1/images/generations".to_string()]);
        let bodies = mock.bodies();
        let upstream_body: Value =
            serde_json::from_slice(&bodies[0]).expect("image request should be JSON");
        assert_eq!(
            upstream_body["model"], "image-alias",
            "chat-only model mapping must not change the image wire identity"
        );
        let spent = budget_limits
            .providers
            .get_provider_usage("openai-primary")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert!(
            (spent - expected_cost).abs() < f64::EPSILON,
            "successful image generation must record full image-token spend"
        );
        mock.stop().await;
    }

    #[tokio::test]
    async fn image_generation_records_flat_output_image_spend_after_success() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "flat-image-alias",
            "gpt-image-1-mini",
        )])
        .await;
        state
            .pricing
            .add_custom_model("gpt-image-1-mini".to_string(), flat_image_model_info(0.06));
        state.budget_limits.providers.set_provider_limit(
            "openai-primary",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "model": "flat-image-alias",
                    "prompt": "make an icon",
                    "size": "1024x1024",
                    "n": 2
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(mock.paths(), vec!["/v1/images/generations".to_string()]);
        let spent = budget_limits
            .providers
            .get_provider_usage("openai-primary")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert!((spent - 0.12).abs() < f64::EPSILON);
        mock.stop().await;
    }

    #[tokio::test]
    async fn image_generation_rejects_token_priced_image_model_without_image_price() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "token-image-alias",
            "gpt-image-1-mini",
        )])
        .await;
        state.pricing.add_custom_model(
            "gpt-image-1-mini".to_string(),
            token_priced_image_model_info(0.01),
        );
        state.budget_limits.providers.set_provider_limit(
            "openai-primary",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "model": "token-image-alias",
                    "prompt": "make an icon",
                    "size": "1024x1024"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(mock.paths().is_empty());
        let spent = budget_limits
            .providers
            .get_provider_usage("openai-primary")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert_eq!(spent, 0.0);
        mock.stop().await;
    }

    #[tokio::test]
    async fn image_generation_records_matching_flat_output_image_variant_spend() {
        let mock = MockImageServer::start().await;
        let state = build_test_state(vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "flat-variant-alias",
            "gpt-image-1-mini",
        )])
        .await;
        state.pricing.add_custom_model(
            "standard/512-x-512/gpt-image-1-mini".to_string(),
            flat_image_model_info(0.05),
        );
        state.pricing.add_custom_model(
            "hd/512-x-512/gpt-image-1-mini".to_string(),
            flat_image_model_info(0.10),
        );
        state.budget_limits.providers.set_provider_limit(
            "openai-primary",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/generations")
                .set_json(json!({
                    "model": "flat-variant-alias",
                    "prompt": "make an icon",
                    "size": "512x512",
                    "quality": "hd",
                    "n": 2
                }))
                .to_request(),
        )
        .await;

        if resp.status() != StatusCode::OK {
            let status = resp.status();
            let body = test::read_body(resp).await;
            panic!(
                "expected variant image generation to succeed, got {status}: {}",
                String::from_utf8_lossy(&body)
            );
        }
        assert_eq!(mock.paths(), vec!["/v1/images/generations".to_string()]);
        let spent = budget_limits
            .providers
            .get_provider_usage("openai-primary")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert!((spent - 0.20).abs() < f64::EPSILON);
        mock.stop().await;
    }

    #[path = "image_router_fallback_routes_edit_fallback_tests.rs"]
    mod edit_fallback_tests;
}
