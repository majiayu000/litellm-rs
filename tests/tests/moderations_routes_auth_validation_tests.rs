use super::*;

#[tokio::test]
async fn moderation_route_requires_auth_when_anonymous_is_disabled() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state =
        build_test_app_state_with_auth(vec![moderation_provider(&mock.base_url)], true, false)
            .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({ "input": "hello" }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["type"], "authentication_error");
    assert!(
        mock.requests().is_empty(),
        "unauthenticated requests must fail before upstream call"
    );

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_allows_authenticated_api_key() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state =
        build_test_app_state_with_auth(vec![moderation_provider(&mock.base_url)], true, false)
            .await;
    let api_key = authenticated_api_key();
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert::<ApiKey>(api_key.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({ "input": "hello" }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(mock.requests().len(), 1);

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_rejects_invalid_request_before_upstream() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let cases = [
        (json!({}), "input"),
        (json!({ "input": "" }), "input"),
        (json!({ "input": "hello", "model": 1 }), "model"),
        (json!({ "input": "hello", "unknown": true }), "Unknown"),
    ];

    for (body, expected_message) in cases {
        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains(expected_message)
        );
    }

    assert!(
        mock.requests().is_empty(),
        "invalid requests must fail before provider call"
    );

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_rejects_unconfigured_model_before_upstream() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![moderation_provider_with_models(
        &mock.base_url,
        vec!["omni-moderation-latest".to_string()],
    )])
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/moderations")
            .set_json(json!({
                "model": "different-moderation-model",
                "input": "hello"
            }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body: Value = test::read_body_json(resp).await;
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("different-moderation-model")
    );
    assert!(mock.requests().is_empty());

    mock.stop_moderation_mock().await;
}
