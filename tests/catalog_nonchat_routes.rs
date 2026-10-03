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
    error_message: Arc<Mutex<String>>,
    image_url: bool,
    total_only: bool,
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
            .json(json!({"error":{"message":state.error_message.lock().unwrap().clone()}}));
    }
    let path = req
        .path()
        .replace("/api/paas/v4", "/v1")
        .replace("/compatible-mode/v1", "/v1")
        .replace("/api/v1", "/v1")
        .replace("/v3/openai", "/v1")
        .replace("/api/v3", "/v1");
    match path.as_str() {
        "/v1/embeddings" | "/embeddings" | "/engines/llama.cpp/v1/embeddings" => HttpResponse::Ok().json(json!({"object":"list","model":"test-model","data":[{"object":"embedding","index":0,"embedding":[0.1,0.2]}],"usage": if state.total_only || req.path().starts_with("/compatible-mode/v1") { json!({"total_tokens":2}) } else { json!({"prompt_tokens":2,"total_tokens":2}) }})),
        "/v1/images/generations" => HttpResponse::Ok().json(if state.image_url { json!({"created":1,"data":[{"url":"https://example.test/generated.png"}]}) } else { json!({"created":1,"data":[{"b64_json":"aW1hZ2U="}]}) }),
        "/v1/audio/speech" => {
            let request: Value = serde_json::from_slice(&body).unwrap();
            assert!(request.get("speed").is_none_or(|v| !v.is_null()));
            let mime = match request["response_format"].as_str() { Some("wav") => "audio/wav", Some("pcm" | "raw") => "audio/pcm", _ => "audio/mpeg" };
            HttpResponse::Ok().insert_header(("content-type", mime)).body("test-audio")
        },
        "/v1/audio/transcriptions" | "/v1/audio/translations" => HttpResponse::Ok().json(json!({"text":"transcribed", "duration":1.0})),
        "/v1/stt" => {
            let multipart = String::from_utf8_lossy(&body);
            if multipart.contains("bad-response") {
                HttpResponse::Ok().json(json!({"text":"missing duration"}))
            } else {
                HttpResponse::Ok().json(json!({"text":"transcribed", "language":"en", "duration":1.25, "words":[{"text":"transcribed","start":0.0,"end":1.25}]}))
            }
        }
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
        error_message: Arc::new(Mutex::new("upstream unavailable".into())),
        image_url: matches!(selector, "zhipu" | "zai"),
        total_only: matches!(selector, "aiml" | "aiml_api"),
        auth: !matches!(
            selector,
            "lm_studio"
                | "vllm"
                | "llamafile"
                | "docker_model_runner"
                | "xinference"
                | "infinity"
                | "oobabooga"
                | "lemonade"
        ),
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
        if seen.auth { "test-key" } else { "" },
        &format!(
            "http://{address}{}",
            match selector {
                "infinity" => "",
                "docker_model_runner" => "/engines/llama.cpp/v1",
                "nanogpt" => "/api/v1",
                "novita" => "/v3/openai",
                "zhipu" | "zai" => "/api/paas/v4",
                "dashscope" | "qwen" => "/compatible-mode/v1",
                "volcengine" => "/api/v3",
                _ => "/v1",
            }
        ),
        vec!["test-model".into()],
    ))
    .await
    .unwrap();
    let router = UnifiedRouter::default();
    let models = if selector == "groq" {
        vec!["whisper-large-v3", "canopylabs/orpheus-v1-english"]
    } else if selector == "xai" {
        vec!["grok-voice-transcribe-2.0"]
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
        "nscale",
        "ovhcloud",
    ] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let provider = selected(&router, ProviderCapability::Embeddings);
        let mut request = embedding_request();
        if !matches!(selector, "nebius" | "lm_studio" | "nscale" | "ovhcloud") {
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

#[tokio::test]
async fn heroku_embeddings_map_float_and_task_type_on_real_dispatch() {
    let (router, upstream, handle) = fixture("heroku", StatusCode::OK).await;
    let request = serde_json::from_value(json!({"model":"test-model","input":"hello","encoding_format":"float","task_type":"search_query"})).unwrap();
    let response = selected(&router, ProviderCapability::Embeddings)
        .create_embeddings(request, RequestContext::default())
        .await
        .unwrap();
    assert_eq!(response.usage.unwrap().total_tokens, 2);
    let body: Value = serde_json::from_slice(&upstream.seen.lock().unwrap()[0].1).unwrap();
    assert_eq!(body["encoding_format"], "raw");
    assert_eq!(body["input_type"], "search_query");
    assert!(body.get("task_type").is_none());
    handle.stop(false).await;
}

#[tokio::test]
async fn newly_verified_cloud_providers_preserve_errors_and_scope() {
    for selector in ["nscale", "ovhcloud", "heroku"] {
        for status in [StatusCode::BAD_REQUEST, StatusCode::TOO_MANY_REQUESTS] {
            let (router, upstream, handle) = fixture(selector, status).await;
            let error = selected(&router, ProviderCapability::Embeddings)
                .create_embeddings(embedding_request(), RequestContext::default())
                .await
                .unwrap_err();
            if status == StatusCode::BAD_REQUEST {
                assert!(matches!(
                    error,
                    ProviderError::InvalidRequest { .. }
                        | ProviderError::ApiError { status: 400, .. }
                ));
            } else {
                assert!(matches!(
                    error,
                    ProviderError::RateLimit {
                        retry_after: Some(7),
                        ..
                    }
                ));
            }
            assert_eq!(upstream.seen.lock().unwrap().len(), 1);
            for capability in [
                ProviderCapability::AudioTranscription,
                ProviderCapability::AudioTranslation,
                ProviderCapability::TextToSpeech,
                ProviderCapability::ImageEdit,
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
}

#[tokio::test]
async fn cloud_embeddings_require_prices_and_obey_gateway_budgets() {
    use actix_web::test;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::middleware::AuthMiddleware;
    let (router, upstream, handle) = fixture("nscale", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::Embeddings);
    let Provider::OpenAILike(provider) = provider else {
        panic!("catalog provider")
    };
    let mut config = litellm_rs::Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = vec![provider_fixtures::mock_provider_config(
        "prod-nscale",
        "nscale",
        "test-key",
        &provider.config().get_api_base(),
        vec!["test-model".into()],
    )];
    let state = litellm_rs::server::HttpServer::new(&config)
        .await
        .unwrap()
        .state()
        .clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let request = || {
        test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(json!({"model":"test-model","input":"hello"}))
            .to_request()
    };
    let response = test::call_service(&app, request()).await;
    assert!(!response.status().is_success());
    let error: Value = test::read_body_json(response).await;
    assert!(
        error.to_string().to_lowercase().contains("pricing"),
        "{error}"
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    let (_, mut price) = state
        .pricing
        .get_model_info_for_provider("openai", "text-embedding-3-small")
        .unwrap();
    price.litellm_provider = "nscale".into();
    price.input_cost_per_token = Some(0.1);
    state.pricing.add_custom_model("test-model".into(), price);
    state.budget_limits.providers.set_provider_limit(
        "prod-nscale",
        ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
    );
    assert_eq!(
        test::call_service(&app, request()).await.status(),
        StatusCode::PAYMENT_REQUIRED
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    state.budget_limits.providers.set_provider_limit(
        "prod-nscale",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let response = test::call_service(&app, request()).await;
    let status = response.status();
    let body: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    let spend = state
        .budget_limits
        .providers
        .get_provider_usage("prod-nscale")
        .unwrap()
        .current_spend;
    assert!((spend - 0.2).abs() < 1e-9, "{spend}");
    handle.stop(false).await;
}

#[tokio::test]
async fn self_hosted_embeddings_reach_their_native_base_paths() {
    for selector in [
        "vllm",
        "hosted_vllm",
        "llamafile",
        "docker_model_runner",
        "xinference",
        "infinity",
        "oobabooga",
        "lemonade",
    ] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let mut request = embedding_request();
        request.dimensions = None; // Default model dimension is portable across these servers.
        let response = selected(&router, ProviderCapability::Embeddings)
            .create_embeddings(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);
        assert_eq!(response.usage.unwrap().prompt_tokens, 2);
        let (path, body) = upstream.seen.lock().unwrap()[0].clone();
        assert_eq!(
            path,
            match selector {
                "infinity" => "/embeddings",
                "docker_model_runner" => "/engines/llama.cpp/v1/embeddings",
                _ => "/v1/embeddings",
            }
        );
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["input"], json!(["hello"]));
        assert!(body.get("dimensions").is_none());
        if selector == "infinity" {
            assert!(
                router
                    .select_deployment_lease_for_capability(
                        "public",
                        &ProviderCapability::ChatCompletion
                    )
                    .is_err()
            );
        }
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn self_hosted_images_and_audio_preserve_native_payloads() {
    for selector in ["xinference", "oobabooga", "lemonade"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let request: ImageGenerationRequest = serde_json::from_value(json!({"model":"test-model","prompt":"mountains","n":1,"size":"512x512","response_format":"b64_json"})).unwrap();
        let response = selected(&router, ProviderCapability::ImageGeneration)
            .create_images(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.data[0].b64_json.as_deref(), Some("aW1hZ2U="));
        let calls = upstream.seen.lock().unwrap().clone();
        assert_eq!(calls[0].0, "/v1/images/generations");
        let body: Value = serde_json::from_slice(&calls[0].1).unwrap();
        assert_eq!(body["n"], 1);
        assert_eq!(body["response_format"], "b64_json");
        handle.stop(false).await;
    }
    for selector in ["vllm", "hosted_vllm", "xinference", "lemonade"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let mut request: TranscriptionRequest =
            serde_json::from_value(json!({"model":"test-model","response_format":"json"})).unwrap();
        request.file = b"local-wave".to_vec();
        request.filename = "input.wav".into();
        let response = selected(&router, ProviderCapability::AudioTranscription)
            .audio_transcription(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.text, "transcribed");
        assert_eq!(response.duration, Some(1.0));
        if selector != "lemonade" {
            let mut request: TranslationRequest =
                serde_json::from_value(json!({"model":"test-model","response_format":"json"}))
                    .unwrap();
            request.file = b"local-wave".to_vec();
            request.filename = "input.wav".into();
            assert_eq!(
                selected(&router, ProviderCapability::AudioTranslation)
                    .audio_translation(request, RequestContext::default())
                    .await
                    .unwrap()
                    .text,
                "transcribed"
            );
        }
        for (path, body) in upstream.seen.lock().unwrap().clone() {
            assert!(path.starts_with("/v1/audio/"));
            let body = String::from_utf8_lossy(&body);
            assert!(body.contains("local-wave"));
            assert!(body.contains("test-model"));
            assert!(body.contains("name=\"file\"; filename=\"input.wav\""));
        }
        handle.stop(false).await;
    }
    for selector in ["xinference", "lemonade"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let request: SpeechRequest = serde_json::from_value(json!({"model":"test-model","input":"hello","voice":"test-voice","response_format":"wav"})).unwrap();
        let response = selected(&router, ProviderCapability::TextToSpeech)
            .text_to_speech(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.audio, b"test-audio");
        assert_eq!(response.content_type, "audio/wav");
        let calls = upstream.seen.lock().unwrap().clone();
        assert_eq!(calls[0].0, "/v1/audio/speech");
        assert_eq!(
            serde_json::from_slice::<Value>(&calls[0].1).unwrap()["response_format"],
            "wav"
        );
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn self_hosted_errors_and_unverified_modes_are_explicit() {
    for selector in [
        "vllm",
        "hosted_vllm",
        "llamafile",
        "docker_model_runner",
        "xinference",
        "infinity",
        "oobabooga",
        "lemonade",
        "lm_studio",
    ] {
        for status in [StatusCode::BAD_REQUEST, StatusCode::TOO_MANY_REQUESTS] {
            let (router, _, handle) = fixture(selector, status).await;
            let error = selected(&router, ProviderCapability::Embeddings)
                .create_embeddings(embedding_request(), RequestContext::default())
                .await
                .unwrap_err();
            if status == StatusCode::BAD_REQUEST {
                assert!(matches!(error, ProviderError::ApiError { status: 400, .. }));
            } else {
                assert!(matches!(
                    error,
                    ProviderError::RateLimit {
                        retry_after: Some(7),
                        ..
                    }
                ));
            }
            for capability in [
                ProviderCapability::ImageEdit,
                ProviderCapability::ImageVariation,
            ] {
                assert!(
                    router
                        .select_deployment_lease_for_capability("public", &capability)
                        .is_err()
                );
            }
            if !matches!(selector, "xinference" | "lemonade") {
                assert!(
                    router
                        .select_deployment_lease_for_capability(
                            "public",
                            &ProviderCapability::TextToSpeech
                        )
                        .is_err()
                );
            }
            if !matches!(selector, "vllm" | "hosted_vllm" | "xinference") {
                assert!(
                    router
                        .select_deployment_lease_for_capability(
                            "public",
                            &ProviderCapability::AudioTranslation
                        )
                        .is_err()
                );
            }
            if matches!(
                selector,
                "llamafile" | "docker_model_runner" | "infinity" | "oobabooga" | "lm_studio"
            ) {
                assert!(
                    router
                        .select_deployment_lease_for_capability(
                            "public",
                            &ProviderCapability::AudioTranscription
                        )
                        .is_err()
                );
            }
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn local_embeddings_require_prices_and_obey_gateway_budgets() {
    use actix_web::test;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::middleware::AuthMiddleware;
    for selector in ["vllm", "dashscope", "qwen"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let provider = selected(&router, ProviderCapability::Embeddings);
        let Provider::OpenAILike(provider) = provider else {
            panic!("catalog provider")
        };
        let mut config = litellm_rs::Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.providers = vec![provider_fixtures::mock_provider_config(
            selector,
            selector,
            if selector == "vllm" { "" } else { "test-key" },
            &provider.config().get_api_base(),
            vec!["test-model".into()],
        )];
        let state = litellm_rs::server::HttpServer::new(&config)
            .await
            .unwrap()
            .state()
            .clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .wrap(AuthMiddleware)
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let request = || {
            test::TestRequest::post()
                .uri("/v1/embeddings")
                .set_json(json!({"model":"test-model","input":"hello"}))
                .to_request()
        };
        let response = test::call_service(&app, request()).await;
        assert!(!response.status().is_success());
        let error: Value = test::read_body_json(response).await;
        assert!(
            error.to_string().to_lowercase().contains("pricing"),
            "{error}"
        );
        assert!(upstream.seen.lock().unwrap().is_empty());
        let (_, mut price) = state
            .pricing
            .get_model_info_for_provider("openai", "text-embedding-3-small")
            .unwrap();
        price.litellm_provider = selector.into();
        price.input_cost_per_token = Some(0.1);
        state.pricing.add_custom_model("test-model".into(), price);
        state.budget_limits.providers.set_provider_limit(
            selector,
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        assert_eq!(
            test::call_service(&app, request()).await.status(),
            StatusCode::PAYMENT_REQUIRED
        );
        assert!(upstream.seen.lock().unwrap().is_empty());
        state.budget_limits.providers.set_provider_limit(
            selector,
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        let response = test::call_service(&app, request()).await;
        let status = response.status();
        let body: Value = test::read_body_json(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        let spend = state
            .budget_limits
            .providers
            .get_provider_usage(selector)
            .unwrap()
            .current_spend;
        assert!((spend - 0.2).abs() < 1e-9, "{spend}");
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn chinese_embeddings_preserve_native_bases_and_float_usage() {
    for selector in ["dashscope", "qwen", "zhipu", "siliconflow"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let request =
            serde_json::from_value(json!({"model":"test-model","input":["你好"]})).unwrap();
        let response = selected(&router, ProviderCapability::Embeddings)
            .create_embeddings(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);
        let usage = response.usage.unwrap();
        assert_eq!(usage.total_tokens, 2);
        assert_eq!(usage.prompt_tokens, 2);
        assert_eq!(usage.completion_tokens, 0);
        let expected = match selector {
            "zhipu" => "/api/paas/v4/embeddings",
            "dashscope" | "qwen" => "/compatible-mode/v1/embeddings",
            _ => "/v1/embeddings",
        };
        assert_eq!(upstream.seen.lock().unwrap()[0].0, expected);
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn siliconflow_base64_embeddings_are_rejected_before_dispatch() {
    let (router, upstream, handle) = fixture("siliconflow", StatusCode::OK).await;
    let request = serde_json::from_value(
        json!({"model":"test-model","input":"hello","encoding_format":"base64"}),
    )
    .unwrap();
    let error = selected(&router, ProviderCapability::Embeddings)
        .create_embeddings(request, RequestContext::default())
        .await
        .unwrap_err();
    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn zhipu_and_zai_images_preserve_url_responses() {
    for selector in ["zhipu", "zai"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let request = serde_json::from_value(
            json!({"model":"test-model","prompt":"山水","size":"1280x1280","quality":"hd","user":"user-123"}),
        )
        .unwrap();
        let response = selected(&router, ProviderCapability::ImageGeneration)
            .create_images(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(
            response.data[0].url.as_deref(),
            Some("https://example.test/generated.png")
        );
        let (path, body) = upstream.seen.lock().unwrap()[0].clone();
        assert_eq!(path, "/api/paas/v4/images/generations");
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["size"], "1280x1280");
        assert_eq!(body["quality"], "hd");
        assert_eq!(body["user_id"], "user-123");
        assert!(body.get("user").is_none());
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn zhipu_speech_uses_wav_and_preserves_explicit_formats() {
    let (router, upstream, handle) = fixture("zhipu", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::TextToSpeech);
    for format in [None, Some("pcm")] {
        let request = serde_json::from_value(
            json!({"model":"glm-tts","input":"你好","voice":"tongtong","response_format":format}),
        )
        .unwrap();
        let response = provider
            .text_to_speech(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.audio, b"test-audio");
        let calls = upstream.seen.lock().unwrap();
        let (path, body) = calls.last().unwrap();
        assert_eq!(path, "/api/paas/v4/audio/speech");
        assert_eq!(
            serde_json::from_slice::<Value>(body).unwrap()["response_format"],
            format.unwrap_or("wav")
        );
    }
    handle.stop(false).await;
}

#[tokio::test]
async fn zhipu_speech_requires_prices_and_settles_unicode_characters() {
    use actix_web::test;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::middleware::AuthMiddleware;
    let (router, upstream, handle) = fixture("zhipu", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::TextToSpeech);
    let Provider::OpenAILike(provider) = provider else {
        panic!("catalog provider")
    };
    let mut config = litellm_rs::Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = vec![provider_fixtures::mock_provider_config(
        "zhipu",
        "zhipu",
        "test-key",
        &provider.config().get_api_base(),
        vec!["test-model".into()],
    )];
    let state = litellm_rs::server::HttpServer::new(&config)
        .await
        .unwrap()
        .state()
        .clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let request = || {
        test::TestRequest::post()
            .uri("/v1/audio/speech")
            .set_json(json!({"model":"test-model","input":"你好","voice":"tongtong"}))
            .to_request()
    };
    let response = test::call_service(&app, request()).await;
    assert!(!response.status().is_success());
    let error: Value = test::read_body_json(response).await;
    assert!(
        error.to_string().to_lowercase().contains("pricing"),
        "{error}"
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    let (_, mut price) = state
        .pricing
        .get_model_info_for_provider("openai", "tts-1")
        .unwrap();
    price.litellm_provider = "zhipu".into();
    price.input_cost_per_token = None;
    price.output_cost_per_token = None;
    price.input_cost_per_character = Some(0.1);
    price.output_cost_per_character = None;
    state.pricing.add_custom_model("test-model".into(), price);
    state.budget_limits.providers.set_provider_limit(
        "zhipu",
        ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
    );
    assert_eq!(
        test::call_service(&app, request()).await.status(),
        StatusCode::PAYMENT_REQUIRED
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    state.budget_limits.providers.set_provider_limit(
        "zhipu",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let response = test::call_service(&app, request()).await;
    let status = response.status();
    let body = test::read_body(response).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body.as_ref(), b"test-audio");
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    let spend = state
        .budget_limits
        .providers
        .get_provider_usage("zhipu")
        .unwrap()
        .current_spend;
    assert!((spend - 0.2).abs() < 1e-9, "{spend}");
    handle.stop(false).await;
}

#[tokio::test]
async fn chinese_nonchat_errors_and_unverified_operations_fail_closed() {
    for selector in ["dashscope", "qwen", "zhipu", "zai", "siliconflow"] {
        for status in [StatusCode::BAD_REQUEST, StatusCode::TOO_MANY_REQUESTS] {
            let (router, _, handle) = fixture(selector, status).await;
            let error = if selector == "zai" {
                selected(&router, ProviderCapability::ImageGeneration)
                    .create_images(
                        serde_json::from_value(json!({"model":"test-model","prompt":"test"}))
                            .unwrap(),
                        RequestContext::default(),
                    )
                    .await
                    .unwrap_err()
            } else {
                selected(&router, ProviderCapability::Embeddings)
                    .create_embeddings(embedding_request(), RequestContext::default())
                    .await
                    .unwrap_err()
            };
            if status == StatusCode::BAD_REQUEST {
                assert!(matches!(
                    error,
                    ProviderError::InvalidRequest { .. }
                        | ProviderError::ApiError { status: 400, .. }
                ));
            } else {
                assert!(matches!(
                    error,
                    ProviderError::RateLimit {
                        retry_after: Some(7),
                        ..
                    }
                ));
            }
            for capability in [
                ProviderCapability::AudioTranscription,
                ProviderCapability::AudioTranslation,
                ProviderCapability::ImageEdit,
            ] {
                assert!(
                    router
                        .select_deployment_lease_for_capability("public", &capability)
                        .is_err()
                );
            }
            if selector != "zhipu" {
                assert!(
                    router
                        .select_deployment_lease_for_capability(
                            "public",
                            &ProviderCapability::TextToSpeech
                        )
                        .is_err()
                );
            }
            if selector == "zai" {
                assert!(
                    router
                        .select_deployment_lease_for_capability(
                            "public",
                            &ProviderCapability::Embeddings
                        )
                        .is_err()
                );
            }
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn nscale_images_are_withheld_until_pixel_pricing_is_supported() {
    let (router, upstream, handle) = fixture("nscale", StatusCode::OK).await;
    assert!(
        router
            .select_deployment_lease_for_capability("public", &ProviderCapability::ImageGeneration)
            .is_err()
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}
#[tokio::test]
async fn aggregator_embeddings_follow_each_official_base() {
    for selector in ["novita", "nanogpt", "galadriel", "featherless"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let response = selected(&router, ProviderCapability::Embeddings)
            .create_embeddings(embedding_request(), RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);
        assert_eq!(response.usage.unwrap().total_tokens, 2);
        let expected = match selector {
            "novita" => "/v3/openai/embeddings",
            "nanogpt" => "/api/v1/embeddings",
            _ => "/v1/embeddings",
        };
        assert_eq!(upstream.seen.lock().unwrap()[0].0, expected);
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn nanogpt_images_use_root_v1_and_galadriel_preserves_standard_response() {
    for selector in ["nanogpt", "galadriel"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let response = selected(&router, ProviderCapability::ImageGeneration)
            .create_images(
                serde_json::from_value(
                    json!({"model":"test-model","prompt":"test","n":1,"size":"1024x1024"}),
                )
                .unwrap(),
                RequestContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(response.data[0].b64_json.as_deref(), Some("aW1hZ2U="));
        assert_eq!(upstream.seen.lock().unwrap()[0].0, "/v1/images/generations");
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn aggregator_audio_preserves_binary_multipart_and_duration() {
    for selector in ["nanogpt", "featherless"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        let request = serde_json::from_value(
            json!({"model":"test-model","input":"hello","voice":"default","response_format":"wav"}),
        )
        .unwrap();
        let response = selected(&router, ProviderCapability::TextToSpeech)
            .text_to_speech(request, RequestContext::default())
            .await
            .unwrap();
        assert_eq!(response.audio, b"test-audio");
        assert_eq!(response.content_type, "audio/wav");
        if selector == "nanogpt" {
            assert_eq!(upstream.seen.lock().unwrap()[0].0, "/api/v1/audio/speech");
            let mut request: TranscriptionRequest =
                serde_json::from_value(json!({"model":"Whisper-Large-V3"})).unwrap();
            request.file = b"test-wave-data".to_vec();
            request.filename = "test.wav".into();
            let response = selected(&router, ProviderCapability::AudioTranscription)
                .audio_transcription(request, RequestContext::default())
                .await
                .unwrap();
            assert_eq!(response.text, "transcribed");
            assert_eq!(response.duration, Some(1.0));
            assert_eq!(
                upstream.seen.lock().unwrap()[1].0,
                "/api/v1/audio/transcriptions"
            );
        } else {
            assert_eq!(upstream.seen.lock().unwrap()[0].0, "/v1/audio/speech");
        }
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn nanogpt_embeddings_require_prices_and_obey_gateway_budgets() {
    use actix_web::test;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::middleware::AuthMiddleware;
    let (router, upstream, handle) = fixture("nanogpt", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::Embeddings);
    let Provider::OpenAILike(provider) = provider else {
        panic!("catalog provider")
    };
    let mut config = litellm_rs::Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = vec![provider_fixtures::mock_provider_config(
        "nanogpt",
        "nanogpt",
        "test-key",
        &provider.config().get_api_base(),
        vec!["test-model".into()],
    )];
    let state = litellm_rs::server::HttpServer::new(&config)
        .await
        .unwrap()
        .state()
        .clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let request = || {
        test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(json!({"model":"test-model","input":"hello"}))
            .to_request()
    };
    let response = test::call_service(&app, request()).await;
    assert!(!response.status().is_success());
    let error: Value = test::read_body_json(response).await;
    assert!(
        error.to_string().to_lowercase().contains("pricing"),
        "{error}"
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    let (_, mut price) = state
        .pricing
        .get_model_info_for_provider("openai", "text-embedding-3-small")
        .unwrap();
    price.litellm_provider = "nanogpt".into();
    price.input_cost_per_token = Some(0.1);
    state.pricing.add_custom_model("test-model".into(), price);
    state.budget_limits.providers.set_provider_limit(
        "nanogpt",
        ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
    );
    assert_eq!(
        test::call_service(&app, request()).await.status(),
        StatusCode::PAYMENT_REQUIRED
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    state.budget_limits.providers.set_provider_limit(
        "nanogpt",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let response = test::call_service(&app, request()).await;
    let status = response.status();
    let body: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    let spend = state
        .budget_limits
        .providers
        .get_provider_usage("nanogpt")
        .unwrap()
        .current_spend;
    assert!((spend - 0.2).abs() < 1e-9, "{spend}");
    handle.stop(false).await;
}

#[tokio::test]
async fn aggregator_errors_and_unverified_operations_fail_closed() {
    for selector in ["novita", "nanogpt", "galadriel", "featherless"] {
        for status in [StatusCode::BAD_REQUEST, StatusCode::TOO_MANY_REQUESTS] {
            let (router, _, handle) = fixture(selector, status).await;
            let error = selected(&router, ProviderCapability::Embeddings)
                .create_embeddings(embedding_request(), RequestContext::default())
                .await
                .unwrap_err();
            if status == StatusCode::BAD_REQUEST {
                assert!(matches!(
                    error,
                    ProviderError::InvalidRequest { .. }
                        | ProviderError::ApiError { status: 400, .. }
                ));
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
                        &ProviderCapability::AudioTranslation
                    )
                    .is_err()
            );
            assert!(
                router
                    .select_deployment_lease_for_capability(
                        "public",
                        &ProviderCapability::ImageEdit
                    )
                    .is_err()
            );
            handle.stop(false).await;
        }
    }
    for selector in ["ai21", "cerebras"] {
        let (router, upstream, handle) = fixture(selector, StatusCode::OK).await;
        for capability in [
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
            ProviderCapability::AudioTranscription,
            ProviderCapability::TextToSpeech,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability("public", &capability)
                    .is_err()
            );
        }
        assert!(upstream.seen.lock().unwrap().is_empty());
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn wandb_project_header_reaches_the_inference_chat_endpoint() {
    let definition = litellm_rs::core::providers::registry::get_definition("wandb").unwrap();
    assert_eq!(definition.base_url, "https://api.inference.wandb.ai/v1");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(|| App::new().route("/v1/chat/completions", web::post().to(|req: HttpRequest| async move {
        assert_eq!(req.headers().get("authorization").unwrap(), "Bearer test-key");
        assert_eq!(req.headers().get("openai-project").unwrap(), "test-team/test-project");
        HttpResponse::Ok().json(json!({"id":"chat-1","object":"chat.completion","created":1,"model":"test-model","choices":[{"index":0,"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}))
    }))).workers(1).listen(listener).unwrap().run();
    let handle = server.handle();
    tokio::spawn(server);
    let mut config = provider_fixtures::mock_provider_config(
        "wandb",
        "wandb",
        "test-key",
        &format!("http://{address}/v1"),
        vec!["test-model".into()],
    );
    config.settings.insert(
        "custom_headers".into(),
        json!({"OpenAI-Project":"test-team/test-project"}),
    );
    let provider = create_provider(config).await.unwrap();
    let router = UnifiedRouter::default();
    router.add_deployment(Deployment::new(
        "wandb-test".into(),
        provider,
        "test-model".into(),
        "public".into(),
    ));
    assert!(
        router
            .select_deployment_lease_for_capability("public", &ProviderCapability::Embeddings)
            .is_err()
    );
    let response = selected(&router, ProviderCapability::ChatCompletion)
        .chat_completion(
            serde_json::from_value(
                json!({"model":"test-model","messages":[{"role":"user","content":"hello"}]}),
            )
            .unwrap(),
            RequestContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(response.usage.unwrap().total_tokens, 2);
    handle.stop(true).await;
}
#[tokio::test]
async fn baichuan_embeddings_preserve_wire_usage_and_errors() {
    for status in [
        StatusCode::OK,
        StatusCode::BAD_REQUEST,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let (router, upstream, handle) = fixture("baichuan", status).await;
        let request =
            serde_json::from_value(json!({"model":"Baichuan-Text-Embedding","input":["hello"]}))
                .unwrap();
        let result = selected(&router, ProviderCapability::Embeddings)
            .create_embeddings(request, RequestContext::default())
            .await;
        match status {
            StatusCode::OK => {
                let response = result.unwrap();
                assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);
                assert_eq!(response.usage.unwrap().total_tokens, 2);
                let calls = upstream.seen.lock().unwrap();
                assert_eq!(calls[0].0, "/v1/embeddings");
                let body: Value = serde_json::from_slice(&calls[0].1).unwrap();
                assert_eq!(body["model"], "Baichuan-Text-Embedding");
                assert_eq!(body["input"], json!(["hello"]));
            }
            StatusCode::BAD_REQUEST => assert!(matches!(
                result.unwrap_err(),
                ProviderError::InvalidRequest { .. } | ProviderError::ApiError { status: 400, .. }
            )),
            _ => assert!(matches!(
                result.unwrap_err(),
                ProviderError::RateLimit {
                    retry_after: Some(7),
                    ..
                }
            )),
        }
        for capability in [
            ProviderCapability::ImageGeneration,
            ProviderCapability::AudioTranscription,
            ProviderCapability::TextToSpeech,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability("public", &capability)
                    .is_err()
            );
        }
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn baichuan_embeddings_require_prices_and_obey_gateway_budgets() {
    use actix_web::test;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::middleware::AuthMiddleware;
    let (router, upstream, handle) = fixture("baichuan", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::Embeddings);
    let Provider::OpenAILike(provider) = provider else {
        panic!("catalog provider")
    };
    let mut config = litellm_rs::Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = vec![provider_fixtures::mock_provider_config(
        "baichuan",
        "baichuan",
        "test-key",
        &provider.config().get_api_base(),
        vec!["test-model".into()],
    )];
    let state = litellm_rs::server::HttpServer::new(&config)
        .await
        .unwrap()
        .state()
        .clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let request = || {
        test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(json!({"model":"test-model","input":"hello"}))
            .to_request()
    };
    let response = test::call_service(&app, request()).await;
    assert!(!response.status().is_success());
    let error: Value = test::read_body_json(response).await;
    assert!(
        error.to_string().to_lowercase().contains("pricing"),
        "{error}"
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    let (_, mut price) = state
        .pricing
        .get_model_info_for_provider("openai", "text-embedding-3-small")
        .unwrap();
    price.litellm_provider = "baichuan".into();
    price.input_cost_per_token = Some(0.1);
    state.pricing.add_custom_model("test-model".into(), price);
    state.budget_limits.providers.set_provider_limit(
        "baichuan",
        ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
    );
    assert_eq!(
        test::call_service(&app, request()).await.status(),
        StatusCode::PAYMENT_REQUIRED
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    state.budget_limits.providers.set_provider_limit(
        "baichuan",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let response = test::call_service(&app, request()).await;
    let status = response.status();
    let body: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    let spend = state
        .budget_limits
        .providers
        .get_provider_usage("baichuan")
        .unwrap()
        .current_spend;
    assert!((spend - 0.2).abs() < 1e-9, "{spend}");
    handle.stop(false).await;
}

#[tokio::test]
async fn baichuan_rejects_truncated_batches_before_dispatch() {
    let (router, upstream, handle) = fixture("baichuan", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::Embeddings);
    for input in [json!(vec!["text"; 17]), json!([])] {
        let request =
            serde_json::from_value(json!({"model":"Baichuan-Text-Embedding","input":input}))
                .unwrap();
        assert!(matches!(
            provider
                .create_embeddings(request, RequestContext::default())
                .await
                .unwrap_err(),
            ProviderError::InvalidRequest { .. }
        ));
    }
    assert!(upstream.seen.lock().unwrap().is_empty());
    let request =
        serde_json::from_value(json!({"model":"Baichuan-Text-Embedding","input":vec!["text"; 16]}))
            .unwrap();
    provider
        .create_embeddings(request, RequestContext::default())
        .await
        .unwrap();
    let calls = upstream.seen.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    let body: Value = serde_json::from_slice(&calls[0].1).unwrap();
    assert_eq!(body["input"].as_array().unwrap().len(), 16);
    handle.stop(false).await;
}

#[tokio::test]
async fn baichuan_balance_429_is_quota_and_rate_429_preserves_retry_after() {
    use litellm_rs::utils::error::CanonicalError;
    let (router, upstream, handle) = fixture("baichuan", StatusCode::TOO_MANY_REQUESTS).await;
    let provider = selected(&router, ProviderCapability::Embeddings);
    for (message, quota) in [
        ("Insufficient account balance, please recharge", true),
        ("Rate limit", false),
    ] {
        *upstream.error_message.lock().unwrap() = message.into();
        let request =
            serde_json::from_value(json!({"model":"Baichuan-Text-Embedding","input":"text"}))
                .unwrap();
        let error = provider
            .create_embeddings(request, RequestContext::default())
            .await
            .unwrap_err();
        assert_eq!(error.canonical_retryable(), !quota);
        if quota {
            assert!(
                matches!(error, ProviderError::QuotaExceeded { .. }),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(
                    error,
                    ProviderError::RateLimit {
                        retry_after: Some(7),
                        ..
                    }
                ),
                "{error:?}"
            );
        }
    }
    assert_eq!(upstream.seen.lock().unwrap().len(), 2);
    handle.stop(false).await;
}
#[tokio::test]
async fn volcengine_embeddings_preserve_wire_usage_and_errors() {
    for status in [
        StatusCode::OK,
        StatusCode::BAD_REQUEST,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let (router, upstream, handle) = fixture("volcengine", status).await;
        let request = serde_json::from_value(
            json!({"model":"doubao-embedding-text-240515","input":["hello"]}),
        )
        .unwrap();
        let result = selected(&router, ProviderCapability::Embeddings)
            .create_embeddings(request, RequestContext::default())
            .await;
        match status {
            StatusCode::OK => {
                let response = result.unwrap();
                assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);
                assert_eq!(response.usage.unwrap().total_tokens, 2);
                let calls = upstream.seen.lock().unwrap();
                assert_eq!(calls[0].0, "/api/v3/embeddings");
                let body: Value = serde_json::from_slice(&calls[0].1).unwrap();
                assert_eq!(body["model"], "doubao-embedding-text-240515");
                assert_eq!(body["input"], json!(["hello"]));
            }
            StatusCode::BAD_REQUEST => assert!(matches!(
                result.unwrap_err(),
                ProviderError::InvalidRequest { .. } | ProviderError::ApiError { status: 400, .. }
            )),
            _ => assert!(matches!(
                result.unwrap_err(),
                ProviderError::RateLimit {
                    retry_after: Some(7),
                    ..
                }
            )),
        }
        for capability in [
            ProviderCapability::ImageGeneration,
            ProviderCapability::AudioTranscription,
            ProviderCapability::TextToSpeech,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability("public", &capability)
                    .is_err()
            );
        }
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn volcengine_embeddings_require_prices_and_obey_gateway_budgets() {
    use actix_web::test;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::middleware::AuthMiddleware;
    let (router, upstream, handle) = fixture("volcengine", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::Embeddings);
    let Provider::OpenAILike(provider) = provider else {
        panic!("catalog provider")
    };
    let mut config = litellm_rs::Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = vec![provider_fixtures::mock_provider_config(
        "volcengine",
        "volcengine",
        "test-key",
        &provider.config().get_api_base(),
        vec!["doubao-embedding-text-240715".into()],
    )];
    let state = litellm_rs::server::HttpServer::new(&config)
        .await
        .unwrap()
        .state()
        .clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let request = || {
        test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(json!({"model":"doubao-embedding-text-240715","input":"hello"}))
            .to_request()
    };
    let response = test::call_service(&app, request()).await;
    assert!(!response.status().is_success());
    let error: Value = test::read_body_json(response).await;
    assert!(
        error.to_string().to_lowercase().contains("pricing"),
        "{error}"
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    let (_, mut price) = state
        .pricing
        .get_model_info_for_provider("openai", "text-embedding-3-small")
        .unwrap();
    price.litellm_provider = "volcengine".into();
    price.input_cost_per_token = Some(0.1);
    state
        .pricing
        .add_custom_model("doubao-embedding-text-240715".into(), price);
    state.budget_limits.providers.set_provider_limit(
        "volcengine",
        ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
    );
    assert_eq!(
        test::call_service(&app, request()).await.status(),
        StatusCode::PAYMENT_REQUIRED
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    state.budget_limits.providers.set_provider_limit(
        "volcengine",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let response = test::call_service(&app, request()).await;
    let status = response.status();
    let body: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    let spend = state
        .budget_limits
        .providers
        .get_provider_usage("volcengine")
        .unwrap()
        .current_spend;
    assert!((spend - 0.2).abs() < 1e-9, "{spend}");
    handle.stop(false).await;
}

#[tokio::test]
async fn volcengine_embedding_string_is_sent_as_one_item_array() {
    let (router, upstream, handle) = fixture("volcengine", StatusCode::OK).await;
    let request =
        serde_json::from_value(json!({"model":"doubao-embedding-text-240515","input":"hello"}))
            .unwrap();
    selected(&router, ProviderCapability::Embeddings)
        .create_embeddings(request, RequestContext::default())
        .await
        .unwrap();
    let calls = upstream.seen.lock().unwrap().clone();
    let body: Value = serde_json::from_slice(&calls[0].1).unwrap();
    assert_eq!(body["input"], json!(["hello"]));
    handle.stop(false).await;
}
#[tokio::test]
async fn compactifai_whisper_routes_only_to_transcription_and_preserves_errors() {
    for status in [StatusCode::OK, StatusCode::TOO_MANY_REQUESTS] {
        let (router, upstream, handle) = fixture("compactifai", status).await;
        let provider = selected(&router, ProviderCapability::ChatCompletion);
        let router = UnifiedRouter::default();
        let model = "cai-whisper-large-v3-turbo-slim";
        router.add_deployment(Deployment::new(
            model.into(),
            provider,
            model.into(),
            "public".into(),
        ));
        for capability in [
            ProviderCapability::ChatCompletion,
            ProviderCapability::AudioTranslation,
            ProviderCapability::TextToSpeech,
            ProviderCapability::Embeddings,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability("public", &capability)
                    .is_err()
            );
        }
        let request = TranscriptionRequest {
            model: model.into(),
            file: vec![1, 2],
            filename: "sample.wav".into(),
            language: None,
            prompt: None,
            response_format: None,
            temperature: None,
            timestamp_granularities: None,
        };
        let result = selected(&router, ProviderCapability::AudioTranscription)
            .audio_transcription(request, RequestContext::default())
            .await;
        if status == StatusCode::OK {
            assert_eq!(result.unwrap().duration, Some(1.0));
        } else {
            assert!(matches!(
                result.unwrap_err(),
                ProviderError::RateLimit {
                    retry_after: Some(7),
                    ..
                }
            ));
        }
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        handle.stop(false).await;
    }
}
#[tokio::test]
async fn multimodal_aggregator_embeddings_normalize_authoritative_usage() {
    for selector in ["aiml", "aiml_api", "comet_api"] {
        for status in [
            StatusCode::OK,
            StatusCode::BAD_REQUEST,
            StatusCode::TOO_MANY_REQUESTS,
        ] {
            let (router, upstream, handle) = fixture(selector, status).await;
            let request = serde_json::from_value(
                json!({"model":"test-model","input":["hello"],"task_type":"document"}),
            )
            .unwrap();
            let result = selected(&router, ProviderCapability::Embeddings)
                .create_embeddings(request, RequestContext::default())
                .await;
            match status {
                StatusCode::OK => {
                    let response = result.unwrap();
                    assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);
                    let usage = response.usage.unwrap();
                    assert_eq!(usage.prompt_tokens, 2);
                    assert_eq!(usage.total_tokens, 2);
                    assert_eq!(usage.completion_tokens, 0);
                    let calls = upstream.seen.lock().unwrap();
                    assert_eq!(calls[0].0, "/v1/embeddings");
                    let body: Value = serde_json::from_slice(&calls[0].1).unwrap();
                    if selector != "comet_api" {
                        assert_eq!(body["input_type"], "document");
                        assert!(body.get("task_type").is_none());
                    }
                }
                StatusCode::BAD_REQUEST => assert!(matches!(
                    result.unwrap_err(),
                    ProviderError::InvalidRequest { .. }
                        | ProviderError::ApiError { status: 400, .. }
                )),
                _ => assert!(matches!(
                    result.unwrap_err(),
                    ProviderError::RateLimit {
                        retry_after: Some(7),
                        ..
                    }
                )),
            }
            for capability in [
                ProviderCapability::ImageGeneration,
                ProviderCapability::AudioTranscription,
                ProviderCapability::TextToSpeech,
            ] {
                assert!(
                    router
                        .select_deployment_lease_for_capability("public", &capability)
                        .is_err()
                );
            }
            assert_eq!(upstream.seen.lock().unwrap().len(), 1);
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn aiml_embeddings_reject_missing_or_invalid_authoritative_usage() {
    for selector in ["aiml", "aiml_api"] {
        for usage in [
            None,
            Some(Value::Null),
            Some(json!({})),
            Some(json!({"total_tokens":"2"})),
            Some(json!({"total_tokens":-1})),
        ] {
            let mut response = json!({
                "object":"list", "model":"test-model",
                "data":[{"object":"embedding","index":0,"embedding":[0.1,0.2]}]
            });
            if let Some(usage) = usage {
                response["usage"] = usage;
            }
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = HttpServer::new(move || {
                App::new().app_data(web::Data::new(response.clone())).route(
                    "/v1/embeddings",
                    web::post().to(|body: web::Data<Value>| async move {
                        HttpResponse::Ok().json(body.get_ref())
                    }),
                )
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
                "test-key",
                &format!("http://{address}/v1"),
                vec!["test-model".into()],
            ))
            .await
            .unwrap();
            let router = UnifiedRouter::default();
            router.add_deployment(Deployment::new(
                "test-model".into(),
                provider,
                "test-model".into(),
                "public".into(),
            ));
            let error = selected(&router, ProviderCapability::Embeddings)
                .create_embeddings(embedding_request(), RequestContext::default())
                .await
                .unwrap_err();
            assert!(matches!(error, ProviderError::ResponseParsing { .. }));
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn aiml_embeddings_require_prices_and_obey_gateway_budgets() {
    use actix_web::test;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::server::middleware::AuthMiddleware;
    let (router, upstream, handle) = fixture("aiml", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::Embeddings);
    let Provider::OpenAILike(provider) = provider else {
        panic!("catalog provider")
    };
    let mut config = litellm_rs::Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.providers = vec![provider_fixtures::mock_provider_config(
        "aiml",
        "aiml",
        "test-key",
        &provider.config().get_api_base(),
        vec!["test-model".into()],
    )];
    let state = litellm_rs::server::HttpServer::new(&config)
        .await
        .unwrap()
        .state()
        .clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let request = || {
        test::TestRequest::post()
            .uri("/v1/embeddings")
            .set_json(json!({"model":"test-model","input":"hello"}))
            .to_request()
    };
    let response = test::call_service(&app, request()).await;
    assert!(!response.status().is_success());
    let error: Value = test::read_body_json(response).await;
    assert!(
        error.to_string().to_lowercase().contains("pricing"),
        "{error}"
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    let (_, mut price) = state
        .pricing
        .get_model_info_for_provider("openai", "text-embedding-3-small")
        .unwrap();
    price.litellm_provider = "aiml".into();
    price.input_cost_per_token = Some(0.1);
    state.pricing.add_custom_model("test-model".into(), price);
    state
        .budget_limits
        .providers
        .set_provider_limit("aiml", ProviderLimitConfig::new(0.01, ResetPeriod::Monthly));
    assert_eq!(
        test::call_service(&app, request()).await.status(),
        StatusCode::PAYMENT_REQUIRED
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    state.budget_limits.providers.set_provider_limit(
        "aiml",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let response = test::call_service(&app, request()).await;
    let status = response.status();
    let body: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    let spend = state
        .budget_limits
        .providers
        .get_provider_usage("aiml")
        .unwrap()
        .current_spend;
    assert!((spend - 0.2).abs() < 1e-9, "{spend}");
    handle.stop(false).await;
}
#[tokio::test]
async fn xai_native_transcription_maps_upload_words_and_upstream_errors() {
    for status in [
        StatusCode::OK,
        StatusCode::UNAUTHORIZED,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::BAD_GATEWAY,
    ] {
        let (router, upstream, handle) = fixture("xai", status).await;
        let provider = selected(&router, ProviderCapability::AudioTranscription);
        for capability in [
            ProviderCapability::ChatCompletion,
            ProviderCapability::AudioTranslation,
            ProviderCapability::TextToSpeech,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability("public", &capability)
                    .is_err()
            );
        }
        assert!(!provider.supports_capability_for_model(
            "grok-voice-transcribe-1.0",
            &ProviderCapability::AudioTranscription
        ));
        let request = TranscriptionRequest {
            model: "xai/grok-voice-transcribe-2.0".into(),
            file: vec![1, 2],
            filename: "sample.wav".into(),
            language: Some("en".into()),
            prompt: None,
            response_format: Some("verbose_json".into()),
            temperature: None,
            timestamp_granularities: Some(vec!["word".into()]),
        };
        let result = provider
            .audio_transcription(request, RequestContext::default())
            .await;
        if status == StatusCode::OK {
            let response = result.unwrap();
            assert_eq!(response.duration, Some(1.25));
            assert_eq!(response.words.unwrap()[0].word, "transcribed");
        } else if status == StatusCode::TOO_MANY_REQUESTS {
            assert!(matches!(
                result.unwrap_err(),
                ProviderError::RateLimit {
                    retry_after: Some(7),
                    ..
                }
            ));
        } else {
            assert!(result.is_err());
        }
        let calls = upstream.seen.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "/v1/stt");
        let body = String::from_utf8_lossy(&calls[0].1);
        assert!(body.contains("grok-voice-transcribe-2.0"));
        assert!(!body.contains("xai/grok"));
        assert!(body.find("name=\"language\"").unwrap() < body.find("name=\"file\"").unwrap());
        assert!(!body.contains("response_format"));
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn xai_transcription_rejects_unsupported_options_and_malformed_success() {
    let (router, upstream, handle) = fixture("xai", StatusCode::OK).await;
    let provider = selected(&router, ProviderCapability::AudioTranscription);
    let mut request = TranscriptionRequest {
        model: "grok-voice-transcribe-2.0".into(),
        file: vec![1, 2],
        filename: "sample.wav".into(),
        language: None,
        prompt: Some("unsupported".into()),
        response_format: None,
        temperature: None,
        timestamp_granularities: None,
    };
    assert!(
        provider
            .audio_transcription(request.clone(), RequestContext::default())
            .await
            .is_err()
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    request.prompt = None;
    request.language = Some("bad-response".into());
    assert!(
        provider
            .audio_transcription(request, RequestContext::default())
            .await
            .is_err()
    );
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    handle.stop(false).await;
}
