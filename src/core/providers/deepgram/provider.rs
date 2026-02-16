//! Main Deepgram Provider Implementation
//!
//! Implements audio capabilities for Deepgram's speech-to-text API.

use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

use super::config::DeepgramConfig;
use super::error::DeepgramErrorMapper;
use super::stt::{self, DeepgramResponse, OpenAITranscriptionResponse, TranscriptionRequest};
use crate::core::providers::base::GlobalPoolManager;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::ProviderConfig as _;
use crate::core::types::health::HealthStatus;
use crate::core::types::{model::ModelInfo, model::ProviderCapability};

/// Provider name constant
const PROVIDER_NAME: &str = "deepgram";

/// Static capabilities for Deepgram provider
const DEEPGRAM_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::AudioTranscription];

/// Deepgram provider implementation
#[derive(Debug, Clone)]
pub struct DeepgramProvider {
    config: DeepgramConfig,
    models: Vec<ModelInfo>,
}

impl DeepgramProvider {
    /// Create a new Deepgram provider instance
    pub async fn new(config: DeepgramConfig) -> Result<Self, ProviderError> {
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
        let config = DeepgramConfig::from_env().with_api_key(api_key);
        Self::new(config).await
    }

    /// Build the list of available models
    pub fn build_model_list() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "nova-2".to_string(),
                name: "Nova 2".to_string(),
                provider: "deepgram".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: true,
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
            ModelInfo {
                id: "nova-2-general".to_string(),
                name: "Nova 2 General".to_string(),
                provider: "deepgram".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: true,
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
            ModelInfo {
                id: "nova-2-meeting".to_string(),
                name: "Nova 2 Meeting".to_string(),
                provider: "deepgram".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: true,
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
            ModelInfo {
                id: "nova-2-phonecall".to_string(),
                name: "Nova 2 Phone Call".to_string(),
                provider: "deepgram".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: true,
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
            ModelInfo {
                id: "nova-2-medical".to_string(),
                name: "Nova 2 Medical".to_string(),
                provider: "deepgram".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: true,
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
            ModelInfo {
                id: "enhanced".to_string(),
                name: "Enhanced".to_string(),
                provider: "deepgram".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: true,
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
            ModelInfo {
                id: "base".to_string(),
                name: "Base".to_string(),
                provider: "deepgram".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: true,
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
        DEEPGRAM_CAPABILITIES
    }

    /// Get available models
    pub fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    /// Get error mapper
    pub fn get_error_mapper(&self) -> DeepgramErrorMapper {
        DeepgramErrorMapper
    }

    /// Speech-to-text transcription
    pub async fn transcribe_audio(
        &self,
        request: TranscriptionRequest,
    ) -> Result<OpenAITranscriptionResponse, ProviderError> {
        debug!("Deepgram STT request: model={}", request.model);

        // Build URL with query parameters
        let url = stt::build_stt_url(&self.config.get_api_base(), &request);

        // Get API key
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key is required"))?;

        // Detect content type
        let content_type = request
            .filename
            .as_ref()
            .map(|f| stt::detect_audio_mime_type(f))
            .unwrap_or("audio/mpeg");

        // Execute request - Deepgram accepts raw binary data
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Token {}", api_key))
            .header("Content-Type", content_type)
            .body(request.file)
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

        let deepgram_response: DeepgramResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                ProviderError::response_parsing(
                    PROVIDER_NAME,
                    format!(
                        "Failed to parse response: {}\nResponse: {}",
                        e, response_text
                    ),
                )
            })?;

        deepgram_response.try_into()
    }

    /// Transcribe audio with simple parameters (convenience method)
    pub async fn transcribe_simple(
        &self,
        file: Vec<u8>,
        model: Option<String>,
        language: Option<String>,
        diarize: Option<bool>,
        punctuate: Option<bool>,
        filename: Option<String>,
    ) -> Result<OpenAITranscriptionResponse, ProviderError> {
        let request = TranscriptionRequest {
            file,
            model: model.unwrap_or_else(|| "nova-2".to_string()),
            language,
            smart_format: Some(true),
            punctuate,
            diarize,
            paragraphs: diarize, // Enable paragraphs if diarization is enabled
            words: Some(true),
            filename,
            ..Default::default()
        };

        self.transcribe_audio(request).await
    }

    /// Map HTTP error to ProviderError
    pub fn map_http_error(status: u16, body: Option<&str>) -> ProviderError {
        let message = body.unwrap_or("Unknown error").to_string();

        match status {
            400 => ProviderError::invalid_request(PROVIDER_NAME, message),
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            402 => ProviderError::quota_exceeded(PROVIDER_NAME, "Usage quota exceeded"),
            403 => ProviderError::authentication(PROVIDER_NAME, "Access forbidden"),
            404 => ProviderError::model_not_found(PROVIDER_NAME, "Model not found"),
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
        // Try a simple API call to verify connectivity
        let url = format!("{}/projects", self.config.get_api_base().replace("/v1", ""));

        let api_key = match self.config.get_api_key() {
            Some(key) => key,
            None => return HealthStatus::Unhealthy,
        };

        let client = reqwest::Client::new();
        match client
            .get(&url)
            .header("Authorization", format!("Token {}", api_key))
            .send()
            .await
        {
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
        let models = DeepgramProvider::build_model_list();
        assert!(!models.is_empty());

        // Check for expected models
        let has_nova2 = models.iter().any(|m| m.id == "nova-2");
        assert!(has_nova2);

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
        }
    }

    #[test]
    fn test_map_http_error() {
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

        let err = DeepgramProvider::map_http_error(503, None);
        assert!(matches!(err, ProviderError::ApiError { .. }));
    }

    #[test]
    fn test_capabilities() {
        assert!(DEEPGRAM_CAPABILITIES.contains(&ProviderCapability::AudioTranscription));
        assert!(!DEEPGRAM_CAPABILITIES.contains(&ProviderCapability::ChatCompletion));
        assert!(!DEEPGRAM_CAPABILITIES.contains(&ProviderCapability::TextToSpeech));
    }
}
