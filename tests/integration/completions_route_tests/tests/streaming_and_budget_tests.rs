use super::*;

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
