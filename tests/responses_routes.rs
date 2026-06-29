#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{App, HttpMessage, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use futures::stream;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
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
        }
    }

    fn provider_config(base_url: &str) -> ProviderConfig {
        ProviderConfig {
            name: "mock-openai-compatible".to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url.to_string()),
            settings: HashMap::from([("skip_api_key".to_string(), Value::Bool(true))]),
            models: vec!["gpt-4o-mini".to_string()],
            ..ProviderConfig::default()
        }
    }

    async fn app_state(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.providers = vec![provider_config(base_url)];

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
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
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created: Value = test::read_body_json(create_resp).await;
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
        assert_eq!(stream_resp.status(), StatusCode::OK);
        let body = test::read_body(stream_resp).await;
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
