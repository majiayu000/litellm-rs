use super::*;

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
async fn test_auth_middleware_allows_legacy_use_api_permission_on_ai_route() {
    let state = build_test_state(true, true).await;
    let principal = seed_valid_principal(&state).await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(AuthMiddleware)
            .route("/v1/chat/completions", web::post().to(auth_probe)),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/chat/completions")
            .insert_header(("x-api-key", principal.raw_api_key.clone()))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(hit_counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_auth_middleware_denies_ai_route_for_disallowed_api_permission() {
    let state = build_test_state(true, true).await;
    let principal =
        seed_principal_with_api_key(&state, vec!["api.embeddings".to_string()], Metadata::new())
            .await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(AuthMiddleware)
            .route("/v1/chat/completions", web::post().to(auth_probe)),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let error = test::try_call_service(&app, request)
        .await
        .expect_err("disallowed operation should fail in auth middleware");

    assert_eq!(
        error.as_response_error().status_code(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
    assert!(error.to_string().contains("operation"));
}

#[tokio::test]
async fn test_auth_middleware_keeps_admin_owned_api_key_permission_restricted() {
    let state = build_test_state(true, true).await;
    let principal = seed_principal_with_role_and_api_key(
        &state,
        UserRole::Admin,
        vec!["api.embeddings".to_string()],
        Metadata::new(),
    )
    .await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(AuthMiddleware)
            .route("/v1/chat/completions", web::post().to(auth_probe)),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let error = test::try_call_service(&app, request)
        .await
        .expect_err("admin-owned limited key should stay limited");

    assert_eq!(
        error.as_response_error().status_code(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_auth_middleware_denies_root_engine_completion_alias_for_disallowed_permission() {
    let state = build_test_state(true, true).await;
    let principal =
        seed_principal_with_api_key(&state, vec!["api.embeddings".to_string()], Metadata::new())
            .await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(AuthMiddleware)
            .route("/engines/gpt-4o/completions", web::post().to(auth_probe)),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/engines/gpt-4o/completions")
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let error = test::try_call_service(&app, request)
        .await
        .expect_err("root engine completions alias should map to completions operation");

    assert_eq!(
        error.as_response_error().status_code(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_auth_middleware_denies_ai_route_for_disallowed_endpoint_policy() {
    let state = build_test_state(true, true).await;
    let mut metadata = Metadata::new();
    metadata.set_extra(
        "__core_keys",
        serde_json::json!({
            "permissions": {
                "allowed_endpoints": ["/v1/embeddings"]
            }
        }),
    );
    let principal =
        seed_principal_with_api_key(&state, vec!["use:api".to_string()], metadata).await;
    let hit_counter = Arc::new(AtomicUsize::new(0));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter.clone()))
            .wrap(AuthMiddleware)
            .route("/v1/chat/completions", web::post().to(auth_probe)),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("x-api-key", principal.raw_api_key.clone()))
        .to_request();
    let error = test::try_call_service(&app, request)
        .await
        .expect_err("disallowed endpoint should fail in auth middleware");

    assert_eq!(
        error.as_response_error().status_code(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(hit_counter.load(Ordering::SeqCst), 0);
    assert!(error.to_string().contains("endpoint"));
}

#[tokio::test]
async fn test_auth_middleware_propagates_api_key_budget_id_context() {
    let state = build_test_state(true, true).await;
    let budget_id = uuid::Uuid::new_v4();
    let mut metadata = Metadata::new();
    metadata.set_extra(
        "__core_keys",
        serde_json::json!({
            "budget_id": budget_id.to_string()
        }),
    );
    let principal =
        seed_principal_with_api_key(&state, vec!["use:api".to_string()], metadata).await;
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
    let payload: AuthProbePayload = test::read_body_json(response).await;
    let budget_id_string = budget_id.to_string();
    assert_eq!(
        payload.api_key_budget_id.as_deref(),
        Some(budget_id_string.as_str())
    );
}
