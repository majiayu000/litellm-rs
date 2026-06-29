#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone)]
    struct MockImageState {
        paths: Arc<Mutex<Vec<String>>>,
    }

    struct MockImageServer {
        base_url: String,
        paths: Arc<Mutex<Vec<String>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockImageServer {
        async fn start() -> Self {
            let paths = Arc::new(Mutex::new(Vec::new()));
            let state = MockImageState {
                paths: Arc::clone(&paths),
            };
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
            let address = listener.local_addr().expect("mock address");
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .route("/v1/images/edits", web::post().to(mock_image_edit))
            })
            .listen(listener)
            .expect("mock server should listen")
            .run();
            let handle = server.handle();
            let task = tokio::spawn(server);
            wait_for_server(address).await;

            Self {
                base_url: format!("http://{address}/v1"),
                paths,
                handle,
                task,
            }
        }

        fn paths(&self) -> Vec<String> {
            self.paths.lock().unwrap().clone()
        }

        async fn stop(self) {
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
        _body: Bytes,
    ) -> HttpResponse {
        state.paths.lock().unwrap().push(request.path().to_string());
        HttpResponse::Ok().json(json!({
            "created": 1710000000,
            "data": [{ "url": "https://images.example.test/edit.png" }]
        }))
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

    fn image_provider(
        name: &str,
        provider_type: &str,
        base_url: &str,
        models: Vec<String>,
    ) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: provider_type.to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some(base_url.to_string()),
            models,
            ..ProviderConfig::default()
        }
    }

    fn image_edit_multipart_body(boundary: &str) -> Vec<u8> {
        let mut body = Vec::new();
        add_text_field(&mut body, boundary, "model", "gpt-image-1-mini");
        add_text_field(&mut body, boundary, "prompt", "make it lighter");
        add_file_field(&mut body, boundary, "image", "input.png", b"png-bytes");
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
        content: &[u8],
    ) {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }

    #[tokio::test]
    async fn native_openai_image_edit_uses_selected_provider_config_after_budget_fallback() {
        let exhausted = MockImageServer::start().await;
        let fallback = MockImageServer::start().await;
        let state = build_test_state(vec![
            image_provider(
                "openai-primary",
                "openai",
                &exhausted.base_url,
                vec!["gpt-image-1-mini".to_string()],
            ),
            image_provider(
                "openai-secondary",
                "openai",
                &fallback.base_url,
                vec!["gpt-image-1-mini".to_string()],
            ),
        ])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "openai-primary",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .providers
            .record_provider_spend("openai-primary", 2.0);
        state.budget_limits.providers.set_provider_limit(
            "openai-secondary",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
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

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(
            body["data"][0]["url"],
            "https://images.example.test/edit.png"
        );
        assert!(exhausted.paths().is_empty());
        assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

        exhausted.stop().await;
        fallback.stop().await;
    }

    #[tokio::test]
    async fn wildcard_openai_compatible_image_edit_tries_next_provider_name_key() {
        let exhausted = MockImageServer::start().await;
        let fallback = MockImageServer::start().await;
        let state = build_test_state(vec![
            image_provider(
                "wild-primary",
                "openai_compatible",
                &exhausted.base_url,
                Vec::new(),
            ),
            image_provider(
                "wild-secondary",
                "openai_compatible",
                &fallback.base_url,
                Vec::new(),
            ),
        ])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "wild-primary",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .providers
            .record_provider_spend("wild-primary", 2.0);
        state.budget_limits.providers.set_provider_limit(
            "wild-secondary",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
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

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(
            body["data"][0]["url"],
            "https://images.example.test/edit.png"
        );
        assert!(exhausted.paths().is_empty());
        assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

        exhausted.stop().await;
        fallback.stop().await;
    }

    #[tokio::test]
    async fn explicit_image_provider_falls_back_to_wildcard_provider() {
        let exhausted = MockImageServer::start().await;
        let fallback = MockImageServer::start().await;
        let state = build_test_state(vec![
            image_provider(
                "explicit-primary",
                "openai_compatible",
                &exhausted.base_url,
                vec!["gpt-image-1-mini".to_string()],
            ),
            image_provider(
                "wild-secondary",
                "openai_compatible",
                &fallback.base_url,
                Vec::new(),
            ),
        ])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "explicit-primary",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .providers
            .record_provider_spend("explicit-primary", 2.0);
        state.budget_limits.providers.set_provider_limit(
            "wild-secondary",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
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

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(
            body["data"][0]["url"],
            "https://images.example.test/edit.png"
        );
        assert!(exhausted.paths().is_empty());
        assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

        exhausted.stop().await;
        fallback.stop().await;
    }
}
