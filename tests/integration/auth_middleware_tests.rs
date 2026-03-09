//! Authentication middleware integration tests
//!
//! Covers pass/fail paths and request context propagation.

#[cfg(test)]
mod tests {
    use actix_web::http::StatusCode;
    use actix_web::{App, HttpMessage, HttpRequest, HttpResponse, test, web};
    use litellm_rs::Config;
    use litellm_rs::core::models::user::types::{User, UserStatus};
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::core::types::context::RequestContext;
    use litellm_rs::server::http::HttpServer;
    use litellm_rs::server::middleware::AuthMiddleware;
    use litellm_rs::server::state::AppState;
    use litellm_rs::utils::auth::crypto::keys::{extract_api_key_prefix, hash_api_key};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const AUTH_PROBE_PATH: &str = "/v1/private/auth-probe";

    #[derive(Debug, Clone)]
    struct SeededPrincipal {
        raw_api_key: String,
        user_id: String,
        api_key_id: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct AuthProbePayload {
        context_present: bool,
        user_present: bool,
        api_key_present: bool,
        request_id: Option<String>,
        user_id: Option<String>,
        api_key_id: Option<String>,
    }

    async fn auth_probe(
        req: HttpRequest,
        hit_counter: web::Data<Arc<AtomicUsize>>,
    ) -> HttpResponse {
        hit_counter.fetch_add(1, Ordering::SeqCst);

        let context = req.extensions().get::<RequestContext>().cloned();
        let user = req.extensions().get::<User>().cloned();
        let api_key = req.extensions().get::<ApiKey>().cloned();

        let payload = AuthProbePayload {
            context_present: context.is_some(),
            user_present: user.is_some(),
            api_key_present: api_key.is_some(),
            request_id: context.as_ref().map(|ctx| ctx.request_id.clone()),
            user_id: context.as_ref().and_then(|ctx| ctx.user_id.clone()),
            api_key_id: context
                .as_ref()
                .and_then(|ctx| ctx.api_key_id().map(|id| id.to_string())),
        };

        HttpResponse::Ok().json(payload)
    }

    async fn build_test_state(enable_jwt: bool, enable_api_key: bool) -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = enable_jwt;
        config.gateway.auth.enable_api_key = enable_api_key;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());

        let server = HttpServer::new(&config)
            .await
            .expect("failed to build HTTP server for auth middleware integration test");
        let state = server.state().clone();
        state
            .storage
            .migrate()
            .await
            .expect("failed to run in-memory DB migrations for auth middleware integration test");
        state
    }

    async fn seed_valid_principal(state: &AppState) -> SeededPrincipal {
        let mut user = User::new(
            "auth-mw-user".to_string(),
            "auth-mw-user@example.com".to_string(),
            "hashed-password".to_string(),
        );
        user.status = UserStatus::Active;

        let user = state
            .storage
            .db()
            .create_user(&user)
            .await
            .expect("failed to insert user for auth middleware integration test");

        let raw_api_key = "gw-valid-auth-middleware-key-123456".to_string();
        let api_key = ApiKey {
            metadata: Metadata::new(),
            name: "auth-middleware-test-key".to_string(),
            key_hash: hash_api_key(&raw_api_key),
            key_prefix: extract_api_key_prefix(&raw_api_key),
            user_id: Some(user.id()),
            team_id: None,
            permissions: vec!["use:api".to_string()],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        };

        let api_key = state
            .storage
            .db()
            .create_api_key(&api_key)
            .await
            .expect("failed to insert API key for auth middleware integration test");

        SeededPrincipal {
            raw_api_key,
            user_id: user.id().to_string(),
            api_key_id: api_key.metadata.id.to_string(),
        }
    }

    #[tokio::test]
    async fn test_auth_middleware_rejects_missing_auth() {
        let state = build_test_state(true, true).await;
        let hit_counter = Arc::new(AtomicUsize::new(0));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .app_data(web::Data::new(hit_counter.clone()))
                .wrap(AuthMiddleware)
                .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
        )
        .await;

        let request = test::TestRequest::get().uri(AUTH_PROBE_PATH).to_request();
        let error = test::try_call_service(&app, request)
            .await
            .expect_err("missing auth should fail in auth middleware");

        assert_eq!(
            error.as_response_error().status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
        assert!(error.to_string().contains("Missing authentication"));
    }

    #[tokio::test]
    async fn test_auth_middleware_rejects_invalid_auth() {
        let state = build_test_state(true, true).await;
        let hit_counter = Arc::new(AtomicUsize::new(0));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .app_data(web::Data::new(hit_counter.clone()))
                .wrap(AuthMiddleware)
                .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
        )
        .await;

        let request = test::TestRequest::get()
            .uri(AUTH_PROBE_PATH)
            .insert_header(("x-api-key", "gw-invalid-auth-middleware-key"))
            .to_request();
        let error = test::try_call_service(&app, request)
            .await
            .expect_err("invalid auth should fail in auth middleware");

        assert_eq!(
            error.as_response_error().status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
        assert!(error.to_string().contains("Invalid API key"));
    }

    #[tokio::test]
    async fn test_auth_middleware_accepts_valid_auth_and_propagates_principal_context() {
        let state = build_test_state(true, true).await;
        let principal = seed_valid_principal(&state).await;
        let hit_counter = Arc::new(AtomicUsize::new(0));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .app_data(web::Data::new(hit_counter.clone()))
                .wrap(AuthMiddleware)
                .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(AUTH_PROBE_PATH)
                .insert_header(("x-api-key", principal.raw_api_key.clone()))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(hit_counter.load(Ordering::SeqCst), 1);

        let payload: AuthProbePayload = test::read_body_json(response).await;
        assert!(payload.context_present);
        assert!(payload.user_present);
        assert!(payload.api_key_present);
        assert_eq!(payload.user_id.as_deref(), Some(principal.user_id.as_str()));
        assert_eq!(
            payload.api_key_id.as_deref(),
            Some(principal.api_key_id.as_str())
        );
        assert!(
            payload
                .request_id
                .as_deref()
                .is_some_and(|value| !value.is_empty()),
            "request context should include a non-empty request id"
        );
    }

    #[tokio::test]
    async fn test_auth_middleware_bypasses_auth_when_disabled_but_sets_context() {
        let state = build_test_state(false, false).await;
        let hit_counter = Arc::new(AtomicUsize::new(0));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .app_data(web::Data::new(hit_counter.clone()))
                .wrap(AuthMiddleware)
                .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::get().uri(AUTH_PROBE_PATH).to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(hit_counter.load(Ordering::SeqCst), 1);

        let payload: AuthProbePayload = test::read_body_json(response).await;
        assert!(payload.context_present);
        assert!(!payload.user_present);
        assert!(!payload.api_key_present);
        assert!(payload.user_id.is_none());
        assert!(payload.api_key_id.is_none());
        assert!(
            payload
                .request_id
                .as_deref()
                .is_some_and(|value| !value.is_empty()),
            "request context should include a non-empty request id in auth-disabled mode"
        );
    }
}
