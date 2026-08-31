use super::*;

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn image_edit_routes_stability_to_native_inpaint_transport() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_server = Arc::clone(&captured);
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("edit should arrive");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = tokio::io::AsyncReadExt::read(&mut socket, &mut buffer)
                .await
                .expect("request should read");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }
        }
        *captured_for_server.lock().expect("capture lock") = request;
        let body = b"native-edited-png";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes())
            .await
            .expect("response headers should write");
        tokio::io::AsyncWriteExt::write_all(&mut socket, body)
            .await
            .expect("response body should write");
    });
    let provider = ProviderConfig {
        name: "stability".to_string(),
        provider_type: "stability".to_string(),
        api_key: "stability-secret".to_string(),
        base_url: Some(format!("http://{address}")),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        models: vec!["inpaint".to_string()],
        ..ProviderConfig::default()
    };
    let state = build_test_state(vec![provider]).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "native-stability-boundary";

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body_for_model(boundary, "inpaint"))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["data"][0]["b64_json"], "bmF0aXZlLWVkaXRlZC1wbmc=");
    server.await.expect("mock server should finish");
    let captured = captured.lock().expect("capture lock");
    let request = String::from_utf8_lossy(&captured);
    assert!(request.starts_with("POST /v2beta/stable-image/edit/inpaint HTTP/1.1"));
    assert!(request.contains("png-bytes"));
    assert!(request.contains("make it lighter"));
}

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
