#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use actix_web::{HttpMessage, dev::Service};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Debug)]
    struct CapturedModerationRequest {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: Value,
    }

    #[derive(Clone)]
    struct MockModerationState {
        captured_requests: Arc<Mutex<Vec<CapturedModerationRequest>>>,
    }

    struct MockModerationServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedModerationRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockModerationServer {
        async fn start_moderation_mock() -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockModerationState {
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
                    .route("/v1/moderations", web::post().to(mock_moderation_create))
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

        fn requests(&self) -> Vec<CapturedModerationRequest> {
            self.captured_requests.lock().unwrap().clone()
        }

        async fn stop_moderation_mock(self) {
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

    async fn mock_moderation_create(
        state: web::Data<MockModerationState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "id": "modr_mock",
            "model": "omni-moderation-latest",
            "results": [{
                "flagged": false,
                "categories": { "violence": false },
                "category_scores": { "violence": 0.0 }
            }]
        }))
    }

    fn capture_request(state: &MockModerationState, request: &HttpRequest, body: Bytes) {
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
            serde_json::from_slice(&body).expect("mock moderation body should be json")
        };

        state
            .captured_requests
            .lock()
            .unwrap()
            .push(CapturedModerationRequest {
                method: request.method().to_string(),
                path: request.path().to_string(),
                headers,
                body,
            });
    }

    async fn build_test_app_state(
        providers: Vec<ProviderConfig>,
    ) -> litellm_rs::server::state::AppState {
        build_test_app_state_with_auth(providers, false, true).await
    }

    async fn build_test_app_state_with_auth(
        providers: Vec<ProviderConfig>,
        enable_api_key: bool,
        allow_anonymous: bool,
    ) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = enable_api_key;
        config.gateway.auth.allow_anonymous = allow_anonymous;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.providers = providers;

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
    }

    fn moderation_provider(base_url: &str) -> ProviderConfig {
        moderation_provider_with_models(base_url, Vec::new())
    }

    fn moderation_provider_with_models(base_url: &str, models: Vec<String>) -> ProviderConfig {
        let mut provider = ProviderConfig {
            name: "mock-openai-compatible".to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some(base_url.to_string()),
            organization: Some("org-test".to_string()),
            project: Some("proj-test".to_string()),
            models,
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

    fn authenticated_api_key() -> ApiKey {
        ApiKey {
            metadata: Metadata::new(),
            name: "test-key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "sk-test".to_string(),
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
    async fn moderation_route_without_provider_fails_closed() {
        let state = build_test_app_state(Vec::new()).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(json!({ "input": "hello" }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "server_error");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Moderation API requires")
        );
    }

    #[tokio::test]
    async fn moderation_route_proxies_request_with_provider_headers() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(json!({
                    "model": "omni-moderation-latest",
                    "input": "moderate this text"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], "modr_mock");
        assert_eq!(body["results"][0]["flagged"], false);

        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/v1/moderations");
        assert_eq!(requests[0].body["input"], "moderate this text");
        assert_eq!(requests[0].body["model"], "omni-moderation-latest");
        assert_eq!(requests[0].headers["authorization"], "Bearer sk-test");
        assert_eq!(requests[0].headers["openai-organization"], "org-test");
        assert_eq!(requests[0].headers["openai-project"], "proj-test");
        assert_eq!(requests[0].headers["x-base-header"], "base-value");
        assert_eq!(requests[0].headers["x-custom-header"], "custom-value");

        mock.stop_moderation_mock().await;
    }

    #[tokio::test]
    async fn root_moderation_alias_proxies_request() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/moderations")
                .set_json(json!({
                    "input": ["one", "two"]
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/moderations");
        assert_eq!(requests[0].body["input"][0], "one");
        assert_eq!(requests[0].body["model"], "omni-moderation-latest");

        mock.stop_moderation_mock().await;
    }

    #[tokio::test]
    async fn moderation_route_uses_default_model_for_provider_selection_when_omitted() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state = build_test_app_state(vec![
            moderation_provider_with_models(
                "http://127.0.0.1:9/v1",
                vec!["unrelated-moderation-model".to_string()],
            ),
            moderation_provider_with_models(
                &mock.base_url,
                vec!["omni-moderation-latest".to_string()],
            ),
        ])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(json!({ "input": "moderate this text" }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/moderations");
        assert_eq!(requests[0].body["model"], "omni-moderation-latest");

        mock.stop_moderation_mock().await;
    }

    #[tokio::test]
    async fn moderation_route_requires_auth_when_anonymous_is_disabled() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state =
            build_test_app_state_with_auth(vec![moderation_provider(&mock.base_url)], true, false)
                .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(json!({ "input": "hello" }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "authentication_error");
        assert!(
            mock.requests().is_empty(),
            "unauthenticated requests must fail before upstream call"
        );

        mock.stop_moderation_mock().await;
    }

    #[tokio::test]
    async fn moderation_route_allows_authenticated_api_key() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state =
            build_test_app_state_with_auth(vec![moderation_provider(&mock.base_url)], true, false)
                .await;
        let api_key = authenticated_api_key();
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

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(json!({ "input": "hello" }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(mock.requests().len(), 1);

        mock.stop_moderation_mock().await;
    }

    #[tokio::test]
    async fn moderation_route_rejects_invalid_request_before_upstream() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let cases = [
            (json!({}), "input"),
            (json!({ "input": "" }), "input"),
            (json!({ "input": "hello", "model": 1 }), "model"),
            (json!({ "input": "hello", "unknown": true }), "Unknown"),
        ];

        for (body, expected_message) in cases {
            let resp = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/moderations")
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
            let body: Value = test::read_body_json(resp).await;
            assert!(
                body["error"]["message"]
                    .as_str()
                    .expect("error message")
                    .contains(expected_message)
            );
        }

        assert!(
            mock.requests().is_empty(),
            "invalid requests must fail before provider call"
        );

        mock.stop_moderation_mock().await;
    }

    #[tokio::test]
    async fn moderation_route_rejects_unconfigured_model_before_upstream() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state = build_test_app_state(vec![moderation_provider_with_models(
            &mock.base_url,
            vec!["omni-moderation-latest".to_string()],
        )])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(json!({
                    "model": "different-moderation-model",
                    "input": "hello"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: Value = test::read_body_json(resp).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("different-moderation-model")
        );
        assert!(mock.requests().is_empty());

        mock.stop_moderation_mock().await;
    }

    #[tokio::test]
    async fn moderation_route_rejects_exhausted_provider_budget_before_upstream() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
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

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(json!({ "input": "hello" }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        let body: Value = test::read_body_json(resp).await;
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

        mock.stop_moderation_mock().await;
    }

    #[tokio::test]
    async fn moderation_route_rejects_exhausted_default_model_budget_before_upstream() {
        let mock = MockModerationServer::start_moderation_mock().await;
        let state = build_test_app_state(vec![moderation_provider(&mock.base_url)]).await;
        state.budget_limits.models.set_model_limit(
            "omni-moderation-latest",
            ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .models
            .record_model_spend("omni-moderation-latest", 2.0);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/moderations")
                .set_json(json!({ "input": "hello" }))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "insufficient_quota");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("model 'omni-moderation-latest' budget exceeded")
        );
        assert!(
            mock.requests().is_empty(),
            "model budget rejection must happen before upstream call"
        );

        mock.stop_moderation_mock().await;
    }
}
