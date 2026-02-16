//! Minimax AI Provider
//!
//! Minimax provides an OpenAI-compatible API with support for their MiniMax-M2 series models.
//! - International: <https://api.minimax.io/v1>
//! - China: <https://api.minimaxi.com/v1>

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

use crate::core::providers::base_provider::{
    BaseHttpClient, BaseProviderConfig, CostCalculator, HeaderBuilder, HttpErrorMapper,
    OpenAIRequestTransformer, UrlBuilder,
};
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
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse},
};

// Re-export submodules
pub mod chat;

// Static capabilities
const MINIMAX_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::FunctionCalling,
];

/// Minimax provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimaxConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://api.minimax.io/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for MinimaxConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.minimax.io/v1".to_string(),
            timeout_seconds: 60,
            max_retries: 3,
        }
    }
}

impl ProviderConfig for MinimaxConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("Minimax API key is required".to_string());
        }
        if self.timeout_seconds == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }
        if self.max_retries > 10 {
            return Err("Max retries should not exceed 10".to_string());
        }
        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        Some(&self.api_key)
    }

    fn api_base(&self) -> Option<&str> {
        Some(&self.api_base)
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_seconds)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// Minimax error type (simplified using ProviderError)
pub type MinimaxError = ProviderError;

/// Minimax error mapper
pub struct MinimaxErrorMapper;

impl ErrorMapper<MinimaxError> for MinimaxErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> MinimaxError {
        HttpErrorMapper::map_status_code("minimax", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> MinimaxError {
        HttpErrorMapper::parse_json_error("minimax", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> MinimaxError {
        ProviderError::network("minimax", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> MinimaxError {
        ProviderError::response_parsing("minimax", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> MinimaxError {
        ProviderError::timeout(
            "minimax",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Minimax provider implementation
#[derive(Debug, Clone)]
pub struct MinimaxProvider {
    config: MinimaxConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl MinimaxProvider {
    /// Create a new Minimax provider instance
    pub async fn new(config: MinimaxConfig) -> Result<Self, MinimaxError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("minimax", e))?;

        // Create base HTTP client using our infrastructure
        let base_config = BaseProviderConfig {
            api_key: Some(config.api_key.clone()),
            api_base: Some(config.api_base.clone()),
            timeout: Some(config.timeout_seconds),
            max_retries: Some(config.max_retries),
            headers: None,
            organization: None,
            api_version: None,
        };

        let base_client = BaseHttpClient::new(base_config)?;

        // Define supported models with pricing (USD per 1k tokens)
        let models = vec![
            ModelInfo {
                id: "MiniMax-M2.1".to_string(),
                name: "MiniMax M2.1".to_string(),
                provider: "minimax".to_string(),
                max_context_length: 1000000,
                max_output_length: Some(16384),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.001),
                output_cost_per_1k_tokens: Some(0.004),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "MiniMax-M2.1-lightning".to_string(),
                name: "MiniMax M2.1 Lightning".to_string(),
                provider: "minimax".to_string(),
                max_context_length: 1000000,
                max_output_length: Some(16384),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.0005),
                output_cost_per_1k_tokens: Some(0.002),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "MiniMax-M2".to_string(),
                name: "MiniMax M2".to_string(),
                provider: "minimax".to_string(),
                max_context_length: 256000,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0008),
                output_cost_per_1k_tokens: Some(0.003),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
        ];

        Ok(Self {
            config,
            base_client,
            models,
        })
    }

    /// Build the complete URL for the chat completions endpoint
    fn build_chat_url(&self) -> String {
        let base = &self.config.api_base;
        if base.ends_with("/chat/completions") {
            base.clone()
        } else if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else if base.ends_with('/') {
            format!("{}v1/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        }
    }
}

#[async_trait]
impl LLMProvider for MinimaxProvider {
    type Config = MinimaxConfig;
    type Error = MinimaxError;
    type ErrorMapper = MinimaxErrorMapper;

    fn name(&self) -> &'static str {
        "minimax"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        MINIMAX_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Minimax chat request: model={}", request.model);

        // Transform request
        let body = self.transform_request(request, context).await?;

        // Build URL based on config
        let url = self.build_chat_url();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("minimax", e.to_string()))?;

        let response = self
            .base_client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("minimax", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("minimax", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("minimax", e.to_string()))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Minimax streaming chat request: model={}", request.model);

        // Transform request
        let mut body = self.transform_request(request, context).await?;
        body["stream"] = serde_json::json!(true);

        // Build URL based on config
        let url = self.build_chat_url();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("minimax", e.to_string()))?;

        let response = self
            .base_client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("minimax", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("minimax", status, body));
        }

        // Parse SSE stream using shared infrastructure
        use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEParser};
        use futures::StreamExt;

        let transformer = OpenAICompatibleTransformer::new("minimax");
        let parser = UnifiedSSEParser::new(transformer);

        // Convert response bytes to stream of ChatChunks
        let byte_stream = response.bytes_stream();
        let stream = byte_stream
            .scan((parser, Vec::new()), |(parser, buffer), bytes_result| {
                futures::future::ready(match bytes_result {
                    Ok(bytes) => match parser.process_bytes(&bytes) {
                        Ok(chunks) => {
                            *buffer = chunks;
                            Some(Ok(buffer.clone()))
                        }
                        Err(e) => Some(Err(e)),
                    },
                    Err(e) => Some(Err(ProviderError::network("minimax", e.to_string()))),
                })
            })
            .map(|result| match result {
                Ok(chunks) => chunks.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            })
            .flat_map(futures::stream::iter);

        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_supported("minimax", "embeddings"))
    }

    async fn health_check(&self) -> HealthStatus {
        // Try a simple models endpoint request
        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/models")
            .build();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .build_reqwest();

        match headers {
            Ok(headers) => {
                match self
                    .base_client
                    .inner()
                    .get(&url)
                    .headers(headers)
                    .send()
                    .await
                {
                    Ok(response) if response.status().is_success() => HealthStatus::Healthy,
                    Ok(response) => {
                        debug!("Minimax health check failed: status={}", response.status());
                        HealthStatus::Unhealthy
                    }
                    Err(e) => {
                        debug!("Minimax health check error: {}", e);
                        HealthStatus::Unhealthy
                    }
                }
            }
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "top_p",
            "max_tokens",
            "stream",
            "stop",
            "presence_penalty",
            "frequency_penalty",
            "n",
            "user",
            "tools",
            "tool_choice",
            "reasoning_split", // Minimax-specific parameter
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Minimax is OpenAI-compatible, pass-through most parameters
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        // Use the OpenAI transformer from base_provider
        Ok(OpenAIRequestTransformer::transform_chat_request(&request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        // Parse Minimax response (OpenAI-compatible format)
        serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("minimax", e.to_string()))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        MinimaxErrorMapper
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // Find model pricing
        let model_info = self
            .models
            .iter()
            .find(|m| m.id == model)
            .ok_or_else(|| ProviderError::model_not_found("minimax", model.to_string()))?;

        let input_cost_per_1k = model_info.input_cost_per_1k_tokens.unwrap_or(0.0);
        let output_cost_per_1k = model_info.output_cost_per_1k_tokens.unwrap_or(0.0);

        Ok(CostCalculator::calculate(
            input_tokens,
            output_tokens,
            input_cost_per_1k,
            output_cost_per_1k,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    fn create_test_config() -> MinimaxConfig {
        MinimaxConfig {
            api_key: "test_api_key".to_string(),
            ..Default::default()
        }
    }

    // ==================== Provider Creation Tests ====================

    #[tokio::test]
    async fn test_minimax_provider_creation() {
        let config = MinimaxConfig {
            api_key: "test_key".to_string(),
            ..Default::default()
        };

        let provider = MinimaxProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "minimax");
        assert!(
            provider
                .capabilities()
                .contains(&ProviderCapability::ChatCompletionStream)
        );
    }

    #[tokio::test]
    async fn test_minimax_provider_creation_custom_base() {
        let config = MinimaxConfig {
            api_key: "test_key".to_string(),
            api_base: "https://api.minimaxi.com/v1".to_string(),
            ..Default::default()
        };

        let provider = MinimaxProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_minimax_provider_creation_no_api_key() {
        let config = MinimaxConfig::default();
        let provider = MinimaxProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_minimax_provider_creation_empty_api_key() {
        let config = MinimaxConfig {
            api_key: "".to_string(),
            ..Default::default()
        };

        let provider = MinimaxProvider::new(config).await;
        assert!(provider.is_err());
    }

    // ==================== Config Validation Tests ====================

    #[test]
    fn test_minimax_config_validation() {
        let mut config = MinimaxConfig::default();
        assert!(config.validate().is_err()); // No API key

        config.api_key = "test_key".to_string();
        assert!(config.validate().is_ok());

        config.timeout_seconds = 0;
        assert!(config.validate().is_err()); // Invalid timeout

        config.timeout_seconds = 60;
        config.max_retries = 11;
        assert!(config.validate().is_err()); // Too many retries
    }

    #[test]
    fn test_minimax_config_default() {
        let config = MinimaxConfig::default();

        assert_eq!(config.api_key, "");
        assert_eq!(config.api_base, "https://api.minimax.io/v1");
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_minimax_config_provider_config_trait() {
        let config = create_test_config();

        assert_eq!(config.api_key(), Some("test_api_key"));
        assert_eq!(config.api_base(), Some("https://api.minimax.io/v1"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
        assert_eq!(config.max_retries(), 3);
    }

    #[test]
    fn test_minimax_config_validation_max_retries_boundary() {
        let mut config = create_test_config();

        config.max_retries = 10;
        assert!(config.validate().is_ok());

        config.max_retries = 11;
        assert!(config.validate().is_err());
    }

    // ==================== Provider Capabilities Tests ====================

    #[tokio::test]
    async fn test_provider_name() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        assert_eq!(provider.name(), "minimax");
    }

    #[tokio::test]
    async fn test_provider_capabilities() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::FunctionCalling));
        assert_eq!(caps.len(), 3);
    }

    #[tokio::test]
    async fn test_provider_models() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "MiniMax-M2.1"));
        assert!(models.iter().any(|m| m.id == "MiniMax-M2.1-lightning"));
        assert!(models.iter().any(|m| m.id == "MiniMax-M2"));
    }

    #[tokio::test]
    async fn test_provider_models_have_pricing() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        for model in models {
            assert_eq!(model.provider, "minimax");
            assert_eq!(model.currency, "USD");
            assert!(model.input_cost_per_1k_tokens.is_some());
            assert!(model.output_cost_per_1k_tokens.is_some());
        }
    }

    #[tokio::test]
    async fn test_provider_models_context_lengths() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        let model_m21 = models.iter().find(|m| m.id == "MiniMax-M2.1").unwrap();
        assert_eq!(model_m21.max_context_length, 1000000);

        let model_m2 = models.iter().find(|m| m.id == "MiniMax-M2").unwrap();
        assert_eq!(model_m2.max_context_length, 256000);
    }

    // ==================== URL Building Tests ====================

    #[tokio::test]
    async fn test_build_chat_url_default() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let url = provider.build_chat_url();
        assert_eq!(url, "https://api.minimax.io/v1/chat/completions");
    }

    #[tokio::test]
    async fn test_build_chat_url_custom_base_with_v1() {
        let config = MinimaxConfig {
            api_key: "test_key".to_string(),
            api_base: "https://api.minimaxi.com/v1".to_string(),
            ..Default::default()
        };
        let provider = MinimaxProvider::new(config).await.unwrap();
        let url = provider.build_chat_url();
        assert_eq!(url, "https://api.minimaxi.com/v1/chat/completions");
    }

    #[tokio::test]
    async fn test_build_chat_url_with_trailing_slash() {
        let config = MinimaxConfig {
            api_key: "test_key".to_string(),
            api_base: "https://api.minimax.io/".to_string(),
            ..Default::default()
        };
        let provider = MinimaxProvider::new(config).await.unwrap();
        let url = provider.build_chat_url();
        assert_eq!(url, "https://api.minimax.io/v1/chat/completions");
    }

    #[tokio::test]
    async fn test_build_chat_url_already_complete() {
        let config = MinimaxConfig {
            api_key: "test_key".to_string(),
            api_base: "https://api.minimax.io/v1/chat/completions".to_string(),
            ..Default::default()
        };
        let provider = MinimaxProvider::new(config).await.unwrap();
        let url = provider.build_chat_url();
        assert_eq!(url, "https://api.minimax.io/v1/chat/completions");
    }

    // ==================== Supported Params Tests ====================

    #[tokio::test]
    async fn test_get_supported_openai_params() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let params = provider.get_supported_openai_params("MiniMax-M2.1");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"top_p"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"stream"));
        assert!(params.contains(&"stop"));
        assert!(params.contains(&"presence_penalty"));
        assert!(params.contains(&"frequency_penalty"));
        assert!(params.contains(&"n"));
        assert!(params.contains(&"user"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"tool_choice"));
        assert!(params.contains(&"reasoning_split")); // Minimax-specific
    }

    // ==================== Map OpenAI Params Tests ====================

    #[tokio::test]
    async fn test_map_openai_params_passthrough() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.7));
        params.insert("max_tokens".to_string(), serde_json::json!(100));
        params.insert("top_p".to_string(), serde_json::json!(0.9));

        let mapped = provider
            .map_openai_params(params.clone(), "MiniMax-M2.1")
            .await
            .unwrap();

        // Minimax is OpenAI-compatible, should pass through
        assert_eq!(mapped, params);
    }

    // ==================== Transform Request Tests ====================

    #[tokio::test]
    async fn test_transform_request_basic() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "MiniMax-M2.1".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.transform_request(request, context).await;

        assert!(result.is_ok());
        let transformed = result.unwrap();
        assert_eq!(transformed["model"], "MiniMax-M2.1");
        assert!(transformed["messages"].is_array());
    }

    #[tokio::test]
    async fn test_transform_request_with_temperature() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "MiniMax-M2.1".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            temperature: Some(0.7),
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.transform_request(request, context).await;

        assert!(result.is_ok());
        let transformed = result.unwrap();
        assert!(transformed.get("temperature").is_some());
    }

    // ==================== Embeddings Not Supported Test ====================

    #[tokio::test]
    async fn test_embeddings_not_supported() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();

        let request = crate::core::types::embedding::EmbeddingRequest {
            model: "MiniMax-M2.1".to_string(),
            input: crate::core::types::embedding::EmbeddingInput::Text("test".to_string()),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        let context = RequestContext::default();
        let result = provider.embeddings(request, context).await;

        assert!(result.is_err());
    }

    // ==================== Cost Calculation Tests ====================

    #[tokio::test]
    async fn test_calculate_cost_m21_model() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("MiniMax-M2.1", 1000, 500).await;
        assert!(cost.is_ok());

        let cost_value = cost.unwrap();
        // MiniMax-M2.1: 0.001 USD input, 0.004 USD output per 1k
        // (1000/1000 * 0.001) + (500/1000 * 0.004) = 0.001 + 0.002 = 0.003
        assert!((cost_value - 0.003).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_calculate_cost_m21_lightning_model() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();

        let cost = provider
            .calculate_cost("MiniMax-M2.1-lightning", 1000, 500)
            .await;
        assert!(cost.is_ok());

        let cost_value = cost.unwrap();
        // MiniMax-M2.1-lightning: 0.0005 USD input, 0.002 USD output per 1k
        // (1000/1000 * 0.0005) + (500/1000 * 0.002) = 0.0005 + 0.001 = 0.0015
        assert!((cost_value - 0.0015).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("unknown-model", 1000, 500).await;
        assert!(cost.is_err());
    }

    #[tokio::test]
    async fn test_calculate_cost_zero_tokens() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("MiniMax-M2.1", 0, 0).await;
        assert!(cost.is_ok());
        assert!((cost.unwrap() - 0.0).abs() < 0.0001);
    }

    // ==================== Error Mapper Tests ====================

    #[test]
    fn test_error_mapper_authentication() {
        let mapper = MinimaxErrorMapper;
        let error = mapper.map_http_error(401, "Unauthorized");

        match error {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "minimax");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_error_mapper_rate_limit() {
        let mapper = MinimaxErrorMapper;
        let error = mapper.map_http_error(429, "Rate limit exceeded");

        match error {
            ProviderError::RateLimit { provider, .. } => {
                assert_eq!(provider, "minimax");
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_error_mapper_network_error() {
        let mapper = MinimaxErrorMapper;
        let error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let mapped = mapper.map_network_error(&error);

        match mapped {
            ProviderError::Network { provider, .. } => {
                assert_eq!(provider, "minimax");
            }
            _ => panic!("Expected Network error"),
        }
    }

    #[test]
    fn test_error_mapper_parsing_error() {
        let mapper = MinimaxErrorMapper;
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid JSON");
        let mapped = mapper.map_parsing_error(&error);

        match mapped {
            ProviderError::ResponseParsing { provider, .. } => {
                assert_eq!(provider, "minimax");
            }
            _ => panic!("Expected ResponseParsing error"),
        }
    }

    #[test]
    fn test_error_mapper_timeout_error() {
        let mapper = MinimaxErrorMapper;
        let mapped = mapper.map_timeout_error(std::time::Duration::from_secs(60));

        match mapped {
            ProviderError::Timeout { provider, .. } => {
                assert_eq!(provider, "minimax");
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    // ==================== Get Error Mapper Tests ====================

    #[tokio::test]
    async fn test_get_error_mapper() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let _mapper = provider.get_error_mapper();
        // Just verify it doesn't panic
    }

    // ==================== Clone/Debug Tests ====================

    #[tokio::test]
    async fn test_provider_clone() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let cloned = provider.clone();

        assert_eq!(provider.name(), cloned.name());
        assert_eq!(provider.models().len(), cloned.models().len());
    }

    #[tokio::test]
    async fn test_provider_debug() {
        let provider = MinimaxProvider::new(create_test_config()).await.unwrap();
        let debug_str = format!("{:?}", provider);

        assert!(debug_str.contains("MinimaxProvider"));
    }

    #[test]
    fn test_config_clone() {
        let config = create_test_config();
        let cloned = config.clone();

        assert_eq!(config.api_key, cloned.api_key);
        assert_eq!(config.api_base, cloned.api_base);
    }

    #[test]
    fn test_config_debug() {
        let config = create_test_config();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("MinimaxConfig"));
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["api_key"], "test_api_key");
        assert_eq!(json["api_base"], "https://api.minimax.io/v1");
        assert_eq!(json["timeout_seconds"], 60);
        assert_eq!(json["max_retries"], 3);
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "api_key": "my_key",
            "api_base": "https://custom.api.com",
            "timeout_seconds": 120,
            "max_retries": 5
        }"#;

        let config: MinimaxConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, "my_key");
        assert_eq!(config.api_base, "https://custom.api.com");
        assert_eq!(config.timeout_seconds, 120);
        assert_eq!(config.max_retries, 5);
    }

    // ==================== Static Capabilities Constant Tests ====================

    #[test]
    fn test_minimax_capabilities_constant() {
        assert!(MINIMAX_CAPABILITIES.contains(&ProviderCapability::ChatCompletion));
        assert!(MINIMAX_CAPABILITIES.contains(&ProviderCapability::ChatCompletionStream));
        assert!(MINIMAX_CAPABILITIES.contains(&ProviderCapability::FunctionCalling));
        assert_eq!(MINIMAX_CAPABILITIES.len(), 3);
    }
}
