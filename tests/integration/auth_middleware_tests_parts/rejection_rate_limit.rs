use super::*;
use serde_json::Value;

#[tokio::test]
async fn test_openai_missing_auth_uses_openai_error_shape() {
    let state = build_test_state(true, true).await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .wrap(AuthMiddleware)
            .wrap(RequestIdMiddleware)
            .configure(|cfg| {
                litellm_rs::server::routes::ai::configure_routes(
                    cfg,
                    litellm_rs::config::models::default_max_body_size(),
                )
            }),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/chat/completions")
            .peer_addr("203.0.113.231:1000".parse().unwrap())
            .insert_header(("x-request-id", "req-openai-auth-missing"))
            .set_json(serde_json::json!({
                "model": "gpt-4o",
                "messages": []
            }))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok()),
        Some("req-openai-auth-missing")
    );
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["type"], "authentication_error");
    assert_eq!(body["error"]["code"], "authentication_error");
    assert_eq!(body["error"]["request_id"], "req-openai-auth-missing");
    assert!(body.get("success").is_none());
}

#[tokio::test]
async fn test_openai_rate_limit_middleware_uses_openai_error_shape() {
    let state = build_test_state_with_rate_limit(false, false, true, None).await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .wrap(RateLimitMiddleware::new(1))
            .wrap(RequestIdMiddleware)
            .configure(|cfg| {
                litellm_rs::server::routes::ai::configure_routes(
                    cfg,
                    litellm_rs::config::models::default_max_body_size(),
                )
            }),
    )
    .await;

    let first = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .peer_addr("203.0.113.232:1000".parse().unwrap())
        .set_json(serde_json::json!({
            "model": "gpt-4o",
            "messages": []
        }))
        .to_request();
    let first_response = test::call_service(&app, first).await;
    assert_ne!(first_response.status(), StatusCode::TOO_MANY_REQUESTS);

    let second = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .peer_addr("203.0.113.232:1001".parse().unwrap())
        .insert_header(("x-request-id", "req-openai-rate-limit"))
        .set_json(serde_json::json!({
            "model": "gpt-4o",
            "messages": []
        }))
        .to_request();
    let second_response = test::call_service(&app, second).await;

    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        second_response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok()),
        Some("req-openai-rate-limit")
    );
    let body: Value = test::read_body_json(second_response).await;
    assert_eq!(body["error"]["type"], "rate_limit_error");
    assert_eq!(body["error"]["code"], "rate_limit_exceeded");
    assert_eq!(body["error"]["request_id"], "req-openai-rate-limit");
    assert!(body.get("success").is_none());
}

#[tokio::test]
async fn test_legacy_openai_alias_json_errors_use_openai_shape() {
    let state = build_test_state_with_rate_limit(false, false, true, None).await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .wrap(RequestIdMiddleware)
            .configure(|cfg| {
                litellm_rs::server::routes::ai::configure_routes(
                    cfg,
                    litellm_rs::config::models::default_max_body_size(),
                )
            }),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/completions")
            .insert_header(("x-request-id", "req-openai-legacy-json"))
            .insert_header(("content-type", "application/json"))
            .set_payload(r#"{"model":"gpt-4o","#)
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "invalid_request");
    assert_eq!(body["error"]["request_id"], "req-openai-legacy-json");
    assert!(body.get("success").is_none());
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

    let response = test::call_service(
        &app,
        test::TestRequest::get().uri(AUTH_PROBE_PATH).to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
    let body = test::read_body(response).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("Missing authentication"));
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

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(AUTH_PROBE_PATH)
            .insert_header(("x-api-key", "gw-invalid-auth-middleware-key"))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
    let body = test::read_body(response).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("Invalid API key"));
}

#[tokio::test]
async fn test_auth_infrastructure_failures_stay_generic_500_without_lockout() {
    let state = build_test_state(true, true).await;
    state
        .storage
        .db()
        .connection()
        .close_by_ref()
        .await
        .expect("test should close the authentication database pool");
    let hit_counter = Arc::new(AtomicUsize::new(0));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(AuthMiddleware)
            .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
    )
    .await;

    for port in 1000..1006 {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(AUTH_PROBE_PATH)
                .peer_addr(format!("203.0.113.196:{port}").parse().unwrap())
                .insert_header(("x-api-key", "gw-auth-infrastructure-failure-960"))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = test::read_body(response).await;
        let body = String::from_utf8_lossy(&body);
        assert_eq!(body, "Authentication service temporarily unavailable");
        for internal_detail in ["Storage error", "Database error", "Connection closed"] {
            assert!(!body.contains(internal_detail));
        }
    }

    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn gh1130_malformed_api_key_policy_is_generic_500_not_detailed_403() {
    let state = build_test_state(true, true).await;
    let mut metadata = Metadata::new();
    metadata.set_extra(
        "__core_keys",
        serde_json::json!({
            "permissions": {
                "allowed_endpoints": "/v1/files",
                "is_admin": "corrupt"
            }
        }),
    );
    let principal =
        seed_principal_with_api_key(&state, vec!["embeddings".to_string()], metadata).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .wrap(AuthMiddleware)
            .configure(|cfg| {
                litellm_rs::server::routes::ai::configure_routes(
                    cfg,
                    litellm_rs::config::models::default_max_body_size(),
                )
            }),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/files")
            .insert_header(("x-api-key", principal.raw_api_key))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = test::read_body(response).await;
    let body = String::from_utf8_lossy(&body);
    assert!(!body.contains("allowed_endpoints"));
    assert!(!body.contains("is_admin"));
    assert!(!body.contains("corrupt"));
    assert!(!body.contains("Forbidden"));
}

#[tokio::test]
async fn test_missing_auth_hits_gateway_rate_limit_before_auth_short_circuit() {
    let state = build_test_state_with_rate_limit(true, true, false, Some(1)).await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(RateLimitMiddleware::new(1))
            .wrap(AuthMiddleware)
            .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
    )
    .await;

    let first = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.101:1000".parse().unwrap())
        .to_request();
    let first_response = test::call_service(&app, first).await;
    assert_eq!(first_response.status(), StatusCode::UNAUTHORIZED);

    let second = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.101:1001".parse().unwrap())
        .to_request();
    let second_response = test::call_service(&app, second).await;
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_rotating_invalid_auth_hits_gateway_rate_limit_before_auth_short_circuit() {
    let state = build_test_state_with_rate_limit(true, true, false, Some(1)).await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(RateLimitMiddleware::new(1))
            .wrap(AuthMiddleware)
            .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
    )
    .await;

    let first = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.102:1000".parse().unwrap())
        .insert_header(("x-api-key", "gw-invalid-auth-middleware-key"))
        .to_request();
    let first_response = test::call_service(&app, first).await;
    assert_eq!(first_response.status(), StatusCode::UNAUTHORIZED);

    let second = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.102:1001".parse().unwrap())
        .insert_header(("x-api-key", "gw-invalid-auth-rate-limit-key-rotated"))
        .to_request();
    let second_response = test::call_service(&app, second).await;
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_requests_per_minute_alias_limits_rejected_auth() {
    let state = build_test_state_with_requests_per_minute_alias(1000, 1).await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(AuthMiddleware)
            .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
    )
    .await;

    let first = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.152:1000".parse().unwrap())
        .to_request();
    let first_response = test::call_service(&app, first).await;
    assert_eq!(first_response.status(), StatusCode::UNAUTHORIZED);

    let second = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.152:1001".parse().unwrap())
        .to_request();
    let second_response = test::call_service(&app, second).await;
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_requests_per_minute_alias_limits_authenticated_requests() {
    let state = build_test_state_with_requests_per_minute_alias(1000, 1).await;
    let principal = seed_valid_principal(&state).await;
    let effective_rpm = state.config.load().gateway.rate_limit.effective_rpm();
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(RateLimitMiddleware::optional(Some(effective_rpm)))
            .wrap(AuthMiddleware)
            .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
    )
    .await;

    let first = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let first_response = test::call_service(&app, first).await;
    assert_eq!(first_response.status(), StatusCode::OK);

    let second = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let second_response = test::call_service(&app, second).await;
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_valid_auth_releases_gateway_auth_attempt_reservation() {
    let state = build_test_state_with_rate_limit(true, true, false, Some(1)).await;
    let principal = seed_valid_principal(&state).await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(RateLimitMiddleware::new(1))
            .wrap(AuthMiddleware)
            .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
    )
    .await;

    let valid = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.103:1000".parse().unwrap())
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let response = test::call_service(&app, valid).await;
    assert_eq!(response.status(), StatusCode::OK);

    let invalid = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.103:1001".parse().unwrap())
        .insert_header(("x-api-key", "gw-invalid-after-valid-auth"))
        .to_request();
    let invalid_response = test::call_service(&app, invalid).await;
    assert_eq!(invalid_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_api_key_rpm_is_enforced_without_gateway_default_rate_limit() {
    let state = build_test_state(true, true).await;
    let principal = seed_valid_principal(&state).await;
    let key_id =
        uuid::Uuid::parse_str(&principal.api_key_id).expect("seeded API key id should be a UUID");
    state
        .storage
        .db()
        .update_api_key_rate_limits(
            key_id,
            &RateLimits {
                rpm: Some(1),
                tpm: None,
                rpd: None,
                tpd: None,
                concurrent: None,
            },
        )
        .await
        .expect("failed to update seeded API key rate limit");
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(RateLimitMiddleware::optional(None))
            .wrap(AuthMiddleware)
            .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
    )
    .await;

    let first = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let first_response = test::call_service(&app, first).await;
    assert_eq!(first_response.status(), StatusCode::OK);

    let second = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let second_response = test::call_service(&app, second).await;
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 1);
}
