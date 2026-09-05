#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures::{mock_provider_config, route_policy_bootstrap_providers};
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use actix_web::{HttpMessage, dev::Service};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::config::models::router::RoutingStrategyConfig;
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::core::net::ProviderEndpointAccess;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Debug)]
    struct CapturedRerankRequest {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: Value,
    }

    #[derive(Clone)]
    struct MockRerankState {
        captured_requests: Arc<Mutex<Vec<CapturedRerankRequest>>>,
    }

    struct MockRerankServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedRerankRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockRerankServer {
        async fn start_rerank_mock() -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockRerankState {
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
                    .route("/v1/rerank", web::post().to(mock_rerank_create))
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

        fn requests(&self) -> Vec<CapturedRerankRequest> {
            self.captured_requests.lock().unwrap().clone()
        }

        async fn stop_rerank_mock(self) {
            self.handle.stop(false).await;
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

    async fn mock_rerank_create(
        state: web::Data<MockRerankState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        let body = capture_request(&state, &request, body);
        match body.get("query").and_then(Value::as_str) {
            Some("force bad request") => {
                return HttpResponse::BadRequest()
                    .json(json!({ "message": "invalid rerank payload" }));
            }
            Some("force unauthorized") => {
                return HttpResponse::Unauthorized().json(json!({ "message": "invalid api key" }));
            }
            Some("force forbidden") => {
                return HttpResponse::Forbidden().json(json!({ "message": "access denied" }));
            }
            Some("force rate limit") => {
                return HttpResponse::TooManyRequests().json(json!({ "message": "slow down" }));
            }
            _ => {}
        }

        HttpResponse::Ok().json(json!({
            "id": "rerank_mock",
            "results": [
                { "index": 1, "relevance_score": 0.91 },
                { "index": 0, "relevance_score": 0.12 }
            ],
            "meta": {
                "billed_units": { "search_units": 1 }
            }
        }))
    }

    fn capture_request(state: &MockRerankState, request: &HttpRequest, body: Bytes) -> Value {
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
            serde_json::from_slice(&body).expect("mock rerank body should be json")
        };

        state
            .captured_requests
            .lock()
            .unwrap()
            .push(CapturedRerankRequest {
                method: request.method().to_string(),
                path: request.path().to_string(),
                headers,
                body: body.clone(),
            });

        body
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
        config.gateway.router.strategy = RoutingStrategyConfig::RoundRobin;
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

    fn cohere_rerank_provider(base_url: &str) -> ProviderConfig {
        cohere_rerank_provider_with_name_and_models(
            "mock-cohere",
            base_url,
            vec!["rerank-english-v3.0".to_string()],
        )
    }

    fn cohere_rerank_provider_with_name_and_models(
        name: &str,
        base_url: &str,
        models: Vec<String>,
    ) -> ProviderConfig {
        let mut provider =
            mock_provider_config(name, "openai_compatible", "sk-test", base_url, models);
        provider.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        provider
    }

    fn jina_rerank_provider_with_models(base_url: &str, models: Vec<String>) -> ProviderConfig {
        let mut provider = mock_provider_config(
            "mock-jina-rerank",
            "openai_compatible",
            "sk-test",
            base_url,
            models,
        );
        provider.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
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

    fn rerank_body() -> Value {
        json!({
            "model": "rerank-english-v3.0",
            "query": "What is machine learning?",
            "documents": [
                "A database migration guide",
                "Machine learning models learn patterns"
            ],
            "top_n": 2,
            "return_documents": true
        })
    }

    #[path = "rerank_routes_policy_tests.rs"]
    mod policy_tests;

    #[tokio::test]
    async fn rerank_route_without_provider_fails_closed() {
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
                .uri("/v1/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("No configured rerank provider")
        );
    }

    #[tokio::test]
    async fn rerank_route_proxies_cohere_provider() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![cohere_rerank_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], "rerank_mock");
        assert_eq!(body["model"], "rerank-english-v3.0");
        assert_eq!(body["results"][0]["index"], 1);
        assert_eq!(
            body["results"][0]["document"],
            "Machine learning models learn patterns"
        );
        assert_eq!(body["usage"]["search_units"], 1);

        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/v1/rerank");
        assert_eq!(requests[0].headers["authorization"], "Bearer sk-test");
        assert_eq!(requests[0].body["model"], "rerank-english-v3.0");
        assert_eq!(requests[0].body["query"], "What is machine learning?");
        assert_eq!(
            requests[0].body["documents"][1],
            "Machine learning models learn patterns"
        );
        assert_eq!(requests[0].body["top_n"], 2);

        mock.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn rerank_route_allows_explicit_configured_cohere_model() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![cohere_rerank_provider_with_name_and_models(
            "cohere-v4-provider",
            &mock.base_url,
            vec!["rerank-v4.0-pro".to_string()],
        )])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = rerank_body();
        body["model"] = json!("rerank-v4.0-pro");

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(body)
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["model"], "rerank-v4.0-pro");

        mock.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn rerank_route_proxies_jina_provider() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![jina_rerank_provider_with_models(
            &mock.base_url,
            vec!["jina-reranker-v3".to_string()],
        )])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = rerank_body();
        body["model"] = json!("jina-reranker-v3");

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(body)
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/rerank");
        assert_eq!(requests[0].headers["authorization"], "Bearer sk-test");
        assert_eq!(requests[0].body["model"], "jina-reranker-v3");

        mock.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn root_rerank_alias_proxies_request() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![cohere_rerank_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(mock.requests().len(), 1);

        mock.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn rerank_route_requires_auth_when_anonymous_is_disabled() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state_with_auth(
            vec![cohere_rerank_provider(&mock.base_url)],
            true,
            false,
        )
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
                .uri("/v1/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert!(
            mock.requests().is_empty(),
            "unauthenticated requests must fail before upstream call"
        );

        mock.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn rerank_route_allows_authenticated_api_key() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state_with_auth(
            vec![cohere_rerank_provider(&mock.base_url)],
            true,
            false,
        )
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
                .uri("/v1/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(mock.requests().len(), 1);

        mock.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn rerank_route_preserves_upstream_error_statuses() {
        let cases = [
            (
                "force bad request",
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "invalid_request",
            ),
            (
                "force unauthorized",
                StatusCode::UNAUTHORIZED,
                "authentication_error",
                "authentication_error",
            ),
            (
                "force forbidden",
                StatusCode::FORBIDDEN,
                "permission_error",
                "permission_denied",
            ),
            (
                "force rate limit",
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_error",
                "rate_limit_exceeded",
            ),
        ];

        for (query, expected_status, expected_type, expected_code) in cases {
            let mock = MockRerankServer::start_rerank_mock().await;
            let state = build_test_app_state(vec![cohere_rerank_provider(&mock.base_url)]).await;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let mut body = rerank_body();
            body["query"] = json!(query);

            let resp = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/rerank")
                    .set_json(body)
                    .to_request(),
            )
            .await;

            assert_eq!(resp.status(), expected_status, "query: {query}");
            let body: Value = test::read_body_json(resp).await;
            assert_eq!(body["error"]["type"], expected_type, "query: {query}");
            assert_eq!(body["error"]["code"], expected_code, "query: {query}");

            let request_count = mock.requests().len();
            if expected_status == StatusCode::TOO_MANY_REQUESTS {
                assert_eq!(
                    request_count, 4,
                    "router retry policy should retry pre-output 429 responses"
                );
            } else {
                assert_eq!(request_count, 1, "query: {query}");
            }
            mock.stop_rerank_mock().await;
        }
    }

    #[tokio::test]
    async fn rerank_route_rejects_unconfigured_model_before_upstream() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![cohere_rerank_provider_with_name_and_models(
            "wrong-model-provider",
            &mock.base_url,
            vec!["rerank-multilingual-v3.0".to_string()],
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
                .uri("/v1/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert!(
            mock.requests().is_empty(),
            "model filtering must happen before upstream call"
        );

        mock.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn rerank_route_rejects_exhausted_provider_budget_before_upstream() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![cohere_rerank_provider(&mock.base_url)]).await;
        state.budget_limits.providers.set_provider_limit(
            "mock-cohere",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .providers
            .record_provider_spend("mock-cohere", 2.0);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(
            mock.requests().is_empty(),
            "budget rejection must happen before upstream call"
        );

        mock.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn rerank_route_resolves_alias_and_uses_router_budget_fallback_provider() {
        let exhausted = MockRerankServer::start_rerank_mock().await;
        let fallback = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![
            cohere_rerank_provider_with_name_and_models(
                "exhausted-cohere",
                &exhausted.base_url,
                vec!["rerank-english-v3.0".to_string()],
            ),
            cohere_rerank_provider_with_name_and_models(
                "fallback-cohere",
                &fallback.base_url,
                vec!["rerank-english-v3.0".to_string()],
            ),
        ])
        .await;
        state
            .unified_router()
            .add_model_alias("public-rerank", "rerank-english-v3.0")
            .expect("runtime rerank alias should install");
        state.budget_limits.providers.set_provider_limit(
            "exhausted-cohere",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .providers
            .record_provider_spend("exhausted-cohere", 2.0);
        state.budget_limits.providers.set_provider_limit(
            "fallback-cohere",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let mut request = rerank_body();
        request["model"] = json!("public-rerank");
        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(request)
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            exhausted.requests().is_empty(),
            "exhausted provider must be skipped before upstream call"
        );
        let requests = fallback.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/rerank");
        assert_eq!(requests[0].body["model"], "rerank-english-v3.0");

        exhausted.stop_rerank_mock().await;
        fallback.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn wildcard_rerank_provider_tries_next_provider_name_key() {
        let exhausted = MockRerankServer::start_rerank_mock().await;
        let fallback = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![
            cohere_rerank_provider_with_name_and_models(
                "wild-primary-cohere",
                &exhausted.base_url,
                Vec::new(),
            ),
            cohere_rerank_provider_with_name_and_models(
                "wild-secondary-cohere",
                &fallback.base_url,
                Vec::new(),
            ),
        ])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "wild-primary-cohere",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .providers
            .record_provider_spend("wild-primary-cohere", 2.0);
        state.budget_limits.providers.set_provider_limit(
            "wild-secondary-cohere",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            exhausted.requests().is_empty(),
            "wildcard exhausted provider must be skipped before upstream call"
        );
        let requests = fallback.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/rerank");
        assert_eq!(requests[0].body["model"], "rerank-english-v3.0");

        exhausted.stop_rerank_mock().await;
        fallback.stop_rerank_mock().await;
    }

    #[tokio::test]
    async fn rerank_route_rejects_exhausted_model_budget_before_upstream() {
        let mock = MockRerankServer::start_rerank_mock().await;
        let state = build_test_app_state(vec![cohere_rerank_provider(&mock.base_url)]).await;
        state.budget_limits.models.set_model_limit(
            "rerank-english-v3.0",
            ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .models
            .record_model_spend("rerank-english-v3.0", 2.0);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(rerank_body())
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(
            mock.requests().is_empty(),
            "model budget rejection must happen before upstream call"
        );

        mock.stop_rerank_mock().await;
    }
}
