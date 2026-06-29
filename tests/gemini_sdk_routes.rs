#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::state::AppState;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[derive(Clone, Debug)]
    struct CapturedGeminiRequest {
        path_and_query: String,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    }

    #[derive(Clone)]
    struct MockGeminiState {
        captured_requests: Arc<Mutex<Vec<CapturedGeminiRequest>>>,
    }

    struct MockGeminiServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedGeminiRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockGeminiServer {
        async fn start() -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockGeminiState {
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

        async fn stop(self) {
            self.handle.stop(true).await;
            let result = self.task.await.expect("mock server task should join");
            if let Err(error) = result {
                panic!("mock server should stop cleanly: {error}");
            }
        }
    }

    struct BrokenGeminiStreamServer {
        base_url: String,
        task: tokio::task::JoinHandle<()>,
    }

    impl BrokenGeminiStreamServer {
        async fn start() -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("broken stream server should bind");
            let address = listener
                .local_addr()
                .expect("broken stream server should have address");
            let task = tokio::spawn(async move {
                let (mut socket, _) = listener
                    .accept()
                    .await
                    .expect("broken stream server should accept");
                let mut buffer = [0_u8; 4096];
                let _ = socket.read(&mut buffer).await;
                let response = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "content-type: text/event-stream\r\n",
                    "content-length: 4096\r\n",
                    "connection: close\r\n",
                    "\r\n",
                    "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial\"}]}}]}\n\n"
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("broken stream server should write partial response");
                let _ = socket.shutdown().await;
            });

            Self {
                base_url: format!("http://{address}"),
                task,
            }
        }

        async fn stop(self) {
            self.task
                .await
                .expect("broken stream server task should join");
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
                headers,
                body: body.to_vec(),
            });

        if request.path().ends_with(":streamGenerateContent") {
            let should_fail_stream = serde_json::from_slice::<Value>(&body)
                .ok()
                .and_then(|value| value.get("forceUpstreamError").and_then(Value::as_bool))
                .unwrap_or(false);
            if should_fail_stream {
                return HttpResponse::BadGateway()
                    .insert_header(("content-type", "text/event-stream"))
                    .body(format!(
                        "event: error\n\
                         data: {{\"error\":{{\"message\":\"upstream failed at {}\"}}}}\n\n",
                        request.uri()
                    ));
            }
            return HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .body(
                    "event: message\n\
                     data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]}}]}\n\n\
                     event: message\n\
                     data: {\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":5,\"totalTokenCount\":15}}\n\n",
                );
        }

        let should_fail = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|value| value.get("forceUpstreamError").and_then(Value::as_bool))
            .unwrap_or(false);
        if should_fail {
            return HttpResponse::BadGateway().json(json!({
                "error": {
                    "message": format!("upstream failed at {}", request.uri())
                }
            }));
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
        config.gateway.providers = providers;

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
    }

    async fn build_auth_required_state(providers: Vec<ProviderConfig>) -> AppState {
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

    fn gemini_provider(name: &str, base_url: &str, models: Vec<String>) -> ProviderConfig {
        let mut provider = ProviderConfig {
            name: name.to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "test-api-key-12345678901234567890".to_string(),
            base_url: Some(base_url.to_string()),
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

    fn gemini_body() -> Value {
        json!({
            "contents": [{
                "role": "user",
                "parts": [{"text": "hello from the Gemini SDK"}]
            }],
            "generationConfig": {"maxOutputTokens": 8}
        })
    }

    fn gemini_upstream_error_body() -> Value {
        let mut body = gemini_body();
        body["forceUpstreamError"] = json!(true);
        body
    }

    #[tokio::test]
    async fn gemini_sdk_routes_without_provider_fail_closed() {
        let state = build_test_state(Vec::new()).await;
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

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: Value = test::read_body_json(response).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Gemini SDK route provider")
        );
    }

    #[tokio::test]
    async fn gemini_sdk_route_proxies_native_body_and_records_spend() {
        let mock = MockGeminiServer::start().await;
        let state = build_test_state(vec![
            gemini_provider(
                "googleai",
                "http://127.0.0.1:9",
                vec!["other-model".to_string()],
            ),
            gemini_provider(
                "gemini",
                &mock.base_url,
                vec!["gemini-3.1-flash-lite".to_string()],
            ),
        ])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            "gemini-3.1-flash-lite",
            ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
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

        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].path_and_query,
            "/v1beta/models/gemini-3.1-flash-lite:generateContent?key=test-api-key-12345678901234567890"
        );
        assert_eq!(requests[0].headers["x-base-header"], "base-value");
        assert_eq!(requests[0].headers["x-custom-header"], "custom-value");
        let upstream_body: Value =
            serde_json::from_slice(&requests[0].body).expect("body should be json");
        assert_eq!(
            upstream_body["contents"][0]["parts"][0]["text"],
            "hello from the Gemini SDK"
        );

        let provider_usage = budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("provider spend should be recorded");
        assert!(provider_usage.current_spend > 0.0);
        let model_usage = budget_limits
            .models
            .get_model_usage("gemini-3.1-flash-lite")
            .expect("model spend should be recorded");
        assert!(model_usage.current_spend > 0.0);

        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_sdk_stream_route_uses_sse_alt_query() {
        let mock = MockGeminiServer::start().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            "gemini-3.1-flash-lite",
            ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
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
                .and_then(|v| v.to_str().ok()),
            Some("text/event-stream")
        );
        let stream_body = test::read_body(response).await;
        let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
        assert!(stream_text.contains("\"usageMetadata\""));
        let requests = mock.requests();
        assert_eq!(
            requests[0].path_and_query,
            "/v1/models/gemini-3.1-flash-lite:streamGenerateContent?alt=sse&key=test-api-key-12345678901234567890"
        );
        let provider_usage = budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("provider stream spend should be recorded");
        assert!(provider_usage.current_spend > 0.0);
        let model_usage = budget_limits
            .models
            .get_model_usage("gemini-3.1-flash-lite")
            .expect("model stream spend should be recorded");
        assert!(model_usage.current_spend > 0.0);

        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_prefixed_sdk_route_is_supported() {
        let mock = MockGeminiServer::start().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
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
                .uri("/gemini/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let requests = mock.requests();
        assert_eq!(
            requests[0].path_and_query,
            "/v1beta/models/gemini-3.1-flash-lite:generateContent?key=test-api-key-12345678901234567890"
        );

        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_rejects_unauthenticated_when_auth_enabled() {
        let mock = MockGeminiServer::start().await;
        let state = build_auth_required_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
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

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(mock.requests().is_empty());
        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_rejects_unsafe_model_segment_before_upstream() {
        let mock = MockGeminiServer::start().await;
        let state =
            build_test_state(vec![gemini_provider("gemini", &mock.base_url, Vec::new())]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/..%2Fgemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(mock.requests().is_empty());
        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_rejects_exhausted_provider_budget_before_upstream() {
        let mock = MockGeminiServer::start().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .record_spend("gemini", "gemini-3.1-flash-lite", 0.01);
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

        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(mock.requests().is_empty());
        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_reserves_budget_before_upstream() {
        let mock = MockGeminiServer::start().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(0.000001, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            "gemini-3.1-flash-lite",
            ModelLimitConfig::new(0.000001, ResetPeriod::Monthly),
        );
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

        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(mock.requests().is_empty());
        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_network_error_does_not_leak_provider_key() {
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            "http://127.0.0.1:9",
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
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

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body: Value = test::read_body_json(response).await;
        let message = body["error"]["message"].as_str().expect("error message");
        assert!(message.contains("Gemini upstream request failed"));
        assert!(!message.contains("test-api-key-12345678901234567890"));
    }

    #[tokio::test]
    async fn gemini_sdk_route_redacts_key_from_upstream_error_body() {
        let mock = MockGeminiServer::start().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
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
                .set_json(gemini_upstream_error_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = test::read_body(response).await;
        let text = String::from_utf8(body.to_vec()).expect("body should be utf8");
        assert!(text.contains("upstream failed"));
        assert!(text.contains("key=[REDACTED]"));
        assert!(!text.contains("test-api-key-12345678901234567890"));

        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_sdk_stream_route_does_not_charge_upstream_error_body() {
        let mock = MockGeminiServer::start().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            "gemini-3.1-flash-lite",
            ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
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
                .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
                .set_json(gemini_upstream_error_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let stream_body = test::read_body(response).await;
        let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
        assert!(stream_text.contains("upstream failed"));
        assert!(stream_text.contains("key=[REDACTED]"));
        assert!(!stream_text.contains("test-api-key-12345678901234567890"));
        let provider_usage = budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("provider budget should exist");
        assert_eq!(provider_usage.current_spend, 0.0);
        let model_usage = budget_limits
            .models
            .get_model_usage("gemini-3.1-flash-lite")
            .expect("model budget should exist");
        assert_eq!(model_usage.current_spend, 0.0);

        mock.stop().await;
    }

    #[tokio::test]
    async fn gemini_sdk_stream_route_releases_budget_on_midstream_read_error() {
        let broken = BrokenGeminiStreamServer::start().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &broken.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            "gemini-3.1-flash-lite",
            ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
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
                .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let stream_body = test::read_body(response).await;
        let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
        assert!(stream_text.contains("partial"));
        assert!(stream_text.contains("Gemini upstream stream error"));
        let provider_usage = budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("provider budget should exist");
        assert_eq!(provider_usage.current_spend, 0.0);
        let model_usage = budget_limits
            .models
            .get_model_usage("gemini-3.1-flash-lite")
            .expect("model budget should exist");
        assert_eq!(model_usage.current_spend, 0.0);

        broken.stop().await;
    }
}
