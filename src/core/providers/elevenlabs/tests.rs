//! Tests for ElevenLabs Provider
//!
//! Comprehensive unit tests for the ElevenLabs provider implementation.

use super::*;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::model::ProviderCapability;

// ==================== Config Tests ====================

#[test]
fn test_config_default() {
    let config = ElevenLabsConfig::default();
    assert!(config.base.api_key.is_none());
    assert_eq!(config.base.timeout, 60);
    assert_eq!(config.base.max_retries, 3);
    assert!(!config.debug);
}

#[test]
fn test_config_custom_values() {
    let config = ElevenLabsConfig::from_env()
        .with_api_key("test-key")
        .with_base_url("https://custom.api.com")
        .with_timeout(120);

    assert_eq!(config.base.api_key.as_deref(), Some("test-key"));
    assert_eq!(
        config.base.api_base.as_deref(),
        Some("https://custom.api.com")
    );
    assert_eq!(config.base.timeout, 120);
}

#[test]
fn test_config_get_api_base_default() {
    let config = ElevenLabsConfig::default();
    assert_eq!(config.get_api_base(), "https://api.elevenlabs.io");
}

#[test]
fn test_config_get_api_key() {
    let config = ElevenLabsConfig::from_env().with_api_key("my-api-key");
    assert_eq!(config.get_api_key(), Some("my-api-key".to_string()));
}

// ==================== Error Tests ====================

#[test]
fn test_error_types() {
    let err = ProviderError::api_error("elevenlabs", 500, "test");
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = ProviderError::authentication("elevenlabs", "bad key");
    assert!(matches!(err, ProviderError::Authentication { .. }));

    let err = ProviderError::rate_limit("elevenlabs", Some(60));
    assert!(matches!(err, ProviderError::RateLimit { .. }));

    let err = ProviderError::model_not_found("elevenlabs", "unknown");
    assert!(matches!(err, ProviderError::ModelNotFound { .. }));

    let err = ProviderError::quota_exceeded("elevenlabs", "limit");
    assert!(matches!(err, ProviderError::QuotaExceeded { .. }));
}

#[test]
fn test_error_display() {
    let err = ProviderError::api_error("elevenlabs", 500, "something went wrong");
    let display = err.to_string();
    assert!(display.contains("something went wrong") || display.contains("API error"));

    let err = ProviderError::model_not_found("elevenlabs", "voice-123");
    let display = err.to_string();
    assert!(display.contains("voice-123") || display.contains("not found"));
}

// ==================== TTS Tests ====================

#[test]
fn test_tts_voice_mappings() {
    let mappings = tts::get_voice_mappings();
    assert!(mappings.contains_key("alloy"));
    assert!(mappings.contains_key("onyx"));
    assert!(mappings.contains_key("coral"));
    assert_eq!(mappings.get("alloy"), Some(&"21m00Tcm4TlvDq8ikWAM"));
}

#[test]
fn test_tts_format_mappings() {
    let mappings = tts::get_format_mappings();
    assert_eq!(mappings.get("mp3"), Some(&"mp3_44100_128"));
    assert_eq!(mappings.get("pcm"), Some(&"pcm_44100"));
    assert_eq!(mappings.get("opus"), Some(&"opus_48000_128"));
}

#[test]
fn test_tts_resolve_voice_id_openai() {
    let voice_id = tts::resolve_voice_id("alloy").unwrap();
    assert_eq!(voice_id, "21m00Tcm4TlvDq8ikWAM");

    let voice_id = tts::resolve_voice_id("ALLOY").unwrap();
    assert_eq!(voice_id, "21m00Tcm4TlvDq8ikWAM");

    let voice_id = tts::resolve_voice_id("  alloy  ").unwrap();
    assert_eq!(voice_id, "21m00Tcm4TlvDq8ikWAM");
}

#[test]
fn test_tts_resolve_voice_id_direct() {
    let voice_id = tts::resolve_voice_id("custom-voice-id").unwrap();
    assert_eq!(voice_id, "custom-voice-id");
}

#[test]
fn test_tts_resolve_voice_id_empty() {
    let result = tts::resolve_voice_id("");
    assert!(result.is_err());

    let result = tts::resolve_voice_id("   ");
    assert!(result.is_err());
}

#[test]
fn test_tts_map_output_format() {
    assert_eq!(tts::map_output_format(Some("mp3")), "mp3_44100_128");
    assert_eq!(tts::map_output_format(Some("pcm")), "pcm_44100");
    assert_eq!(tts::map_output_format(Some("opus")), "opus_48000_128");
    assert_eq!(tts::map_output_format(None), "mp3_44100_128");
    assert_eq!(tts::map_output_format(Some("unknown")), "mp3_44100_128");
}

#[test]
fn test_tts_build_url() {
    let url = tts::build_tts_url("https://api.elevenlabs.io", "voice-123", Some("mp3"));
    assert_eq!(
        url,
        "https://api.elevenlabs.io/v1/text-to-speech/voice-123?output_format=mp3_44100_128"
    );

    let url = tts::build_tts_url("https://api.elevenlabs.io/", "voice-123", None);
    assert_eq!(
        url,
        "https://api.elevenlabs.io/v1/text-to-speech/voice-123?output_format=mp3_44100_128"
    );
}

#[test]
fn test_tts_model_enum() {
    assert_eq!(
        tts::TTSModel::MultilingualV2.as_str(),
        "eleven_multilingual_v2"
    );
    assert_eq!(tts::TTSModel::TurboV2_5.as_str(), "eleven_turbo_v2_5");
    assert_eq!(tts::TTSModel::TurboV2.as_str(), "eleven_turbo_v2");
    assert_eq!(
        tts::TTSModel::MonolingualV1.as_str(),
        "eleven_monolingual_v1"
    );

    assert_eq!(
        tts::TTSModel::parse("eleven_multilingual_v2"),
        Some(tts::TTSModel::MultilingualV2)
    );
    assert_eq!(tts::TTSModel::parse("invalid"), None);
}

#[test]
fn test_voice_settings_serialization() {
    let settings = tts::VoiceSettings {
        stability: Some(0.5),
        similarity_boost: Some(0.75),
        style: Some(0.3),
        use_speaker_boost: Some(true),
        speed: Some(1.0),
    };

    let json = serde_json::to_value(&settings).unwrap();
    assert!((json["stability"].as_f64().unwrap() - 0.5).abs() < 0.01);
    assert!((json["similarity_boost"].as_f64().unwrap() - 0.75).abs() < 0.01);
    assert!((json["style"].as_f64().unwrap() - 0.3).abs() < 0.01);
    assert_eq!(json["use_speaker_boost"], true);
    assert!((json["speed"].as_f64().unwrap() - 1.0).abs() < 0.01);
}

#[test]
fn test_voice_settings_skip_none() {
    let settings = tts::VoiceSettings {
        stability: Some(0.5),
        ..Default::default()
    };

    let json = serde_json::to_value(&settings).unwrap();
    assert!(json.get("stability").is_some());
    assert!(json.get("similarity_boost").is_none());
}

// ==================== STT Tests ====================

#[test]
fn test_stt_model_enum() {
    assert_eq!(stt::STTModel::ScribeV1.as_str(), "scribe_v1");
    assert_eq!(
        stt::STTModel::parse("scribe_v1"),
        Some(stt::STTModel::ScribeV1)
    );
    assert_eq!(stt::STTModel::parse("invalid"), None);
}

#[test]
fn test_stt_build_url() {
    let url = stt::build_stt_url("https://api.elevenlabs.io");
    assert_eq!(url, "https://api.elevenlabs.io/v1/speech-to-text");

    let url = stt::build_stt_url("https://api.elevenlabs.io/");
    assert_eq!(url, "https://api.elevenlabs.io/v1/speech-to-text");
}

#[test]
fn test_stt_supported_formats() {
    let formats = stt::supported_audio_formats();
    assert!(formats.contains(&"mp3"));
    assert!(formats.contains(&"wav"));
    assert!(formats.contains(&"m4a"));
    assert!(formats.contains(&"webm"));
    assert!(formats.contains(&"flac"));
}

#[test]
fn test_stt_transcription_response_to_openai() {
    let response = stt::TranscriptionResponse {
        text: "Hello world".to_string(),
        language_code: Some("en".to_string()),
        words: Some(vec![
            stt::WordInfo {
                text: "Hello".to_string(),
                start: 0.0,
                end: 0.5,
                word_type: "word".to_string(),
            },
            stt::WordInfo {
                text: " ".to_string(),
                start: 0.5,
                end: 0.6,
                word_type: "spacing".to_string(),
            },
            stt::WordInfo {
                text: "world".to_string(),
                start: 0.6,
                end: 1.0,
                word_type: "word".to_string(),
            },
        ]),
    };

    let openai: stt::OpenAITranscriptionResponse = response.into();
    assert_eq!(openai.text, "Hello world");
    assert_eq!(openai.task, "transcribe");
    assert_eq!(openai.language, "en");

    let words = openai.words.unwrap();
    assert_eq!(words.len(), 2); // Spacing filtered out
    assert_eq!(words[0].word, "Hello");
    assert_eq!(words[1].word, "world");
}

#[test]
fn test_stt_max_file_size() {
    assert_eq!(stt::MAX_FILE_SIZE, 100 * 1024 * 1024); // 100MB
}

// ==================== Provider Tests ====================

#[tokio::test]
async fn test_provider_capabilities() {
    let config = ElevenLabsConfig::from_env().with_api_key("test-key");
    let provider = ElevenLabsProvider::new(config).await.unwrap();
    let capabilities = provider.capabilities();

    assert!(capabilities.contains(&ProviderCapability::TextToSpeech));
    assert!(capabilities.contains(&ProviderCapability::AudioTranscription));
    assert!(!capabilities.contains(&ProviderCapability::ChatCompletion));
}

#[test]
fn test_provider_build_model_list() {
    let models = ElevenLabsProvider::build_model_list();
    assert!(!models.is_empty());

    // Check for TTS models
    let has_tts = models.iter().any(|m| m.id == "eleven_multilingual_v2");
    assert!(has_tts);

    // Check for STT models
    let has_stt = models.iter().any(|m| m.id == "scribe_v1");
    assert!(has_stt);

    // Verify model attributes
    for model in &models {
        assert_eq!(model.provider, "elevenlabs");
        assert!(!model.capabilities.is_empty());
    }
}

#[test]
fn test_provider_map_http_error() {
    let err = ElevenLabsProvider::map_http_error(400, Some("Bad request"));
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));

    let err = ElevenLabsProvider::map_http_error(401, None);
    assert!(matches!(err, ProviderError::Authentication { .. }));

    let err = ElevenLabsProvider::map_http_error(402, Some("Quota"));
    assert!(matches!(err, ProviderError::QuotaExceeded { .. }));

    let err = ElevenLabsProvider::map_http_error(403, None);
    assert!(matches!(err, ProviderError::Authentication { .. }));

    let err = ElevenLabsProvider::map_http_error(404, None);
    assert!(matches!(err, ProviderError::ModelNotFound { .. }));

    let err = ElevenLabsProvider::map_http_error(429, None);
    assert!(matches!(err, ProviderError::RateLimit { .. }));

    let err = ElevenLabsProvider::map_http_error(500, None);
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = ElevenLabsProvider::map_http_error(502, None);
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = ElevenLabsProvider::map_http_error(503, None);
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = ElevenLabsProvider::map_http_error(418, Some("I'm a teapot"));
    assert!(matches!(err, ProviderError::ApiError { .. }));
}

// ==================== Error Mapper Tests ====================

#[test]
fn test_error_mapper_http_errors() {
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let mapper = ElevenLabsErrorMapper;

    let err = mapper.map_http_error(400, "Invalid parameter");
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));

    let err = mapper.map_http_error(401, "");
    assert!(matches!(err, ProviderError::Authentication { .. }));

    let err = mapper.map_http_error(402, "");
    assert!(matches!(err, ProviderError::QuotaExceeded { .. }));

    let err = mapper.map_http_error(404, "");
    assert!(matches!(err, ProviderError::ModelNotFound { .. }));

    let err = mapper.map_http_error(429, "");
    assert!(matches!(err, ProviderError::RateLimit { .. }));

    let err = mapper.map_http_error(500, "");
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = mapper.map_http_error(502, "");
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = mapper.map_http_error(503, "");
    assert!(matches!(err, ProviderError::ApiError { .. }));
}

#[test]
fn test_error_mapper_empty_body() {
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let mapper = ElevenLabsErrorMapper;
    let err = mapper.map_http_error(400, "");
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));
}

// ==================== Integration-like Tests ====================

#[tokio::test]
async fn test_provider_creation_without_key() {
    // Clear any environment variable for this test
    unsafe { std::env::remove_var("ELEVENLABS_API_KEY") };

    let config = ElevenLabsConfig::default();
    let result = ElevenLabsProvider::new(config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_provider_creation_with_key() {
    let config = ElevenLabsConfig::from_env().with_api_key("test-api-key");

    let result = ElevenLabsProvider::new(config).await;
    assert!(result.is_ok());

    let provider = result.unwrap();
    assert_eq!(provider.name(), "elevenlabs");
    assert!(!provider.models().is_empty());
}

#[tokio::test]
async fn test_provider_with_api_key() {
    let result = ElevenLabsProvider::with_api_key("test-key").await;
    assert!(result.is_ok());

    let provider = result.unwrap();
    assert_eq!(provider.name(), "elevenlabs");
}
