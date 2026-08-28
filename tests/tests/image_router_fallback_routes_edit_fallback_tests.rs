use super::*;

#[tokio::test]
async fn image_edit_records_flat_output_image_spend_after_success() {
    let mock = MockImageServer::start().await;
    let provider = with_explicit_image_identity(
        image_provider(
            "openai-primary",
            "openai",
            &mock.base_url,
            vec!["flat-image-model".to_string()],
        ),
        "flat-image-model",
        "flat-image-model",
    );
    let state = build_route_policy_test_state_with_custom_pricing(
        vec![provider],
        HashMap::from([("flat-image-model".to_string(), flat_image_model_info(0.06))]),
    )
    .await;
    state.budget_limits.providers.set_provider_limit(
        "openai-primary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-flat-image-boundary";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body_for_model(
                boundary,
                "flat-image-model",
                2,
            ))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(mock.paths(), vec!["/v1/images/edits".to_string()]);
    let upstream_bodies = mock.bodies();
    let upstream_body = String::from_utf8_lossy(&upstream_bodies[0]);
    assert!(upstream_body.contains("name=\"model\"\r\n\r\nflat-image-model"));
    let spent = budget_limits
        .providers
        .get_provider_usage("openai-primary")
        .map(|usage| usage.current_spend)
        .unwrap_or_default();
    assert!((spent - 0.12).abs() < f64::EPSILON);
    mock.stop().await;
}

#[tokio::test]
async fn native_openai_image_edit_uses_selected_provider_config_after_budget_fallback() {
    let exhausted = MockImageServer::start().await;
    let fallback = MockImageServer::start().await;
    let state = build_route_policy_test_state(vec![
        image_provider(
            "openai-primary",
            "openai",
            &exhausted.base_url,
            vec!["gpt-image-1-mini".to_string()],
        ),
        image_provider(
            "openai-secondary",
            "openai",
            &fallback.base_url,
            vec!["gpt-image-1-mini".to_string()],
        ),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "openai-primary",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("openai-primary", 2.0);
    state.budget_limits.providers.set_provider_limit(
        "openai-secondary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
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

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"][0]["url"],
        "https://images.example.test/edit.png"
    );
    assert!(exhausted.paths().is_empty());
    assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

    exhausted.stop().await;
    fallback.stop().await;
}

#[tokio::test]
async fn wildcard_openai_compatible_image_edit_tries_next_provider_name_key() {
    let exhausted = MockImageServer::start().await;
    let fallback = MockImageServer::start().await;
    let state = build_route_policy_test_state(vec![
        image_provider(
            "wild-primary",
            "openai_compatible",
            &exhausted.base_url,
            Vec::new(),
        ),
        image_provider(
            "wild-secondary",
            "openai_compatible",
            &fallback.base_url,
            Vec::new(),
        ),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "wild-primary",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("wild-primary", 2.0);
    state.budget_limits.providers.set_provider_limit(
        "wild-secondary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
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

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"][0]["url"],
        "https://images.example.test/edit.png"
    );
    assert!(exhausted.paths().is_empty());
    assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

    exhausted.stop().await;
    fallback.stop().await;
}

#[tokio::test]
async fn explicit_image_provider_falls_back_to_wildcard_provider() {
    let exhausted = MockImageServer::start().await;
    let fallback = MockImageServer::start().await;
    let state = build_route_policy_test_state(vec![
        image_provider(
            "explicit-primary",
            "openai_compatible",
            &exhausted.base_url,
            vec!["gpt-image-1-mini".to_string()],
        ),
        image_provider(
            "wild-secondary",
            "openai_compatible",
            &fallback.base_url,
            Vec::new(),
        ),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "explicit-primary",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("explicit-primary", 2.0);
    state.budget_limits.providers.set_provider_limit(
        "wild-secondary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
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

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"][0]["url"],
        "https://images.example.test/edit.png"
    );
    assert!(exhausted.paths().is_empty());
    assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

    exhausted.stop().await;
    fallback.stop().await;
}
