use super::*;
use std::process::Command;

async fn native_provider_with_headers(provider_type: ProviderType, base_url: &str) -> Provider {
    Provider::from_config_async(
        provider_type,
        json!({
            "api_key": "native-audio-secret",
            "base_url": base_url,
            "endpoint_access": "private_network",
            "headers": {
                "x-proxy-route": "native-audio",
                "Authorization": "attacker-controlled",
                "Xi-Api-Key": "attacker-controlled"
            }
        }),
    )
    .await
    .expect("native audio provider with headers should be created")
}

#[tokio::test]
async fn deepgram_accepts_aac_and_flac_speech_formats() {
    let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
    let provider = native_provider(ProviderType::Deepgram, &mock.base_url, 5).await;

    for format in ["aac", "flac"] {
        provider
            .text_to_speech(
                SpeechRequest {
                    input: format!("Deepgram {format}"),
                    model: "aura-2-thalia-en".to_string(),
                    voice: "ignored-by-deepgram".to_string(),
                    response_format: Some(format.to_string()),
                    speed: None,
                },
                RequestContext::new(),
            )
            .await
            .unwrap_or_else(|error| panic!("Deepgram should accept {format}: {error}"));
    }

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].uri.contains("encoding=aac"));
    assert!(requests[1].uri.contains("encoding=flac"));
    mock.shutdown().await;
}

#[tokio::test]
async fn native_audio_rejects_nonzero_environment_retries() {
    for (provider, key) in [
        ("deepgram", "DEEPGRAM_MAX_RETRIES"),
        ("elevenlabs", "ELEVENLABS_MAX_RETRIES"),
    ] {
        let status = Command::new(std::env::current_exe().expect("test executable should exist"))
            .args([
                "--exact",
                "tests::followup_tests::native_audio_env_retry_child",
                "--nocapture",
            ])
            .env("NATIVE_AUDIO_RETRY_CHILD_PROVIDER", provider)
            .env_remove("DEEPGRAM_MAX_RETRIES")
            .env_remove("ELEVENLABS_MAX_RETRIES")
            .env(key, "2")
            .status()
            .expect("isolated retry validation test should run");
        assert!(status.success(), "retry validation failed for {provider}");
    }
}

#[tokio::test]
async fn native_audio_env_retry_child() {
    let Ok(provider) = std::env::var("NATIVE_AUDIO_RETRY_CHILD_PROVIDER") else {
        return;
    };
    let provider_type = match provider.as_str() {
        "deepgram" => ProviderType::Deepgram,
        "elevenlabs" => ProviderType::ElevenLabs,
        other => panic!("unexpected child provider: {other}"),
    };
    let error =
        Provider::from_config_async(provider_type, json!({"api_key": "native-audio-secret"}))
            .await
            .expect_err("environment retries must not be silently disabled");
    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("max_retries"));
}

#[tokio::test]
async fn native_audio_applies_custom_headers_without_overriding_auth() {
    let mock = MockAudioServer::start(StatusCode::OK, Duration::ZERO).await;
    let deepgram = native_provider_with_headers(ProviderType::Deepgram, &mock.base_url).await;
    deepgram
        .audio_transcription(transcription_request("nova-3"), RequestContext::new())
        .await
        .expect("Deepgram transcription should succeed");
    deepgram
        .text_to_speech(
            SpeechRequest {
                input: "Deepgram headers".to_string(),
                model: "aura-2-thalia-en".to_string(),
                voice: "ignored-by-deepgram".to_string(),
                response_format: Some("mp3".to_string()),
                speed: None,
            },
            RequestContext::new(),
        )
        .await
        .expect("Deepgram speech should succeed");

    let elevenlabs = native_provider_with_headers(ProviderType::ElevenLabs, &mock.base_url).await;
    elevenlabs
        .audio_transcription(transcription_request("scribe_v1"), RequestContext::new())
        .await
        .expect("ElevenLabs transcription should succeed");
    elevenlabs
        .text_to_speech(
            SpeechRequest {
                input: "ElevenLabs headers".to_string(),
                model: "eleven_v3".to_string(),
                voice: "voice-original-123".to_string(),
                response_format: Some("mp3".to_string()),
                speed: None,
            },
            RequestContext::new(),
        )
        .await
        .expect("ElevenLabs speech should succeed");

    let requests = mock.requests();
    assert_eq!(requests.len(), 4);
    assert!(requests.iter().all(
        |request| request.headers.get("x-proxy-route").map(String::as_str) == Some("native-audio")
    ));
    for request in &requests[..2] {
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Token native-audio-secret")
        );
    }
    for request in &requests[2..] {
        assert_eq!(
            request.headers.get("xi-api-key").map(String::as_str),
            Some("native-audio-secret")
        );
    }
    mock.shutdown().await;
}
