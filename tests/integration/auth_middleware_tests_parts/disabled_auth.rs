use super::*;

#[tokio::test]
async fn test_auth_middleware_fails_closed_when_disabled_without_anonymous_opt_in() {
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

    let request = test::TestRequest::get().uri(AUTH_PROBE_PATH).to_request();
    let error = test::try_call_service(&app, request)
        .await
        .expect_err("disabled auth without allow_anonymous should fail closed");

    assert_eq!(
        error.as_response_error().status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
    assert!(
        error
            .to_string()
            .contains("Authentication is not configured")
    );
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
