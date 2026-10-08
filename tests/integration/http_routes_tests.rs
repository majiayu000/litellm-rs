//! HTTP integration tests for core API routes
//!
//! Tests the middleware stack against the actual route handlers using
//! actix-web's in-process test utilities.

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use crate::common::providers::mock_provider_config;
    use actix_web::http::StatusCode;
    use actix_web::{App, HttpResponse, HttpServer, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::integrations::{
        CallbackRuntime, IntegrationManager, IntegrationManagerConfig,
    };
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::core::traits::integration::{
        EmbeddingEndEvent, EmbeddingStartEvent, Integration, IntegrationResult, LlmEndEvent,
        LlmErrorEvent, LlmStartEvent,
    };
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::middleware::AuthMiddleware;
    use litellm_rs::server::routes;
    use litellm_rs::server::state::AppState;
    use litellm_rs::utils::auth::crypto::keys::{extract_api_key_prefix, hash_api_key};
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone)]
    enum RecordedEmbeddingCallback {
        Start(EmbeddingStartEvent),
        End(EmbeddingEndEvent),
    }

    struct EmbeddingCallbackRecorder {
        events: Arc<Mutex<Vec<RecordedEmbeddingCallback>>>,
    }

    #[async_trait::async_trait]
    impl Integration for EmbeddingCallbackRecorder {
        fn name(&self) -> &'static str {
            "embedding-route-recorder"
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn on_llm_start(&self, _event: &LlmStartEvent) -> IntegrationResult<()> {
            Ok(())
        }

        async fn on_llm_end(&self, _event: &LlmEndEvent) -> IntegrationResult<()> {
            Ok(())
        }

        async fn on_llm_error(&self, _event: &LlmErrorEvent) -> IntegrationResult<()> {
            Ok(())
        }

        async fn on_embedding_start(&self, event: &EmbeddingStartEvent) -> IntegrationResult<()> {
            self.events
                .lock()
                .expect("embedding callback events")
                .push(RecordedEmbeddingCallback::Start(event.clone()));
            Ok(())
        }

        async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
            self.events
                .lock()
                .expect("embedding callback events")
                .push(RecordedEmbeddingCallback::End(event.clone()));
            Ok(())
        }

        async fn flush(&self) -> IntegrationResult<()> {
            Ok(())
        }

        async fn shutdown(&self) -> IntegrationResult<()> {
            Ok(())
        }
    }

    async fn build_embedding_callback_runtime(
        events: Arc<Mutex<Vec<RecordedEmbeddingCallback>>>,
    ) -> CallbackRuntime {
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default().parallel(false),
        ));
        manager
            .register(Arc::new(EmbeddingCallbackRecorder { events }))
            .await;
        CallbackRuntime::new(manager, 8).expect("embedding callback runtime")
    }

    async fn build_state_with_config(mut config: Config) -> AppState {
        if config.gateway.providers.is_empty() {
            config.gateway.providers.push(ProviderConfig {
                name: "disabled-test-bootstrap".to_string(),
                provider_type: "openai".to_string(),
                api_key: "sk-test".to_string(),
                enabled: false,
                ..ProviderConfig::default()
            });
        }
        let server = match GatewayHttpServer::new(&config).await {
            Ok(server) => server,
            Err(err) => panic!("failed to build HTTP server for integration test: {err}"),
        };
        let state = server.state().clone();
        if let Err(err) = state.storage.migrate().await {
            panic!("failed to run in-memory DB migrations: {err}");
        }
        state
    }

    /// Build an AppState with auth enabled (both JWT and API key).
    async fn build_auth_enabled_state() -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = true;
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());

        build_state_with_config(config).await
    }

    /// Build an AppState with auth disabled.
    async fn build_auth_disabled_state() -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());

        build_state_with_config(config).await
    }

    async fn seed_readiness_api_key(state: &AppState) -> String {
        let raw_key = "gw-readiness-authenticated-fixture".to_string();
        let api_key = ApiKey {
            metadata: Metadata::new(),
            name: "readiness-test-key".to_string(),
            key_hash: hash_api_key(&raw_key, None),
            key_prefix: extract_api_key_prefix(&raw_key),
            user_id: None,
            team_id: None,
            permissions: Vec::new(),
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        };
        state
            .storage
            .db()
            .create_api_key(&api_key)
            .await
            .expect("readiness API key should persist");
        raw_key
    }

    async fn build_openai_alias_state(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers = vec![mock_provider_config(
            "mock-openai",
            "openai",
            "sk-test",
            base_url,
            vec!["text-embedding-3-small".to_string()],
        )];

        build_state_with_config(config).await
    }

    async fn build_openai_alias_state_with_cache(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.cache.enabled = true;
        config.gateway.providers = vec![mock_provider_config(
            "mock-openai",
            "openai",
            "sk-test",
            base_url,
            vec!["text-embedding-3-small".to_string()],
        )];

        build_state_with_config(config).await
    }

    async fn cache_state_without_routing_retries(state: AppState) -> AppState {
        use litellm_rs::core::router::{RouterConfig, UnifiedRouter};

        let config = state.config();
        let router = UnifiedRouter::from_gateway_config_with_aliases_and_pricing(
            &config.gateway.providers,
            Some(RouterConfig {
                num_retries: 0,
                enable_pre_call_checks: false,
                ..Default::default()
            }),
            &config.gateway.model_aliases,
            Arc::clone(&state.pricing),
        )
        .await
        .expect("cache admission router should initialize");
        let mut revision = state.pin_runtime().as_ref().clone();
        revision.unified_router = Arc::new(router);
        AppState::new_with_runtime(
            revision,
            state.auth.as_ref().clone(),
            state.storage.as_ref().clone(),
            Arc::clone(&state.pricing),
            Arc::clone(&state.budget_limits),
        )
    }

    async fn cache_admission_minute_guard() {
        let second = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            % 60;
        if second >= 40 {
            tokio::time::sleep(Duration::from_secs(60 - second)).await;
        }
    }

    fn set_cache_test_tpm(state: &AppState, model: &str, limit: u64) {
        let router = state.unified_router();
        let ids = router.get_deployments_for_model(model);
        assert_eq!(ids.len(), 1);
        let mut deployment = router.get_deployment(&ids[0]).unwrap().as_ref().clone();
        deployment.config.tpm_limit = Some(limit);
        router.add_deployment(deployment);
    }

    async fn build_openai_compatible_embeddings_state(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers = vec![mock_provider_config(
            "mock-openai-compatible",
            "openai_compatible",
            "sk-test",
            base_url,
            vec!["text-embedding-3-small".to_string()],
        )];

        build_state_with_config(config).await
    }

    async fn build_openai_compatible_image_state(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers = vec![mock_provider_config(
            "mock-openai-compatible",
            "openai_compatible",
            "sk-test",
            base_url,
            vec!["gpt-image-1-mini".to_string()],
        )];

        build_state_with_config(config).await
    }

    async fn build_openai_compatible_audio_state(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers = vec![mock_provider_config(
            "mock-openai-compatible",
            "openai_compatible",
            "sk-test",
            base_url,
            vec!["whisper-1".to_string(), "tts-1".to_string()],
        )];

        build_state_with_config(config).await
    }

    async fn mock_embeddings(
        captured_requests: web::Data<Arc<Mutex<Vec<Value>>>>,
        payload: web::Json<Value>,
    ) -> HttpResponse {
        captured_requests.lock().unwrap().push(payload.into_inner());

        HttpResponse::Ok().json(serde_json::json!({
            "object": "list",
            "data": [{
                "object": "embedding",
                "index": 0,
                "embedding": [0.1, 0.2]
            }],
            "model": "text-embedding-3-small",
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 0,
                "total_tokens": 1
            }
        }))
    }

    async fn mock_image_generations(
        captured_requests: web::Data<Arc<Mutex<Vec<Value>>>>,
        payload: web::Json<Value>,
    ) -> HttpResponse {
        captured_requests.lock().unwrap().push(payload.into_inner());

        HttpResponse::Ok().json(serde_json::json!({
            "created": 1710000000,
            "data": [{
                "url": "https://images.example.test/gen.png"
            }]
        }))
    }

    async fn mock_audio_speech(
        captured_requests: web::Data<Arc<Mutex<Vec<Value>>>>,
        payload: web::Json<Value>,
    ) -> HttpResponse {
        captured_requests.lock().unwrap().push(payload.into_inner());
        HttpResponse::Ok()
            .content_type("audio/mpeg")
            .body(Bytes::from_static(b"mock-audio"))
    }

    async fn mock_audio_transcriptions(
        captured_bodies: web::Data<Arc<Mutex<Vec<Vec<u8>>>>>,
        body: Bytes,
    ) -> HttpResponse {
        captured_bodies.lock().unwrap().push(body.to_vec());
        HttpResponse::Ok().json(serde_json::json!({ "text": "hello world" }))
    }

    /// Construct an actix-web test app with AuthMiddleware and route
    /// configurations matching the real server layout.
    fn build_test_app(
        state: AppState,
    ) -> App<
        impl actix_web::dev::ServiceFactory<
            actix_web::dev::ServiceRequest,
            Config = (),
            Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
            Error = actix_web::Error,
            InitError = (),
        >,
    > {
        let budget_limits = web::Data::new(Arc::clone(&state.budget_limits));

        App::new()
            .app_data(web::Data::new(state))
            .app_data(budget_limits)
            .wrap(AuthMiddleware)
            .configure(routes::health::configure_routes)
            .configure(routes::ai::configure_routes)
    }

    // ---------------------------------------------------------------
    // 1. GET /health — public route, always returns 200
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_health_returns_200() {
        let state = build_auth_enabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        // /health is now liveness-only: returns "alive" unconditionally.
        // Readiness (which considers provider + storage health) lives at /health/ready.
        assert_eq!(body["data"]["status"], "alive");
        assert!(body["data"]["version"].is_string());
    }

    #[tokio::test]
    async fn test_health_accessible_even_with_auth_enabled() {
        // /health is a public route — it must succeed regardless of auth config.
        let state = build_auth_enabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert!(body["data"]["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_readiness_accessible_even_with_auth_enabled() {
        let state = build_auth_enabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get().uri("/health/ready").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["data"]["ready"], false);
        assert_eq!(body["data"]["reason"], "not ready");
        assert!(body["data"].get("storage").is_none());
        assert!(body["data"].get("providers").is_none());
    }

    #[tokio::test]
    async fn test_authenticated_readiness_preserves_component_details() {
        let state = build_auth_enabled_state().await;
        let api_key = seed_readiness_api_key(&state).await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get()
            .uri("/health/ready")
            .insert_header(("x-api-key", api_key))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["data"]["reason"], "no providers enabled");
        assert!(body["data"]["storage"].is_object());
        assert!(body["data"]["providers"].is_object());
        assert_eq!(body["data"]["providers"]["total_providers"], 1);
        assert_eq!(body["data"]["providers"]["enabled_providers"], 0);
    }

    #[tokio::test]
    async fn test_detailed_health_requires_auth() {
        let state = build_auth_enabled_state().await;
        let app = test::init_service(build_test_app(state)).await;
        let req = test::TestRequest::get()
            .uri("/health/detailed")
            .to_request();

        match test::try_call_service(&app, req).await {
            Err(err) => {
                assert_eq!(
                    err.as_response_error().status_code(),
                    StatusCode::UNAUTHORIZED,
                );
            }
            Ok(resp) => assert_eq!(resp.status(), StatusCode::UNAUTHORIZED),
        }
    }

    #[tokio::test]
    async fn test_readiness_reports_storage_failure_from_storage_layer() {
        let tempdir = match tempfile::tempdir() {
            Ok(tempdir) => tempdir,
            Err(err) => panic!("failed to create temp dir: {err}"),
        };
        let storage_path = tempdir.path().join("files");

        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.storage.files.local_path = Some(storage_path.to_string_lossy().into_owned());
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());

        let state = build_state_with_config(config).await;
        let api_key = seed_readiness_api_key(&state).await;
        if let Err(err) = std::fs::remove_dir_all(&storage_path) {
            panic!("failed to remove storage dir: {err}");
        }
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get()
            .uri("/health/ready")
            .insert_header(("x-api-key", api_key))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["data"]["ready"], false);
        assert_eq!(body["data"]["reason"], "storage unhealthy");
        assert_eq!(body["data"]["storage"]["overall"], false);
        assert_eq!(body["data"]["storage"]["files"], false);
    }

    #[tokio::test]
    async fn test_readiness_reports_unknown_enabled_provider() {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.database.url.clear();
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        // This case exercises the fail-closed pre-evidence state. Prevent the
        // default active probe from replacing Unknown with a live outcome.
        config.gateway.router.load_balancer.health_check_enabled = false;
        config.gateway.providers.push(ProviderConfig {
            name: "openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["gpt-4".to_string()],
            ..ProviderConfig::default()
        });

        let state = build_state_with_config(config).await;
        let api_key = seed_readiness_api_key(&state).await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get()
            .uri("/health/ready")
            .insert_header(("x-api-key", api_key))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["data"]["ready"], false);
        assert_eq!(
            body["data"]["reason"],
            "one or more providers have unknown status"
        );
        assert_eq!(body["data"]["storage"]["overall"], true);
        assert_eq!(body["data"]["providers"]["aggregate"], "unknown");
        assert_eq!(body["data"]["providers"]["enabled_providers"], 1);
    }

    // ---------------------------------------------------------------
    // 2. POST /v1/chat/completions without auth — returns 401
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_chat_completions_without_auth_returns_401() {
        let state = build_auth_enabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/chat/completions")
            .set_json(serde_json::json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}]
            }))
            .to_request();

        match test::try_call_service(&app, req).await {
            Err(err) => {
                assert_eq!(
                    err.as_response_error().status_code(),
                    StatusCode::UNAUTHORIZED,
                );
            }
            Ok(resp) => {
                // Some middleware stacks convert errors into responses
                assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
            }
        }
    }

    // ---------------------------------------------------------------
    // 3. POST /v1/chat/completions with invalid JSON body — returns 400
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_chat_completions_invalid_json_returns_400() {
        // Use auth-disabled state so the request reaches the route handler.
        let state = build_auth_disabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/chat/completions")
            .insert_header(("content-type", "application/json"))
            .set_payload("{ not valid json !!!")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_chat_completions_missing_required_fields_returns_400() {
        let state = build_auth_disabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        // Send valid JSON but missing required "messages" field
        let req = test::TestRequest::post()
            .uri("/v1/chat/completions")
            .set_json(serde_json::json!({
                "model": "gpt-4"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_engine_embedding_alias_uses_path_model() {
        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route("/embeddings", web::post().to(mock_embeddings))
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = build_openai_alias_state(&format!("http://{address}")).await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/engines/text-embedding-3-small/embeddings")
            .set_json(serde_json::json!({
                "model": "body-model",
                "input": "hello"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        handle.stop(true).await;
        let _ = task.await;

        assert_eq!(resp.status(), StatusCode::OK);
        let requests = captured_requests.lock().unwrap().clone();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["model"], "text-embedding-3-small");
        assert_ne!(requests[0]["model"], "body-model");
    }

    #[tokio::test]
    async fn test_embeddings_use_response_cache() {
        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route("/embeddings", web::post().to(mock_embeddings))
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = build_openai_alias_state_with_cache(&format!("http://{address}")).await;
        let app = test::init_service(build_test_app(state.clone())).await;

        for _ in 0..2 {
            let req = test::TestRequest::post()
                .uri("/v1/embeddings")
                .set_json(serde_json::json!({
                    "model": "text-embedding-3-small",
                    "input": "hello"
                }))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::OK);
            let body: Value = test::read_body_json(resp).await;
            let first_value = body["data"][0]["embedding"][0]
                .as_f64()
                .expect("embedding value should be numeric");
            assert!((first_value - 0.1).abs() < 0.000_001);
        }

        assert_eq!(captured_requests.lock().unwrap().len(), 1);
        let router = state.unified_router();
        let ids = router.get_deployments_for_model("text-embedding-3-small");
        assert_eq!(ids.len(), 1);
        let mut replacement = (*router.get_deployment(&ids[0]).unwrap()).clone();
        router.remove_deployment(&ids[0]);
        replacement.id = "replacement-embedding-deployment".into();
        router.add_deployment(replacement);
        for _ in 0..2 {
            let req = test::TestRequest::post()
                .uri("/v1/embeddings")
                .set_json(serde_json::json!({"model":"text-embedding-3-small","input":"hello"}))
                .to_request();
            assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
        }

        handle.stop(true).await;
        let _ = task.await;

        assert_eq!(
            captured_requests.lock().unwrap().len(),
            2,
            "each deployment must execute once, then reuse only its own cached vector"
        );
    }

    #[tokio::test]
    async fn test_embedding_cache_hit_preserves_tpm_and_exact_deployment_context() {
        use actix_web::HttpMessage;
        use litellm_rs::core::types::context::RequestContext;
        use std::sync::atomic::Ordering::Relaxed;

        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener.local_addr().unwrap();
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route("/embeddings", web::post().to(mock_embeddings))
        })
        .listen(listener)
        .unwrap()
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        let state = cache_state_without_routing_retries(
            build_openai_alias_state_with_cache(&format!("http://{address}")).await,
        )
        .await;
        let model = "text-embedding-3-small";
        set_cache_test_tpm(&state, model, 2_048);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(routes::ai::configure_routes),
        )
        .await;
        cache_admission_minute_guard().await;
        let payload = serde_json::json!({
            "model": model,
            "input": "embedding cache reservation ".repeat(64),
            "user": "untrusted-body-user"
        });
        let request = |body: &Value, owner: &str| {
            let req = test::TestRequest::post()
                .uri("/v1/embeddings")
                .set_json(body)
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id(owner));
            req
        };
        let warm = test::call_service(&app, request(&payload, "cache-owner")).await;
        assert_eq!(warm.status(), StatusCode::OK);
        let warm_body: Value = test::read_body_json(warm).await;
        assert_eq!(warm_body["usage"]["total_tokens"], 1);
        assert_eq!(captured_requests.lock().unwrap().len(), 1);
        let router = state.unified_router();
        let id = router.get_deployments_for_model(model).pop().unwrap();
        let deployment = router.get_deployment(&id).unwrap();
        assert_eq!(deployment.state.tpm_current.load(Relaxed), 1);

        // One remaining token fits the cache replay minimum, while the long
        // input still requires the full estimate on every uncached operation.
        set_cache_test_tpm(&state, model, 2);
        let replay = test::call_service(&app, request(&payload, "cache-owner")).await;
        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body: Value = test::read_body_json(replay).await;
        assert_eq!(replay_body["data"], warm_body["data"]);
        assert_eq!(captured_requests.lock().unwrap().len(), 1);
        assert_eq!(deployment.state.tpm_current.load(Relaxed), 1);
        assert_eq!(deployment.state.active_requests.load(Relaxed), 0);

        let mut miss = payload.clone();
        miss["input"] = serde_json::json!("different uncached embedding input ".repeat(64));
        for (body, owner) in [(&miss, "cache-owner"), (&payload, "different-owner")] {
            let denied = test::call_service(&app, request(body, owner)).await;
            assert_eq!(denied.status(), StatusCode::SERVICE_UNAVAILABLE);
            let error: Value = test::read_body_json(denied).await;
            assert_eq!(error["error"]["code"], "provider_unavailable");
            assert_eq!(captured_requests.lock().unwrap().len(), 1);
            assert_eq!(deployment.state.tpm_current.load(Relaxed), 1);
            assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
        }

        // The route stored its selected provider model, not a model-free entry.
        let cache = state.response_cache().unwrap();
        let mut cache_request: litellm_rs::core::models::openai::EmbeddingRequest =
            serde_json::from_value(payload.clone()).unwrap();
        assert!(
            cache
                .get_embedding_response(&cache_request, Some("user:cache-owner"), &id)
                .await
                .unwrap()
                .is_some()
        );
        cache_request.model = "text-embedding-3-large".to_string();
        assert!(
            cache
                .get_embedding_response(&cache_request, Some("user:cache-owner"), &id)
                .await
                .unwrap()
                .is_none()
        );

        // The old deployment still has a matching cached vector. Replacing
        // only its ID must not let that entry discount the new deployment's
        // reservation or become the payload replayed by its operation.
        let mut replacement = router.get_deployment(&id).unwrap().as_ref().clone();
        replacement.id = format!("{id}-uncached-replacement");
        let replacement_id = replacement.id.clone();
        router.remove_deployment(&id);
        router.add_deployment(replacement);
        let denied = test::call_service(&app, request(&payload, "cache-owner")).await;
        assert_eq!(denied.status(), StatusCode::SERVICE_UNAVAILABLE);
        let error: Value = test::read_body_json(denied).await;
        assert_eq!(error["error"]["code"], "provider_unavailable");
        assert_eq!(captured_requests.lock().unwrap().len(), 1);

        // With enough capacity the replacement must dispatch once, then only
        // its own exact model/deployment/context entry may be replayed.
        set_cache_test_tpm(&state, model, 2_048);
        let admitted = test::call_service(&app, request(&payload, "cache-owner")).await;
        assert_eq!(admitted.status(), StatusCode::OK);
        let _: Value = test::read_body_json(admitted).await;
        assert_eq!(captured_requests.lock().unwrap().len(), 2);
        let replacement = router.get_deployment(&replacement_id).unwrap();
        assert_eq!(replacement.state.tpm_current.load(Relaxed), 2);
        set_cache_test_tpm(&state, model, 3);
        let replay = test::call_service(&app, request(&payload, "cache-owner")).await;
        assert_eq!(replay.status(), StatusCode::OK);
        let _: Value = test::read_body_json(replay).await;
        assert_eq!(captured_requests.lock().unwrap().len(), 2);
        assert_eq!(replacement.state.tpm_current.load(Relaxed), 2);
        assert_eq!(replacement.state.active_requests.load(Relaxed), 0);
        assert!(
            captured_requests
                .lock()
                .unwrap()
                .iter()
                .all(|request| request["model"] == model)
        );

        handle.stop(true).await;
        let _ = task.await;
    }

    #[tokio::test]
    async fn test_embeddings_route_emits_embedding_hooks() {
        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route("/embeddings", web::post().to(mock_embeddings))
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let events = Arc::new(Mutex::new(Vec::new()));
        let runtime = build_embedding_callback_runtime(Arc::clone(&events)).await;
        let state = build_openai_alias_state(&format!("http://{address}"))
            .await
            .with_callbacks(runtime.dispatcher());
        let app = test::init_service(build_test_app(state)).await;
        let req = test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(serde_json::json!({
                "model": "text-embedding-3-small",
                "input": ["hello", "world"]
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        runtime.shutdown().await.expect("callback runtime shutdown");
        handle.stop(true).await;
        task.await
            .expect("mock embedding server task")
            .expect("mock embedding server result");

        let events = events.lock().expect("embedding callback events").clone();
        assert_eq!(events.len(), 2);
        let RecordedEmbeddingCallback::Start(start) = &events[0] else {
            panic!("first callback must be embedding start");
        };
        let RecordedEmbeddingCallback::End(end) = &events[1] else {
            panic!("second callback must be embedding end");
        };
        assert_eq!(start.request_id, end.request_id);
        assert_eq!(start.model, "text-embedding-3-small");
        assert_eq!(start.provider.as_deref(), Some("mock-openai"));
        assert_eq!(start.input_count, 2);
        assert_eq!(end.provider.as_deref(), Some("mock-openai"));
        assert_eq!(end.total_tokens, Some(1));
    }

    #[tokio::test]
    async fn test_embeddings_rejects_exhausted_provider_budget_before_upstream() {
        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route("/embeddings", web::post().to(mock_embeddings))
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = build_openai_alias_state(&format!("http://{address}")).await;
        state.budget_limits.providers.set_provider_limit(
            "mock-openai",
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .record_spend("mock-openai", "text-embedding-3-small", 0.01);
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(serde_json::json!({
                "model": "text-embedding-3-small",
                "input": "hello"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        handle.stop(true).await;
        let _ = task.await;

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        let body: Value = test::read_body_json(resp).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("provider 'mock-openai' budget exceeded")
        );
        assert!(
            captured_requests.lock().unwrap().is_empty(),
            "budget rejection must happen before upstream embedding call"
        );
    }

    #[tokio::test]
    async fn test_embeddings_record_provider_spend_after_success() {
        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route("/embeddings", web::post().to(mock_embeddings))
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = build_openai_alias_state(&format!("http://{address}")).await;
        state.budget_limits.providers.set_provider_limit(
            "mock-openai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = Arc::clone(&state.budget_limits);
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(serde_json::json!({
                "model": "text-embedding-3-small",
                "input": "hello"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        handle.stop(true).await;
        let _ = task.await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(captured_requests.lock().unwrap().len(), 1);
        let spent = budget_limits
            .providers
            .get_provider_usage("mock-openai")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert!(spent > 0.0, "successful embedding usage must record spend");
    }

    #[tokio::test]
    async fn test_embeddings_route_proxies_openai_compatible_provider() {
        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route("/embeddings", web::post().to(mock_embeddings))
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = build_openai_compatible_embeddings_state(&format!("http://{address}")).await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(serde_json::json!({
                "model": "text-embedding-3-small",
                "input": "hello"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        handle.stop(true).await;
        let _ = task.await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["data"][0]["index"], 0);
        assert_eq!(
            body["data"][0]["embedding"].as_array().map(Vec::len),
            Some(2)
        );
        let requests = captured_requests.lock().unwrap().clone();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["model"], "text-embedding-3-small");
        assert_eq!(requests[0]["input"], "hello");
    }

    #[tokio::test]
    async fn test_image_generations_route_proxies_openai_compatible_provider() {
        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route(
                    "/images/generations",
                    web::post().to(mock_image_generations),
                )
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = build_openai_compatible_image_state(&format!("http://{address}")).await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/images/generations")
            .set_json(serde_json::json!({
                "model": "gpt-image-1-mini",
                "prompt": "a red cube"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        handle.stop(true).await;
        let _ = task.await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(
            body["data"][0]["url"],
            "https://images.example.test/gen.png"
        );
        let requests = captured_requests.lock().unwrap().clone();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["model"], "gpt-image-1-mini");
        assert_eq!(requests[0]["prompt"], "a red cube");
    }

    #[tokio::test]
    async fn test_audio_speech_rejects_streaming_before_upstream() {
        let state = build_auth_disabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(serde_json::json!({
                "model": "tts-1",
                "input": "hello",
                "voice": "alloy",
                "stream": true
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("Streaming speech is not supported")
        );
    }

    #[tokio::test]
    async fn test_audio_speech_route_proxies_openai_compatible_provider() {
        let captured_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_requests);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route("/audio/speech", web::post().to(mock_audio_speech))
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = build_openai_compatible_audio_state(&format!("http://{address}")).await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(serde_json::json!({
                "model": "tts-1",
                "input": "hello",
                "voice": "alloy"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;

        handle.stop(true).await;
        let _ = task.await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_ref(), b"mock-audio");
        let requests = captured_requests.lock().unwrap().clone();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["model"], "tts-1");
        assert_eq!(requests[0]["input"], "hello");
    }

    #[tokio::test]
    async fn test_audio_transcriptions_route_proxies_openai_compatible_provider() {
        let captured_bodies = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have local address");
        let captured_for_server = Arc::clone(&captured_bodies);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(Arc::clone(&captured_for_server)))
                .route(
                    "/audio/transcriptions",
                    web::post().to(mock_audio_transcriptions),
                )
        })
        .listen(listener)
        .expect("mock server should listen")
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let state = build_openai_compatible_audio_state(&format!("http://{address}")).await;
        let app = test::init_service(build_test_app(state)).await;
        let boundary = "litellm-rs-audio-boundary";
        let mut payload = Vec::new();
        payload.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        payload.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        payload.extend_from_slice(b"whisper-1\r\n");
        payload.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        payload.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"sample.mp3\"\r\n",
        );
        payload.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        payload.extend_from_slice(&[b'a'; 32]);
        payload.extend_from_slice(b"\r\n");
        payload.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        let req = test::TestRequest::post()
            .uri("/v1/audio/transcriptions")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body: Value = test::read_body_json(resp).await;

        handle.stop(true).await;
        let _ = task.await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["text"], "hello world");
        let captured = captured_bodies.lock().unwrap().clone();
        assert_eq!(captured.len(), 1);
        let upstream = String::from_utf8_lossy(&captured[0]);
        assert!(upstream.contains("whisper-1"));
        assert!(upstream.contains("sample.mp3"));
    }

    // ---------------------------------------------------------------
    // 4. GET /v1/models — returns 200 with model list structure
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_list_models_returns_200_with_list_structure() {
        // Use auth-disabled state so the request reaches the handler.
        let state = build_auth_disabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get().uri("/v1/models").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["object"], "list");
        assert!(
            body["data"].is_array(),
            "models response should have a 'data' array"
        );
    }

    #[tokio::test]
    async fn test_list_models_without_auth_returns_401() {
        let state = build_auth_enabled_state().await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get().uri("/v1/models").to_request();

        match test::try_call_service(&app, req).await {
            Err(err) => {
                assert_eq!(
                    err.as_response_error().status_code(),
                    StatusCode::UNAUTHORIZED,
                );
            }
            Ok(resp) => {
                // Some middleware stacks convert errors into responses
                assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
            }
        }
    }
}
