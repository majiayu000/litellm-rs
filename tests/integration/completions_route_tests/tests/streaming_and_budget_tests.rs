use super::*;

#[tokio::test]
#[cfg(feature = "providers-extended")]
async fn gemini_invalid_terminal_usage_is_not_serialized_or_settled_as_valid() {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("Gemini mock listener should bind");
    let address = listener
        .local_addr()
        .expect("Gemini mock should have local address");
    let upstream = HttpServer::new(|| {
        App::new().default_service(web::post().to(|| async {
            let valid = concat!(
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]}}],",
                "\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":2,",
                "\"totalTokenCount\":3}}\n\n"
            );
            let invalid = concat!(
                "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":4,",
                "\"totalTokenCount\":4}}\n\n"
            );
            HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .streaming(stream::iter([
                    Ok::<Bytes, actix_web::Error>(Bytes::from_static(valid.as_bytes())),
                    Ok::<Bytes, actix_web::Error>(Bytes::from_static(invalid.as_bytes())),
                    Ok::<Bytes, actix_web::Error>(Bytes::from_static(b"data: [DONE]\n\n")),
                ]))
        }))
    })
    .listen(listener)
    .expect("Gemini mock should listen")
    .run();
    let upstream_handle = upstream.handle();
    let upstream_task = tokio::spawn(upstream);
    tokio::time::sleep(Duration::from_millis(20)).await;

    let mut config = Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = vec![mock_provider_config(
        "gemini",
        "gemini",
        "test-key-12345678901234567890",
        &format!("http://{address}"),
        vec!["gemini-2.5-flash".to_string()],
    )];
    let gateway = GatewayHttpServer::new(&config)
        .await
        .expect("gateway should initialize with Gemini");
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = build_callback_runtime(Arc::clone(&events)).await;
    let state = gateway.state().clone().with_callbacks(runtime.dispatcher());
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-2.5-flash",
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
            .uri("/v1/completions")
            .set_json(json!({
                "model": "gemini-2.5-flash",
                "prompt": "hello",
                "stream": true,
                "max_tokens": 8,
                "stream_options": {"include_usage": true}
            }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(response).await.to_vec())
        .expect("completion stream should be utf8");
    assert!(body.contains("\"text\":\"ok\""));
    assert!(!body.contains("__litellm"));
    assert!(!body.contains("\"prompt_tokens\":0"));

    runtime
        .shutdown()
        .await
        .expect("callback runtime should drain");
    {
        let events = events
            .lock()
            .expect("callback events should not be poisoned");
        let RecordedCallback::End(end) = events.last().expect("terminal callback should exist")
        else {
            panic!("completion stream should end successfully");
        };
        assert_eq!(end.input_tokens, None);
        assert_eq!(end.output_tokens, None);
        assert_eq!(end.cost_usd, None);
    }
    assert!(
        budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("no-usage reservation should settle")
            .current_spend
            > 0.0
    );

    upstream_handle.stop(true).await;
    upstream_task
        .await
        .expect("Gemini mock task should join")
        .expect("Gemini mock should stop cleanly");
}

#[tokio::test]
#[cfg(feature = "providers-extended")]
async fn gemini_invalid_terminal_usage_is_not_serialized_by_chat_or_settled_as_valid() {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("Gemini mock listener should bind");
    let address = listener
        .local_addr()
        .expect("Gemini mock should have local address");
    let upstream = HttpServer::new(|| {
        App::new().default_service(web::post().to(|| async {
            let valid = concat!(
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]}}],",
                "\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":2,",
                "\"totalTokenCount\":3}}\n\n"
            );
            let invalid = concat!(
                "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":4,",
                "\"totalTokenCount\":4}}\n\n"
            );
            HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .streaming(stream::iter([
                    Ok::<Bytes, actix_web::Error>(Bytes::from_static(valid.as_bytes())),
                    Ok::<Bytes, actix_web::Error>(Bytes::from_static(invalid.as_bytes())),
                    Ok::<Bytes, actix_web::Error>(Bytes::from_static(b"data: [DONE]\n\n")),
                ]))
        }))
    })
    .listen(listener)
    .expect("Gemini mock should listen")
    .run();
    let upstream_handle = upstream.handle();
    let upstream_task = tokio::spawn(upstream);
    tokio::time::sleep(Duration::from_millis(20)).await;

    let mut config = Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = vec![mock_provider_config(
        "gemini",
        "gemini",
        "test-key-12345678901234567890",
        &format!("http://{address}"),
        vec!["gemini-2.5-flash".to_string()],
    )];
    let gateway = GatewayHttpServer::new(&config)
        .await
        .expect("gateway should initialize with Gemini");
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = build_callback_runtime(Arc::clone(&events)).await;
    let state = gateway.state().clone().with_callbacks(runtime.dispatcher());
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-2.5-flash",
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
            .uri("/v1/chat/completions")
            .set_json(json!({
                "model": "gemini-2.5-flash",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": true,
                "max_tokens": 8,
                "stream_options": {"include_usage": true}
            }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(response).await.to_vec())
        .expect("chat stream should be utf8");
    assert!(body.contains("\"content\":\"ok\""));
    assert!(!body.contains("__litellm"));
    assert!(!body.contains("\"prompt_tokens\":0"));

    runtime
        .shutdown()
        .await
        .expect("callback runtime should drain");
    {
        let events = events
            .lock()
            .expect("callback events should not be poisoned");
        let RecordedCallback::End(end) = events.last().expect("terminal callback should exist")
        else {
            panic!("chat stream should end successfully");
        };
        assert_eq!(end.input_tokens, None);
        assert_eq!(end.output_tokens, None);
        assert_eq!(end.cost_usd, None);
    }
    assert!(
        budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("no-usage reservation should settle")
            .current_spend
            > 0.0
    );

    upstream_handle.stop(true).await;
    upstream_task
        .await
        .expect("Gemini mock task should join")
        .expect("Gemini mock should stop cleanly");
}

#[tokio::test]
async fn test_completions_provider_failure_maps_to_rate_limit() {
    let mock_server = MockOpenAIServer::start(MockScenario::RateLimitFailure).await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = build_callback_runtime(Arc::clone(&events)).await;
    let state = build_test_app_state(&mock_server.base_url)
        .await
        .with_callbacks(runtime.dispatcher());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/completions")
        .set_json(completion_request(Some(false)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    let body: Value = test::read_body_json(resp).await;
    assert!(body.get("success").is_none());
    assert_eq!(body["error"]["type"], "rate_limit_error");
    assert_eq!(body["error"]["param"], Value::Null);
    assert_eq!(body["error"]["code"], "rate_limit_exceeded");
    assert!(body["error"].get("retryable").is_none());
    let message = body["error"]["message"]
        .as_str()
        .expect("provider error body should have OpenAI error message");
    assert!(message.to_lowercase().contains("rate limit"));
    assert!(!message.contains("sk-completion-secret"));

    let requests = mock_server.requests();
    assert!(!requests.is_empty());
    assert!(requests.iter().all(|request| request["model"] == "gpt-4o"));
    runtime
        .shutdown()
        .await
        .expect("callback runtime should drain");
    let events = events
        .lock()
        .expect("callback events should not be poisoned")
        .clone();
    assert_eq!(events.len(), 2);
    let RecordedCallback::Error(error) = &events[1] else {
        panic!("provider failure should emit one terminal error callback");
    };
    assert_eq!(error.error_type.as_deref(), Some("provider_error"));
    assert_eq!(error.error_message, "provider request failed");
    assert!(!error.error_message.contains("sk-completion-secret"));

    mock_server.shutdown().await;
}

#[tokio::test]
async fn test_completions_streaming_echo_prefixes_prompt_once() {
    let mock_server = MockOpenAIServer::start(MockScenario::StreamingSuccess).await;
    let state = build_test_app_state(&mock_server.base_url).await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let mut body = completion_request(Some(true));
    body["echo"] = Value::Bool(true);

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/completions")
            .set_json(body)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let body_text = String::from_utf8(body.to_vec()).expect("streaming body should be utf8");
    assert!(body_text.contains("\"text\":\"HelloHel\""));
    assert!(body_text.contains("\"text\":\"lo\""));
    assert_eq!(body_text.matches("Hello").count(), 1);

    mock_server.shutdown().await;
}

#[tokio::test]
async fn test_completions_stream_timeout_before_output_does_not_record_spend() {
    let mock_server = MockOpenAIServer::start(MockScenario::StreamingIdle).await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = build_callback_runtime(Arc::clone(&events)).await;
    let state = build_test_app_state_with_idle_timeout(&mock_server.base_url, Some(1))
        .await
        .with_callbacks(runtime.dispatcher());
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .models
        .set_model_limit("gpt-4o", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));
    let budget_limits = state.budget_limits.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/completions")
            .set_json(completion_request(Some(true)))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body_text = String::from_utf8(body.to_vec()).expect("streaming body should be utf8");
    assert!(body_text.contains("Stream idle timeout"));

    let provider_spend = budget_limits
        .providers
        .get_provider_usage("openai")
        .map(|usage| usage.current_spend)
        .unwrap_or(0.0);
    let model_spend = budget_limits
        .models
        .get_model_usage("gpt-4o")
        .map(|usage| usage.current_spend)
        .unwrap_or(0.0);
    assert_eq!(provider_spend, 0.0);
    assert_eq!(model_spend, 0.0);
    runtime
        .shutdown()
        .await
        .expect("callback runtime should drain");
    let events = events
        .lock()
        .expect("callback events should not be poisoned")
        .clone();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], RecordedCallback::Start(_)));
    let RecordedCallback::Error(error) = &events[1] else {
        panic!("stream timeout should emit one terminal error callback");
    };
    assert_eq!(error.error_type.as_deref(), Some("timeout"));
    assert_eq!(error.error_message, "provider request timed out");

    mock_server.shutdown().await;
}

async fn assert_zero_idle_timeout_observes_client_disconnect(uri: &str, request: Value) {
    let mock_server = MockOpenAIServer::start(MockScenario::StreamingIdle).await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let error_notify = Arc::new(tokio::sync::Notify::new());
    let runtime =
        build_notifying_callback_runtime(Arc::clone(&events), Arc::clone(&error_notify)).await;
    let state = build_test_app_state_with_idle_timeout(&mock_server.base_url, Some(0))
        .await
        .with_callbacks(runtime.dispatcher());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(uri)
            .set_json(request)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let disconnected = error_notify.notified();
    drop(response);
    tokio::time::timeout(Duration::from_secs(2), disconnected)
        .await
        .expect("stream worker should observe the dropped response body");
    runtime
        .shutdown()
        .await
        .expect("callback runtime should drain");

    let events = events
        .lock()
        .expect("callback events should not be poisoned")
        .clone();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], RecordedCallback::Start(_)));
    let RecordedCallback::Error(error) = &events[1] else {
        panic!("client disconnect should emit one terminal error callback");
    };
    assert_eq!(error.error_type.as_deref(), Some("client_disconnect"));

    mock_server.abort().await;
}

#[tokio::test]
async fn test_completions_zero_idle_timeout_observes_client_disconnect() {
    assert_zero_idle_timeout_observes_client_disconnect(
        "/v1/completions",
        completion_request(Some(true)),
    )
    .await;
}

#[tokio::test]
async fn test_chat_zero_idle_timeout_observes_client_disconnect() {
    assert_zero_idle_timeout_observes_client_disconnect(
        "/v1/chat/completions",
        json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        }),
    )
    .await;
}

#[tokio::test]
async fn test_completions_streaming_response_sends_sse_and_done() {
    let mock_server = MockOpenAIServer::start(MockScenario::StreamingSuccess).await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = build_callback_runtime(Arc::clone(&events)).await;
    let state = build_test_app_state(&mock_server.base_url)
        .await
        .with_callbacks(runtime.dispatcher());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/completions")
        .set_json(completion_request(Some(true)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.starts_with("text/event-stream"));

    let body = test::read_body(resp).await;
    let body_text = String::from_utf8(body.to_vec()).expect("streaming body should be utf8");
    assert!(body_text.contains("data: {"));
    assert!(body_text.contains("\"object\":\"text_completion\""));
    assert!(body_text.contains("\"text\":\"Hel\""));
    assert!(body_text.contains("\"text\":\"lo\""));
    assert!(body_text.contains("[DONE]"));

    let requests = mock_server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["stream"], true);
    runtime
        .shutdown()
        .await
        .expect("callback runtime should drain");
    let events = events
        .lock()
        .expect("callback events should not be poisoned")
        .clone();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], RecordedCallback::Start(_)));
    assert!(matches!(events[1], RecordedCallback::End(_)));

    mock_server.shutdown().await;
}

#[tokio::test]
async fn test_completions_success_records_budget_spend() {
    let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
    let state = build_test_app_state(&mock_server.base_url).await;
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .models
        .set_model_limit("gpt-4o", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));
    let budget_limits = state.budget_limits.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/completions")
        .set_json(completion_request(None))
        .to_request();

    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected completions budget response: {}",
        String::from_utf8_lossy(&body)
    );

    let provider_usage = budget_limits
        .providers
        .get_provider_usage("openai")
        .expect("provider spend should be recorded");
    assert!(provider_usage.current_spend > 0.0);
    let model_usage = budget_limits
        .models
        .get_model_usage("gpt-4o")
        .expect("model spend should be recorded");
    assert!(model_usage.current_spend > 0.0);

    mock_server.shutdown().await;
}

#[tokio::test]
async fn test_completions_budget_rejection_emits_no_callback_lifecycle() {
    let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime = build_callback_runtime(Arc::clone(&events)).await;
    let state = build_test_app_state(&mock_server.base_url)
        .await
        .with_callbacks(runtime.dispatcher());
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("openai", 2.0);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/completions")
            .set_json(completion_request(None))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert!(mock_server.requests().is_empty());
    runtime
        .shutdown()
        .await
        .expect("callback runtime should drain");
    let events = events
        .lock()
        .expect("callback events should not be poisoned")
        .clone();
    assert!(
        events.is_empty(),
        "pre-provider budget rejection must not emit lifecycle callbacks"
    );

    mock_server.shutdown().await;
}
