//! Completions route integration tests

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{App, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use futures::stream;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::state::AppState;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

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
        ProviderConfig {
            name: "openai".to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "sk-completion-secret".to_string(),
            base_url: Some(base_url.to_string()),
            settings: HashMap::from([
                ("skip_api_key".to_string(), serde_json::Value::Bool(true)),
                (
                    "provider_name".to_string(),
                    serde_json::Value::String("openai".to_string()),
                ),
            ]),
            models: vec!["gpt-4o".to_string()],
            ..ProviderConfig::default()
        }
    }

    async fn build_test_app_state(base_url: &str) -> AppState {
        build_test_app_state_with_idle_timeout(base_url, None).await
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

    #[tokio::test]
    async fn test_completions_provider_failure_maps_to_rate_limit() {
        let mock_server = MockOpenAIServer::start(MockScenario::RateLimitFailure).await;
        let state = build_test_app_state(&mock_server.base_url).await;

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
        let state = build_test_app_state_with_idle_timeout(&mock_server.base_url, Some(1)).await;
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

        mock_server.shutdown().await;
    }

    #[tokio::test]
    async fn test_completions_streaming_response_sends_sse_and_done() {
        let mock_server = MockOpenAIServer::start(MockScenario::StreamingSuccess).await;
        let state = build_test_app_state(&mock_server.base_url).await;

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
}
