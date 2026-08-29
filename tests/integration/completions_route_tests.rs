//! Completions route integration tests

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use crate::common::providers::mock_provider_config;
    use actix_web::{
        App, HttpMessage, HttpResponse, HttpServer, dev::Service, http::StatusCode, test, web,
    };
    use bytes::Bytes;
    use futures::stream;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::integrations::{
        CallbackRuntime, Integration, IntegrationManager, IntegrationResult, LlmEndEvent,
        LlmErrorEvent, LlmStartEvent,
    };
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::core::types::context::RequestContext;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::state::AppState;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone)]
    enum RecordedCallback {
        Start(LlmStartEvent),
        End(LlmEndEvent),
        Error(LlmErrorEvent),
    }

    struct RecordingCallback {
        events: Arc<Mutex<Vec<RecordedCallback>>>,
        error_notify: Option<Arc<tokio::sync::Notify>>,
    }

    #[async_trait::async_trait]
    impl Integration for RecordingCallback {
        fn name(&self) -> &'static str {
            "route-test-recorder"
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()> {
            self.events
                .lock()
                .expect("callback events should not be poisoned")
                .push(RecordedCallback::Start(event.clone()));
            Ok(())
        }

        async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
            self.events
                .lock()
                .expect("callback events should not be poisoned")
                .push(RecordedCallback::End(event.clone()));
            Ok(())
        }

        async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
            self.events
                .lock()
                .expect("callback events should not be poisoned")
                .push(RecordedCallback::Error(event.clone()));
            if let Some(notify) = &self.error_notify {
                notify.notify_one();
            }
            Ok(())
        }

        async fn flush(&self) -> IntegrationResult<()> {
            Ok(())
        }

        async fn shutdown(&self) -> IntegrationResult<()> {
            Ok(())
        }
    }

    #[derive(Clone, Copy)]
    enum MockScenario {
        NonStreamingSuccess,
        RateLimitFailure,
        StreamingSuccess,
        StreamingIdle,
    }

    #[derive(Clone)]
    struct MockServerState {
        scenario: MockScenario,
        captured_requests: Arc<Mutex<Vec<Value>>>,
    }

    struct MockOpenAIServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<Value>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockOpenAIServer {
        async fn start(scenario: MockScenario) -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockServerState {
                scenario,
                captured_requests: Arc::clone(&captured_requests),
            };

            let listener = std::net::TcpListener::bind("127.0.0.1:0")
                .expect("mock server listener should bind");
            let address = listener
                .local_addr()
                .expect("mock server should have local addr");

            let server = HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .route("/chat/completions", web::post().to(mock_chat_completions))
            })
            .listen(listener)
            .expect("mock server should listen")
            .run();

            let handle = server.handle();
            let task = tokio::spawn(server);

            tokio::time::sleep(Duration::from_millis(20)).await;

            Self {
                base_url: format!("http://{}", address),
                captured_requests,
                handle,
                task,
            }
        }

        fn requests(&self) -> Vec<Value> {
            self.captured_requests.lock().unwrap().clone()
        }

        async fn shutdown(self) {
            self.handle.stop(true).await;
            let _ = self.task.await;
        }

        async fn abort(self) {
            self.handle.stop(false).await;
            let _ = self.task.await;
        }
    }

    async fn mock_chat_completions(
        state: web::Data<MockServerState>,
        payload: web::Json<Value>,
    ) -> HttpResponse {
        state
            .captured_requests
            .lock()
            .unwrap()
            .push(payload.into_inner());

        match state.scenario {
            MockScenario::NonStreamingSuccess => HttpResponse::Ok().json(json!({
                "id": "chatcmpl-success-1",
                "object": "chat.completion",
                "created": 1_707_000_000_i64,
                "model": "gpt-4o",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "mocked response"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 6,
                    "total_tokens": 16
                }
            })),
            MockScenario::RateLimitFailure => HttpResponse::TooManyRequests().json(json!({
                "error": {
                    "type": "rate_limit_error",
                    "code": "rate_limit_exceeded",
                    "message": "Rate limit exceeded for sk-completion-secret",
                    "retry_after": 2
                }
            })),
            MockScenario::StreamingSuccess => {
                let chunk_1 = r#"data: {"id":"chatcmpl-stream-1","object":"chat.completion.chunk","created":1707000001,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"},"finish_reason":null}]}"#;
                let chunk_2 = r#"data: {"id":"chatcmpl-stream-1","object":"chat.completion.chunk","created":1707000001,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;

                let stream = stream::iter(vec![
                    Ok::<Bytes, actix_web::Error>(Bytes::from(format!("{chunk_1}\n\n"))),
                    Ok::<Bytes, actix_web::Error>(Bytes::from(format!("{chunk_2}\n\n"))),
                    Ok::<Bytes, actix_web::Error>(Bytes::from("data: [DONE]\n\n")),
                ]);

                HttpResponse::Ok()
                    .insert_header(("Content-Type", "text/event-stream"))
                    .streaming(stream)
            }
            MockScenario::StreamingIdle => {
                let stream = stream::pending::<Result<Bytes, actix_web::Error>>();
                HttpResponse::Ok()
                    .insert_header(("Content-Type", "text/event-stream"))
                    .streaming(stream)
            }
        }
    }

    fn build_provider_config(base_url: &str) -> ProviderConfig {
        let mut provider = mock_provider_config(
            "openai",
            "openai_compatible",
            "sk-completion-secret",
            base_url,
            vec!["gpt-4o".to_string()],
        );
        provider.settings = HashMap::from([
            ("skip_api_key".to_string(), serde_json::Value::Bool(true)),
            (
                "provider_name".to_string(),
                serde_json::Value::String("openai".to_string()),
            ),
        ]);
        provider
    }

    async fn build_test_app_state(base_url: &str) -> AppState {
        build_test_app_state_with_idle_timeout(base_url, None).await
    }

    async fn build_test_app_state_with_cache(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.cache.enabled = true;
        config.gateway.providers = vec![build_provider_config(base_url)];

        let server = GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize for cache tests");
        server.state().clone()
    }

    async fn build_test_app_state_with_idle_timeout(
        base_url: &str,
        stream_idle_timeout: Option<u64>,
    ) -> AppState {
        let mut config = Config::default();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        if let Some(stream_idle_timeout) = stream_idle_timeout {
            config.gateway.server.stream_idle_timeout = stream_idle_timeout;
        }
        config.gateway.providers = vec![build_provider_config(base_url)];

        let server = GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize for tests");
        server.state().clone()
    }

    async fn build_callback_runtime(events: Arc<Mutex<Vec<RecordedCallback>>>) -> CallbackRuntime {
        let manager = Arc::new(IntegrationManager::with_defaults());
        manager
            .register(Arc::new(RecordingCallback {
                events,
                error_notify: None,
            }))
            .await;
        CallbackRuntime::new(manager, 8).expect("callback runtime should initialize")
    }

    async fn build_notifying_callback_runtime(
        events: Arc<Mutex<Vec<RecordedCallback>>>,
        error_notify: Arc<tokio::sync::Notify>,
    ) -> CallbackRuntime {
        let manager = Arc::new(IntegrationManager::with_defaults());
        manager
            .register(Arc::new(RecordingCallback {
                events,
                error_notify: Some(error_notify),
            }))
            .await;
        CallbackRuntime::new(manager, 8).expect("callback runtime should initialize")
    }

    fn completion_request(stream: Option<bool>) -> Value {
        json!({
            "model": "gpt-4o",
            "prompt": "Hello",
            "max_tokens": 16,
            "stream": stream
        })
    }

    fn completion_request_without_model() -> Value {
        json!({
            "prompt": "Hello",
            "max_tokens": 16
        })
    }

    fn api_key_with_invalid_runtime_permissions() -> ApiKey {
        let mut metadata = Metadata::new();
        metadata.extra.insert(
            "__core_keys".to_string(),
            json!({
                "permissions": {
                    "allowed_models": "gpt-*"
                }
            }),
        );

        ApiKey {
            metadata,
            name: "completion-invalid-policy-key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-completion-invalid".to_string(),
            user_id: None,
            team_id: None,
            permissions: Vec::new(),
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        }
    }

    #[tokio::test]
    async fn test_completions_non_stream_success_openai_envelope() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
        let state = build_test_app_state(&mock_server.base_url).await;

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
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("success").is_none());
        assert_eq!(body["id"], "chatcmpl-success-1");
        assert_eq!(body["object"], "text_completion");
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["choices"][0]["text"], "mocked response");
        assert_eq!(body["usage"]["total_tokens"], 16);

        let requests = mock_server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["messages"][0]["role"], "user");
        assert_eq!(requests[0]["messages"][0]["content"], "Hello");
        assert!(requests[0].get("stream").is_none());
        assert!(requests[0].get("stream_options").is_none());

        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_completions_route_emits_metadata_only_callback_lifecycle() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
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
        let request = test::TestRequest::post()
            .uri("/v1/completions")
            .set_json(completion_request(None))
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let _: Value = test::read_body_json(response).await;
        runtime
            .shutdown()
            .await
            .expect("callback runtime should drain");

        let events = events
            .lock()
            .expect("callback events should not be poisoned")
            .clone();
        assert_eq!(events.len(), 2);
        let (RecordedCallback::Start(start), RecordedCallback::End(end)) = (&events[0], &events[1])
        else {
            panic!("route should emit one ordered start/end callback pair");
        };
        assert_eq!(start.request_id, end.request_id);
        assert_eq!(start.model, "gpt-4o");
        assert_eq!(start.input, Value::Null);
        assert_eq!(end.output, Value::Null);
        assert_eq!(end.provider.as_deref(), Some("openai"));
        assert_eq!(end.input_tokens, Some(10));
        assert_eq!(end.output_tokens, Some(6));
        assert!(end.cost_usd.is_some());
        assert!(end.metadata.contains_key("outcome"));

        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_completions_non_stream_uses_response_cache() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
        let state = build_test_app_state_with_cache(&mock_server.base_url).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        for _ in 0..2 {
            let req = test::TestRequest::post()
                .uri("/v1/completions")
                .set_json(completion_request(None))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::OK);
            let body: Value = test::read_body_json(resp).await;
            assert_eq!(body["choices"][0]["text"], "mocked response");
        }

        let requests = mock_server.requests();
        assert_eq!(
            requests.len(),
            1,
            "second identical request should hit cache"
        );

        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_responses_api_bypasses_chat_response_cache() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
        let state = build_test_app_state_with_cache(&mock_server.base_url).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let chat_req = {
            let req = test::TestRequest::post()
                .uri("/v1/chat/completions")
                .set_json(json!({
                    "model": "gpt-4o",
                    "messages": [{"role": "user", "content": "Hello"}],
                    "max_tokens": 16,
                    "max_completion_tokens": 16
                }))
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("cache-owner"));
            req
        };
        let chat_resp = test::call_service(&app, chat_req).await;
        assert_eq!(chat_resp.status(), StatusCode::OK);

        let responses_req = {
            let req = test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(json!({
                    "model": "gpt-4o",
                    "input": "Hello",
                    "max_output_tokens": 16
                }))
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("cache-owner"));
            req
        };
        let responses_resp = test::call_service(&app, responses_req).await;
        assert_eq!(responses_resp.status(), StatusCode::OK);

        let requests = mock_server.requests();
        assert_eq!(
            requests.len(),
            2,
            "Responses API calls should not reuse chat-completion cache entries"
        );

        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_completions_runtime_policy_errors_use_openai_shape() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
        let state = build_test_app_state(&mock_server.base_url).await;
        let api_key = api_key_with_invalid_runtime_permissions();

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

        let req = test::TestRequest::post()
            .uri("/v1/completions")
            .set_json(completion_request(None))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("success").is_none());
        assert_eq!(body["error"]["type"], "permission_error");
        assert_eq!(body["error"]["code"], "permission_denied");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("API key runtime policy is invalid")
        );
        assert!(mock_server.requests().is_empty());
        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_responses_runtime_policy_errors_use_openai_shape() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
        let state = build_test_app_state(&mock_server.base_url).await;
        let api_key = api_key_with_invalid_runtime_permissions();

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

        let req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(json!({
                "model": "gpt-4o",
                "input": "Hello",
                "max_output_tokens": 16
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("success").is_none());
        assert_eq!(body["error"]["type"], "permission_error");
        assert_eq!(body["error"]["code"], "permission_denied");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("API key runtime policy is invalid")
        );
        assert!(mock_server.requests().is_empty());
        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_completions_alias_routes_and_path_model_override() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
        let state = build_test_app_state(&mock_server.base_url).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let cases = [
            ("/completions", completion_request(None)),
            (
                "/engines/gpt-4o/completions",
                completion_request_without_model(),
            ),
            (
                "/v1/engines/gpt-4o/completions",
                json!({
                    "model": "wrong-model",
                    "prompt": "Hello",
                    "max_tokens": 16
                }),
            ),
            (
                "/openai/deployments/gpt-4o/completions",
                json!({
                    "model": "wrong-model",
                    "prompt": "Hello",
                    "max_tokens": 16
                }),
            ),
        ];

        for (uri, body) in cases {
            let resp = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(uri)
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::OK, "route should work: {uri}");
            let body: Value = test::read_body_json(resp).await;
            assert_eq!(body["object"], "text_completion");
        }

        let requests = mock_server.requests();
        assert_eq!(requests.len(), 4);
        assert!(requests.iter().all(|request| request["model"] == "gpt-4o"));

        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_completions_bad_request_validation() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
        let state = build_test_app_state(&mock_server.base_url).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/completions")
            .set_json(json!({
                "model": "",
                "prompt": "Hello"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("success").is_none());
        let error_message = body["error"]["message"]
            .as_str()
            .expect("validation response should contain OpenAI error message");
        assert!(error_message.contains("Model name cannot be empty"));
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["param"], Value::Null);
        assert_eq!(body["error"]["code"], "invalid_request");

        let requests = mock_server.requests();
        assert!(requests.is_empty());

        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_completions_rejects_invalid_scalar_fields() {
        let mock_server = MockOpenAIServer::start(MockScenario::NonStreamingSuccess).await;
        let state = build_test_app_state(&mock_server.base_url).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let cases = [
            (
                json!({"model":"gpt-4o","prompt":"Hello","max_tokens":-1}),
                "max_tokens",
            ),
            (
                json!({"model":"gpt-4o","prompt":"Hello","stream":"true"}),
                "stream",
            ),
            (
                json!({"model":"gpt-4o","prompt":"Hello","echo":"false"}),
                "echo",
            ),
            (json!({"model":"gpt-4o","prompt":"Hello","n":-1}), "n"),
            (
                json!({"model":"gpt-4o","prompt":"Hello","logprobs":"1"}),
                "logprobs",
            ),
            (
                json!({"model":"gpt-4o","prompt":"Hello","logprobs":1}),
                "logprobs",
            ),
            (
                json!({"model":"gpt-4o","prompt":"Hello","stream_options":{"include_usage":true}}),
                "stream_options",
            ),
            (
                json!({"model":"gpt-4o","prompt":"Hello","stream_options":{"include_usage":"true"}}),
                "include_usage",
            ),
        ];

        for (body, expected_message) in cases {
            let resp = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/completions")
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
            let body: Value = test::read_body_json(resp).await;
            let message = body["error"]["message"]
                .as_str()
                .expect("invalid scalar response should have message");
            assert!(
                message.contains(expected_message),
                "message '{message}' should mention '{expected_message}'"
            );
        }

        assert!(mock_server.requests().is_empty());
        mock_server.shutdown().await;
    }

    #[path = "streaming_and_budget_tests.rs"]
    mod streaming_and_budget_tests;
}
