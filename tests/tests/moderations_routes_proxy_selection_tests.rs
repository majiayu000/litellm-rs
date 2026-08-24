use super::*;

#[tokio::test]
async fn moderation_route_without_provider_fails_closed() {
    let state = build_test_app_state(Vec::new()).await;
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

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["type"], "server_error");
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("Moderation API requires")
    );
}

#[tokio::test]
async fn moderation_route_proxies_request_with_provider_headers() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
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
                "model": "omni-moderation-latest",
                "input": "moderate this text"
            }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], "modr_mock");
    assert_eq!(body["results"][0]["flagged"], false);

    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/moderations");
    assert_eq!(requests[0].body["input"], "moderate this text");
    assert_eq!(requests[0].body["model"], "omni-moderation-latest");
    assert_eq!(requests[0].headers["authorization"], "Bearer sk-test");
    assert_eq!(requests[0].headers["openai-organization"], "org-test");
    assert_eq!(requests[0].headers["openai-project"], "proj-test");
    assert_eq!(requests[0].headers["x-base-header"], "base-value");
    assert_eq!(requests[0].headers["x-custom-header"], "custom-value");

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_preserves_upstream_403() {
    let mock = MockModerationServer::start_moderation_mock_with_status(StatusCode::FORBIDDEN).await;
    let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let response = test::call_service(
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

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["message"], "moderation access denied");
    assert_eq!(body["error"]["type"], "permission_error");
    assert_eq!(body["error"]["code"], "permission_denied");
    assert_eq!(mock.requests().len(), 1);
    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_resolves_alias_to_provider_fallback_model() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
    state
        .unified_router
        .add_model_alias("public-moderation", "mock-openai-compatible")
        .expect("provider fallback alias should install");
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
                "model": "public-moderation",
                "input": "moderate this text"
            }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].body["model"], "mock-openai-compatible");

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn root_moderation_alias_proxies_request() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/moderations")
            .set_json(json!({
                "input": ["one", "two"]
            }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/moderations");
    assert_eq!(requests[0].body["input"][0], "one");
    assert_eq!(requests[0].body["model"], "omni-moderation-latest");

    mock.stop_moderation_mock().await;
}

#[tokio::test]
async fn moderation_route_uses_default_model_for_provider_selection_when_omitted() {
    let mock = MockModerationServer::start_moderation_mock().await;
    let state = build_test_app_state(vec![
        moderation_provider_with_models(
            "http://127.0.0.1:9/v1",
            vec!["unrelated-moderation-model".to_string()],
        ),
        moderation_provider_with_models(&mock.base_url, vec!["omni-moderation-latest".to_string()]),
    ])
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
