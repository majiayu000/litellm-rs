#![cfg(all(feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, web};
use litellm_rs::core::{
    audio::types::{SpeechRequest, TranscriptionRequest, TranslationRequest},
    providers::{Provider, ProviderError, create_provider},
    router::{Deployment, UnifiedRouter},
    types::{
        context::RequestContext, embedding::EmbeddingRequest, image::ImageGenerationRequest,
        model::ProviderCapability,
    },
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

type RecordedCalls = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Clone)]
struct Upstream {
    seen: RecordedCalls,
    status: StatusCode,
    auth: bool,
}
async fn respond(req: HttpRequest, body: web::Bytes, state: web::Data<Upstream>) -> HttpResponse {
    assert_eq!(
        req.headers()
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        state.auth.then_some("Bearer test-key")
    );
    state
        .seen
        .lock()
        .unwrap()
        .push((req.path().into(), body.to_vec()));
    if state.status != StatusCode::OK {
        return HttpResponse::build(state.status)
            .insert_header(("retry-after", "7"))
            .json(json!({"error":{"message":"upstream unavailable"}}));
    }
    match req.path() {
        "/v1/embeddings" => HttpResponse::Ok().json(json!({"object":"list","model":"test-model","data":[{"object":"embedding","index":0,"embedding":[0.1,0.2]}],"usage":{"prompt_tokens":2,"total_tokens":2}})),
        "/v1/images/generations" => HttpResponse::Ok().json(json!({"created":1,"data":[{"b64_json":"aW1hZ2U="}]})),
        "/v1/audio/speech" => {
            let request: Value = serde_json::from_slice(&body).unwrap();
            assert!(request.get("speed").is_none_or(|v| !v.is_null()));
            let mime = if request["response_format"] == "wav" { "audio/wav" } else { "audio/mpeg" };
            HttpResponse::Ok().insert_header(("content-type", mime)).body("test-audio")
        },
        "/v1/audio/transcriptions" | "/v1/audio/translations" => HttpResponse::Ok().json(json!({"text":"transcribed", "duration":1.0})),
        _ => HttpResponse::NotFound().finish(),
    }
}
async fn fixture(
    selector: &str,
    status: StatusCode,
) -> (UnifiedRouter, Upstream, actix_web::dev::ServerHandle) {
    let seen = Upstream {
        seen: Arc::default(),
        status,
        auth: selector != "lm_studio",
    };
    let server_data = seen.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(server_data.clone()))
            .default_service(web::post().to(respond))
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let provider = create_provider(provider_fixtures::mock_provider_config(
        selector,
        selector,
        if selector == "lm_studio" {
            ""
        } else {
            "test-key"
        },
        &format!("http://{address}/v1"),
        vec!["test-model".into()],
    ))
    .await
    .unwrap();
    let router = UnifiedRouter::default();
    let models = if selector == "groq" {
        vec!["whisper-large-v3", "canopylabs/orpheus-v1-english"]
    } else {
        vec!["test-model"]
    };
    for model in models {
        router.add_deployment(Deployment::new(
            model.into(),
            provider.clone(),
            model.into(),
            "public".into(),
        ));
    }
    (router, seen, handle)
}
fn selected(router: &UnifiedRouter, capability: ProviderCapability) -> Provider {
    let lease = router
        .select_deployment_lease_for_capability("public", &capability)
        .unwrap();
    lease.deployment().provider.clone()
}
fn embedding_request() -> EmbeddingRequest {
    serde_json::from_value(json!({"model":"test-model","input":["hello"],"dimensions":2})).unwrap()
}

#[tokio::test]
async fn named_catalog_embeddings_reach_the_verified_endpoint() {
    for selector in [
        "together",
        "together_ai",
        "fireworks",
        "fireworks_ai",
        "deepinfra",
        "openrouter",
        "nebius",
        "nvidia_nim",
        "lm_studio",
    ] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let provider = selected(&router, ProviderCapability::Embeddings);
        let mut request = embedding_request();
        if !matches!(selector, "nebius" | "lm_studio") {
            request.task_type = Some("query".into());
        }
        if selector == "nvidia_nim" {
            request.truncation = Some(true);
            request.dimensions = None;
        }
        let response = provider
            .create_embeddings(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);
        assert_eq!(response.usage.as_ref().unwrap().prompt_tokens, 2);
        assert_eq!(response.usage.as_ref().unwrap().completion_tokens, 0);
        let (path, body) = upstream.seen.lock().unwrap()[0].clone();
        assert_eq!(path, "/v1/embeddings");
        let body: Value = serde_json::from_slice(&body).unwrap();
        if selector == "nvidia_nim" {
            assert!(body.get("dimensions").is_none());
        } else {
            assert_eq!(body["dimensions"], 2);
        }
        if matches!(
            selector,
            "fireworks" | "fireworks_ai" | "openrouter" | "nvidia_nim"
        ) {
            assert_eq!(body["input_type"], "query");
            assert!(body.get("task_type").is_none());
        }
        if selector == "nvidia_nim" {
            assert_eq!(body["truncate"], "END");
            assert!(body.get("truncation").is_none());
        }
        assert_eq!(body["model"], "test-model");
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn named_catalog_images_reach_the_verified_endpoint() {
    for selector in ["together", "together_ai", "deepinfra"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let provider = selected(&router, ProviderCapability::ImageGeneration);
        let request: ImageGenerationRequest = serde_json::from_value(
            json!({"model":"test-model","prompt":"mountains", "size":"1024x1024", "n":1}),
        )
        .unwrap();
        let response = provider
            .create_images(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.data[0].b64_json.as_deref(), Some("aW1hZ2U="));
        let (path, body) = upstream.seen.lock().unwrap()[0].clone();
        assert_eq!(path, "/v1/images/generations");
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["prompt"],
            "mountains"
        );
        assert!(
            router
                .select_deployment_lease_for_capability("public", &ProviderCapability::ImageEdit)
                .is_err()
        );
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn named_audio_preserves_binary_and_multipart_protocols() {
    for selector in ["together", "together_ai", "groq"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let speech_model = if selector == "groq" {
            "canopylabs/orpheus-v1-english"
        } else {
            "test-model"
        };
        let whisper_model = if selector == "groq" {
            "whisper-large-v3"
        } else {
            "test-model"
        };
        let provider = selected(&router, ProviderCapability::TextToSpeech);
        let speech: SpeechRequest = serde_json::from_value(
            json!({"model":speech_model,"input":"hello", "voice":"test-voice"}),
        )
        .unwrap();
        assert_eq!(
            provider
                .text_to_speech(speech, RequestContext::default())
                .await
                .unwrap()
                .audio,
            b"test-audio"
        );
        let mut transcription: TranscriptionRequest =
            serde_json::from_value(json!({"model":whisper_model, "language":"en"})).unwrap();
        transcription.file = b"test-wave-data".to_vec();
        transcription.filename = "test.wav".into();
        let provider = selected(&router, ProviderCapability::AudioTranscription);
        assert_eq!(
            provider
                .audio_transcription(transcription, RequestContext::default())
                .await
                .unwrap()
                .text,
            "transcribed"
        );
        let mut translation: TranslationRequest =
            serde_json::from_value(json!({"model":whisper_model})).unwrap();
        translation.file = b"test-wave-data".to_vec();
        translation.filename = "test.wav".into();
        let provider = selected(&router, ProviderCapability::AudioTranslation);
        assert_eq!(
            provider
                .audio_translation(translation, RequestContext::default())
                .await
                .unwrap()
                .text,
            "transcribed"
        );
        let calls = upstream.seen.lock().unwrap().clone();
        assert_eq!(calls.len(), 3);
        assert_eq!(
            serde_json::from_slice::<Value>(&calls[0].1).unwrap()["response_format"],
            if selector == "groq" { "wav" } else { "mp3" }
        );
        for (path, body) in &calls[1..] {
            assert!(path.starts_with("/v1/audio/"));
            let body = String::from_utf8_lossy(body);
            assert!(
                body.contains("name=\"file\"; filename=\"test.wav\""),
                "{body}"
            );
            assert!(body.contains("test-wave-data"));
            assert!(
                body.to_ascii_lowercase()
                    .contains("content-type: audio/wav")
            );
            assert!(body.contains(whisper_model));
            if selector == "groq" {
                assert!(body.contains("verbose_json"));
            }
            assert!(body.find("name=\"model\"").unwrap() < body.find("name=\"file\"").unwrap());
        }
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn named_catalog_errors_and_unverified_capabilities_fail_closed() {
    for status in [StatusCode::FORBIDDEN, StatusCode::TOO_MANY_REQUESTS] {
        let (router, _, handle) = fixture("fireworks_ai", status).await;
        let error = selected(&router, ProviderCapability::Embeddings)
            .create_embeddings(embedding_request(), RequestContext::default())
            .await
            .unwrap_err();
        if status == StatusCode::FORBIDDEN {
            assert!(matches!(error, ProviderError::ApiError { status: 403, .. }));
        } else {
            assert!(matches!(
                error,
                ProviderError::RateLimit {
                    retry_after: Some(7),
                    ..
                }
            ));
        }
        assert!(
            router
                .select_deployment_lease_for_capability(
                    "public",
                    &ProviderCapability::ImageGeneration
                )
                .is_err()
        );
        handle.stop(false).await;
    }
    let (router, upstream, handle) = fixture("deepseek", StatusCode::OK).await;
    assert!(
        router
            .select_deployment_lease_for_capability("public", &ProviderCapability::Embeddings)
            .is_err()
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn together_audio_preserves_retry_after() {
    let (router, _, handle) = fixture("together", StatusCode::TOO_MANY_REQUESTS).await;
    let provider = selected(&router, ProviderCapability::TextToSpeech);
    let speech =
        serde_json::from_value(json!({"model":"test-model","input":"hello","voice":"test"}))
            .unwrap();
    let transcription = serde_json::from_value(json!({"model":"test-model"})).unwrap();
    let translation = serde_json::from_value(json!({"model":"test-model"})).unwrap();
    for error in [
        provider
            .text_to_speech(speech, RequestContext::default())
            .await
            .map(|_| ())
            .unwrap_err(),
        provider
            .audio_transcription(transcription, RequestContext::default())
            .await
            .map(|_| ())
            .unwrap_err(),
        provider
            .audio_translation(translation, RequestContext::default())
            .await
            .map(|_| ())
            .unwrap_err(),
    ] {
        assert!(matches!(
            error,
            ProviderError::RateLimit {
                retry_after: Some(7),
                ..
            }
        ));
    }
    handle.stop(false).await;
}

#[tokio::test]
async fn together_pcm_uses_raw_wire_format_and_pcm_response_type() {
    let (router, upstream, handle) = fixture("together_ai", StatusCode::OK).await;
    let speech = serde_json::from_value(
        json!({"model":"test-model","input":"hello","voice":"test","response_format":"pcm"}),
    )
    .unwrap();
    let response = selected(&router, ProviderCapability::TextToSpeech)
        .text_to_speech(speech, RequestContext::default())
        .await
        .unwrap();
    assert_eq!(response.content_type, "audio/pcm");
    let calls = upstream.seen.lock().unwrap().clone();
    assert_eq!(
        serde_json::from_slice::<Value>(&calls[0].1).unwrap()["response_format"],
        "raw"
    );
    handle.stop(false).await;
}

#[tokio::test]
async fn groq_audio_preserves_errors_and_does_not_advertise_images_or_embeddings() {
    for status in [StatusCode::BAD_REQUEST, StatusCode::TOO_MANY_REQUESTS] {
        let (router, _, handle) = fixture("groq", status).await;
        let speech: SpeechRequest = serde_json::from_value(
            json!({"model":"canopylabs/orpheus-v1-english","input":"hello","voice":"troy","response_format":"mp3"}),
        )
        .unwrap();
        let error = selected(&router, ProviderCapability::TextToSpeech)
            .text_to_speech(speech, RequestContext::default())
            .await
            .err()
            .unwrap();
        if status == StatusCode::TOO_MANY_REQUESTS {
            assert!(matches!(
                error,
                ProviderError::RateLimit {
                    retry_after: Some(7),
                    ..
                }
            ));
        } else {
            assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        }
        for capability in [
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability("public", &capability)
                    .is_err()
            );
        }
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn groq_routing_checks_each_concrete_audio_model() {
    let (router, upstream, handle) = fixture("groq", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::TextToSpeech);
    for (model, transcribe, translate, speak) in [
        ("llama-3.3-70b-versatile", false, false, false),
        ("whisper-large-v3", true, true, false),
        ("whisper-large-v3-turbo", true, false, false),
        ("canopylabs/orpheus-v1-english", false, false, true),
        ("canopylabs/orpheus-arabic-saudi", false, false, true),
    ] {
        let router = UnifiedRouter::default();
        router.add_deployment(Deployment::new(
            model.into(),
            provider.clone(),
            model.into(),
            "public".into(),
        ));
        for (capability, expected) in [
            (ProviderCapability::AudioTranscription, transcribe),
            (ProviderCapability::AudioTranslation, translate),
            (ProviderCapability::TextToSpeech, speak),
        ] {
            assert_eq!(
                router
                    .select_deployment_lease_for_capability("public", &capability)
                    .is_ok(),
                expected,
                "{model} {capability:?}"
            );
        }
        if transcribe || speak {
            assert!(
                !provider.supports_capability_for_model(model, &ProviderCapability::ChatCompletion)
            );
        }
    }
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn verified_embedding_profiles_preserve_errors_and_reject_unverified_operations() {
    for selector in ["openrouter", "nebius", "nvidia_nim", "lm_studio"] {
        let (router, _, handle) = fixture(selector, StatusCode::TOO_MANY_REQUESTS).await;
        let error = selected(&router, ProviderCapability::Embeddings)
            .create_embeddings(embedding_request(), RequestContext::default())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            ProviderError::RateLimit {
                retry_after: Some(7),
                ..
            }
        ));
        for capability in [
            ProviderCapability::ImageGeneration,
            ProviderCapability::TextToSpeech,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability("public", &capability)
                    .is_err()
            );
        }
        handle.stop(false).await;
    }
}
