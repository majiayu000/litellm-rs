#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures::mock_provider_config;
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::pricing_service::LiteLLMModelInfo;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Debug)]
    struct CapturedAudioRequest {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    }

    #[derive(Clone)]
    struct MockAudioState {
        duration: Option<f64>,
        captured_requests: Arc<Mutex<Vec<CapturedAudioRequest>>>,
    }

    struct MockAudioServer {
        base_url: String,
        captured_requests: Arc<Mutex<Vec<CapturedAudioRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockAudioServer {
        async fn start() -> Self {
            Self::start_with_duration(None).await
        }
        async fn start_with_duration(duration: Option<f64>) -> Self {
            let captured_requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockAudioState {
                duration,
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
                    .route(
                        "/audio/transcriptions",
                        web::post().to(mock_audio_transcriptions),
                    )
                    .route(
                        "/audio/translations",
                        web::post().to(mock_audio_translations),
                    )
                    .route("/audio/speech", web::post().to(mock_audio_speech))
                    .route("/stt", web::post().to(mock_xai_transcriptions))
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

        fn requests(&self) -> Vec<CapturedAudioRequest> {
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

    async fn mock_audio_transcriptions(
        state: web::Data<MockAudioState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({ "text": "mock transcript", "duration":state.duration }))
    }

    async fn mock_xai_transcriptions(
        state: web::Data<MockAudioState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({"text":"mock transcript", "language":"en", "duration":state.duration, "words":[{"text":"mock", "start":0.0, "end":0.5}]}))
    }

    async fn mock_audio_translations(
        state: web::Data<MockAudioState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok().json(json!({ "text": "mock translation", "duration":state.duration }))
    }

    async fn mock_audio_speech(
        state: web::Data<MockAudioState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        capture_request(&state, &request, body);
        HttpResponse::Ok()
            .insert_header(("Content-Type", "audio/mpeg"))
            .body(Bytes::from_static(b"mock-audio"))
    }

    fn capture_request(state: &MockAudioState, request: &HttpRequest, body: Bytes) {
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
            .push(CapturedAudioRequest {
                method: request.method().to_string(),
                path: request.path().to_string(),
                headers,
                body: body.to_vec(),
            });
    }

    async fn build_audio_state(base_url: &str) -> litellm_rs::server::state::AppState {
        build_audio_state_with_models(base_url, vec!["whisper-1".to_string(), "tts-1".to_string()])
            .await
    }

    async fn build_audio_state_with_models(
        base_url: &str,
        models: Vec<String>,
    ) -> litellm_rs::server::state::AppState {
        build_audio_state_for_provider(base_url, models, "openai").await
    }
    async fn build_audio_state_for_provider(
        base_url: &str,
        models: Vec<String>,
        provider: &str,
    ) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers = vec![mock_provider_config(
            &format!("mock-{provider}-audio"),
            provider,
            "sk-test",
            base_url,
            models,
        )];

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
    }

    fn add_test_audio_pricing(state: &litellm_rs::server::state::AppState, model: &str) {
        state.pricing.add_custom_model(
            model.to_string(),
            LiteLLMModelInfo {
                max_tokens: Some(4096),
                max_input_tokens: Some(4096),
                max_output_tokens: Some(4096),
                input_cost_per_token: Some(0.00001),
                output_cost_per_token: Some(0.00002),
                input_cost_per_character: None,
                output_cost_per_character: None,
                cost_per_second: None,
                litellm_provider: "openai".to_string(),
                mode: "audio_speech".to_string(),
                supports_function_calling: Some(false),
                supports_vision: Some(false),
                supports_streaming: Some(false),
                supports_parallel_function_calling: Some(false),
                supports_system_message: Some(false),
                extra: HashMap::new(),
            },
        );
    }

    fn add_test_time_audio_pricing(
        state: &litellm_rs::server::state::AppState,
        model: &str,
        cost_per_second: f64,
    ) {
        state.pricing.add_custom_model(
            model.to_string(),
            LiteLLMModelInfo {
                max_tokens: None,
                max_input_tokens: None,
                max_output_tokens: None,
                input_cost_per_token: None,
                output_cost_per_token: None,
                input_cost_per_character: None,
                output_cost_per_character: None,
                cost_per_second: Some(cost_per_second),
                litellm_provider: "openai".to_string(),
                mode: "audio_transcription".to_string(),
                supports_function_calling: Some(false),
                supports_vision: Some(false),
                supports_streaming: Some(false),
                supports_parallel_function_calling: Some(false),
                supports_system_message: Some(false),
                extra: HashMap::new(),
            },
        );
    }

    fn add_test_time_and_token_speech_pricing(
        state: &litellm_rs::server::state::AppState,
        model: &str,
    ) {
        state.pricing.add_custom_model(
            model.to_string(),
            LiteLLMModelInfo {
                max_tokens: Some(4096),
                max_input_tokens: Some(4096),
                max_output_tokens: Some(4096),
                input_cost_per_token: Some(0.00001),
                output_cost_per_token: Some(0.00002),
                input_cost_per_character: None,
                output_cost_per_character: None,
                cost_per_second: None,
                litellm_provider: "openai".to_string(),
                mode: "audio_speech".to_string(),
                supports_function_calling: Some(false),
                supports_vision: Some(false),
                supports_streaming: Some(false),
                supports_parallel_function_calling: Some(false),
                supports_system_message: Some(false),
                extra: HashMap::from([("output_cost_per_second".to_string(), json!(0.001))]),
            },
        );
    }

    fn audio_multipart_body(
        boundary: &str,
        model: &str,
        filename: &str,
        file_content: &[u8],
    ) -> Vec<u8> {
        audio_multipart_body_with_fields(boundary, model, filename, file_content, &[])
    }

    fn audio_multipart_body_with_fields(
        boundary: &str,
        model: &str,
        filename: &str,
        file_content: &[u8],
        fields: &[(&str, &str)],
    ) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body.extend_from_slice(model.as_bytes());
        body.extend_from_slice(b"\r\n");
        for (name, value) in fields {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body.extend_from_slice(value.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body.extend_from_slice(file_content);
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    #[tokio::test]
    async fn audio_transcriptions_route_executes_openai_provider() {
        let mock = MockAudioServer::start().await;
        let state = build_audio_state(&mock.base_url).await;
        add_test_time_audio_pricing(&state, "whisper-1", 0.001);
        state.budget_limits.providers.set_provider_limit(
            "mock-openai-audio",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-audio-boundary";
        let audio_content = vec![b'a'; 32_000];

        let mut multipart = Vec::new();
        for granularity in ["word", "segment"] {
            multipart.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"timestamp_granularities[]\"\r\n\r\n{granularity}\r\n").as_bytes());
        }
        multipart.extend(audio_multipart_body(
            boundary,
            "whisper-1",
            "sample.mp3",
            &audio_content,
        ));
        let req = test::TestRequest::post()
            .uri("/v1/audio/transcriptions")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(multipart)
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["text"], "mock transcript");

        let captured = mock.requests();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].method, "POST");
        assert_eq!(captured[0].path, "/audio/transcriptions");
        assert!(
            captured[0]
                .headers
                .get("content-type")
                .expect("multipart content type")
                .contains("multipart/form-data")
        );
        let multipart_body = String::from_utf8_lossy(&captured[0].body);
        assert_eq!(
            multipart_body
                .matches("name=\"timestamp_granularities[]\"")
                .count(),
            2
        );
        assert!(multipart_body.contains("\r\nword\r\n"));
        assert!(multipart_body.contains("\r\nsegment\r\n"));
        assert!(multipart_body.contains("name=\"model\""));
        assert!(multipart_body.contains("whisper-1"));
        assert!(multipart_body.contains("filename=\"sample.mp3\""));
        let spent = budget_limits
            .providers
            .get_provider_usage("mock-openai-audio")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert!(
            (spent - 0.002).abs() < f64::EPSILON,
            "successful time-priced transcription must record spend"
        );
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn audio_translations_route_executes_openai_provider() {
        let mock = MockAudioServer::start().await;
        let state = build_audio_state(&mock.base_url).await;
        add_test_time_audio_pricing(&state, "whisper-1", 0.001);
        state.budget_limits.providers.set_provider_limit(
            "mock-openai-audio",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-audio-boundary";
        let audio_content = vec![b'a'; 32_000];

        let req = test::TestRequest::post()
            .uri("/v1/audio/translations")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(audio_multipart_body(
                boundary,
                "whisper-1",
                "sample.mp3",
                &audio_content,
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["text"], "mock translation");

        let captured = mock.requests();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].path, "/audio/translations");
        let multipart_body = String::from_utf8_lossy(&captured[0].body);
        assert!(multipart_body.contains("whisper-1"));
        assert!(multipart_body.contains("filename=\"sample.mp3\""));
        let spent = budget_limits
            .providers
            .get_provider_usage("mock-openai-audio")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert!(
            (spent - 0.002).abs() < f64::EPSILON,
            "successful time-priced translation must record spend"
        );
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn xai_transcription_reserves_and_settles_native_duration() {
        for (duration, limit, priced, expected_status, language) in [
            (3.0, 1.0, true, StatusCode::OK, false),
            (75.0, 1.0, true, StatusCode::OK, false),
            (3.0, 0.001, true, StatusCode::PAYMENT_REQUIRED, false),
            (3.0, 1.0, false, StatusCode::BAD_REQUEST, false),
            (3.0, 1.0, true, StatusCode::OK, true),
        ] {
            let mock = MockAudioServer::start_with_duration(Some(duration)).await;
            let model = "grok-voice-transcribe-2.0";
            let state =
                build_audio_state_for_provider(&mock.base_url, vec![model.into()], "xai").await;
            state.pricing.add_custom_model(model.into(), serde_json::from_value(json!({
                "litellm_provider":"xai", "mode":"audio_transcription", "input_cost_per_second":priced.then_some(0.001)
            })).unwrap());
            state
                .budget_limits
                .providers
                .set_provider_limit("xai", ProviderLimitConfig::new(limit, ResetPeriod::Monthly));
            let budgets = state.budget_limits.clone();
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let boundary = "xai-duration";
            let request = test::TestRequest::post()
                .uri("/v1/audio/transcriptions")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(audio_multipart_body_with_fields(
                    boundary,
                    model,
                    "sample.mp3",
                    &vec![b'a'; 32_000],
                    if language { &[("language", "en")] } else { &[] },
                ))
                .to_request();
            let response = test::call_service(&app, request).await;
            assert_eq!(
                response.status(),
                expected_status,
                "duration={duration}, limit={limit}, priced={priced}"
            );
            if expected_status == StatusCode::OK {
                let body: Value = test::read_body_json(response).await;
                assert_eq!(body["duration"], duration);
                let spent = budgets
                    .providers
                    .get_provider_usage("xai")
                    .unwrap()
                    .current_spend;
                assert!((spent - duration * 0.001).abs() < 1e-10);
                let requests = mock.requests();
                assert_eq!(requests.len(), 1);
                assert_eq!(requests[0].path, "/stt");
                let multipart = String::from_utf8_lossy(&requests[0].body);
                if language {
                    assert!(multipart.contains("name=\"format\"\r\n\r\ntrue\r\n"));
                    assert!(
                        multipart.find("name=\"format\"").unwrap()
                            < multipart.find("name=\"file\"").unwrap()
                    );
                    assert!(multipart.contains("name=\"language\"\r\n\r\nen\r\n"));
                } else {
                    assert!(!multipart.contains("name=\"format\""));
                }
                assert!(!String::from_utf8_lossy(&requests[0].body).contains("response_format"));
            } else {
                assert!(mock.requests().is_empty());
            }
            mock.shutdown().await;
        }
    }

    #[tokio::test]
    async fn groq_translation_settles_returned_duration_with_minimum_and_estimate_fallback() {
        for duration in [Some(25.0), Some(3.0), Some(0.0), Some(-5.0), None] {
            let mock = MockAudioServer::start_with_duration(duration).await;
            let state = build_audio_state_for_provider(
                &mock.base_url,
                vec!["whisper-large-v3".into()],
                "groq",
            )
            .await;
            state.budget_limits.providers.set_provider_limit(
                "groq",
                ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
            );
            let (_, info) = state
                .pricing
                .get_model_info_for_provider("groq", "whisper-large-v3")
                .unwrap();
            let rate = info.extra["input_cost_per_second"].as_f64().unwrap();
            assert!(rate > 0.0);
            let budgets = state.budget_limits.clone();
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let boundary = "groq-duration";
            let req = test::TestRequest::post()
                .uri("/v1/audio/translations")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(audio_multipart_body(
                    boundary,
                    "whisper-large-v3",
                    "sample.mp3",
                    &vec![b'a'; 32_000],
                ))
                .to_request();
            let response = test::call_service(&app, req).await;
            assert_eq!(response.status(), StatusCode::OK);
            let body: Value = test::read_body_json(response).await;
            assert_eq!(body["text"], "mock translation");
            let spent = budgets
                .providers
                .get_provider_usage("groq")
                .unwrap()
                .current_spend;
            let expected_seconds = duration.filter(|d| *d > 0.0).unwrap_or(2.0).max(10.0);
            assert!(
                (spent - expected_seconds * rate).abs() < 1e-10,
                "duration={duration:?}, spent={spent}"
            );
            let captured = mock.requests();
            assert_eq!(captured.len(), 1);
            assert!(String::from_utf8_lossy(&captured[0].body).contains("verbose_json"));
            mock.shutdown().await;
        }
    }

    #[tokio::test]
    async fn audio_speech_route_executes_openai_provider() {
        let mock = MockAudioServer::start().await;
        let state = build_audio_state(&mock.base_url).await;
        add_test_audio_pricing(&state, "tts-1");
        state.budget_limits.providers.set_provider_limit(
            "mock-openai-audio",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(json!({
                "model": "tts-1",
                "input": "hello from litellm rs",
                "voice": "alloy",
                "response_format": "mp3"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("audio/mpeg")
        );
        let body = test::read_body(resp).await;
        assert_eq!(body.as_ref(), b"mock-audio");

        let captured = mock.requests();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].method, "POST");
        assert_eq!(captured[0].path, "/audio/speech");
        let forwarded: Value =
            serde_json::from_slice(&captured[0].body).expect("speech request should be json");
        assert_eq!(forwarded["model"], "tts-1");
        assert_eq!(forwarded["input"], "hello from litellm rs");
        assert_eq!(forwarded["voice"], "alloy");
        let spent = budget_limits
            .providers
            .get_provider_usage("mock-openai-audio")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert!(spent > 0.0, "successful audio speech must record spend");
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn audio_speech_uses_token_pricing_when_output_duration_is_unknown() {
        let mock = MockAudioServer::start().await;
        let state = build_audio_state(&mock.base_url).await;
        add_test_time_and_token_speech_pricing(&state, "tts-1");
        state.budget_limits.providers.set_provider_limit(
            "mock-openai-audio",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(json!({
                "model": "tts-1",
                "input": "twenty characters!!!",
                "voice": "alloy"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let spent = budget_limits
            .providers
            .get_provider_usage("mock-openai-audio")
            .map(|usage| usage.current_spend)
            .unwrap_or_default();
        assert!(
            (spent - 0.00015).abs() < f64::EPSILON,
            "speech without output duration must use the five-token input/output estimate"
        );
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn audio_routes_reject_exhausted_provider_budget_before_upstream() {
        assert_audio_budget_rejected("/v1/audio/transcriptions", "transcription").await;
        assert_audio_budget_rejected("/v1/audio/translations", "translation").await;

        let mock = MockAudioServer::start().await;
        let state = build_audio_state(&mock.base_url).await;
        state.budget_limits.providers.set_provider_limit(
            "mock-openai-audio",
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .record_spend("mock-openai-audio", "tts-1", 0.01);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(json!({
                "model": "tts-1",
                "input": "hello from litellm rs",
                "voice": "alloy"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(
            mock.requests().is_empty(),
            "speech budget rejection must happen before upstream"
        );
        mock.shutdown().await;
    }

    async fn assert_audio_budget_rejected(uri: &str, route_name: &str) {
        let mock = MockAudioServer::start().await;
        let state = build_audio_state(&mock.base_url).await;
        state.budget_limits.providers.set_provider_limit(
            "mock-openai-audio",
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .record_spend("mock-openai-audio", "whisper-1", 0.01);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-audio-boundary";

        let req = test::TestRequest::post()
            .uri(uri)
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(audio_multipart_body(
                boundary,
                "whisper-1",
                "sample.mp3",
                b"audio-bytes",
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(
            mock.requests().is_empty(),
            "{route_name} budget rejection must happen before upstream"
        );
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn audio_speech_rejects_non_tts_openai_model_before_provider_call() {
        let mock = MockAudioServer::start().await;
        let state = build_audio_state_with_models(&mock.base_url, vec!["gpt-4".to_string()]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(json!({
                "model": "gpt-4",
                "input": "hello from litellm rs",
                "voice": "alloy"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(
            mock.requests().is_empty(),
            "unsupported model must fail before provider execution"
        );
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn audio_routes_reject_invalid_temperature_before_provider_call() {
        assert_invalid_temperature_rejected("/v1/audio/transcriptions").await;
        assert_invalid_temperature_rejected("/v1/audio/translations").await;
    }

    async fn assert_invalid_temperature_rejected(uri: &str) {
        let mock = MockAudioServer::start().await;
        let state = build_audio_state(&mock.base_url).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-audio-boundary";

        let req = test::TestRequest::post()
            .uri(uri)
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(audio_multipart_body_with_fields(
                boundary,
                "whisper-1",
                "sample.mp3",
                b"audio-bytes",
                &[("temperature", "not-a-number")],
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(
            body["error"]["message"],
            "temperature must be a valid number"
        );
        assert!(
            mock.requests().is_empty(),
            "invalid temperature must fail before provider execution"
        );
        mock.shutdown().await;
    }
    #[tokio::test]
    async fn compactifai_transcription_reserves_and_settles_a_minute_minimum() {
        for (duration, limit, priced, expected_status) in [
            (3.0, 1.0, true, StatusCode::OK),
            (75.0, 1.0, true, StatusCode::OK),
            (75.0, 0.07, true, StatusCode::OK),
            (3.0, 0.059, true, StatusCode::PAYMENT_REQUIRED),
            (3.0, 1.0, false, StatusCode::BAD_REQUEST),
        ] {
            let mock = MockAudioServer::start_with_duration(Some(duration)).await;
            let model = "cai-whisper-large-v3-turbo-slim";
            let state =
                build_audio_state_for_provider(&mock.base_url, vec![model.into()], "compactifai")
                    .await;
            if priced {
                state.pricing.add_custom_model(model.into(), serde_json::from_value(json!({
                    "litellm_provider":"compactifai", "mode":"audio_transcription", "input_cost_per_second":0.001
                })).unwrap());
            }
            state.budget_limits.providers.set_provider_limit(
                "compactifai",
                ProviderLimitConfig::new(limit, ResetPeriod::Monthly),
            );
            state
                .budget_limits
                .models
                .set_model_limit(model, ModelLimitConfig::new(limit, ResetPeriod::Monthly));
            let budgets = state.budget_limits.clone();
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let boundary = "compactifai-duration";
            let request = test::TestRequest::post()
                .uri("/v1/audio/transcriptions")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(audio_multipart_body(
                    boundary,
                    model,
                    "sample.mp3",
                    &vec![b'a'; 32_000],
                ))
                .to_request();
            let response = test::call_service(&app, request).await;
            assert_eq!(
                response.status(),
                expected_status,
                "duration={duration}, limit={limit}, priced={priced}"
            );
            if expected_status == StatusCode::OK {
                let body: Value = test::read_body_json(response).await;
                assert_eq!(body["duration"], duration);
                let spent = budgets
                    .providers
                    .get_provider_usage("compactifai")
                    .unwrap()
                    .current_spend;
                assert!((spent - duration.max(60.0) * 0.001).abs() < 1e-10);
                assert!(
                    (budgets.models.get_model_usage(model).unwrap().current_spend - spent).abs()
                        < 1e-10
                );
                if duration * 0.001 > limit {
                    assert!(spent > limit);
                    let request = test::TestRequest::post()
                        .uri("/v1/audio/transcriptions")
                        .insert_header((
                            "content-type",
                            format!("multipart/form-data; boundary={boundary}"),
                        ))
                        .set_payload(audio_multipart_body(
                            boundary,
                            model,
                            "sample.mp3",
                            &vec![b'a'; 32_000],
                        ))
                        .to_request();
                    let response = test::call_service(&app, request).await;
                    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
                }
                let requests = mock.requests();
                assert_eq!(requests.len(), 1);
                assert_eq!(requests[0].path, "/audio/transcriptions");
                assert!(String::from_utf8_lossy(&requests[0].body).contains("verbose_json"));
            } else {
                assert!(mock.requests().is_empty());
            }
            mock.shutdown().await;
        }
    }

    #[tokio::test]
    async fn audio_upload_routes_keep_audio_and_prompt_admission_after_settlement() {
        use std::sync::atomic::Ordering::Relaxed;

        for uri in ["/v1/audio/transcriptions", "/v1/audio/translations"] {
            for prompt in [None, Some("guide")] {
                let mock = MockAudioServer::start().await;
                let state = build_audio_state(&mock.base_url).await;
                add_test_time_audio_pricing(&state, "whisper-1", 0.001);
                let file = vec![b'a'; 256];
                let estimate = 64 + u32::from(prompt.is_some()) * 2;
                let mut next = state.config().as_ref().clone();
                next.gateway.providers[0].tpm = estimate;
                next.gateway.providers[0].rpm = 100;
                next.gateway.providers[0].max_concurrent_requests = 100;
                state.apply_runtime(next).await.unwrap();
                let deployment = state
                    .unified_router()
                    .get_deployment("mock-openai-audio-whisper-1")
                    .unwrap();
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(state))
                        .configure(litellm_rs::server::routes::ai::configure_routes),
                )
                .await;
                let boundary = "audio-admission-boundary";
                let fields = prompt
                    .map(|text| vec![("prompt", text)])
                    .unwrap_or_default();
                let body = audio_multipart_body_with_fields(
                    boundary,
                    "whisper-1",
                    "sample.mp3",
                    &file,
                    &fields,
                );
                loop {
                    let second = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        % 60;
                    if second < 40 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(61 - second)).await;
                }
                let response = tokio::time::timeout(
                    Duration::from_secs(5),
                    test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri(uri)
                            .insert_header((
                                "content-type",
                                format!("multipart/form-data; boundary={boundary}"),
                            ))
                            .set_payload(body.clone())
                            .to_request(),
                    ),
                )
                .await
                .unwrap();
                assert_eq!(response.status(), StatusCode::OK, "{uri}: {prompt:?}");
                let _: Value = test::read_body_json(response).await;
                assert_eq!(mock.requests().len(), 1);
                assert_eq!(deployment.state.tpm_current.load(Relaxed), 0);
                // The next denied request and unchanged upstream count verify the retained estimate.
                assert_eq!(deployment.state.rpm_current.load(Relaxed), 1);
                assert_eq!(deployment.state.success_requests.load(Relaxed), 1);
                assert_eq!(deployment.state.fail_requests.load(Relaxed), 0);
                assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
                let next = tokio::time::timeout(
                    Duration::from_secs(1),
                    test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri(uri)
                            .insert_header((
                                "content-type",
                                format!("multipart/form-data; boundary={boundary}"),
                            ))
                            .set_payload(body)
                            .to_request(),
                    ),
                )
                .await;
                if let Ok(response) = next {
                    assert!(!response.status().is_success());
                }
                assert_eq!(mock.requests().len(), 1);
                assert_eq!(deployment.state.tpm_current.load(Relaxed), 0);
                // The next denied request and unchanged upstream count verify the retained estimate.
                assert_eq!(deployment.state.rpm_current.load(Relaxed), 1);
                assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
                mock.shutdown().await;
            }
        }
    }
}
