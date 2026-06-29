#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Debug)]
    struct CapturedBatchRequest {
        method: String,
        path: String,
        query: String,
        headers: HashMap<String, String>,
        body: Value,
    }

    #[derive(Clone)]
    struct MockBatchServerState {
        captured_requests: Arc<Mutex<Vec<CapturedBatchRequest>>>,
    }

    struct MockBatchServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedBatchRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockBatchServer {
        async fn start_batch_mock() -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockBatchServerState {
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
                    .route("/v1/batches", web::post().to(mock_batch_create))
                    .route("/v1/batches", web::get().to(mock_batch_list))
                    .route("/v1/batches/{batch_id}", web::get().to(mock_batch_get))
                    .route(
                        "/v1/batches/{batch_id}/cancel",
                        web::post().to(mock_batch_cancel),
                    )
            })
            .listen(listener)
            .expect("mock server should listen")
            .run();
            let handle = server.handle();
            let task = tokio::spawn(server);
            tokio::time::sleep(Duration::from_millis(20)).await;

            Self {
                base_url: format!("http://{address}/v1"),
                captured_requests,
                handle,
                task,
            }
        }

        fn requests(&self) -> Vec<CapturedBatchRequest> {
            self.captured_requests.lock().unwrap().clone()
        }

        async fn stop_batch_mock(self) {
            self.handle.stop(true).await;
            let result = self.task.await.expect("mock server task should join");
            if let Err(error) = result {
                panic!("mock server should stop cleanly: {error}");
            }
        }
    }

    async fn mock_batch_create(
        state: web::Data<MockBatchServerState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Accepted().json(json!({
            "id": "batch_mock",
            "object": "batch",
            "status": "validating"
        }))
    }

    async fn mock_batch_list(
        state: web::Data<MockBatchServerState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "object": "list",
            "data": [],
            "has_more": false
        }))
    }

    async fn mock_batch_get(
        state: web::Data<MockBatchServerState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "id": "batch_123",
            "object": "batch",
            "status": "in_progress"
        }))
    }

    async fn mock_batch_cancel(
        state: web::Data<MockBatchServerState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "id": "batch_123",
            "object": "batch",
            "status": "cancelled"
        }))
    }

    fn capture_request(state: &MockBatchServerState, request: &HttpRequest, body: Bytes) {
        let headers = request
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
            })
            .collect();
        let body = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).expect("mock batch body should be json")
        };

        state
            .captured_requests
            .lock()
            .unwrap()
            .push(CapturedBatchRequest {
                method: request.method().to_string(),
                path: request.path().to_string(),
                query: request.query_string().to_string(),
                headers,
                body,
            });
    }

    async fn build_test_app_state(
        providers: Vec<ProviderConfig>,
    ) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.providers = providers;

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
    }

    fn batch_route_provider_with_headers(base_url: &str) -> ProviderConfig {
        let mut provider = ProviderConfig {
            name: "mock-openai-compatible".to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some(base_url.to_string()),
            organization: Some("org-test".to_string()),
            project: Some("proj-test".to_string()),
            ..ProviderConfig::default()
        };
        provider.settings = HashMap::from([
            (
                "headers".to_string(),
                json!({
                    "X-Base-Header": "base-value",
                    "X-Ignored-Non-String": false
                }),
            ),
            (
                "custom_headers".to_string(),
                json!({
                    "X-Custom-Header": "custom-value"
                }),
            ),
        ]);
        provider
    }

    #[tokio::test]
    async fn route_without_batch_provider_fails_closed() {
        let state = build_test_app_state(Vec::new()).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let create_req = test::TestRequest::post()
            .uri("/v1/batches")
            .set_json(json!({
                "input_file_id": "file_123",
                "endpoint": "/v1/chat/completions",
                "completion_window": "24h"
            }))
            .to_request();
        let create_resp = test::call_service(&app, create_req).await;

        assert_eq!(create_resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: Value = test::read_body_json(create_resp).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Batch API requires")
        );
        assert_eq!(body["error"]["type"], "server_error");
    }

    #[tokio::test]
    async fn route_proxies_batch_lifecycle_with_provider_headers() {
        let mock_server = MockBatchServer::start_batch_mock().await;
        let state = build_test_app_state(vec![batch_route_provider_with_headers(
            &mock_server.base_url,
        )])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let create_req = test::TestRequest::post()
            .uri("/v1/batches")
            .set_json(json!({
                "input_file_id": "file_123",
                "endpoint": "/v1/chat/completions",
                "completion_window": "24h"
            }))
            .to_request();
        let create_resp = test::call_service(&app, create_req).await;
        assert_eq!(create_resp.status(), StatusCode::ACCEPTED);

        let list_req = test::TestRequest::get()
            .uri("/v1/batches?after=batch_prev&limit=7")
            .to_request();
        let list_resp = test::call_service(&app, list_req).await;
        assert_eq!(list_resp.status(), StatusCode::OK);

        let get_req = test::TestRequest::get()
            .uri("/v1/batches/batch_123")
            .to_request();
        let get_resp = test::call_service(&app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::OK);

        let cancel_req = test::TestRequest::post()
            .uri("/v1/batches/batch_123/cancel")
            .to_request();
        let cancel_resp = test::call_service(&app, cancel_req).await;
        assert_eq!(cancel_resp.status(), StatusCode::OK);

        let requests = mock_server.requests();
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/v1/batches");
        assert_eq!(requests[0].body["input_file_id"], "file_123");
        assert_eq!(requests[0].headers["authorization"], "Bearer sk-test");
        assert_eq!(requests[0].headers["openai-organization"], "org-test");
        assert_eq!(requests[0].headers["openai-project"], "proj-test");
        assert_eq!(requests[0].headers["x-base-header"], "base-value");
        assert_eq!(requests[0].headers["x-custom-header"], "custom-value");
        assert_eq!(requests[1].method, "GET");
        assert_eq!(requests[1].path, "/v1/batches");
        assert_eq!(requests[1].query, "after=batch_prev&limit=7");
        assert_eq!(requests[2].method, "GET");
        assert_eq!(requests[2].path, "/v1/batches/batch_123");
        assert_eq!(requests[3].method, "POST");
        assert_eq!(requests[3].path, "/v1/batches/batch_123/cancel");

        mock_server.stop_batch_mock().await;
    }
}
