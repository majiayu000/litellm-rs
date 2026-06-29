#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Debug)]
    struct CapturedFineTuningRequest {
        method: String,
        path: String,
        query: String,
        headers: HashMap<String, String>,
        body: Value,
    }

    #[derive(Clone)]
    struct MockFineTuningState {
        captured_requests: Arc<Mutex<Vec<CapturedFineTuningRequest>>>,
    }

    struct MockFineTuningServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedFineTuningRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockFineTuningServer {
        async fn start() -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockFineTuningState {
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
                    .route("/v1/fine_tuning/jobs", web::post().to(mock_create_job))
                    .route("/v1/fine_tuning/jobs", web::get().to(mock_list_jobs))
                    .route("/v1/fine_tuning/jobs/{job_id}", web::get().to(mock_get_job))
                    .route(
                        "/v1/fine_tuning/jobs/{job_id}/cancel",
                        web::post().to(mock_cancel_job),
                    )
                    .route(
                        "/v1/fine_tuning/jobs/{job_id}/events",
                        web::get().to(mock_list_events),
                    )
                    .route(
                        "/v1/fine_tuning/jobs/{job_id}/checkpoints",
                        web::get().to(mock_list_checkpoints),
                    )
            })
            .listen(listener)
            .expect("mock server should listen")
            .run();
            let handle = server.handle();
            let task = tokio::spawn(server);
            wait_for_server(address).await;

            Self {
                base_url: format!("http://{address}/v1"),
                captured_requests,
                handle,
                task,
            }
        }

        fn requests(&self) -> Vec<CapturedFineTuningRequest> {
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

    async fn wait_for_server(address: std::net::SocketAddr) {
        for _ in 0..20 {
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("mock server did not accept connections at {address}");
    }

    async fn mock_create_job(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(job_json("ftjob_mock", "queued"))
    }

    async fn mock_list_jobs(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "object": "list",
            "data": [job_json("ftjob_mock", "running")],
            "has_more": false
        }))
    }

    async fn mock_get_job(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(job_json("ftjob_mock", "running"))
    }

    async fn mock_cancel_job(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(job_json("ftjob_mock", "cancelled"))
    }

    async fn mock_list_events(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "object": "list",
            "data": [{
                "id": "ftevent_mock",
                "object": "fine_tuning.job.event",
                "level": "info",
                "message": "Job started",
                "created_at": 1710000001
            }],
            "has_more": false
        }))
    }

    async fn mock_list_checkpoints(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "object": "list",
            "data": [{
                "id": "ftckpt_mock",
                "object": "fine_tuning.job.checkpoint",
                "fine_tuning_job_id": "ftjob_mock",
                "step_number": 1,
                "fine_tuned_model_checkpoint": "ft:gpt-4o-mini:mock",
                "created_at": 1710000002
            }]
        }))
    }

    fn job_json(id: &str, status: &str) -> Value {
        json!({
            "id": id,
            "object": "fine_tuning.job",
            "model": "gpt-4o-mini",
            "status": status,
            "training_file": "file-train",
            "created_at": 1710000000
        })
    }

    fn capture_request(state: &MockFineTuningState, request: &HttpRequest, body: Bytes) {
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
            serde_json::from_slice(&body).expect("mock fine-tuning body should be json")
        };

        state
            .captured_requests
            .lock()
            .unwrap()
            .push(CapturedFineTuningRequest {
                method: request.method().to_string(),
                path: request.path().to_string(),
                query: request.query_string().to_string(),
                headers,
                body,
            });
    }

    async fn build_test_state(
        providers: Vec<ProviderConfig>,
    ) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.providers = providers;

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
    }

    fn fine_tuning_provider(base_url: &str) -> ProviderConfig {
        let mut provider = ProviderConfig {
            name: "mock-openai-compatible".to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some(base_url.to_string()),
            organization: Some("org-test".to_string()),
            project: Some("proj-test".to_string()),
            models: vec!["gpt-4o-mini".to_string()],
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
    async fn fine_tuning_route_without_provider_fails_closed() {
        let state = build_test_state(Vec::new()).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/fine_tuning/jobs")
            .set_json(json!({
                "model": "gpt-4o-mini",
                "training_file": "file-train"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: Value = test::read_body_json(resp).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Fine-tuning API requires")
        );
        assert_eq!(body["error"]["type"], "server_error");
    }

    #[tokio::test]
    async fn fine_tuning_routes_proxy_provider_lifecycle() {
        let mock = MockFineTuningServer::start().await;
        let state = build_test_state(vec![fine_tuning_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let create_req = test::TestRequest::post()
            .uri("/v1/fine_tuning/jobs")
            .set_json(json!({
                "model": "gpt-4o-mini",
                "training_file": "file-train",
                "hyperparameters": { "n_epochs": 2 },
                "metadata": { "owner": "route-test" }
            }))
            .to_request();
        let create_resp = test::call_service(&app, create_req).await;
        assert_eq!(create_resp.status(), StatusCode::OK);
        let create_body: Value = test::read_body_json(create_resp).await;
        assert_eq!(create_body["id"], "ftjob_mock");
        assert_eq!(create_body["provider"], "mock-openai-compatible");

        let list_resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/fine_tuning/jobs?after=ftjob_prev&limit=1")
                .to_request(),
        )
        .await;
        assert_eq!(list_resp.status(), StatusCode::OK);

        let get_resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/fine_tuning/jobs/ftjob_mock")
                .to_request(),
        )
        .await;
        assert_eq!(get_resp.status(), StatusCode::OK);

        let cancel_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/fine_tuning/jobs/ftjob_mock/cancel")
                .to_request(),
        )
        .await;
        assert_eq!(cancel_resp.status(), StatusCode::OK);

        let events_resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/fine_tuning/jobs/ftjob_mock/events?after=ftevent_prev&limit=2")
                .to_request(),
        )
        .await;
        assert_eq!(events_resp.status(), StatusCode::OK);

        let checkpoints_resp = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/fine_tuning/jobs/ftjob_mock/checkpoints")
                .to_request(),
        )
        .await;
        assert_eq!(checkpoints_resp.status(), StatusCode::OK);
        let checkpoints_body: Value = test::read_body_json(checkpoints_resp).await;
        assert_eq!(checkpoints_body["object"], "list");
        assert_eq!(checkpoints_body["data"][0]["id"], "ftckpt_mock");

        let captured = mock.requests();
        assert_eq!(captured.len(), 6);
        assert_eq!(captured[0].method, "POST");
        assert_eq!(captured[0].path, "/v1/fine_tuning/jobs");
        assert_eq!(captured[0].body["model"], "gpt-4o-mini");
        assert_eq!(captured[0].body["training_file"], "file-train");
        assert_eq!(captured[0].body["hyperparameters"]["n_epochs"], 2);
        assert_eq!(captured[0].body["metadata"]["owner"], "route-test");
        assert_eq!(
            captured[0].headers.get("authorization").map(String::as_str),
            Some("Bearer sk-test")
        );
        assert_eq!(
            captured[0]
                .headers
                .get("openai-organization")
                .map(String::as_str),
            Some("org-test")
        );
        assert_eq!(
            captured[0]
                .headers
                .get("openai-project")
                .map(String::as_str),
            Some("proj-test")
        );
        assert_eq!(
            captured[0].headers.get("x-base-header").map(String::as_str),
            Some("base-value")
        );
        assert_eq!(
            captured[0]
                .headers
                .get("x-custom-header")
                .map(String::as_str),
            Some("custom-value")
        );
        assert_eq!(captured[1].path, "/v1/fine_tuning/jobs");
        assert_eq!(captured[1].query, "after=ftjob_prev&limit=1");
        assert_eq!(captured[2].path, "/v1/fine_tuning/jobs/ftjob_mock");
        assert_eq!(captured[3].path, "/v1/fine_tuning/jobs/ftjob_mock/cancel");
        assert_eq!(captured[4].path, "/v1/fine_tuning/jobs/ftjob_mock/events");
        assert_eq!(captured[4].query, "after=ftevent_prev&limit=2");
        assert_eq!(
            captured[5].path,
            "/v1/fine_tuning/jobs/ftjob_mock/checkpoints"
        );

        mock.shutdown().await;
    }

    #[tokio::test]
    async fn fine_tuning_create_rejects_exhausted_provider_budget_before_upstream() {
        let mock = MockFineTuningServer::start().await;
        let state = build_test_state(vec![fine_tuning_provider(&mock.base_url)]).await;
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

        let create_req = test::TestRequest::post()
            .uri("/v1/fine_tuning/jobs")
            .set_json(json!({
                "model": "gpt-4o-mini",
                "training_file": "file-train"
            }))
            .to_request();
        let create_resp = test::call_service(&app, create_req).await;

        assert_eq!(create_resp.status(), StatusCode::PAYMENT_REQUIRED);
        let body: Value = test::read_body_json(create_resp).await;
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

        mock.shutdown().await;
    }
}
