//! Tests for Deepgram Provider
//!
//! Comprehensive unit tests for the Deepgram provider implementation.

use super::*;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::model::ProviderCapability;

// ==================== Config Tests ====================

#[test]
fn test_config_default() {
    let config = DeepgramConfig::default();
    assert!(config.base.api_key.is_none());
    assert_eq!(config.base.timeout, 60);
    assert_eq!(config.base.max_retries, 3);
    assert!(!config.debug);
}

#[test]
fn test_config_custom_values() {
    let config = DeepgramConfig::from_env()
        .with_api_key("test-key")
        .with_base_url("https://custom.api.com")
        .with_timeout(180);

    assert_eq!(config.base.api_key.as_deref(), Some("test-key"));
    assert_eq!(
        config.base.api_base.as_deref(),
        Some("https://custom.api.com")
    );
    assert_eq!(config.base.timeout, 180);
    assert!(!config.debug);
}

#[test]
fn test_config_get_api_base_default() {
    let config = DeepgramConfig::default();
    assert_eq!(config.get_api_base(), "https://api.deepgram.com/v1");
}

#[test]
fn test_config_get_api_key() {
    let config = DeepgramConfig::from_env().with_api_key("my-api-key");
    assert_eq!(config.get_api_key(), Some("my-api-key".to_string()));
}

// ==================== Error Tests ====================

#[test]
fn test_error_types() {
    let err = ProviderError::api_error("deepgram", 500, "test");
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = ProviderError::authentication("deepgram", "bad key");
    assert!(matches!(err, ProviderError::Authentication { .. }));

    let err = ProviderError::rate_limit("deepgram", Some(60));
    assert!(matches!(err, ProviderError::RateLimit { .. }));

    let err = ProviderError::model_not_found("deepgram", "unknown");
    assert!(matches!(err, ProviderError::ModelNotFound { .. }));

    let err = ProviderError::quota_exceeded("deepgram", "limit");
    assert!(matches!(err, ProviderError::QuotaExceeded { .. }));
}

#[test]
fn test_error_display() {
    let err = ProviderError::api_error("deepgram", 500, "something went wrong");
    let display = err.to_string();
    assert!(display.contains("something went wrong") || display.contains("API error"));

    let err = ProviderError::model_not_found("deepgram", "nova-3");
    let display = err.to_string();
    assert!(display.contains("nova-3") || display.contains("not found"));
}

// ==================== STT Tests ====================

#[test]
fn test_stt_model_enum() {
    assert_eq!(stt::STTModel::Nova2.as_str(), "nova-2");
    assert_eq!(stt::STTModel::Nova2Meeting.as_str(), "nova-2-meeting");
    assert_eq!(stt::STTModel::Nova2Medical.as_str(), "nova-2-medical");
    assert_eq!(stt::STTModel::Enhanced.as_str(), "enhanced");
    assert_eq!(stt::STTModel::Base.as_str(), "base");

    assert_eq!(stt::STTModel::parse("nova-2"), Some(stt::STTModel::Nova2));
    assert_eq!(
        stt::STTModel::parse("nova-2-meeting"),
        Some(stt::STTModel::Nova2Meeting)
    );
    assert_eq!(stt::STTModel::parse("invalid"), None);
}

#[test]
fn test_stt_build_query_params() {
    let request = stt::TranscriptionRequest {
        model: "nova-2".to_string(),
        language: Some("en-US".to_string()),
        punctuate: Some(true),
        diarize: Some(true),
        words: Some(true),
        ..Default::default()
    };

    let params = stt::build_query_params(&request);
    assert!(params.contains("model=nova-2"));
    assert!(params.contains("language=en-US"));
    assert!(params.contains("punctuate=true"));
    assert!(params.contains("diarize=true"));
    assert!(params.contains("words=true"));
}

#[test]
fn test_stt_build_url() {
    let request = stt::TranscriptionRequest {
        model: "nova-2".to_string(),
        language: Some("en".to_string()),
        ..Default::default()
    };

    let url = stt::build_stt_url("https://api.deepgram.com/v1", &request);
    assert!(url.starts_with("https://api.deepgram.com/v1/listen?"));
    assert!(url.contains("model=nova-2"));
    assert!(url.contains("language=en"));
}

#[test]
fn test_stt_detect_audio_mime_type() {
    assert_eq!(stt::detect_audio_mime_type("audio.mp3"), "audio/mpeg");
    assert_eq!(stt::detect_audio_mime_type("audio.wav"), "audio/wav");
    assert_eq!(stt::detect_audio_mime_type("audio.m4a"), "audio/mp4");
    assert_eq!(stt::detect_audio_mime_type("audio.webm"), "audio/webm");
    assert_eq!(stt::detect_audio_mime_type("audio.flac"), "audio/flac");
    assert_eq!(stt::detect_audio_mime_type("audio.unknown"), "audio/mpeg");
}

#[test]
fn test_stt_supported_formats() {
    let formats = stt::supported_audio_formats();
    assert!(formats.contains(&"mp3"));
    assert!(formats.contains(&"wav"));
    assert!(formats.contains(&"flac"));
    assert!(formats.contains(&"webm"));
    assert!(formats.contains(&"opus"));
}

#[test]
fn test_stt_transcription_request_default() {
    let request = stt::TranscriptionRequest::default();
    assert_eq!(request.model, "nova-2");
    assert!(request.language.is_none());
    assert!(request.diarize.is_none());
    assert!(request.punctuate.is_none());
}

#[test]
fn test_stt_word_info_deserialization() {
    let json = r#"{
        "word": "hello",
        "start": 0.5,
        "end": 1.0,
        "confidence": 0.95,
        "speaker": 0,
        "punctuated_word": "Hello,"
    }"#;

    let word: stt::WordInfo = serde_json::from_str(json).unwrap();
    assert_eq!(word.word, "hello");
    assert_eq!(word.start, 0.5);
    assert_eq!(word.end, 1.0);
    assert_eq!(word.confidence, 0.95);
    assert_eq!(word.speaker, Some(0));
    assert_eq!(word.punctuated_word, Some("Hello,".to_string()));
}

#[test]
fn test_openai_transcription_response_serialization() {
    let response = stt::OpenAITranscriptionResponse {
        text: "Hello world".to_string(),
        task: "transcribe".to_string(),
        language: "en".to_string(),
        duration: Some(2.5),
        words: Some(vec![
            stt::OpenAIWordInfo {
                word: "Hello".to_string(),
                start: 0.0,
                end: 0.5,
            },
            stt::OpenAIWordInfo {
                word: "world".to_string(),
                start: 0.6,
                end: 1.0,
            },
        ]),
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["text"], "Hello world");
    assert_eq!(json["task"], "transcribe");
    assert_eq!(json["language"], "en");
    assert_eq!(json["duration"], 2.5);
    assert!(json["words"].is_array());
}

// ==================== Provider Tests ====================

#[tokio::test]
async fn test_provider_capabilities() {
    let config = DeepgramConfig::from_env().with_api_key("test-key");
    let provider = DeepgramProvider::new(config).await.unwrap();
    let capabilities = provider.capabilities();

    assert!(capabilities.contains(&ProviderCapability::AudioTranscription));
    assert!(!capabilities.contains(&ProviderCapability::ChatCompletion));
    assert!(!capabilities.contains(&ProviderCapability::TextToSpeech));
}

#[test]
fn test_provider_build_model_list() {
    let models = DeepgramProvider::build_model_list();
    assert!(!models.is_empty());

    // Check for specific models
    let has_nova2 = models.iter().any(|m| m.id == "nova-2");
    assert!(has_nova2);

    let has_nova2_meeting = models.iter().any(|m| m.id == "nova-2-meeting");
    assert!(has_nova2_meeting);

    let has_enhanced = models.iter().any(|m| m.id == "enhanced");
    assert!(has_enhanced);

    // Verify model attributes
    for model in &models {
        assert_eq!(model.provider, "deepgram");
        assert!(
            model
                .capabilities
                .contains(&ProviderCapability::AudioTranscription)
        );
        assert!(model.supports_multimodal); // Audio input
    }
}

#[test]
fn test_provider_map_http_error() {
    let err = DeepgramProvider::map_http_error(400, Some("Bad request"));
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));

    let err = DeepgramProvider::map_http_error(401, None);
    assert!(matches!(err, ProviderError::Authentication { .. }));

    let err = DeepgramProvider::map_http_error(402, Some("Quota"));
    assert!(matches!(err, ProviderError::QuotaExceeded { .. }));

    let err = DeepgramProvider::map_http_error(403, None);
    assert!(matches!(err, ProviderError::Authentication { .. }));

    let err = DeepgramProvider::map_http_error(404, None);
    assert!(matches!(err, ProviderError::ModelNotFound { .. }));

    let err = DeepgramProvider::map_http_error(429, None);
    assert!(matches!(err, ProviderError::RateLimit { .. }));

    let err = DeepgramProvider::map_http_error(500, None);
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = DeepgramProvider::map_http_error(502, None);
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = DeepgramProvider::map_http_error(503, None);
    assert!(matches!(err, ProviderError::ApiError { .. }));

    let err = DeepgramProvider::map_http_error(418, Some("I'm a teapot"));
    assert!(matches!(err, ProviderError::ApiError { .. }));
}

// ==================== Error Mapper Tests ====================

#[test]
fn test_error_mapper_http_errors() {
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let mapper = DeepgramErrorMapper;

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

    let err = mapper.map_http_error(503, "");
    assert!(matches!(err, ProviderError::ApiError { .. }));
}

#[test]
fn test_error_mapper_empty_body() {
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let mapper = DeepgramErrorMapper;
    let err = mapper.map_http_error(400, "");
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));
}

// ==================== Integration-like Tests ====================

#[tokio::test]
async fn test_provider_creation_without_key() {
    // Clear any environment variable for this test
    unsafe { std::env::remove_var("DEEPGRAM_API_KEY") };

    let config = DeepgramConfig::default();
    let result = DeepgramProvider::new(config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_provider_creation_with_key() {
    let config = DeepgramConfig::from_env().with_api_key("test-api-key");

    let result = DeepgramProvider::new(config).await;
    assert!(result.is_ok());

    let provider = result.unwrap();
    assert_eq!(provider.name(), "deepgram");
    assert!(!provider.models().is_empty());
}

#[tokio::test]
async fn test_provider_with_api_key() {
    let result = DeepgramProvider::with_api_key("test-key").await;
    assert!(result.is_ok());

    let provider = result.unwrap();
    assert_eq!(provider.name(), "deepgram");
}

// ==================== Response Conversion Tests ====================

#[test]
fn test_deepgram_response_to_openai_simple() {
    let deepgram_response = stt::DeepgramResponse {
        metadata: stt::ResponseMetadata {
            transaction_key: None,
            request_id: "req-123".to_string(),
            sha256: None,
            created: None,
            duration: 5.5,
            channels: Some(1),
            models: None,
            model_info: None,
        },
        results: stt::TranscriptionResults {
            channels: vec![stt::ChannelResult {
                alternatives: vec![stt::TranscriptionAlternative {
                    transcript: "Hello world".to_string(),
                    confidence: 0.95,
                    words: Some(vec![
                        stt::WordInfo {
                            word: "Hello".to_string(),
                            start: 0.0,
                            end: 0.5,
                            confidence: 0.98,
                            speaker: None,
                            punctuated_word: None,
                        },
                        stt::WordInfo {
                            word: "world".to_string(),
                            start: 0.6,
                            end: 1.0,
                            confidence: 0.92,
                            speaker: None,
                            punctuated_word: None,
                        },
                    ]),
                    paragraphs: None,
                }],
                detected_language: Some("en".to_string()),
                language_confidence: Some(0.99),
            }],
            utterances: None,
        },
    };

    let openai_response: stt::OpenAITranscriptionResponse = deepgram_response.try_into().unwrap();
    assert_eq!(openai_response.text, "Hello world");
    assert_eq!(openai_response.task, "transcribe");
    assert_eq!(openai_response.language, "en");
    assert_eq!(openai_response.duration, Some(5.5));

    let words = openai_response.words.unwrap();
    assert_eq!(words.len(), 2);
    assert_eq!(words[0].word, "Hello");
    assert_eq!(words[1].word, "world");
}

#[test]
fn test_deepgram_response_to_openai_with_diarization() {
    let deepgram_response = stt::DeepgramResponse {
        metadata: stt::ResponseMetadata {
            transaction_key: None,
            request_id: "req-456".to_string(),
            sha256: None,
            created: None,
            duration: 10.0,
            channels: Some(1),
            models: None,
            model_info: None,
        },
        results: stt::TranscriptionResults {
            channels: vec![stt::ChannelResult {
                alternatives: vec![stt::TranscriptionAlternative {
                    transcript: "Hello Hi".to_string(),
                    confidence: 0.90,
                    words: Some(vec![
                        stt::WordInfo {
                            word: "Hello".to_string(),
                            start: 0.0,
                            end: 0.5,
                            confidence: 0.95,
                            speaker: Some(0),
                            punctuated_word: Some("Hello.".to_string()),
                        },
                        stt::WordInfo {
                            word: "Hi".to_string(),
                            start: 1.0,
                            end: 1.5,
                            confidence: 0.93,
                            speaker: Some(1),
                            punctuated_word: Some("Hi!".to_string()),
                        },
                    ]),
                    paragraphs: None,
                }],
                detected_language: Some("en".to_string()),
                language_confidence: Some(0.98),
            }],
            utterances: None,
        },
    };

    let openai_response: stt::OpenAITranscriptionResponse = deepgram_response.try_into().unwrap();
    // With diarization but no paragraphs, should reconstruct transcript
    assert!(openai_response.text.contains("Speaker 0"));
    assert!(openai_response.text.contains("Speaker 1"));
}
