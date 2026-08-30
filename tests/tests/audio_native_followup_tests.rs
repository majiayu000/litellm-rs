use super::*;
use std::ffi::OsString;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

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
    let _lock = ENV_LOCK.lock().await;
    for (provider_type, key) in [
        (ProviderType::Deepgram, "DEEPGRAM_MAX_RETRIES"),
        (ProviderType::ElevenLabs, "ELEVENLABS_MAX_RETRIES"),
    ] {
        let guard = EnvVarGuard::set(key, "2");
        let error =
            Provider::from_config_async(provider_type, json!({"api_key": "native-audio-secret"}))
                .await
                .expect_err("environment retries must not be silently disabled");
        assert!(matches!(error, ProviderError::Configuration { .. }));
        assert!(error.to_string().contains("max_retries"));
        drop(guard);
    }
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
