#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{
        App, HttpRequest, HttpResponse, HttpServer,
        http::{Method, StatusCode},
        test, web,
    };
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::state::AppState;
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
        failure_status: Option<StatusCode>,
    }

    struct MockBatchServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedBatchRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockBatchServer {
        async fn start_batch_mock() -> Self {
            Self::start_batch_mock_with_status(None).await
        }

        async fn start_failing_batch_mock(status: StatusCode) -> Self {
            Self::start_batch_mock_with_status(Some(status)).await
        }

        async fn start_batch_mock_with_status(failure_status: Option<StatusCode>) -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockBatchServerState {
                captured_requests: Arc::clone(&captured_requests),
                failure_status,
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
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
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
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
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
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
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
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
        HttpResponse::Ok().json(json!({
            "id": "batch_123",
            "object": "batch",
            "status": "cancelled"
        }))
    }

    fn maybe_failure_response(
        state: &MockBatchServerState,
        request: &HttpRequest,
    ) -> Option<HttpResponse> {
        state.failure_status.map(|status| {
            HttpResponse::build(status).json(json!({
                "error": {
                    "message": format!("forced upstream {status} at {}", request.uri())
                }
            }))
        })
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

    async fn build_test_app_state(providers: Vec<ProviderConfig>) -> AppState {
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

    fn batch_route_provider(name: &str, base_url: &str) -> ProviderConfig {
        let mut provider = batch_route_provider_with_headers(base_url);
        provider.name = name.to_string();
        provider.organization = None;
        provider.project = None;
        provider.settings.clear();
        provider
    }

    fn configure_exhausted_primary_budget(state: &AppState, provider_name: &str) {
        state.budget_limits.providers.set_provider_limit(
            provider_name,
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        state.budget_limits.record_spend(provider_name, "", 0.01);
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

    #[tokio::test]
    async fn route_uses_batch_fallback_when_primary_budget_exhausted() {
        let primary = MockBatchServer::start_batch_mock().await;
        let fallback = MockBatchServer::start_batch_mock().await;
        let state = build_test_app_state(vec![
            batch_route_provider("primary-batch", &primary.base_url),
            batch_route_provider("fallback-batch", &fallback.base_url),
        ])
        .await;
        configure_exhausted_primary_budget(&state, "primary-batch");
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let create_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/batches")
                .set_json(json!({
                    "input_file_id": "file_123",
                    "endpoint": "/v1/chat/completions",
                    "completion_window": "24h"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(create_resp.status(), StatusCode::ACCEPTED);
        assert!(
            primary.requests().is_empty(),
            "budget fallback must skip the exhausted batch provider before upstream"
        );
        let fallback_requests = fallback.requests();
        assert_eq!(fallback_requests.len(), 1);
        assert_eq!(fallback_requests[0].method, "POST");
        assert_eq!(fallback_requests[0].path, "/v1/batches");
        assert_eq!(fallback_requests[0].body["input_file_id"], "file_123");

        primary.stop_batch_mock().await;
        fallback.stop_batch_mock().await;
    }

    #[tokio::test]
    async fn route_uses_batch_fallback_when_primary_upstream_fails() {
        let primary =
            MockBatchServer::start_failing_batch_mock(StatusCode::SERVICE_UNAVAILABLE).await;
        let fallback = MockBatchServer::start_batch_mock().await;
        let state = build_test_app_state(vec![
            batch_route_provider("primary-batch", &primary.base_url),
            batch_route_provider("fallback-batch", &fallback.base_url),
        ])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let scenarios = [
            (Method::POST, "/v1/batches", "/v1/batches", ""),
            (
                Method::GET,
                "/v1/batches?after=batch_prev&limit=7",
                "/v1/batches",
                "after=batch_prev&limit=7",
            ),
            (
                Method::GET,
                "/v1/batches/batch_123",
                "/v1/batches/batch_123",
                "",
            ),
            (
                Method::POST,
                "/v1/batches/batch_123/cancel",
                "/v1/batches/batch_123/cancel",
                "",
            ),
        ];
        for (method, uri, _, _) in &scenarios {
            let mut request = test::TestRequest::with_uri(uri).method(method.clone());
            if *uri == "/v1/batches" {
                request = request.set_json(json!({
                    "input_file_id": "file_123",
                    "endpoint": "/v1/chat/completions",
                    "completion_window": "24h"
                }));
            }
            let response = test::call_service(&app, request.to_request()).await;
            assert!(
                response.status().is_success(),
                "{uri} should succeed via fallback, got {}",
                response.status()
            );
        }

        assert_eq!(primary.requests().len(), scenarios.len());
        let fallback_requests = fallback.requests();
        assert_eq!(fallback_requests.len(), scenarios.len());
        assert_eq!(fallback_requests[0].body["input_file_id"], "file_123");
        for (request, (method, _, path, query)) in fallback_requests.iter().zip(scenarios.iter()) {
            assert_eq!(request.method, method.as_str());
            assert_eq!(request.path, *path);
            assert_eq!(request.query, *query);
        }

        primary.stop_batch_mock().await;
        fallback.stop_batch_mock().await;
    }

    #[tokio::test]
    async fn route_does_not_validate_unreached_batch_fallback_provider() {
        let primary = MockBatchServer::start_batch_mock().await;
        let mut broken_fallback =
            batch_route_provider("broken-fallback", "https://unused.invalid/v1");
        broken_fallback.settings = HashMap::from([(
            "headers".to_string(),
            json!({
                "invalid header name": "not reached"
            }),
        )]);
        let state = build_test_app_state(vec![
            batch_route_provider("primary-batch", &primary.base_url),
            broken_fallback,
        ])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let create_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/batches")
                .set_json(json!({
                    "input_file_id": "file_123",
                    "endpoint": "/v1/chat/completions",
                    "completion_window": "24h"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(create_resp.status(), StatusCode::ACCEPTED);
        let primary_requests = primary.requests();
        assert_eq!(primary_requests.len(), 1);
        assert_eq!(primary_requests[0].path, "/v1/batches");

        primary.stop_batch_mock().await;
    }
}
