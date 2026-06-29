#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Debug)]
    struct CapturedImageRequest {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    }

    #[derive(Clone)]
    struct MockImageState {
        captured_requests: Arc<Mutex<Vec<CapturedImageRequest>>>,
    }

    struct MockImageServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedImageRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockImageServer {
        async fn start_image_mock() -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockImageState {
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
                    .route("/v1/images/edits", web::post().to(mock_image_edit))
                    .route(
                        "/v1/images/variations",
                        web::post().to(mock_image_variation),
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

        fn requests(&self) -> Vec<CapturedImageRequest> {
            self.captured_requests.lock().unwrap().clone()
        }

        async fn stop_image_mock(self) {
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

    async fn mock_image_edit(
        state: web::Data<MockImageState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "created": 1710000000,
            "data": [{ "url": "https://images.example.test/edit.png" }]
        }))
    }

    async fn mock_image_variation(
        state: web::Data<MockImageState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({
            "created": 1710000001,
            "data": [{ "b64_json": "dmFyaWF0aW9u" }]
        }))
    }

    fn capture_request(state: &MockImageState, request: &HttpRequest, body: Bytes) {
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
            .push(CapturedImageRequest {
                method: request.method().to_string(),
                path: request.path().to_string(),
                headers,
                body: body.to_vec(),
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

    async fn build_auth_required_state(
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

    fn image_route_provider(base_url: &str) -> ProviderConfig {
        image_route_provider_with_name_and_models("mock-openai-compatible", base_url, Vec::new())
    }

    fn image_route_provider_with_name_and_models(
        name: &str,
        base_url: &str,
        models: Vec<String>,
    ) -> ProviderConfig {
        let mut provider = ProviderConfig {
            name: name.to_string(),
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

    fn image_edit_multipart_body(boundary: &str) -> Vec<u8> {
        let mut body = Vec::new();
        add_text_field(&mut body, boundary, "model", "gpt-image-1-mini");
        add_text_field(&mut body, boundary, "prompt", "make it lighter");
        add_text_field(&mut body, boundary, "size", "1024x1024");
        add_file_field(
            &mut body,
            boundary,
            "image",
            "input.png",
            "image/png",
            b"png-bytes",
        );
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    fn image_variation_multipart_body(boundary: &str) -> Vec<u8> {
        let mut body = Vec::new();
        add_text_field(&mut body, boundary, "model", "gpt-image-1-mini");
        add_text_field(&mut body, boundary, "n", "1");
        add_file_field(
            &mut body,
            boundary,
            "image",
            "source.png",
            "image/png",
            b"source-png-bytes",
        );
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    fn image_variation_without_model_multipart_body(boundary: &str) -> Vec<u8> {
        let mut body = Vec::new();
        add_text_field(&mut body, boundary, "n", "1");
        add_file_field(
            &mut body,
            boundary,
            "image",
            "source.png",
            "image/png",
            b"source-png-bytes",
        );
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    fn image_edit_unpriced_model_multipart_body(boundary: &str) -> Vec<u8> {
        let mut body = Vec::new();
        add_text_field(&mut body, boundary, "model", "unpriced-image-model");
        add_text_field(&mut body, boundary, "prompt", "make it lighter");
        add_file_field(
            &mut body,
            boundary,
            "image",
            "input.png",
            "image/png",
            b"png-bytes",
        );
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    fn add_text_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    fn add_file_field(
        body: &mut Vec<u8>,
        boundary: &str,
        name: &str,
        filename: &str,
        content_type: &str,
        content: &[u8],
    ) {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }

    #[tokio::test]
    async fn image_edit_and_variation_routes_without_provider_fail_closed() {
        let state = build_test_state(Vec::new()).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-image-boundary";

        let edit_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/edits")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_edit_multipart_body(boundary))
                .to_request(),
        )
        .await;
        assert_eq!(edit_resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let edit_body: Value = test::read_body_json(edit_resp).await;
        assert!(
            edit_body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Image edits and variations API requires")
        );
        assert_eq!(edit_body["error"]["type"], "server_error");

        let variation_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/variations")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_variation_multipart_body(boundary))
                .to_request(),
        )
        .await;
        assert_eq!(variation_resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let variation_body: Value = test::read_body_json(variation_resp).await;
        assert!(
            variation_body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Image edits and variations API requires")
        );
    }

    #[tokio::test]
    async fn image_edit_and_variation_routes_proxy_openai_compatible_provider() {
        let mock = MockImageServer::start_image_mock().await;
        let state = build_test_state(vec![
            image_route_provider_with_name_and_models(
                "wrong-model-provider",
                "http://127.0.0.1:9/v1",
                vec!["other-image-model".to_string()],
            ),
            image_route_provider_with_name_and_models(
                "mock-openai-compatible",
                &mock.base_url,
                vec!["gpt-image-1-mini".to_string()],
            ),
        ])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "mock-openai-compatible",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            "gpt-image-1-mini",
            ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-image-boundary";

        let edit_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/edits")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_edit_multipart_body(boundary))
                .to_request(),
        )
        .await;
        assert_eq!(edit_resp.status(), StatusCode::OK);
        let edit_body: Value = test::read_body_json(edit_resp).await;
        assert_eq!(
            edit_body["data"][0]["url"],
            "https://images.example.test/edit.png"
        );

        let variation_resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/variations")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_variation_multipart_body(boundary))
                .to_request(),
        )
        .await;
        assert_eq!(variation_resp.status(), StatusCode::OK);
        let variation_body: Value = test::read_body_json(variation_resp).await;
        assert_eq!(variation_body["data"][0]["b64_json"], "dmFyaWF0aW9u");

        let captured = mock.requests();
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0].method, "POST");
        assert_eq!(captured[0].path, "/v1/images/edits");
        assert_eq!(captured[1].path, "/v1/images/variations");
        for request in &captured {
            assert!(
                request
                    .headers
                    .get("content-type")
                    .expect("multipart content type")
                    .contains("multipart/form-data")
            );
            assert_eq!(request.headers["authorization"], "Bearer sk-test");
            assert_eq!(request.headers["openai-organization"], "org-test");
            assert_eq!(request.headers["openai-project"], "proj-test");
            assert_eq!(request.headers["x-base-header"], "base-value");
            assert_eq!(request.headers["x-custom-header"], "custom-value");
            let multipart_body = String::from_utf8_lossy(&request.body);
            assert!(multipart_body.contains("name=\"model\""));
            assert!(multipart_body.contains("gpt-image-1-mini"));
            assert!(multipart_body.contains("name=\"image\""));
        }
        let edit_multipart = String::from_utf8_lossy(&captured[0].body);
        assert!(edit_multipart.contains("name=\"prompt\""));
        assert!(edit_multipart.contains("make it lighter"));
        assert!(edit_multipart.contains("filename=\"input.png\""));
        let variation_multipart = String::from_utf8_lossy(&captured[1].body);
        assert!(variation_multipart.contains("filename=\"source.png\""));
        assert!(
            budget_limits
                .providers
                .get_provider_usage("mock-openai-compatible")
                .expect("provider spend should be recorded")
                .current_spend
                > 0.0
        );
        assert!(
            budget_limits
                .models
                .get_model_usage("gpt-image-1-mini")
                .expect("model spend should be recorded")
                .current_spend
                > 0.0
        );

        mock.stop_image_mock().await;
    }

    #[tokio::test]
    async fn image_edit_rejects_unauthenticated_request_when_auth_is_enabled() {
        let mock = MockImageServer::start_image_mock().await;
        let state = build_auth_required_state(vec![image_route_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-image-boundary";

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/edits")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_edit_multipart_body(boundary))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "authentication_error");
        assert!(
            mock.requests().is_empty(),
            "unauthorized requests must not reach upstream"
        );

        mock.stop_image_mock().await;
    }

    #[tokio::test]
    async fn image_variation_rejects_missing_model_before_upstream() {
        let mock = MockImageServer::start_image_mock().await;
        let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-image-boundary";

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/variations")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_variation_without_model_multipart_body(boundary))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(
            body["error"]["message"],
            "Validation error: model is required"
        );
        assert!(
            mock.requests().is_empty(),
            "missing model must fail before upstream call"
        );

        mock.stop_image_mock().await;
    }

    #[tokio::test]
    async fn image_edit_rejects_unpriced_model_before_upstream() {
        let mock = MockImageServer::start_image_mock().await;
        let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-image-boundary";

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/edits")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_edit_unpriced_model_multipart_body(boundary))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("unpriced-image-model")
        );
        assert!(
            mock.requests().is_empty(),
            "unpriced model must fail before upstream call"
        );

        mock.stop_image_mock().await;
    }

    #[tokio::test]
    async fn image_edit_rejects_exhausted_provider_budget_before_upstream() {
        let mock = MockImageServer::start_image_mock().await;
        let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
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
        let boundary = "litellm-rs-image-boundary";

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/edits")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_edit_multipart_body(boundary))
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

        mock.stop_image_mock().await;
    }

    #[tokio::test]
    async fn image_edit_rejects_exhausted_model_budget_before_upstream() {
        let mock = MockImageServer::start_image_mock().await;
        let state = build_test_state(vec![image_route_provider(&mock.base_url)]).await;
        state.budget_limits.models.set_model_limit(
            "gpt-image-1-mini",
            ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .models
            .record_model_spend("gpt-image-1-mini", 2.0);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-image-boundary";

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/edits")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(image_edit_multipart_body(boundary))
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
                .contains("model 'gpt-image-1-mini' budget exceeded")
        );
        assert!(
            mock.requests().is_empty(),
            "model budget rejection must happen before upstream call"
        );

        mock.stop_image_mock().await;
    }
}
