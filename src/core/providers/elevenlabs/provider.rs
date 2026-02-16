//! Main ElevenLabs Provider Implementation
//!
//! Implements audio capabilities for ElevenLabs' text-to-speech and speech-to-text APIs.

use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

use super::config::ElevenLabsConfig;
use super::error::ElevenLabsErrorMapper;
use super::stt::{self, TranscriptionRequest, TranscriptionResponse};
use super::tts::{self, TextToSpeechRequest, TextToSpeechResponse, VoiceSettings};
use crate::core::providers::base::GlobalPoolManager;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::ProviderConfig as _;
use crate::core::types::health::HealthStatus;
use crate::core::types::{model::ModelInfo, model::ProviderCapability};

/// Provider name constant
const PROVIDER_NAME: &str = "elevenlabs";

/// Static capabilities for ElevenLabs provider
const ELEVENLABS_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::TextToSpeech,
    ProviderCapability::AudioTranscription,
];

/// ElevenLabs provider implementation
#[derive(Debug, Clone)]
pub struct ElevenLabsProvider {
    config: ElevenLabsConfig,
    models: Vec<ModelInfo>,
}

impl ElevenLabsProvider {
    /// Create a new ElevenLabs provider instance
    pub async fn new(config: ElevenLabsConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;

        // Create pool manager
        let _pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                PROVIDER_NAME,
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build model list
        let models = Self::build_model_list();

        Ok(Self { config, models })
    }

    /// Create provider with API key only
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = ElevenLabsConfig::from_env().with_api_key(api_key);
        Self::new(config).await
    }

    /// Build the list of available models
    pub fn build_model_list() -> Vec<ModelInfo> {
        vec![
            // TTS Models
            ModelInfo {
                id: "eleven_multilingual_v2".to_string(),
                name: "Multilingual v2".to_string(),
                provider: "elevenlabs".to_string(),
                max_context_length: 5000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::TextToSpeech],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "eleven_turbo_v2_5".to_string(),
                name: "Turbo v2.5".to_string(),
                provider: "elevenlabs".to_string(),
                max_context_length: 5000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::TextToSpeech],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "eleven_turbo_v2".to_string(),
                name: "Turbo v2".to_string(),
                provider: "elevenlabs".to_string(),
                max_context_length: 5000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::TextToSpeech],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "eleven_monolingual_v1".to_string(),
                name: "Monolingual v1".to_string(),
                provider: "elevenlabs".to_string(),
                max_context_length: 5000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::TextToSpeech],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            // STT Models
            ModelInfo {
                id: "scribe_v1".to_string(),
                name: "Scribe v1".to_string(),
                provider: "elevenlabs".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: true,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::AudioTranscription],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
        ]
    }

    /// Get provider name
    pub fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    /// Get provider capabilities
    pub fn capabilities(&self) -> &'static [ProviderCapability] {
        ELEVENLABS_CAPABILITIES
    }

    /// Get available models
    pub fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    /// Get error mapper
    pub fn get_error_mapper(&self) -> ElevenLabsErrorMapper {
        ElevenLabsErrorMapper
    }

    /// Text-to-speech synthesis
    pub async fn text_to_speech(
        &self,
        text: &str,
        voice: &str,
        model: Option<&str>,
        voice_settings: Option<VoiceSettings>,
        output_format: Option<&str>,
    ) -> Result<TextToSpeechResponse, ProviderError> {
        debug!("ElevenLabs TTS request: voice={}", voice);

        // Resolve voice ID
        let voice_id = tts::resolve_voice_id(voice)?;

        // Build URL
        let url = tts::build_tts_url(&self.config.get_api_base(), &voice_id, output_format);

        // Build request body
        let request = TextToSpeechRequest {
            text: text.to_string(),
            model_id: model.unwrap_or("eleven_multilingual_v2").to_string(),
            voice_settings,
            pronunciation_dictionary_locators: None,
            seed: None,
            previous_text: None,
            next_text: None,
            previous_request_ids: None,
            next_request_ids: None,
        };

        let body = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request(PROVIDER_NAME, e.to_string()))?;

        // Get API key
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key is required"))?;

        // Execute request
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("xi-api-key", &api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(Self::map_http_error(status, body.as_deref()));
        }

        // Extract headers
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("audio/mpeg")
            .to_string();

        let request_id = response
            .headers()
            .get("request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let character_cost = response
            .headers()
            .get("character-cost")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok());

        // Get audio data
        let audio_data = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?
            .to_vec();

        Ok(TextToSpeechResponse {
            audio_data,
            content_type,
            character_cost,
            request_id,
        })
    }

    /// Speech-to-text transcription
    pub async fn transcribe_audio(
        &self,
        file: Vec<u8>,
        model: Option<String>,
        language: Option<String>,
        temperature: Option<f32>,
        filename: Option<String>,
    ) -> Result<TranscriptionResponse, ProviderError> {
        debug!("ElevenLabs STT request");

        let request = TranscriptionRequest {
            file,
            model_id: model.unwrap_or_else(|| "scribe_v1".to_string()),
            language_code: language,
            temperature,
            filename,
        };

        // Validate file size
        if request.file.len() > stt::MAX_FILE_SIZE {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                format!(
                    "Audio file too large (max {}MB)",
                    stt::MAX_FILE_SIZE / 1024 / 1024
                ),
            ));
        }

        // Create multipart form
        let form = stt::create_multipart_form(&request)?;

        // Build URL
        let url = stt::build_stt_url(&self.config.get_api_base());

        // Get API key
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key is required"))?;

        // Execute request
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("xi-api-key", &api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(Self::map_http_error(status, body.as_deref()));
        }

        // Parse response
        let response_text = response.text().await.map_err(|e| {
            ProviderError::response_parsing(
                PROVIDER_NAME,
                format!("Failed to read response: {}", e),
            )
        })?;

        serde_json::from_str::<TranscriptionResponse>(&response_text).map_err(|e| {
            ProviderError::response_parsing(
                PROVIDER_NAME,
                format!("Failed to parse response: {}", e),
            )
        })
    }

    /// Map HTTP error to ProviderError
    pub fn map_http_error(status: u16, body: Option<&str>) -> ProviderError {
        let message = body.unwrap_or("Unknown error").to_string();

        match status {
            400 => ProviderError::invalid_request(PROVIDER_NAME, message),
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            402 => ProviderError::quota_exceeded(PROVIDER_NAME, "Character quota exceeded"),
            403 => ProviderError::authentication(PROVIDER_NAME, "Access forbidden"),
            404 => ProviderError::model_not_found(PROVIDER_NAME, "Voice not found"),
            429 => ProviderError::rate_limit(PROVIDER_NAME, Some(60)),
            500 => ProviderError::api_error(PROVIDER_NAME, 500, "Internal server error"),
            502 | 503 => ProviderError::api_error(PROVIDER_NAME, status, "Service unavailable"),
            _ => ProviderError::api_error(
                PROVIDER_NAME,
                status,
                format!("HTTP error {}: {}", status, message),
            ),
        }
    }

    /// Health check
    pub async fn health_check(&self) -> HealthStatus {
        // Try to get user info to verify API key
        let url = format!("{}/v1/user", self.config.get_api_base());

        let api_key = match self.config.get_api_key() {
            Some(key) => key,
            None => return HealthStatus::Unhealthy,
        };

        let client = reqwest::Client::new();
        match client.get(&url).header("xi-api-key", &api_key).send().await {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            _ => HealthStatus::Unhealthy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_model_list() {
        let models = ElevenLabsProvider::build_model_list();
        assert!(!models.is_empty());

        // Check TTS models
        let tts_models: Vec<_> = models
            .iter()
            .filter(|m| m.capabilities.contains(&ProviderCapability::TextToSpeech))
            .collect();
        assert!(!tts_models.is_empty());

        // Check STT models
        let stt_models: Vec<_> = models
            .iter()
            .filter(|m| {
                m.capabilities
                    .contains(&ProviderCapability::AudioTranscription)
            })
            .collect();
        assert!(!stt_models.is_empty());
    }

    #[test]
    fn test_map_http_error() {
        let err = ElevenLabsProvider::map_http_error(401, Some("Unauthorized"));
        assert!(matches!(err, ProviderError::Authentication { .. }));

        let err = ElevenLabsProvider::map_http_error(402, Some("Quota exceeded"));
        assert!(matches!(err, ProviderError::QuotaExceeded { .. }));

        let err = ElevenLabsProvider::map_http_error(404, Some("Not found"));
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));

        let err = ElevenLabsProvider::map_http_error(429, Some("Too many requests"));
        assert!(matches!(err, ProviderError::RateLimit { .. }));

        let err = ElevenLabsProvider::map_http_error(503, Some("Service unavailable"));
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_capabilities() {
        assert!(ELEVENLABS_CAPABILITIES.contains(&ProviderCapability::TextToSpeech));
        assert!(ELEVENLABS_CAPABILITIES.contains(&ProviderCapability::AudioTranscription));
    }
}
