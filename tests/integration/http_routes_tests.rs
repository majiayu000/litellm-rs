//! HTTP integration tests for core API routes
//!
//! Tests the middleware stack against the actual route handlers using
//! actix-web's in-process test utilities.

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::http::StatusCode;
    use actix_web::{App, HttpResponse, HttpServer, test, web};
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::middleware::AuthMiddleware;
    use litellm_rs::server::routes;
    use litellm_rs::server::state::AppState;
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    async fn build_state_with_config(config: Config) -> AppState {
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

    async fn build_openai_alias_state(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers = vec![ProviderConfig {
            name: "mock-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some(base_url.to_string()),
            models: vec!["text-embedding-3-small".to_string()],
            ..ProviderConfig::default()
        }];

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
    async fn test_readiness_reports_storage_failure_from_storage_layer() {
        let tempdir = match tempfile::tempdir() {
            Ok(tempdir) => tempdir,
            Err(err) => panic!("failed to create temp dir: {err}"),
        };
        let storage_path = tempdir.path().join("files");

        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.storage.files.local_path = Some(storage_path.to_string_lossy().into_owned());
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());

        let state = build_state_with_config(config).await;
        if let Err(err) = std::fs::remove_dir_all(&storage_path) {
            panic!("failed to remove storage dir: {err}");
        }
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get().uri("/health/ready").to_request();
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
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.database.url.clear();
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers.push(ProviderConfig {
            name: "openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["gpt-4".to_string()],
            ..ProviderConfig::default()
        });

        let state = build_state_with_config(config).await;
        let app = test::init_service(build_test_app(state)).await;

        let req = test::TestRequest::get().uri("/health/ready").to_request();
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
