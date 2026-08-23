use super::*;

#[tokio::test]
async fn moderation_route_rejects_exhausted_provider_budget_before_upstream() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
    state.budget_limits.providers.set_provider_limit(
        "mock-openai-compatible",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("mock-openai-compatible", 2.0);
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({ "input": "hello" }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["type"], "insufficient_quota");
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("provider 'mock-openai-compatible' budget exceeded")
    );
    assert!(
        mock.requests().is_empty(),
        "budget rejection must happen before upstream call"
    );

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_uses_router_budget_fallback_provider() {
    let exhausted = MockModerationServer::start_moderation_mock().await;
    let fallback = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![
        named_moderation_provider(
            "exhausted-moderation-provider",
            &exhausted.base_url,
            vec!["omni-moderation-latest".to_string()],
        ),
        named_moderation_provider(
            "fallback-moderation-provider",
            &fallback.base_url,
            vec!["omni-moderation-latest".to_string()],
        ),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "exhausted-moderation-provider",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("exhausted-moderation-provider", 2.0);
    state.budget_limits.providers.set_provider_limit(
        "fallback-moderation-provider",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({
                "model": "omni-moderation-latest",
                "input": "moderate this text"
            }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        exhausted.requests().is_empty(),
        "exhausted provider must be skipped before upstream call"
    );
    let requests = fallback.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/moderations");
    assert_eq!(requests[0].body["model"], "omni-moderation-latest");

    exhausted.stop_moderation_mock().await;
    fallback.stop_moderation_mock().await;
}

#[tokio::test]
async fn native_openai_moderation_route_uses_default_model_with_empty_config_models() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![named_moderation_provider_with_type(
        "mock-openai-native",
        "openai",
        &mock.base_url,
        Vec::new(),
    )])
    .await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({ "input": "moderate this text" }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/moderations");
    assert_eq!(requests[0].body["model"], "omni-moderation-latest");

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn openai_compatible_named_openai_uses_provider_name_wildcard_fallback() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![named_moderation_provider_with_type(
        "openai",
        "openai_compatible",
        &mock.base_url,
        Vec::new(),
    )])
    .await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({ "input": "moderate this text" }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/moderations");
    assert_eq!(requests[0].body["model"], "omni-moderation-latest");

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_uses_wildcard_provider_name_fallback() {
    let exhausted = MockModerationServer::start_moderation_mock().await;
    let fallback = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![
        named_moderation_provider(
            "wildcard-moderation-primary",
            &exhausted.base_url,
            Vec::new(),
        ),
        named_moderation_provider(
            "wildcard-moderation-secondary",
            &fallback.base_url,
            Vec::new(),
        ),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "wildcard-moderation-primary",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("wildcard-moderation-primary", 2.0);
    state.budget_limits.providers.set_provider_limit(
        "wildcard-moderation-secondary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({ "input": "moderate this text" }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        exhausted.requests().is_empty(),
        "exhausted wildcard provider must be skipped before upstream call"
    );
    let requests = fallback.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/moderations");
    assert_eq!(requests[0].body["model"], "omni-moderation-latest");

    exhausted.stop_moderation_mock().await;
    fallback.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_rejects_exhausted_default_model_budget_before_upstream() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
    state.budget_limits.models.set_model_limit(
        "omni-moderation-latest",
        ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .models
        .record_model_spend("omni-moderation-latest", 2.0);
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({ "input": "hello" }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["type"], "insufficient_quota");
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("model 'omni-moderation-latest' budget exceeded")
    );
    assert!(
        mock.requests().is_empty(),
        "model budget rejection must happen before upstream call"
    );

    mock.stop_moderation_mock().await;
}
