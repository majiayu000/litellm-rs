//! HTTP integration tests for core API routes
//!
//! Tests the middleware stack against the actual route handlers using
//! actix-web's in-process test utilities.

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::http::StatusCode;
    use actix_web::{App, test, web};
    use litellm_rs::Config;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::middleware::AuthMiddleware;
    use litellm_rs::server::routes;
    use litellm_rs::server::state::AppState;
    use serde_json::Value;
    use std::sync::Arc;

    /// Build an AppState with auth enabled (both JWT and API key).
    async fn build_auth_enabled_state() -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = true;
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());

        let server = GatewayHttpServer::new(&config)
            .await
            .expect("failed to build HTTP server for integration test");
        let state = server.state().clone();
        state
            .storage
            .migrate()
            .await
            .expect("failed to run in-memory DB migrations");
        state
    }

    /// Build an AppState with auth disabled.
    async fn build_auth_disabled_state() -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());

        let server = GatewayHttpServer::new(&config)
            .await
            .expect("failed to build HTTP server for integration test");
        let state = server.state().clone();
        state
            .storage
            .migrate()
            .await
            .expect("failed to run in-memory DB migrations");
        state
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
