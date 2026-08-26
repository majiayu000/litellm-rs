use super::*;

#[tokio::test]
async fn server_rejects_disabled_auth_without_anonymous_opt_in() {
    let mut config = Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = false;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    add_disabled_bootstrap_provider(&mut config);

    let error = match HttpServer::new(&config).await {
        Err(error) => error,
        Ok(_) => panic!("invalid auth configuration must fail before server initialization"),
    };
    assert!(
        error
            .to_string()
            .contains("Both JWT and API key authentication are disabled")
    );
}

#[tokio::test]
async fn auth_middleware_fails_closed_if_invalid_runtime_config_bypasses_validation() {
    let state = build_test_state_with_rate_limit(false, false, true, None).await;
    let mut runtime_config = state.config().as_ref().clone();
    runtime_config.gateway.auth.allow_anonymous = false;
    state.config.store(runtime_config);
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
    assert!(String::from_utf8_lossy(&body).contains("Authentication is not configured"));
}

#[tokio::test]
async fn test_auth_middleware_bypasses_auth_when_disabled_but_sets_context() {
    let state = build_test_state_with_rate_limit(false, false, true, None).await;
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
