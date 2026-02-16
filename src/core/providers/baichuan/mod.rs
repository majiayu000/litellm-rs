//! Baichuan AI Provider
//!
//! Baichuan AI (百川智能) platform integration

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::core::providers::base_provider::{BaseHttpClient, BaseProviderConfig};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    error_mapper::trait_def::ErrorMapper, provider::ProviderConfig,
    provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    image::ImageGenerationRequest,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse, ImageGenerationResponse},
};

const PROVIDER_NAME: &str = "baichuan";

/// Baichuan AI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaichuanConfig {
    /// API key for Baichuan AI
    pub api_key: Option<String>,
    /// API base URL (default: <https://api.baichuan-ai.com/v1>)
    pub api_base: Option<String>,
    /// Timeout in seconds
    pub timeout: u64,
    /// Max retries
    pub max_retries: u32,
}

impl Default for BaichuanConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: Some("https://api.baichuan-ai.com/v1".to_string()),
            timeout: 60,
            max_retries: 3,
        }
    }
}

impl BaichuanConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let api_key = std::env::var("BAICHUAN_API_KEY").ok();

        let api_base = std::env::var("BAICHUAN_API_BASE")
            .unwrap_or_else(|_| "https://api.baichuan-ai.com/v1".to_string());

        Ok(Self {
            api_key,
            api_base: Some(api_base),
            timeout: 60,
            max_retries: 3,
        })
    }

    /// Get effective API base URL
    pub fn get_effective_api_base(&self) -> &str {
        self.api_base
            .as_deref()
            .unwrap_or("https://api.baichuan-ai.com/v1")
    }
}

/// Baichuan AI error type (alias to unified ProviderError)
pub type BaichuanError = ProviderError;

/// Baichuan AI provider
#[derive(Debug, Clone)]
pub struct BaichuanProvider {
    config: BaichuanConfig,
}

impl BaichuanProvider {
    /// Create new Baichuan AI provider
    pub fn new(config: BaichuanConfig) -> Result<Self, BaichuanError> {
        let base_config = BaseProviderConfig {
            api_key: config.api_key.clone(),
            api_base: config.api_base.clone(),
            timeout: Some(config.timeout),
            max_retries: Some(config.max_retries),
            headers: None,
            organization: None,
            api_version: None,
        };

        let _base_client = BaseHttpClient::new(base_config)
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e.to_string()))?;

        Ok(Self { config })
    }

    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, BaichuanError> {
        let config = BaichuanConfig::from_env()?;
        Self::new(config)
    }

    /// Get supported models
    fn get_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "Baichuan2-Turbo".to_string(),
                name: "Baichuan2-Turbo".to_string(),
                provider: PROVIDER_NAME.to_string(),
                max_context_length: 32000,
                max_output_length: Some(2048),
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.008),
                output_cost_per_1k_tokens: Some(0.008),
                currency: "CNY".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ],
                ..Default::default()
            },
            ModelInfo {
                id: "Baichuan2-Turbo-192k".to_string(),
                name: "Baichuan2-Turbo-192k".to_string(),
                provider: PROVIDER_NAME.to_string(),
                max_context_length: 192000,
                max_output_length: Some(4096),
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.016),
                output_cost_per_1k_tokens: Some(0.016),
                currency: "CNY".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ],
                ..Default::default()
            },
            ModelInfo {
                id: "Baichuan2-53B".to_string(),
                name: "Baichuan2-53B".to_string(),
                provider: PROVIDER_NAME.to_string(),
                max_context_length: 4096,
                max_output_length: Some(2048),
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.02),
                output_cost_per_1k_tokens: Some(0.02),
                currency: "CNY".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ],
                ..Default::default()
            },
        ]
    }
}

/// Baichuan AI error mapper
#[derive(Debug)]
pub struct BaichuanErrorMapper;

impl ErrorMapper<BaichuanError> for BaichuanErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> BaichuanError {
        match status_code {
            401 => ProviderError::authentication(
                PROVIDER_NAME,
                format!("Invalid API key: {}", response_body),
            ),
            403 => ProviderError::authentication(
                PROVIDER_NAME,
                format!("Permission denied: {}", response_body),
            ),
            404 => ProviderError::model_not_found(
                PROVIDER_NAME,
                format!("Model not found: {}", response_body),
            ),
            429 => ProviderError::rate_limit(PROVIDER_NAME, None),
            500..=599 => ProviderError::api_error(
                PROVIDER_NAME,
                status_code,
                format!("Server error: {}", response_body),
            ),
            _ => ProviderError::api_error(
                PROVIDER_NAME,
                status_code,
                format!("HTTP {}: {}", status_code, response_body),
            ),
        }
    }
}

#[async_trait]
impl LLMProvider for BaichuanProvider {
    type Config = BaichuanConfig;
    type Error = BaichuanError;
    type ErrorMapper = BaichuanErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        static MODELS: &[ModelInfo] = &[];
        MODELS
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["temperature", "max_tokens", "top_p", "top_k", "stream"]
    }

    async fn map_openai_params(
        &self,
        params: std::collections::HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, Self::Error> {
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        use serde_json::json;

        let mut body = json!({
            "model": request.model,
            "messages": request.messages,
        });

        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }

        if request.stream {
            body["stream"] = json!(true);
        }

        Ok(body)
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_implemented(
            PROVIDER_NAME,
            "Response transformation not yet implemented",
        ))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        BaichuanErrorMapper
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        let models = Self::get_models();
        let model_info = models.iter().find(|m| m.id == model);

        if let Some(info) = model_info {
            let input_cost =
                info.input_cost_per_1k_tokens.unwrap_or(0.0) * (input_tokens as f64 / 1000.0);
            let output_cost =
                info.output_cost_per_1k_tokens.unwrap_or(0.0) * (output_tokens as f64 / 1000.0);
            Ok(input_cost + output_cost)
        } else {
            Ok(0.0)
        }
    }

    fn supports_model(&self, model: &str) -> bool {
        model.to_lowercase().contains("baichuan")
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.api_key.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_implemented(
            PROVIDER_NAME,
            "Chat completion not yet implemented",
        ))
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        Err(ProviderError::not_implemented(
            PROVIDER_NAME,
            "Streaming not yet implemented",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_supported(PROVIDER_NAME, "Embeddings"))
    }

    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        Err(ProviderError::not_supported(
            PROVIDER_NAME,
            "Image generation",
        ))
    }
}

impl ProviderConfig for BaichuanConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_none() {
            return Err("Baichuan AI API key is required".to_string());
        }
        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.api_base.as_deref()
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = BaichuanConfig::default();
        assert_eq!(config.api_base.unwrap(), "https://api.baichuan-ai.com/v1");
        assert_eq!(config.timeout, 60);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_config_validation() {
        let mut config = BaichuanConfig::default();
        assert!(config.validate().is_err());

        config.api_key = Some("test-key".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = BaichuanConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let provider = BaichuanProvider::new(config).unwrap();
        assert_eq!(provider.name(), "baichuan");
    }

    #[test]
    fn test_supports_model() {
        let config = BaichuanConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let provider = BaichuanProvider::new(config).unwrap();

        assert!(provider.supports_model("Baichuan2-Turbo"));
        assert!(provider.supports_model("baichuan2-53b"));
        assert!(!provider.supports_model("gpt-4"));
    }

    #[test]
    fn test_model_info() {
        let models = BaichuanProvider::get_models();
        assert_eq!(models.len(), 3);

        let turbo = models.iter().find(|m| m.id == "Baichuan2-Turbo").unwrap();
        assert_eq!(turbo.max_context_length, 32000);
        assert_eq!(turbo.currency, "CNY");

        let turbo_192k = models
            .iter()
            .find(|m| m.id == "Baichuan2-Turbo-192k")
            .unwrap();
        assert_eq!(turbo_192k.max_context_length, 192000);

        let model_53b = models.iter().find(|m| m.id == "Baichuan2-53B").unwrap();
        assert_eq!(model_53b.max_context_length, 4096);
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let config = BaichuanConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let provider = BaichuanProvider::new(config).unwrap();

        // Test Baichuan2-Turbo cost calculation
        let cost = provider
            .calculate_cost("Baichuan2-Turbo", 1000, 1000)
            .await
            .unwrap();
        assert_eq!(cost, 0.016); // (0.008 * 1) + (0.008 * 1)

        // Test Baichuan2-Turbo-192k cost calculation
        let cost = provider
            .calculate_cost("Baichuan2-Turbo-192k", 1000, 1000)
            .await
            .unwrap();
        assert_eq!(cost, 0.032); // (0.016 * 1) + (0.016 * 1)
    }

    #[test]
    fn test_error_mapper() {
        let mapper = BaichuanErrorMapper;

        let error = mapper.map_http_error(401, "Unauthorized");
        assert!(matches!(error, ProviderError::Authentication { .. }));

        let error = mapper.map_http_error(404, "Not found");
        assert!(matches!(error, ProviderError::ModelNotFound { .. }));

        let error = mapper.map_http_error(429, "Too many requests");
        assert!(matches!(error, ProviderError::RateLimit { .. }));

        let error = mapper.map_http_error(500, "Internal error");
        assert!(matches!(error, ProviderError::ApiError { .. }));
    }
}
