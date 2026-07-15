use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn gemini_sdk_route_executes_selected_runtime_provider_snapshot() {
    let selected = MockGeminiServer::launch().await;
    let replacement = MockGeminiServer::launch().await;
    let configured = |name: &str, base_url: &str, api_key: &str, header_value: &str| {
        let mut provider = gemini_provider(name, base_url, Vec::new());
        provider.api_key = api_key.to_string();
        provider
            .settings
            .insert("provider_name".to_string(), json!("gemini"));
        provider.settings.insert(
            "custom_headers".to_string(),
            json!({"X-Custom-Header": header_value}),
        );
        provider
    };
    let state = build_test_state(vec![configured(
        "runtime-alias",
        &selected.base_url,
        "selected-runtime-key",
        "selected-runtime-header",
    )])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-3.1-flash-lite",
        ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let mut replaced_config = state.config().as_ref().clone();
    replaced_config.gateway.providers = vec![configured(
        "runtime-alias",
        &replacement.base_url,
        "replacement-key",
        "replacement-header",
    )];
    state.config.store(replaced_config);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let requests = selected.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].path_and_query,
        "/v1beta/models/gemini-3.1-flash-lite:generateContent?key=selected-runtime-key"
    );
    assert_eq!(
        requests[0].headers["x-custom-header"],
        "selected-runtime-header"
    );
    assert!(replacement.requests().is_empty());
    let provider_usage = budget_limits
        .providers
        .get_provider_usage("gemini")
        .expect("selected runtime provider budget should exist");
    assert!(provider_usage.current_spend > 0.0);
    let model_usage = budget_limits
        .models
        .get_model_usage("gemini-3.1-flash-lite")
        .expect("requested model budget should exist");
    assert!(model_usage.current_spend > 0.0);
    selected.shutdown().await;
    replacement.shutdown().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("delayed upstream should bind");
    let address = listener.local_addr().expect("delayed upstream address");
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should connect");
        let mut request = [0_u8; 4096];
        socket
            .read_exact(&mut request[..1])
            .await
            .expect("request should read");
        tokio::time::sleep(Duration::from_millis(1_200)).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}")
            .await
            .expect("response should write");
    });
    let mut selected = gemini_provider(
        "gemini",
        &format!("http://{address}"),
        vec!["gemini-3.1-flash-lite".to_string()],
    );
    selected.timeout = 2;
    let mut replacement = selected.clone();
    let state = build_test_state(vec![selected]).await;
    replacement.base_url = Some("http://127.0.0.1:9".to_string());
    replacement.timeout = 1;
    let mut replaced_config = state.config().as_ref().clone();
    replaced_config.gateway.providers = vec![replacement];
    state.config.store(replaced_config);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let started = Instant::now();
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(started.elapsed() >= Duration::from_millis(1_100));
    upstream.await.expect("delayed upstream should finish");
}

#[tokio::test]
async fn gemini_sdk_route_network_error_does_not_leak_provider_key() {
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        "http://127.0.0.1:9",
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body: Value = test::read_body_json(response).await;
    let message = body["error"]["message"].as_str().expect("error message");
    assert!(message.contains("Gemini upstream request failed"));
    assert!(!message.contains("test-api-key-12345678901234567890"));
}

#[tokio::test]
async fn gemini_sdk_route_redacts_key_from_upstream_error_body() {
    let mock = MockGeminiServer::launch().await;
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        &mock.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_upstream_error_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = test::read_body(response).await;
    let text = String::from_utf8(body.to_vec()).expect("body should be utf8");
    assert!(text.contains("upstream failed"));
    assert!(text.contains("key=[REDACTED]"));
    assert!(!text.contains("test-api-key-12345678901234567890"));

    mock.shutdown().await;
}

#[tokio::test]
async fn gemini_sdk_stream_route_does_not_charge_upstream_error_body() {
    let mock = MockGeminiServer::launch().await;
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        &mock.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-3.1-flash-lite",
        ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
            .set_json(gemini_upstream_error_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let stream_body = test::read_body(response).await;
    let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
    assert!(stream_text.contains("upstream failed"));
    assert!(stream_text.contains("key=[REDACTED]"));
    assert!(!stream_text.contains("test-api-key-12345678901234567890"));
    let provider_usage = budget_limits
        .providers
        .get_provider_usage("gemini")
        .expect("provider budget should exist");
    assert_eq!(provider_usage.current_spend, 0.0);
    let model_usage = budget_limits
        .models
        .get_model_usage("gemini-3.1-flash-lite")
        .expect("model budget should exist");
    assert_eq!(model_usage.current_spend, 0.0);

    mock.shutdown().await;
}

#[tokio::test]
async fn gemini_sdk_stream_route_releases_budget_on_midstream_read_error() {
    let broken = BrokenGeminiStreamServer::launch().await;
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        &broken.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-3.1-flash-lite",
        ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let stream_body = test::read_body(response).await;
    let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
    assert!(stream_text.contains("partial"));
    assert!(stream_text.contains("Gemini upstream stream error"));
    let provider_usage = budget_limits
        .providers
        .get_provider_usage("gemini")
        .expect("provider budget should exist");
    assert_eq!(provider_usage.current_spend, 0.0);
    let model_usage = budget_limits
        .models
        .get_model_usage("gemini-3.1-flash-lite")
        .expect("model budget should exist");
    assert_eq!(model_usage.current_spend, 0.0);

    broken.shutdown().await;
}

#[tokio::test]
async fn gemini_sdk_stream_body_is_not_cut_off_by_ordinary_timeout() {
    let delayed = DelayedGeminiStreamServer::launch(Duration::from_millis(1_200)).await;
    let mut provider = gemini_provider(
        "gemini",
        &delayed.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    );
    provider.timeout = 1;
    let state = build_test_state(vec![provider]).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let started = Instant::now();

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = tokio::time::timeout(Duration::from_secs(3), test::read_body(response))
        .await
        .expect("delayed SSE body should complete")
        .to_vec();
    let body = String::from_utf8(body).expect("stream body should be utf8");
    assert!(body.contains("delayed"));
    assert!(started.elapsed() >= Duration::from_millis(1_100));
    delayed.shutdown().await;
}
