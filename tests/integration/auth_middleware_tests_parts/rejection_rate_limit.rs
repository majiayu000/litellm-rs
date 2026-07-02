use super::*;

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
    let first_error = test::try_call_service(&app, first)
        .await
        .expect_err("first missing-auth request should fail in auth middleware");
    assert_eq!(
        first_error.as_response_error().status_code(),
        StatusCode::UNAUTHORIZED
    );

    let second = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.101:1001".parse().unwrap())
        .to_request();
    let second_error = test::try_call_service(&app, second)
        .await
        .expect_err("second missing-auth request should hit gateway rate limit");
    assert_eq!(
        second_error.as_response_error().status_code(),
        StatusCode::TOO_MANY_REQUESTS
    );
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
        .insert_header(("x-api-key", "gw-invalid-auth-rate-limit-key"))
        .to_request();
    let first_error = test::try_call_service(&app, first)
        .await
        .expect_err("first invalid-auth request should fail in auth middleware");
    assert_eq!(
        first_error.as_response_error().status_code(),
        StatusCode::UNAUTHORIZED
    );

    let second = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.102:1001".parse().unwrap())
        .insert_header(("x-api-key", "gw-invalid-auth-rate-limit-key-rotated"))
        .to_request();
    let second_error = test::try_call_service(&app, second)
        .await
        .expect_err("second rotating invalid-auth request should hit gateway rate limit");
    assert_eq!(
        second_error.as_response_error().status_code(),
        StatusCode::TOO_MANY_REQUESTS
    );
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
    let first_error = test::try_call_service(&app, first)
        .await
        .expect_err("first missing-auth request should fail in auth middleware");
    assert_eq!(
        first_error.as_response_error().status_code(),
        StatusCode::UNAUTHORIZED
    );

    let second = test::TestRequest::get()
        .uri(AUTH_PROBE_PATH)
        .peer_addr("203.0.113.152:1001".parse().unwrap())
        .to_request();
    let second_error = test::try_call_service(&app, second)
        .await
        .expect_err("second missing-auth request should hit requests_per_minute limit");
    assert_eq!(
        second_error.as_response_error().status_code(),
        StatusCode::TOO_MANY_REQUESTS
    );
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
    let second_error = test::try_call_service(&app, second)
        .await
        .expect_err("second authenticated request should hit requests_per_minute limit");
    assert_eq!(
        second_error.as_response_error().status_code(),
        StatusCode::TOO_MANY_REQUESTS
    );
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
    let invalid_error = test::try_call_service(&app, invalid)
        .await
        .expect_err("first invalid auth after valid auth should not inherit the reservation");
    assert_eq!(
        invalid_error.as_response_error().status_code(),
        StatusCode::UNAUTHORIZED
    );
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
    let second_error = test::try_call_service(&app, second)
        .await
        .expect_err("second request should hit the API key rpm limit");

    assert_eq!(
        second_error.as_response_error().status_code(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(hit_counter.load(Ordering::SeqCst), 1);
}
