//! Qwen Provider
//!
//! Qwen (Alibaba Tongyi Qianwen) platform integration

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

/// Provider name constant
pub const PROVIDER_NAME: &str = "qwen";

/// Default API base URL
pub const DEFAULT_API_BASE: &str = "https://dashscope.aliyuncs.com/api/v1";

/// Qwen configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenConfig {
    /// API key for Qwen
    pub api_key: Option<String>,
    /// API base URL (default: <https://dashscope.aliyuncs.com/api/v1>)
    pub api_base: Option<String>,
    /// Timeout in seconds
    pub timeout: u64,
    /// Max retries
    pub max_retries: u32,
}

impl Default for QwenConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: Some(DEFAULT_API_BASE.to_string()),
            timeout: 60,
            max_retries: 3,
        }
    }
}

impl QwenConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let api_key = std::env::var("QWEN_API_KEY")
            .or_else(|_| std::env::var("DASHSCOPE_API_KEY"))
            .ok();

        let api_base =
            std::env::var("QWEN_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.to_string());

        Ok(Self {
            api_key,
            api_base: Some(api_base),
            timeout: 60,
            max_retries: 3,
        })
    }

    /// Get effective API base URL
    pub fn get_effective_api_base(&self) -> &str {
        self.api_base.as_deref().unwrap_or(DEFAULT_API_BASE)
    }
}

/// Qwen provider
#[derive(Debug, Clone)]
pub struct QwenProvider {
    config: QwenConfig,
    models: Vec<ModelInfo>,
}

impl QwenProvider {
    /// Create new Qwen provider
    pub fn new(config: QwenConfig) -> Result<Self, ProviderError> {
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

        let models = Self::build_models();

        Ok(Self { config, models })
    }

    fn build_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "qwen-turbo".to_string(),
                name: "Qwen Turbo".to_string(),
                provider: PROVIDER_NAME.to_string(),
                max_context_length: 8192,
                max_output_length: Some(2048),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0008),
                output_cost_per_1k_tokens: Some(0.002),
                currency: "CNY".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ],
                ..Default::default()
            },
            ModelInfo {
                id: "qwen-plus".to_string(),
                name: "Qwen Plus".to_string(),
                provider: PROVIDER_NAME.to_string(),
                max_context_length: 32768,
                max_output_length: Some(2048),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.004),
                output_cost_per_1k_tokens: Some(0.012),
                currency: "CNY".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ],
                ..Default::default()
            },
            ModelInfo {
                id: "qwen-max".to_string(),
                name: "Qwen Max".to_string(),
                provider: PROVIDER_NAME.to_string(),
                max_context_length: 8192,
                max_output_length: Some(2048),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.04),
                output_cost_per_1k_tokens: Some(0.12),
                currency: "CNY".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ],
                ..Default::default()
            },
            ModelInfo {
                id: "qwen-max-longcontext".to_string(),
                name: "Qwen Max Long Context".to_string(),
                provider: PROVIDER_NAME.to_string(),
                max_context_length: 30000,
                max_output_length: Some(2048),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.04),
                output_cost_per_1k_tokens: Some(0.12),
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

/// Qwen error mapper
#[derive(Debug)]
pub struct QwenErrorMapper;

impl ErrorMapper<ProviderError> for QwenErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
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
impl LLMProvider for QwenProvider {
    type Config = QwenConfig;
    type Error = ProviderError;
    type ErrorMapper = QwenErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
        ];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["temperature", "max_tokens", "top_p", "stream", "stop"]
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

        if let Some(stop) = request.stop {
            body["stop"] = json!(stop);
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
        QwenErrorMapper
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        let model_info = self
            .models
            .iter()
            .find(|m| m.id == model)
            .ok_or_else(|| ProviderError::model_not_found(PROVIDER_NAME, model.to_string()))?;

        let input_cost =
            model_info.input_cost_per_1k_tokens.unwrap_or(0.0) * input_tokens as f64 / 1000.0;
        let output_cost =
            model_info.output_cost_per_1k_tokens.unwrap_or(0.0) * output_tokens as f64 / 1000.0;

        Ok(input_cost + output_cost)
    }

    fn supports_model(&self, model: &str) -> bool {
        model.contains("qwen") || model.contains("tongyi")
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
        Err(ProviderError::not_implemented(
            PROVIDER_NAME,
            "Embeddings not yet implemented",
        ))
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

impl ProviderConfig for QwenConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_none() {
            return Err("Qwen API key is required".to_string());
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
    fn test_default_config() {
        let config = QwenConfig::default();
        assert_eq!(config.api_base, Some(DEFAULT_API_BASE.to_string()));
        assert_eq!(config.timeout, 60);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_config_validation() {
        let mut config = QwenConfig::default();

        // Should fail without API key
        assert!(config.validate().is_err());

        // Should pass with API key
        config.api_key = Some("test-key".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_model_support() {
        let config = QwenConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let provider = QwenProvider::new(config).unwrap();

        assert!(provider.supports_model("qwen-turbo"));
        assert!(provider.supports_model("qwen-plus"));
        assert!(provider.supports_model("qwen-max"));
        assert!(provider.supports_model("qwen-max-longcontext"));
        assert!(!provider.supports_model("gpt-4"));
    }

    #[test]
    fn test_models_list() {
        let config = QwenConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let provider = QwenProvider::new(config).unwrap();

        let models = provider.models();
        assert_eq!(models.len(), 4);
        assert_eq!(models[0].id, "qwen-turbo");
        assert_eq!(models[1].id, "qwen-plus");
        assert_eq!(models[2].id, "qwen-max");
        assert_eq!(models[3].id, "qwen-max-longcontext");
    }

    #[test]
    fn test_capabilities() {
        let config = QwenConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let provider = QwenProvider::new(config).unwrap();

        let capabilities = provider.capabilities();
        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::Embeddings));
    }

    #[test]
    fn test_provider_name() {
        let config = QwenConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let provider = QwenProvider::new(config).unwrap();

        assert_eq!(provider.name(), PROVIDER_NAME);
    }
}
