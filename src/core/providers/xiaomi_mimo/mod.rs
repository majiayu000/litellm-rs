//! Xiaomi MIMO Provider
//!
//! Xiaomi MIMO AI platform integration

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

/// Xiaomi MIMO configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XiaomiMIMOConfig {
    /// API key for Xiaomi MIMO
    pub api_key: Option<String>,
    /// API base URL (default: <https://api.xiaomi.com/mimo>)
    pub api_base: Option<String>,
    /// Timeout in seconds
    pub timeout: u64,
    /// Max retries
    pub max_retries: u32,
}

impl Default for XiaomiMIMOConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: Some("https://api.xiaomi.com/mimo".to_string()),
            timeout: 60,
            max_retries: 3,
        }
    }
}

impl XiaomiMIMOConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self, XiaomiMIMOError> {
        let api_key = std::env::var("XIAOMI_MIMO_API_KEY")
            .or_else(|_| std::env::var("XIAOMI_API_KEY"))
            .ok();

        let api_base = std::env::var("XIAOMI_MIMO_API_BASE")
            .unwrap_or_else(|_| "https://api.xiaomi.com/mimo".to_string());

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
            .unwrap_or("https://api.xiaomi.com/mimo")
    }
}

/// Xiaomi MIMO error type (alias to unified ProviderError)
pub type XiaomiMIMOError = ProviderError;

/// Xiaomi MIMO provider
#[derive(Debug, Clone)]
pub struct XiaomiMIMOProvider {
    config: XiaomiMIMOConfig,
}

impl XiaomiMIMOProvider {
    /// Create new Xiaomi MIMO provider
    pub fn new(config: XiaomiMIMOConfig) -> Result<Self, XiaomiMIMOError> {
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
            .map_err(|e| ProviderError::configuration("xiaomi_mimo", e.to_string()))?;

        Ok(Self { config })
    }
}

/// Xiaomi MIMO error mapper
#[derive(Debug)]
pub struct XiaomiMIMOErrorMapper;

impl ErrorMapper<XiaomiMIMOError> for XiaomiMIMOErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> XiaomiMIMOError {
        match status_code {
            401 => ProviderError::authentication(
                "xiaomi_mimo",
                format!("Invalid API key: {}", response_body),
            ),
            403 => ProviderError::authentication(
                "xiaomi_mimo",
                format!("Permission denied: {}", response_body),
            ),
            404 => ProviderError::model_not_found(
                "xiaomi_mimo",
                format!("Model not found: {}", response_body),
            ),
            429 => ProviderError::rate_limit("xiaomi_mimo", None),
            500..=599 => ProviderError::api_error(
                "xiaomi_mimo",
                status_code,
                format!("Server error: {}", response_body),
            ),
            _ => ProviderError::api_error(
                "xiaomi_mimo",
                status_code,
                format!("HTTP {}: {}", status_code, response_body),
            ),
        }
    }
}

#[async_trait]
impl LLMProvider for XiaomiMIMOProvider {
    type Config = XiaomiMIMOConfig;
    type Error = XiaomiMIMOError;
    type ErrorMapper = XiaomiMIMOErrorMapper;

    fn name(&self) -> &'static str {
        "xiaomi_mimo"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &[]
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["temperature", "max_tokens", "top_p", "stream"]
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
            "xiaomi_mimo",
            "Response transformation not yet implemented",
        ))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        XiaomiMIMOErrorMapper
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        Ok(0.0)
    }

    fn supports_model(&self, model: &str) -> bool {
        model.contains("mimo") || model.contains("xiaomi")
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
            "xiaomi_mimo",
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
            "xiaomi_mimo",
            "Streaming not yet implemented",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_supported("xiaomi_mimo", "Embeddings"))
    }

    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        Err(ProviderError::not_supported(
            "xiaomi_mimo",
            "Image generation",
        ))
    }
}

impl ProviderConfig for XiaomiMIMOConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_none() {
            return Err("Xiaomi MIMO API key is required".to_string());
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
