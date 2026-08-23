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
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::core::net::ProviderEndpointAccess;
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

    fn moderation_provider(base_url: &str) -> ProviderConfig {
        moderation_provider_with_models(base_url, Vec::new())
    }

    fn moderation_provider_with_models(base_url: &str, models: Vec<String>) -> ProviderConfig {
        named_moderation_provider_with_type(
            "mock-openai-compatible",
            "openai_compatible",
            base_url,
            models,
        )
    }

    fn named_moderation_provider(
        name: &str,
        base_url: &str,
        models: Vec<String>,
    ) -> ProviderConfig {
        named_moderation_provider_with_type(name, "openai_compatible", base_url, models)
    }

    fn named_moderation_provider_with_type(
        name: &str,
        provider_type: &str,
        base_url: &str,
        models: Vec<String>,
    ) -> ProviderConfig {
        let mut provider = mock_provider_config(name, provider_type, "sk-test", base_url, models);
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
    async fn public_only_moderation_route_rejects_loopback_before_connect() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let mut provider = moderation_provider(&format!("http://{address}/v1"));
        provider.endpoint_access = ProviderEndpointAccess::PublicOnly;
        let state = build_test_app_state(vec![provider]).await;
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
                .uri("/v1/moderations")
                .set_json(json!({
                    "model": "omni-moderation-latest",
                    "input": "listener must remain untouched"
                }))
                .to_request(),
        )
        .await;

        assert!(!response.status().is_success());
        let body: Value = test::read_body_json(response).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("SSRF protection"))
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "public-only moderation route must not connect to loopback listener"
        );
    }

    #[path = "moderations_routes_auth_validation_tests.rs"]
    mod auth_validation_tests;
    #[path = "moderations_routes_budget_fallback_tests.rs"]
    mod budget_fallback_tests;
    #[path = "moderations_routes_proxy_selection_tests.rs"]
    mod proxy_selection_tests;
}
