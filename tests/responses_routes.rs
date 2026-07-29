#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures::mock_provider_config;
    use actix_web::{App, HttpMessage, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use futures::stream;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    #[cfg(feature = "providers-extended")]
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::integrations::{
        CallbackRuntime, Integration, IntegrationManager, IntegrationResult, LlmEndEvent,
        LlmErrorEvent, LlmStartEvent,
    };
    use litellm_rs::core::types::context::RequestContext;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::state::AppState;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Copy)]
    enum MockScenario {
        NonStreaming,
        Streaming,
        StreamingIdle,
    }

    #[derive(Clone)]
    enum RecordedCallback {
        Start,
        #[allow(dead_code)]
        End(LlmEndEvent),
        Error(LlmErrorEvent),
    }

    struct RecordingCallback {
        events: Arc<Mutex<Vec<RecordedCallback>>>,
        error_notify: Arc<tokio::sync::Notify>,
    }

    #[async_trait::async_trait]
    impl Integration for RecordingCallback {
        fn name(&self) -> &'static str {
            "responses-route-test-recorder"
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn on_llm_start(&self, _event: &LlmStartEvent) -> IntegrationResult<()> {
            self.events.lock().unwrap().push(RecordedCallback::Start);
            Ok(())
        }

        async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
            self.events
                .lock()
                .unwrap()
                .push(RecordedCallback::End(event.clone()));
            Ok(())
        }

        async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
            self.events
                .lock()
                .unwrap()
                .push(RecordedCallback::Error(event.clone()));
            self.error_notify.notify_one();
            Ok(())
        }

        async fn flush(&self) -> IntegrationResult<()> {
            Ok(())
        }

        async fn shutdown(&self) -> IntegrationResult<()> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MockServerState {
        scenario: MockScenario,
        captured_requests: Arc<Mutex<Vec<Value>>>,
    }

    struct MockOpenAiServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<Value>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockOpenAiServer {
        async fn start(scenario: MockScenario) -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockServerState {
                scenario,
                captured_requests: Arc::clone(&captured_requests),
            };
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
            let address = listener
                .local_addr()
                .expect("mock server should have address");
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
                base_url: format!("http://{address}"),
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
            let result = self.task.await.expect("mock server task should join");
            if let Err(error) = result {
                panic!("mock server should stop cleanly: {error}");
            }
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
            MockScenario::NonStreaming => HttpResponse::Ok().json(json!({
                "id": "chatcmpl-response-route",
                "object": "chat.completion",
                "created": 4_102_444_800_i64,
                "model": "gpt-4o-mini",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "mocked response"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 4,
                    "completion_tokens": 3,
                    "total_tokens": 7
                }
            })),
            MockScenario::Streaming => {
                let chunk_1 = r#"data: {"id":"chatcmpl-response-stream","object":"chat.completion.chunk","created":1707000001,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"},"finish_reason":null}]}"#;
                let chunk_2 = r#"data: {"id":"chatcmpl-response-stream","object":"chat.completion.chunk","created":1707000001,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":"stop"}]}"#;
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

    fn provider_config(base_url: &str) -> ProviderConfig {
        let mut provider = mock_provider_config(
            "mock-openai-compatible",
            "openai_compatible",
            "test-key",
            base_url,
            vec!["gpt-4o-mini".to_string()],
        );
        provider.settings = HashMap::from([("skip_api_key".to_string(), Value::Bool(true))]);
        provider
    }

    async fn app_state(base_url: &str) -> AppState {
        app_state_with_idle_timeout(base_url, None).await
    }

    async fn app_state_with_idle_timeout(
        base_url: &str,
        stream_idle_timeout: Option<u64>,
    ) -> AppState {
        let mut config = Config::default();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        if let Some(stream_idle_timeout) = stream_idle_timeout {
            config.gateway.server.stream_idle_timeout = stream_idle_timeout;
        }
        config.gateway.providers = vec![provider_config(base_url)];

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
    }

    async fn callback_runtime(
        events: Arc<Mutex<Vec<RecordedCallback>>>,
        error_notify: Arc<tokio::sync::Notify>,
    ) -> CallbackRuntime {
        let manager = Arc::new(IntegrationManager::with_defaults());
        manager
            .register(Arc::new(RecordingCallback {
                events,
                error_notify,
            }))
            .await;
        CallbackRuntime::new(manager, 8).expect("callback runtime should initialize")
    }

    macro_rules! with_user {
        ($req:expr, $user_id:expr) => {{
            let req = $req;
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id($user_id));
            req
        }};
    }

    fn response_request(stream: Option<bool>) -> Value {
        json!({
            "model": "gpt-4o-mini",
            "input": "Hello",
            "stream": stream
        })
    }

    #[tokio::test]
    async fn create_retrieve_input_items_and_delete_response() {
        let mock = MockOpenAiServer::start(MockScenario::NonStreaming).await;
        let state = app_state(&mock.base_url).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let create_req = with_user!(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(response_request(None))
                .to_request(),
            "owner-a"
        );
        let create_resp = test::call_service(&app, create_req).await;
        let create_status = create_resp.status();
        let create_body = test::read_body(create_resp).await;
        assert_eq!(
            create_status,
            StatusCode::OK,
            "create response body: {}",
            String::from_utf8_lossy(&create_body)
        );
        let created: Value =
            serde_json::from_slice(&create_body).expect("create response should be JSON");
        let response_id = created["id"].as_str().expect("response id").to_string();
        assert_eq!(created["object"], "response");

        let get_req = with_user!(
            test::TestRequest::get()
                .uri(&format!("/v1/responses/{response_id}"))
                .to_request(),
            "owner-a"
        );
        let get_resp = test::call_service(&app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::OK);
        let fetched: Value = test::read_body_json(get_resp).await;
        assert_eq!(fetched["id"], response_id);

        let cross_owner_req = with_user!(
            test::TestRequest::get()
                .uri(&format!("/v1/responses/{response_id}"))
                .to_request(),
            "owner-b"
        );
        let cross_owner_resp = test::call_service(&app, cross_owner_req).await;
        assert_eq!(cross_owner_resp.status(), StatusCode::NOT_FOUND);

        let items_req = with_user!(
            test::TestRequest::get()
                .uri(&format!("/v1/responses/{response_id}/input_items?limit=1"))
                .to_request(),
            "owner-a"
        );
        let items_resp = test::call_service(&app, items_req).await;
        assert_eq!(items_resp.status(), StatusCode::OK);
        let items: Value = test::read_body_json(items_resp).await;
        assert_eq!(items["object"], "list");
        assert!(
            items["data"][0]["id"]
                .as_str()
                .unwrap()
                .starts_with("item_")
        );
        assert_eq!(items["data"][0]["type"], "message");
        assert_eq!(items["first_id"], items["data"][0]["id"]);
        assert_eq!(items["last_id"], items["data"][0]["id"]);

        let include_req = with_user!(
            test::TestRequest::get()
                .uri(&format!(
                    "/v1/responses/{response_id}/input_items?include=file_search_call.results"
                ))
                .to_request(),
            "owner-a"
        );
        let include_resp = test::call_service(&app, include_req).await;
        assert_eq!(include_resp.status(), StatusCode::BAD_REQUEST);

        let delete_req = with_user!(
            test::TestRequest::delete()
                .uri(&format!("/v1/responses/{response_id}"))
                .to_request(),
            "owner-a"
        );
        let delete_resp = test::call_service(&app, delete_req).await;
        assert_eq!(delete_resp.status(), StatusCode::OK);
        let deleted: Value = test::read_body_json(delete_resp).await;
        assert_eq!(deleted["id"], response_id);
        assert_eq!(deleted["object"], "response");
        assert_eq!(deleted["deleted"], true);

        assert_eq!(mock.requests().len(), 1);
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn streamed_response_is_stored_after_completion_event() {
        let mock = MockOpenAiServer::start(MockScenario::Streaming).await;
        let state = app_state(&mock.base_url).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let stream_req = with_user!(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(response_request(Some(true)))
                .to_request(),
            "stream-owner"
        );
        let stream_resp = test::call_service(&app, stream_req).await;
        let stream_status = stream_resp.status();
        let body = test::read_body(stream_resp).await;
        assert_eq!(
            stream_status,
            StatusCode::OK,
            "stream response body: {}",
            String::from_utf8_lossy(&body)
        );
        let response_id = response_id_from_sse(&String::from_utf8_lossy(&body));

        let get_req = with_user!(
            test::TestRequest::get()
                .uri(&format!("/v1/responses/{response_id}"))
                .to_request(),
            "stream-owner"
        );
        let get_resp = test::call_service(&app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::OK);
        let fetched: Value = test::read_body_json(get_resp).await;
        assert_eq!(fetched["id"], response_id);
        assert_eq!(fetched["output"][0]["content"][0]["text"], "Hello");

        mock.shutdown().await;
    }

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
            vec!["gemini-1.5-flash".to_string()],
        )];
        let gateway = GatewayHttpServer::new(&config)
            .await
            .expect("gateway should initialize with Gemini");
        let events = Arc::new(Mutex::new(Vec::new()));
        let error_notify = Arc::new(tokio::sync::Notify::new());
        let runtime = callback_runtime(Arc::clone(&events), error_notify).await;
        let state = gateway.state().clone().with_callbacks(runtime.dispatcher());
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            "gemini-1.5-flash",
            ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let request = with_user!(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(json!({
                    "model": "gemini-1.5-flash",
                    "input": "hello",
                    "stream": true
                }))
                .to_request(),
            "gemini-owner"
        );
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(response).await.to_vec())
            .expect("responses stream should be utf8");
        assert!(body.contains("\"type\":\"response.completed\""));
        assert!(body.contains("\"text\":\"ok\""));
        assert!(!body.contains("__litellm"));
        assert!(!body.contains("\"input_tokens\":0"));

        let completed = body
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|payload| serde_json::from_str::<Value>(payload).ok())
            .find(|event| event["type"] == "response.completed")
            .expect("responses stream should include a completed event");
        assert!(completed["response"]["usage"].is_null());

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
                panic!("responses stream should end successfully");
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
    async fn streamed_response_zero_timeout_observes_client_disconnect() {
        let mock = MockOpenAiServer::start(MockScenario::StreamingIdle).await;
        let events = Arc::new(Mutex::new(Vec::new()));
        let error_notify = Arc::new(tokio::sync::Notify::new());
        let runtime = callback_runtime(Arc::clone(&events), Arc::clone(&error_notify)).await;
        let state = app_state_with_idle_timeout(&mock.base_url, Some(0))
            .await
            .with_callbacks(runtime.dispatcher());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let request = with_user!(
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(response_request(Some(true)))
                .to_request(),
            "disconnect-owner"
        );
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);

        let disconnected = error_notify.notified();
        drop(response);
        tokio::time::timeout(Duration::from_secs(2), disconnected)
            .await
            .expect("responses worker should observe the dropped response body");
        runtime
            .shutdown()
            .await
            .expect("callback runtime should drain");

        let events = events.lock().unwrap().clone();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], RecordedCallback::Start));
        let RecordedCallback::Error(error) = &events[1] else {
            panic!("client disconnect should emit one terminal error callback");
        };
        assert_eq!(error.error_type.as_deref(), Some("client_disconnect"));

        mock.abort().await;
    }

    fn response_id_from_sse(body: &str) -> String {
        for line in body.lines() {
            let Some(payload) = line.strip_prefix("data: ") else {
                continue;
            };
            if payload == "[DONE]" {
                continue;
            }
            let event: Value = serde_json::from_str(payload).expect("SSE payload should be json");
            if event["type"] == "response.completed" {
                return event["response"]["id"]
                    .as_str()
                    .expect("completed response should have id")
                    .to_string();
            }
        }
        panic!("response.completed event not found");
    }
}
