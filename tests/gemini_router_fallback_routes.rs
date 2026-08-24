#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures::{mock_provider_config, route_policy_bootstrap_providers};
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::net::ProviderEndpointAccess;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::state::AppState;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    const GEMINI_MODEL: &str = "gemini-3.1-flash-lite";
    const GEMINI_API_KEY: &str = "test-api-key-12345678901234567890";

    #[derive(Clone, Debug)]
    struct CapturedGeminiRequest {
        path_and_query: String,
        body: Vec<u8>,
    }

    #[derive(Clone)]
    struct MockGeminiState {
        captured_requests: Arc<Mutex<Vec<CapturedGeminiRequest>>>,
        failure_status: Option<StatusCode>,
    }

    struct MockGeminiServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedGeminiRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockGeminiServer {
        async fn start_gemini_mock() -> Self {
            Self::start_gemini_mock_with_status(None).await
        }

        async fn start_failing_gemini_mock(status: StatusCode) -> Self {
            Self::start_gemini_mock_with_status(Some(status)).await
        }

        async fn start_gemini_mock_with_status(failure_status: Option<StatusCode>) -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockGeminiState {
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
                    .default_service(web::post().to(mock_gemini))
            })
            .listen(listener)
            .expect("mock server should listen")
            .run();
            let handle = server.handle();
            let task = tokio::spawn(server);
            wait_for_server(address).await;

            Self {
                base_url: format!("http://{address}"),
                captured_requests,
                handle,
                task,
            }
        }

        fn requests(&self) -> Vec<CapturedGeminiRequest> {
            self.captured_requests.lock().unwrap().clone()
        }

        async fn stop_gemini_mock(self) {
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

    async fn mock_gemini(
        state: web::Data<MockGeminiState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        state
            .captured_requests
            .lock()
            .unwrap()
            .push(CapturedGeminiRequest {
                path_and_query: request
                    .uri()
                    .path_and_query()
                    .expect("request should have path")
                    .as_str()
                    .to_string(),
                body: body.to_vec(),
            });

        if let Some(status) = state.failure_status {
            return HttpResponse::build(status).json(json!({
                "error": {
                    "message": format!("forced upstream {status} at {}", request.uri())
                }
            }));
        }

        if request.path().ends_with(":streamGenerateContent") {
            return HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .body(
                    "event: message\n\
                     data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]}}]}\n\n\
                     event: message\n\
                     data: {\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":5,\"totalTokenCount\":15}}\n\n",
                );
        }

        HttpResponse::Ok().json(json!({
            "candidates": [{"content": {"parts": [{"text": "ok"}]}}],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15,
                "cachedContentTokenCount": 2,
                "thoughtsTokenCount": 1
            }
        }))
    }

    async fn build_test_state(providers: Vec<ProviderConfig>) -> AppState {
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

    fn gemini_provider(name: &str, base_url: &str) -> ProviderConfig {
        let mut provider = mock_provider_config(
            name,
            "openai_compatible",
            GEMINI_API_KEY,
            base_url,
            vec![GEMINI_MODEL.to_string()],
        );
        provider.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        provider
    }

    fn gemini_body() -> Value {
        json!({
            "contents": [{
                "role": "user",
                "parts": [{"text": "hello from the Gemini SDK"}]
            }],
            "generationConfig": {"maxOutputTokens": 8}
        })
    }

    fn configure_exhausted_primary_budget(state: &AppState) {
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .record_spend("gemini", GEMINI_MODEL, 0.01);
        state.budget_limits.providers.set_provider_limit(
            "googleai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            GEMINI_MODEL,
            ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
    }

    #[tokio::test]
    async fn gemini_sdk_route_uses_router_budget_fallback_provider() {
        let primary = MockGeminiServer::start_gemini_mock().await;
        let fallback = MockGeminiServer::start_gemini_mock().await;
        let state = build_test_state(vec![
            gemini_provider("gemini", &primary.base_url),
            gemini_provider("googleai", &fallback.base_url),
        ])
        .await;
        configure_exhausted_primary_budget(&state);
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["candidates"][0]["content"]["parts"][0]["text"], "ok");
        assert!(
            primary.requests().is_empty(),
            "router fallback must skip exhausted Gemini provider before upstream"
        );
        let fallback_requests = fallback.requests();
        assert_eq!(fallback_requests.len(), 1);
        assert_eq!(
            fallback_requests[0].path_and_query,
            format!("/v1beta/models/{GEMINI_MODEL}:generateContent?key={GEMINI_API_KEY}")
        );
        let upstream_body: Value =
            serde_json::from_slice(&fallback_requests[0].body).expect("body should be json");
        assert_eq!(
            upstream_body["contents"][0]["parts"][0]["text"],
            "hello from the Gemini SDK"
        );
        let fallback_usage = budget_limits
            .providers
            .get_provider_usage("googleai")
            .expect("fallback provider spend should be recorded");
        assert!(fallback_usage.current_spend > 0.0);

        primary.stop_gemini_mock().await;
        fallback.stop_gemini_mock().await;
    }

    #[tokio::test]
    async fn gemini_sdk_stream_route_uses_router_budget_fallback_provider() {
        let primary = MockGeminiServer::start_gemini_mock().await;
        let fallback = MockGeminiServer::start_gemini_mock().await;
        let state = build_test_state(vec![
            gemini_provider("gemini", &primary.base_url),
            gemini_provider("googleai", &fallback.base_url),
        ])
        .await;
        configure_exhausted_primary_budget(&state);
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/models/gemini-3.1-flash-lite:streamGenerateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        let stream_body = test::read_body(response).await;
        let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
        assert!(stream_text.contains("\"usageMetadata\""));
        assert!(
            primary.requests().is_empty(),
            "router fallback must skip exhausted Gemini stream provider before upstream"
        );
        let fallback_requests = fallback.requests();
        assert_eq!(fallback_requests.len(), 1);
        assert_eq!(
            fallback_requests[0].path_and_query,
            format!("/v1/models/{GEMINI_MODEL}:streamGenerateContent?alt=sse&key={GEMINI_API_KEY}")
        );
        let fallback_usage = budget_limits
            .providers
            .get_provider_usage("googleai")
            .expect("fallback provider stream spend should be recorded");
        assert!(fallback_usage.current_spend > 0.0);

        primary.stop_gemini_mock().await;
        fallback.stop_gemini_mock().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_uses_router_upstream_error_fallback_provider() {
        let primary =
            MockGeminiServer::start_failing_gemini_mock(StatusCode::SERVICE_UNAVAILABLE).await;
        let fallback = MockGeminiServer::start_gemini_mock().await;
        let state = build_test_state(vec![
            gemini_provider("gemini", &primary.base_url),
            gemini_provider("googleai", &fallback.base_url),
        ])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(primary.requests().len(), 1);
        let fallback_requests = fallback.requests();
        assert_eq!(fallback_requests.len(), 1);
        assert_eq!(
            fallback_requests[0].path_and_query,
            format!("/v1beta/models/{GEMINI_MODEL}:generateContent?key={GEMINI_API_KEY}")
        );

        primary.stop_gemini_mock().await;
        fallback.stop_gemini_mock().await;
    }

    #[tokio::test]
    async fn gemini_sdk_stream_route_uses_router_upstream_error_fallback_provider() {
        let primary =
            MockGeminiServer::start_failing_gemini_mock(StatusCode::SERVICE_UNAVAILABLE).await;
        let fallback = MockGeminiServer::start_gemini_mock().await;
        let state = build_test_state(vec![
            gemini_provider("gemini", &primary.base_url),
            gemini_provider("googleai", &fallback.base_url),
        ])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/models/gemini-3.1-flash-lite:streamGenerateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let stream_body = test::read_body(response).await;
        let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
        assert!(stream_text.contains("\"usageMetadata\""));
        assert_eq!(primary.requests().len(), 1);
        let fallback_requests = fallback.requests();
        assert_eq!(fallback_requests.len(), 1);
        assert_eq!(
            fallback_requests[0].path_and_query,
            format!("/v1/models/{GEMINI_MODEL}:streamGenerateContent?alt=sse&key={GEMINI_API_KEY}")
        );

        primary.stop_gemini_mock().await;
        fallback.stop_gemini_mock().await;
    }
}
