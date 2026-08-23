#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures::{mock_provider_config, route_policy_bootstrap_providers};
    use actix_web::{
        App, HttpRequest, HttpResponse, HttpServer,
        http::{Method, StatusCode},
        test, web,
    };
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::net::ProviderEndpointAccess;
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
        failure_status: Option<StatusCode>,
    }

    struct MockFineTuningServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedFineTuningRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockFineTuningServer {
        async fn start() -> Self {
            Self::start_with_status(None).await
        }

        async fn start_failing(status: StatusCode) -> Self {
            Self::start_with_status(Some(status)).await
        }

        async fn start_with_status(failure_status: Option<StatusCode>) -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockFineTuningState {
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
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
        HttpResponse::Ok().json(job_json("ftjob_mock", "queued"))
    }

    async fn mock_list_jobs(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
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
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
        HttpResponse::Ok().json(job_json("ftjob_mock", "running"))
    }

    async fn mock_cancel_job(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
        HttpResponse::Ok().json(job_json("ftjob_mock", "cancelled"))
    }

    async fn mock_list_events(
        state: web::Data<MockFineTuningState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
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
        if let Some(response) = maybe_failure_response(&state, &request) {
            return response;
        }
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

    fn maybe_failure_response(
        state: &MockFineTuningState,
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
        config.gateway.providers = route_policy_bootstrap_providers(&providers);

        let state = GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone();
        let mut runtime_config = state.config().as_ref().clone();
        runtime_config.gateway.providers = providers;
        state.config.store(runtime_config);
        state
    }

    fn fine_tuning_provider(base_url: &str) -> ProviderConfig {
        let mut provider = mock_provider_config(
            "mock-openai-compatible",
            "openai_compatible",
            "sk-test",
            base_url,
            vec!["gpt-4o-mini".to_string()],
        );
        provider.organization = Some("org-test".to_string());
        provider.project = Some("proj-test".to_string());
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
        provider.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        provider
    }

    fn named_fine_tuning_provider(name: &str, base_url: &str) -> ProviderConfig {
        let mut provider = fine_tuning_provider(base_url);
        provider.name = name.to_string();
        provider.organization = None;
        provider.project = None;
        provider.settings.clear();
        provider
    }

    #[path = "fine_tuning_routes_policy_tests.rs"]
    mod policy_tests;

    #[tokio::test]
    async fn fine_tuning_route_without_provider_fails_closed() {
        let state = build_test_state(Vec::new()).await;
        let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
            litellm_rs::server::routes::ai::configure_routes(
                cfg,
                litellm_rs::config::models::default_max_body_size(),
            )
        }))
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
        let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
            litellm_rs::server::routes::ai::configure_routes(
                cfg,
                litellm_rs::config::models::default_max_body_size(),
            )
        }))
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
        let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
            litellm_rs::server::routes::ai::configure_routes(
                cfg,
                litellm_rs::config::models::default_max_body_size(),
            )
        }))
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

    #[tokio::test]
    async fn fine_tuning_create_uses_fallback_when_primary_budget_exhausted() {
        let primary = MockFineTuningServer::start().await;
        let fallback = MockFineTuningServer::start().await;
        let mut primary_provider =
            named_fine_tuning_provider("primary-fine-tuning", &primary.base_url);
        primary_provider.settings = HashMap::from([(
            "headers".to_string(),
            json!({
                "invalid header name": "budget-skip"
            }),
        )]);
        let state = build_test_state(vec![
            primary_provider,
            named_fine_tuning_provider("fallback-fine-tuning", &fallback.base_url),
        ])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "primary-fine-tuning",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .providers
            .record_provider_spend("primary-fine-tuning", 2.0);
        let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
            litellm_rs::server::routes::ai::configure_routes(
                cfg,
                litellm_rs::config::models::default_max_body_size(),
            )
        }))
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/fine_tuning/jobs")
                .set_json(json!({
                    "model": "gpt-4o-mini",
                    "training_file": "file-train"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["provider"], "fallback-fine-tuning");
        assert!(
            primary.requests().is_empty(),
            "budget fallback must skip exhausted fine-tuning provider before upstream"
        );
        let fallback_requests = fallback.requests();
        assert_eq!(fallback_requests.len(), 1);
        assert_eq!(fallback_requests[0].path, "/v1/fine_tuning/jobs");

        primary.shutdown().await;
        fallback.shutdown().await;
    }

    #[tokio::test]
    async fn fine_tuning_routes_use_fallback_when_primary_upstream_fails() {
        let primary = MockFineTuningServer::start_failing(StatusCode::SERVICE_UNAVAILABLE).await;
        let fallback = MockFineTuningServer::start().await;
        let state = build_test_state(vec![
            named_fine_tuning_provider("primary-fine-tuning", &primary.base_url),
            named_fine_tuning_provider("fallback-fine-tuning", &fallback.base_url),
        ])
        .await;
        let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
            litellm_rs::server::routes::ai::configure_routes(
                cfg,
                litellm_rs::config::models::default_max_body_size(),
            )
        }))
        .await;

        let scenarios = [
            (
                Method::POST,
                "/v1/fine_tuning/jobs",
                "/v1/fine_tuning/jobs",
                "",
            ),
            (
                Method::GET,
                "/v1/fine_tuning/jobs?after=ftjob_prev&limit=1",
                "/v1/fine_tuning/jobs",
                "after=ftjob_prev&limit=1",
            ),
            (
                Method::GET,
                "/v1/fine_tuning/jobs/ftjob_mock",
                "/v1/fine_tuning/jobs/ftjob_mock",
                "",
            ),
            (
                Method::POST,
                "/v1/fine_tuning/jobs/ftjob_mock/cancel",
                "/v1/fine_tuning/jobs/ftjob_mock/cancel",
                "",
            ),
            (
                Method::GET,
                "/v1/fine_tuning/jobs/ftjob_mock/events?after=ftevent_prev&limit=2",
                "/v1/fine_tuning/jobs/ftjob_mock/events",
                "after=ftevent_prev&limit=2",
            ),
            (
                Method::GET,
                "/v1/fine_tuning/jobs/ftjob_mock/checkpoints",
                "/v1/fine_tuning/jobs/ftjob_mock/checkpoints",
                "",
            ),
        ];
        for (method, uri, _, _) in &scenarios {
            let mut request = test::TestRequest::with_uri(uri).method(method.clone());
            if *uri == "/v1/fine_tuning/jobs" {
                request = request.set_json(json!({
                    "model": "gpt-4o-mini",
                    "training_file": "file-train"
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
        assert_eq!(fallback_requests[0].body["training_file"], "file-train");
        for (request, (method, _, path, query)) in fallback_requests.iter().zip(scenarios.iter()) {
            assert_eq!(request.method, method.as_str());
            assert_eq!(request.path, *path);
            assert_eq!(request.query, *query);
        }

        primary.shutdown().await;
        fallback.shutdown().await;
    }

    #[tokio::test]
    async fn fine_tuning_route_does_not_validate_unreached_fallback_provider() {
        let primary = MockFineTuningServer::start().await;
        let mut broken_fallback =
            named_fine_tuning_provider("broken-fine-tuning", "https://unused.invalid/v1");
        broken_fallback.settings = HashMap::from([(
            "headers".to_string(),
            json!({
                "invalid header name": "not reached"
            }),
        )]);
        let state = build_test_state(vec![
            named_fine_tuning_provider("primary-fine-tuning", &primary.base_url),
            broken_fallback,
        ])
        .await;
        let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
            litellm_rs::server::routes::ai::configure_routes(
                cfg,
                litellm_rs::config::models::default_max_body_size(),
            )
        }))
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/fine_tuning/jobs")
                .set_json(json!({
                    "model": "gpt-4o-mini",
                    "training_file": "file-train"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let primary_requests = primary.requests();
        assert_eq!(primary_requests.len(), 1);
        assert_eq!(primary_requests[0].path, "/v1/fine_tuning/jobs");

        primary.shutdown().await;
    }
}
