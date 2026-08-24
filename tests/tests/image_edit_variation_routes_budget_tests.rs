use super::*;

#[tokio::test]
async fn image_variation_rejects_missing_model_before_upstream() {
    let mock = MockImageServer::start_image_mock().await;
    let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-image-boundary";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/variations")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_variation_without_model_multipart_body(boundary))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        "Validation error: model is required"
    );
    assert!(
        mock.requests().is_empty(),
        "missing model must fail before upstream call"
    );

    mock.stop_image_mock().await;
}

#[tokio::test]
async fn image_edit_rejects_unpriced_model_before_upstream() {
    let mock = MockImageServer::start_image_mock().await;
    let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-image-boundary";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_unpriced_model_multipart_body(boundary))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "model_not_priced");
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("unpriced-image-model")
    );
    assert!(
        mock.requests().is_empty(),
        "unpriced model must fail before upstream call"
    );

    mock.stop_image_mock().await;
}

#[tokio::test]
async fn image_edit_rejects_exhausted_provider_budget_before_upstream() {
    let mock = MockImageServer::start_image_mock().await;
    let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
    state.budget_limits.providers.set_provider_limit(
        "mock-openai-compatible",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("mock-openai-compatible", 2.0);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-image-boundary";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body(boundary))
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

    mock.stop_image_mock().await;
}

#[tokio::test]
async fn image_edit_rejects_provider_budget_that_cannot_cover_estimated_cost_before_upstream() {
    let mock = MockImageServer::start_image_mock().await;
    let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
    let mut usage = PricingUsage::new(4, 0);
    usage.image_tokens = Some(1024);
    let estimated_cost = state
        .pricing
        .calculate_loaded_usage_cost_for_provider("openai", "gpt-image-1-mini", &usage)
        .expect("image pricing should be available")
        .total_cost;
    assert!(estimated_cost > 0.0);
    state.budget_limits.providers.set_provider_limit(
        "mock-openai-compatible",
        ProviderLimitConfig::new(estimated_cost / 2.0, ResetPeriod::Monthly),
    );
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-image-boundary";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body(boundary))
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
        "estimated-cost budget rejection must happen before upstream call"
    );

    mock.stop_image_mock().await;
}

#[tokio::test]
async fn image_edit_rejects_exhausted_model_budget_before_upstream() {
    let mock = MockImageServer::start_image_mock().await;
    let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
    state.budget_limits.models.set_model_limit(
        "gpt-image-1-mini",
        ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .models
        .record_model_spend("gpt-image-1-mini", 2.0);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-image-boundary";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body(boundary))
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
            .contains("model 'gpt-image-1-mini' budget exceeded")
    );
    assert!(
        mock.requests().is_empty(),
        "model budget rejection must happen before upstream call"
    );

    mock.stop_image_mock().await;
}
