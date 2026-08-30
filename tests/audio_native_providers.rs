#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[allow(dead_code)]
#[path = "common/providers.rs"]
mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures;
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::core::audio::types::{SpeechRequest, TranscriptionRequest};
    use litellm_rs::core::providers::{Provider, ProviderError, ProviderType};
    use litellm_rs::core::types::{context::RequestContext, model::ProviderCapability};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Debug)]
    struct CapturedRequest {
        uri: String,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    }

    #[derive(Clone)]
    struct MockState {
        requests: Arc<Mutex<Vec<CapturedRequest>>>,
        status: StatusCode,
        delay: Duration,
    }

    struct MockAudioServer {
        base_url: String,
        requests: Arc<Mutex<Vec<CapturedRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockAudioServer {
        async fn start(status: StatusCode, delay: Duration) -> Self {
            let requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockState {
                requests: Arc::clone(&requests),
                status,
                delay,
            };
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("mock audio server should bind");
            let address = listener
                .local_addr()
                .expect("mock audio server should have an address");
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .default_service(web::to(mock_audio_provider))
            })
            .listen(listener)
            .expect("mock audio server should listen")
            .run();
            let handle = server.handle();
            let task = tokio::spawn(server);
            wait_for_server(address).await;
            Self {
                base_url: format!("http://{address}"),
                requests,
                handle,
                task,
            }
        }

        fn requests(&self) -> Vec<CapturedRequest> {
            self.requests
                .lock()
                .expect("captured request lock should be available")
                .clone()
        }

        async fn shutdown(self) {
            self.handle.stop(true).await;
            self.task
                .await
                .expect("mock server task should join")
                .expect("mock server should stop cleanly");
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

    async fn mock_audio_provider(
        state: web::Data<MockState>,
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
            .requests
            .lock()
            .expect("captured request lock should be available")
            .push(CapturedRequest {
                uri: request.uri().to_string(),
                headers,
                body: body.to_vec(),
            });

        if !state.delay.is_zero() {
            tokio::time::sleep(state.delay).await;
        }
        if !state.status.is_success() {
            return HttpResponse::build(state.status)
                .insert_header(("retry-after", "7"))
                .json(json!({"detail": {"message": "provider rejected request"}}));
        }

        match request.path() {
            "/v1/listen" => HttpResponse::Ok().json(json!({
                "metadata": {"duration": 1.25},
                "results": {"channels": [{"alternatives": [{
                    "transcript": "deepgram transcript",
                    "languages": ["en"],
                    "words": [{"word": "deepgram", "start": 0.0, "end": 0.4}]
                }]}]}
            })),
            "/v1/speak" => HttpResponse::Ok()
                .insert_header(("content-type", "audio/wav"))
                .body(Bytes::from_static(b"deepgram-audio")),
            "/v1/speech-to-text" => HttpResponse::Ok().json(json!({
                "text": "elevenlabs transcript",
                "language_code": "en",
                "words": [{"text": "elevenlabs", "type": "word", "start": 0.0, "end": 0.5}]
            })),
            path if path.starts_with("/v1/text-to-speech/") => HttpResponse::Ok()
                .insert_header(("content-type", "audio/mpeg"))
                .body(Bytes::from_static(b"elevenlabs-audio")),
            _ => HttpResponse::NotFound().finish(),
        }
    }

    async fn native_provider(
        provider_type: ProviderType,
        base_url: &str,
        timeout: u64,
    ) -> Provider {
        Provider::from_config_async(
            provider_type,
            json!({
                "api_key": "native-audio-secret",
                "base_url": base_url,
                "endpoint_access": "private_network",
                "timeout": timeout
            }),
        )
        .await
        .expect("native audio provider should be created")
    }

    fn transcription_request(model: &str) -> TranscriptionRequest {
        TranscriptionRequest {
            file: b"RIFF-test-audio".to_vec(),
            filename: "sample.wav".to_string(),
            model: model.to_string(),
            language: Some("en".to_string()),
            prompt: None,
            response_format: Some("verbose_json".to_string()),
            temperature: Some(0.2),
            timestamp_granularities: Some(vec!["word".to_string()]),
        }
    }

    #[tokio::test]
    async fn deepgram_native_stt_and_tts_preserve_wire_contract() {
        let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
        let provider = native_provider(ProviderType::Deepgram, &mock.base_url, 5).await;

        let transcript = provider
            .audio_transcription(transcription_request("nova-3"), RequestContext::new())
            .await
            .expect("Deepgram transcription should succeed");
        assert_eq!(transcript.text, "deepgram transcript");
        assert_eq!(transcript.language.as_deref(), Some("en"));
        assert_eq!(transcript.duration, Some(1.25));

        let speech = provider
            .text_to_speech(
                SpeechRequest {
                    input: "Deepgram says hello".to_string(),
                    model: "aura-2-thalia-en".to_string(),
                    voice: "caller-required-voice".to_string(),
                    response_format: Some("wav".to_string()),
                    speed: Some(1.25),
                },
                RequestContext::new(),
            )
            .await
            .expect("Deepgram speech should succeed");
        assert_eq!(speech.audio, b"deepgram-audio");
        assert_eq!(speech.content_type, "audio/wav");

        let requests = mock.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].uri.starts_with("/v1/listen?"));
        assert!(requests[0].uri.contains("model=nova-3"));
        assert!(requests[0].uri.contains("language=en"));
        assert_eq!(
            requests[0].headers.get("authorization").map(String::as_str),
            Some("Token native-audio-secret")
        );
        assert_eq!(
            requests[0].headers.get("content-type").map(String::as_str),
            Some("audio/wav")
        );
        assert_eq!(requests[0].body, b"RIFF-test-audio");
        assert!(requests[1].uri.contains("model=aura-2-thalia-en"));
        assert!(requests[1].uri.contains("encoding=linear16"));
        assert!(requests[1].uri.contains("container=wav"));
        assert!(requests[1].uri.contains("speed=1.25"));
        let tts_body: Value = serde_json::from_slice(&requests[1].body)
            .expect("Deepgram speech request should be JSON");
        assert_eq!(tts_body, json!({"text": "Deepgram says hello"}));
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn elevenlabs_native_stt_and_tts_preserve_wire_contract() {
        let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
        let provider = native_provider(ProviderType::ElevenLabs, &mock.base_url, 5).await;

        let transcript = provider
            .audio_transcription(
                transcription_request("scribe_v1_experimental"),
                RequestContext::new(),
            )
            .await
            .expect("ElevenLabs transcription should succeed");
        assert_eq!(transcript.text, "elevenlabs transcript");
        assert_eq!(transcript.language.as_deref(), Some("en"));
        assert_eq!(transcript.duration, None);

        let speech = provider
            .text_to_speech(
                SpeechRequest {
                    input: "ElevenLabs says hello".to_string(),
                    model: "eleven_v3".to_string(),
                    voice: "voice-original-123".to_string(),
                    response_format: Some("mp3_22050_32".to_string()),
                    speed: None,
                },
                RequestContext::new(),
            )
            .await
            .expect("ElevenLabs speech should succeed");
        assert_eq!(speech.audio, b"elevenlabs-audio");
        assert_eq!(speech.content_type, "audio/mpeg");

        let requests = mock.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].uri, "/v1/speech-to-text");
        assert_eq!(
            requests[0].headers.get("xi-api-key").map(String::as_str),
            Some("native-audio-secret")
        );
        let multipart = String::from_utf8_lossy(&requests[0].body);
        assert!(multipart.contains("name=\"model_id\""));
        assert!(multipart.contains("scribe_v1_experimental"));
        assert!(multipart.contains("filename=\"sample.wav\""));
        assert!(multipart.contains("name=\"language_code\""));
        assert!(multipart.contains("name=\"temperature\""));
        assert_eq!(
            requests[1].uri,
            "/v1/text-to-speech/voice-original-123?output_format=mp3_22050_32"
        );
        let tts_body: Value = serde_json::from_slice(&requests[1].body)
            .expect("ElevenLabs speech request should be JSON");
        assert_eq!(tts_body["text"], "ElevenLabs says hello");
        assert_eq!(tts_body["model_id"], "eleven_v3");
        assert!(tts_body.get("voice_settings").is_none());
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn elevenlabs_rejects_standard_tts_speed_before_upstream_io() {
        let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
        let provider = native_provider(ProviderType::ElevenLabs, &mock.base_url, 5).await;

        let result = provider
            .text_to_speech(
                SpeechRequest {
                    input: "unsupported speed".to_string(),
                    model: "eleven_v3".to_string(),
                    voice: "voice-original-123".to_string(),
                    response_format: Some("mp3".to_string()),
                    speed: Some(0.9),
                },
                RequestContext::new(),
            )
            .await;
        let error = match result {
            Ok(_) => panic!("ElevenLabs standard TTS speed must fail before I/O"),
            Err(error) => error,
        };

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        assert!(mock.requests().is_empty());
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn elevenlabs_accepts_neutral_standard_tts_speed() {
        let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
        let provider = native_provider(ProviderType::ElevenLabs, &mock.base_url, 5).await;

        let response = provider
            .text_to_speech(
                SpeechRequest {
                    input: "neutral speed".to_string(),
                    model: "eleven_v3".to_string(),
                    voice: "voice-original-123".to_string(),
                    response_format: Some("mp3".to_string()),
                    speed: Some(1.0),
                },
                RequestContext::new(),
            )
            .await
            .expect("neutral ElevenLabs speed should preserve standard TTS semantics");

        assert_eq!(response.audio, b"elevenlabs-audio");
        assert_eq!(mock.requests().len(), 1);
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn native_audio_custom_v1_base_does_not_duplicate_version_segment() {
        let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
        let versioned_base = format!("{}/v1", mock.base_url);

        let deepgram = native_provider(ProviderType::Deepgram, &versioned_base, 5).await;
        deepgram
            .audio_transcription(transcription_request("nova-3"), RequestContext::new())
            .await
            .expect("versioned Deepgram base should succeed");

        let elevenlabs = native_provider(ProviderType::ElevenLabs, &versioned_base, 5).await;
        elevenlabs
            .text_to_speech(
                SpeechRequest {
                    input: "versioned base".to_string(),
                    model: "eleven_v3".to_string(),
                    voice: "voice-original-123".to_string(),
                    response_format: Some("mp3".to_string()),
                    speed: None,
                },
                RequestContext::new(),
            )
            .await
            .expect("versioned ElevenLabs base should succeed");

        let requests = mock.requests();
        assert!(requests[0].uri.starts_with("/v1/listen?"));
        assert!(requests[1].uri.starts_with("/v1/text-to-speech/"));
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn deepgram_omitted_language_enables_detection() {
        let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
        let provider = native_provider(ProviderType::Deepgram, &mock.base_url, 5).await;
        let mut request = transcription_request("nova-3");
        request.language = None;

        provider
            .audio_transcription(request, RequestContext::new())
            .await
            .expect("Deepgram automatic language detection should succeed");

        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        let request_url = url::Url::parse(&format!("http://mock{}", requests[0].uri))
            .expect("captured Deepgram URI should parse");
        let query = request_url.query_pairs().collect::<HashMap<_, _>>();
        assert_eq!(
            query.get("detect_language").map(|value| value.as_ref()),
            Some("true")
        );
        assert!(!query.contains_key("language"));
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn native_audio_errors_and_timeout_are_structured() {
        let rate_limited =
            MockAudioServer::start(StatusCode::TOO_MANY_REQUESTS, Duration::ZERO).await;
        let deepgram = native_provider(ProviderType::Deepgram, &rate_limited.base_url, 5).await;
        let error = deepgram
            .audio_transcription(transcription_request("nova-3"), RequestContext::new())
            .await
            .expect_err("Deepgram 429 should fail");
        assert!(matches!(
            error,
            ProviderError::RateLimit {
                retry_after: Some(7),
                ..
            }
        ));
        rate_limited.shutdown().await;

        let invalid =
            MockAudioServer::start(StatusCode::UNPROCESSABLE_ENTITY, Duration::ZERO).await;
        let elevenlabs = native_provider(ProviderType::ElevenLabs, &invalid.base_url, 5).await;
        let error = elevenlabs
            .audio_transcription(transcription_request("scribe_v1"), RequestContext::new())
            .await
            .expect_err("ElevenLabs 422 should fail");
        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        invalid.shutdown().await;

        let slow = MockAudioServer::start(StatusCode::OK, Duration::from_millis(1_200)).await;
        let elevenlabs = native_provider(ProviderType::ElevenLabs, &slow.base_url, 1).await;
        let error = elevenlabs
            .audio_transcription(transcription_request("scribe_v1"), RequestContext::new())
            .await
            .expect_err("configured timeout should fail");
        assert!(matches!(error, ProviderError::Timeout { .. }));
        slow.shutdown().await;
    }

    #[tokio::test]
    async fn native_audio_capabilities_are_model_exact_and_token_cost_fails_closed() {
        let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
        let deepgram = native_provider(ProviderType::Deepgram, &mock.base_url, 5).await;
        assert!(
            deepgram
                .supports_capability_for_model("nova-3", &ProviderCapability::AudioTranscription)
        );
        assert!(deepgram.supports_capability_for_model(
            "nova-3-general",
            &ProviderCapability::AudioTranscription
        ));
        assert!(deepgram.supports_capability_for_model(
            "nova-3-medical",
            &ProviderCapability::AudioTranscription
        ));
        assert!(
            !deepgram.supports_capability_for_model("nova-3", &ProviderCapability::TextToSpeech)
        );
        assert!(
            deepgram.supports_capability_for_model(
                "aura-2-thalia-en",
                &ProviderCapability::TextToSpeech
            )
        );
        assert!(!deepgram.supports_capability_for_model(
            "unknown-audio-model",
            &ProviderCapability::AudioTranscription
        ));
        let error = deepgram
            .calculate_cost("nova-3", 1_000, 0)
            .await
            .expect_err("time/character pricing must not use token scalars");
        assert!(matches!(error, ProviderError::NotSupported { .. }));

        let elevenlabs = native_provider(ProviderType::ElevenLabs, &mock.base_url, 5).await;
        assert!(elevenlabs.supports_capability_for_model(
            "scribe_v1_experimental",
            &ProviderCapability::AudioTranscription
        ));
        assert!(
            !elevenlabs.supports_capability_for_model(
                "scribe_v2",
                &ProviderCapability::AudioTranscription
            )
        );
        assert!(
            !elevenlabs
                .supports_capability_for_model("scribe_v1", &ProviderCapability::TextToSpeech)
        );
        assert!(
            elevenlabs
                .supports_capability_for_model("eleven_v3", &ProviderCapability::TextToSpeech)
        );
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn native_audio_factory_rejects_unsupported_max_retries() {
        for provider_type in [ProviderType::Deepgram, ProviderType::ElevenLabs] {
            let error = Provider::from_config_async(
                provider_type,
                json!({
                    "api_key": "native-audio-secret",
                    "max_retries": 2
                }),
            )
            .await
            .expect_err("native audio max_retries must not be accepted while it is unused");

            assert!(matches!(error, ProviderError::Configuration { .. }));
            assert!(error.to_string().contains("max_retries"));
        }
    }

    async fn gateway_state(base_url: &str) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers = vec![
            provider_fixtures::mock_provider_config(
                "deepgram-audio",
                "deepgram",
                "deepgram-secret",
                base_url,
                vec!["nova-3-general".to_string(), "aura-2-thalia-en".to_string()],
            ),
            provider_fixtures::mock_provider_config(
                "elevenlabs-audio",
                "elevenlabs",
                "elevenlabs-secret",
                base_url,
                vec!["eleven_v3".to_string()],
            ),
        ];
        GatewayHttpServer::new(&config)
            .await
            .expect("gateway should initialize with native audio providers")
            .state()
            .clone()
    }

    fn audio_multipart(boundary: &str, model: &str) -> Vec<u8> {
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{model}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.wav\"\r\nContent-Type: audio/wav\r\n\r\nRIFF-route-audio\r\n--{boundary}--\r\n"
        )
        .into_bytes()
    }

    #[tokio::test]
    async fn gateway_routes_select_native_deepgram_and_elevenlabs() {
        let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
        let state = gateway_state(&mock.base_url).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let boundary = "native-audio-route-boundary";
        let request = test::TestRequest::post()
            .uri("/v1/audio/transcriptions")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(audio_multipart(boundary, "nova-3-general"))
            .to_request();
        let response = test::call_service(&app, request).await;
        let status = response.status();
        let body = test::read_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "transcription route response: {}",
            String::from_utf8_lossy(&body)
        );

        let request = test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(json!({
                "model": "eleven_v3",
                "input": "route dispatch",
                "voice": "route-voice-id",
                "response_format": "mp3_44100_128"
            }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            test::read_body(response).await.as_ref(),
            b"elevenlabs-audio"
        );

        let request = test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(json!({
                "model": "aura-2-thalia-en",
                "input": "priced Deepgram route",
                "voice": "ignored-by-deepgram",
                "response_format": "wav"
            }))
            .to_request();
        let response = test::call_service(&app, request).await;
        let status = response.status();
        let body = test::read_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "Deepgram TTS route response: {}",
            String::from_utf8_lossy(&body)
        );
        assert_eq!(body.as_ref(), b"deepgram-audio");

        let requests = mock.requests();
        assert_eq!(requests.len(), 3);
        assert!(requests[0].uri.starts_with("/v1/listen?"));
        assert!(requests[0].uri.contains("model=nova-3-general"));
        assert_eq!(
            requests[0].headers.get("authorization").map(String::as_str),
            Some("Token deepgram-secret")
        );
        assert_eq!(
            requests[1].headers.get("xi-api-key").map(String::as_str),
            Some("elevenlabs-secret")
        );
        assert!(
            requests[1]
                .uri
                .starts_with("/v1/text-to-speech/route-voice-id")
        );
        assert!(requests[2].uri.contains("model=aura-2-thalia-en"));
        mock.shutdown().await;
    }
}
