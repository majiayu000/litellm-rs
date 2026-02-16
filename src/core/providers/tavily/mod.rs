//! Tavily Provider
//!
//! Tavily AI search API integration

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

/// Tavily configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TavilyConfig {
    /// API key for Tavily
    pub api_key: Option<String>,
    /// API base URL (default: <https://api.tavily.com>)
    pub api_base: Option<String>,
    /// Timeout in seconds
    pub timeout: u64,
    /// Max retries
    pub max_retries: u32,
}

impl Default for TavilyConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: Some("https://api.tavily.com".to_string()),
            timeout: 30,
            max_retries: 3,
        }
    }
}

impl TavilyConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self, TavilyError> {
        let api_key = std::env::var("TAVILY_API_KEY").ok();

        let api_base = std::env::var("TAVILY_API_BASE")
            .unwrap_or_else(|_| "https://api.tavily.com".to_string());

        Ok(Self {
            api_key,
            api_base: Some(api_base),
            timeout: 30,
            max_retries: 3,
        })
    }

    /// Get effective API base URL
    pub fn get_effective_api_base(&self) -> &str {
        self.api_base.as_deref().unwrap_or("https://api.tavily.com")
    }
}

/// Tavily error type (alias to unified ProviderError)
pub type TavilyError = ProviderError;

/// Tavily provider
#[derive(Debug, Clone)]
pub struct TavilyProvider {
    config: TavilyConfig,
}

impl TavilyProvider {
    /// Create new Tavily provider
    pub fn new(config: TavilyConfig) -> Result<Self, TavilyError> {
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
            .map_err(|e| ProviderError::configuration("tavily", e.to_string()))?;

        Ok(Self { config })
    }
}

/// Tavily error mapper
#[derive(Debug)]
pub struct TavilyErrorMapper;

impl ErrorMapper<TavilyError> for TavilyErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> TavilyError {
        match status_code {
            401 => ProviderError::authentication(
                "tavily",
                format!("Invalid API key: {}", response_body),
            ),
            403 => ProviderError::authentication(
                "tavily",
                format!("Permission denied: {}", response_body),
            ),
            404 => ProviderError::model_not_found(
                "tavily",
                format!("Endpoint not found: {}", response_body),
            ),
            429 => ProviderError::rate_limit("tavily", None),
            500..=599 => ProviderError::api_error(
                "tavily",
                status_code,
                format!("Server error: {}", response_body),
            ),
            _ => ProviderError::api_error(
                "tavily",
                status_code,
                format!("HTTP {}: {}", status_code, response_body),
            ),
        }
    }
}

#[async_trait]
impl LLMProvider for TavilyProvider {
    type Config = TavilyConfig;
    type Error = TavilyError;
    type ErrorMapper = TavilyErrorMapper;

    fn name(&self) -> &'static str {
        "tavily"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &[]
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["query", "search_depth", "max_results"]
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
        Ok(json!({ "model": request.model }))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_supported("tavily", "Chat completion"))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        TavilyErrorMapper
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        Ok(0.0)
    }

    fn supports_model(&self, _model: &str) -> bool {
        false
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
        Err(ProviderError::not_supported("tavily", "Chat completion"))
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        Err(ProviderError::not_supported("tavily", "Streaming"))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_supported("tavily", "Embeddings"))
    }

    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        Err(ProviderError::not_supported("tavily", "Image generation"))
    }
}

impl ProviderConfig for TavilyConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_none() {
            return Err("Tavily API key is required".to_string());
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
