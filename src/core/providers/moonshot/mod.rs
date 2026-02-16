//! Moonshot AI Provider (Refactored)
//!
//! Moonshot (Dark Side of the Moon) AI model integration using the base infrastructure.
//! This implementation eliminates the need for common_utils.rs.

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

// Use base infrastructure instead of common_utils
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
const MOONSHOT_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::FunctionCalling,
];

/// Moonshot provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoonshotConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://api.moonshot.cn/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for MoonshotConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.moonshot.cn/v1".to_string(),
            timeout_seconds: 60, // Longer timeout for large context
            max_retries: 3,
        }
    }
}

impl ProviderConfig for MoonshotConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("Moonshot API key is required".to_string());
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

/// Moonshot error type (simplified using ProviderError)
pub type MoonshotError = ProviderError;

/// Moonshot error mapper
pub struct MoonshotErrorMapper;

impl ErrorMapper<MoonshotError> for MoonshotErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> MoonshotError {
        HttpErrorMapper::map_status_code("moonshot", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> MoonshotError {
        HttpErrorMapper::parse_json_error("moonshot", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> MoonshotError {
        ProviderError::network("moonshot", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> MoonshotError {
        ProviderError::response_parsing("moonshot", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> MoonshotError {
        ProviderError::timeout(
            "moonshot",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Moonshot provider implementation (refactored)
#[derive(Debug, Clone)]
pub struct MoonshotProvider {
    config: MoonshotConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl MoonshotProvider {
    /// Create a new Moonshot provider instance
    pub async fn new(config: MoonshotConfig) -> Result<Self, MoonshotError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("moonshot", e))?;

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

        // Define supported models with pricing
        let models = vec![
            ModelInfo {
                id: "moonshot-v1-8k".to_string(),
                name: "Moonshot V1 8K".to_string(),
                provider: "moonshot".to_string(),
                max_context_length: 8000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.01),
                output_cost_per_1k_tokens: Some(0.02),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "moonshot-v1-32k".to_string(),
                name: "Moonshot V1 32K".to_string(),
                provider: "moonshot".to_string(),
                max_context_length: 32000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.02),
                output_cost_per_1k_tokens: Some(0.04),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "moonshot-v1-128k".to_string(),
                name: "Moonshot V1 128K".to_string(),
                provider: "moonshot".to_string(),
                max_context_length: 128000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.03),
                output_cost_per_1k_tokens: Some(0.06),
                currency: "CNY".to_string(),
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
}

#[async_trait]
impl LLMProvider for MoonshotProvider {
    type Config = MoonshotConfig;
    type Error = MoonshotError;
    type ErrorMapper = MoonshotErrorMapper;

    fn name(&self) -> &'static str {
        "moonshot"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        MOONSHOT_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Moonshot chat request: model={}", request.model);

        // Transform request
        let body = self.transform_request(request, context).await?;

        // Direct HTTP call using BaseHttpClient
        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/chat/completions")
            .build();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("moonshot", e.to_string()))?;

        let response = self
            .base_client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("moonshot", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("moonshot", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("moonshot", e.to_string()))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Moonshot streaming chat request: model={}", request.model);

        // Transform request
        let mut body = self.transform_request(request, context).await?;
        body["stream"] = serde_json::json!(true);

        // Execute streaming request directly
        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/chat/completions")
            .build();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("moonshot", e.to_string()))?;

        let response = self
            .base_client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("moonshot", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("moonshot", status, body));
        }

        // Parse SSE stream using shared infrastructure
        use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEParser};
        use futures::StreamExt;

        let transformer = OpenAICompatibleTransformer::new("moonshot");
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
                    Err(e) => Some(Err(ProviderError::network("moonshot", e.to_string()))),
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
        Err(ProviderError::not_supported("moonshot", "embeddings"))
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
                        debug!("Moonshot health check failed: status={}", response.status());
                        HealthStatus::Unhealthy
                    }
                    Err(e) => {
                        debug!("Moonshot health check error: {}", e);
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
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Moonshot is OpenAI-compatible, pass-through most parameters
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
        // Parse Moonshot response (OpenAI-compatible format)
        serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("moonshot", e.to_string()))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        MoonshotErrorMapper
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
            .ok_or_else(|| ProviderError::model_not_found("moonshot", model.to_string()))?;

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

    fn create_test_config() -> MoonshotConfig {
        MoonshotConfig {
            api_key: "test_api_key".to_string(),
            ..Default::default()
        }
    }

    // ==================== Provider Creation Tests ====================

    #[tokio::test]
    async fn test_moonshot_provider_creation() {
        let config = MoonshotConfig {
            api_key: "test_key".to_string(),
            ..Default::default()
        };

        let provider = MoonshotProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "moonshot");
        assert!(
            provider
                .capabilities()
                .contains(&ProviderCapability::ChatCompletionStream)
        );
    }

    #[tokio::test]
    async fn test_moonshot_provider_creation_custom_base() {
        let config = MoonshotConfig {
            api_key: "test_key".to_string(),
            api_base: "https://custom.moonshot.cn/v1".to_string(),
            ..Default::default()
        };

        let provider = MoonshotProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_moonshot_provider_creation_no_api_key() {
        let config = MoonshotConfig::default();
        let provider = MoonshotProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_moonshot_provider_creation_empty_api_key() {
        let config = MoonshotConfig {
            api_key: "".to_string(),
            ..Default::default()
        };

        let provider = MoonshotProvider::new(config).await;
        assert!(provider.is_err());
    }

    // ==================== Config Validation Tests ====================

    #[test]
    fn test_moonshot_config_validation() {
        let mut config = MoonshotConfig::default();
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
    fn test_moonshot_config_default() {
        let config = MoonshotConfig::default();

        assert_eq!(config.api_key, "");
        assert_eq!(config.api_base, "https://api.moonshot.cn/v1");
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_moonshot_config_provider_config_trait() {
        let config = create_test_config();

        assert_eq!(config.api_key(), Some("test_api_key"));
        assert_eq!(config.api_base(), Some("https://api.moonshot.cn/v1"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
        assert_eq!(config.max_retries(), 3);
    }

    #[test]
    fn test_moonshot_config_validation_max_retries_boundary() {
        let mut config = create_test_config();

        config.max_retries = 10;
        assert!(config.validate().is_ok());

        config.max_retries = 11;
        assert!(config.validate().is_err());
    }

    // ==================== Provider Capabilities Tests ====================

    #[tokio::test]
    async fn test_provider_name() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        assert_eq!(provider.name(), "moonshot");
    }

    #[tokio::test]
    async fn test_provider_capabilities() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::FunctionCalling));
        assert_eq!(caps.len(), 3);
    }

    #[tokio::test]
    async fn test_provider_models() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "moonshot-v1-8k"));
        assert!(models.iter().any(|m| m.id == "moonshot-v1-32k"));
        assert!(models.iter().any(|m| m.id == "moonshot-v1-128k"));
    }

    #[tokio::test]
    async fn test_provider_models_have_pricing() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        for model in models {
            assert_eq!(model.provider, "moonshot");
            assert_eq!(model.currency, "CNY");
            assert!(model.input_cost_per_1k_tokens.is_some());
            assert!(model.output_cost_per_1k_tokens.is_some());
        }
    }

    #[tokio::test]
    async fn test_provider_models_context_lengths() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        let model_8k = models.iter().find(|m| m.id == "moonshot-v1-8k").unwrap();
        assert_eq!(model_8k.max_context_length, 8000);

        let model_32k = models.iter().find(|m| m.id == "moonshot-v1-32k").unwrap();
        assert_eq!(model_32k.max_context_length, 32000);

        let model_128k = models.iter().find(|m| m.id == "moonshot-v1-128k").unwrap();
        assert_eq!(model_128k.max_context_length, 128000);
    }

    // ==================== Supported Params Tests ====================

    #[tokio::test]
    async fn test_get_supported_openai_params() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        let params = provider.get_supported_openai_params("moonshot-v1-8k");

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
    }

    // ==================== Map OpenAI Params Tests ====================

    #[tokio::test]
    async fn test_map_openai_params_passthrough() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.7));
        params.insert("max_tokens".to_string(), serde_json::json!(100));
        params.insert("top_p".to_string(), serde_json::json!(0.9));

        let mapped = provider
            .map_openai_params(params.clone(), "moonshot-v1-8k")
            .await
            .unwrap();

        // Moonshot is OpenAI-compatible, should pass through
        assert_eq!(mapped, params);
    }

    #[tokio::test]
    async fn test_map_openai_params_with_penalties() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("presence_penalty".to_string(), serde_json::json!(0.5));
        params.insert("frequency_penalty".to_string(), serde_json::json!(0.3));

        let mapped = provider
            .map_openai_params(params.clone(), "moonshot-v1-8k")
            .await
            .unwrap();

        assert_eq!(
            mapped.get("presence_penalty").unwrap(),
            &serde_json::json!(0.5)
        );
        assert_eq!(
            mapped.get("frequency_penalty").unwrap(),
            &serde_json::json!(0.3)
        );
    }

    // ==================== Transform Request Tests ====================

    #[tokio::test]
    async fn test_transform_request_basic() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "moonshot-v1-8k".to_string(),
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
        assert_eq!(transformed["model"], "moonshot-v1-8k");
        assert!(transformed["messages"].is_array());
    }

    #[tokio::test]
    async fn test_transform_request_with_temperature() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "moonshot-v1-32k".to_string(),
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

    #[tokio::test]
    async fn test_transform_request_multiple_messages() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "moonshot-v1-8k".to_string(),
            messages: vec![
                ChatMessage {
                    role: MessageRole::System,
                    content: Some(MessageContent::Text(
                        "You are a helpful assistant.".to_string(),
                    )),
                    ..Default::default()
                },
                ChatMessage {
                    role: MessageRole::User,
                    content: Some(MessageContent::Text("Hello".to_string())),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.transform_request(request, context).await;

        assert!(result.is_ok());
        let transformed = result.unwrap();
        let messages = transformed["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
    }

    // ==================== Embeddings Not Supported Test ====================

    #[tokio::test]
    async fn test_embeddings_not_supported() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let request = crate::core::types::embedding::EmbeddingRequest {
            model: "moonshot-v1-8k".to_string(),
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
    async fn test_calculate_cost_8k_model() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("moonshot-v1-8k", 1000, 500).await;
        assert!(cost.is_ok());

        let cost_value = cost.unwrap();
        // moonshot-v1-8k: 0.01 CNY input, 0.02 CNY output per 1k
        // (1000/1000 * 0.01) + (500/1000 * 0.02) = 0.01 + 0.01 = 0.02
        assert!((cost_value - 0.02).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_calculate_cost_32k_model() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("moonshot-v1-32k", 1000, 500).await;
        assert!(cost.is_ok());

        let cost_value = cost.unwrap();
        // moonshot-v1-32k: 0.02 CNY input, 0.04 CNY output per 1k
        // (1000/1000 * 0.02) + (500/1000 * 0.04) = 0.02 + 0.02 = 0.04
        assert!((cost_value - 0.04).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_calculate_cost_128k_model() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("moonshot-v1-128k", 1000, 500).await;
        assert!(cost.is_ok());

        let cost_value = cost.unwrap();
        // moonshot-v1-128k: 0.03 CNY input, 0.06 CNY output per 1k
        // (1000/1000 * 0.03) + (500/1000 * 0.06) = 0.03 + 0.03 = 0.06
        assert!((cost_value - 0.06).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("unknown-model", 1000, 500).await;
        assert!(cost.is_err());
    }

    #[tokio::test]
    async fn test_calculate_cost_zero_tokens() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("moonshot-v1-8k", 0, 0).await;
        assert!(cost.is_ok());
        assert!((cost.unwrap() - 0.0).abs() < 0.0001);
    }

    // ==================== Error Mapper Tests ====================

    #[test]
    fn test_error_mapper_authentication() {
        let mapper = MoonshotErrorMapper;
        let error = mapper.map_http_error(401, "Unauthorized");

        match error {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "moonshot");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_error_mapper_rate_limit() {
        let mapper = MoonshotErrorMapper;
        let error = mapper.map_http_error(429, "Rate limit exceeded");

        match error {
            ProviderError::RateLimit { provider, .. } => {
                assert_eq!(provider, "moonshot");
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_error_mapper_network_error() {
        let mapper = MoonshotErrorMapper;
        let error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let mapped = mapper.map_network_error(&error);

        match mapped {
            ProviderError::Network { provider, .. } => {
                assert_eq!(provider, "moonshot");
            }
            _ => panic!("Expected Network error"),
        }
    }

    #[test]
    fn test_error_mapper_parsing_error() {
        let mapper = MoonshotErrorMapper;
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid JSON");
        let mapped = mapper.map_parsing_error(&error);

        match mapped {
            ProviderError::ResponseParsing { provider, .. } => {
                assert_eq!(provider, "moonshot");
            }
            _ => panic!("Expected ResponseParsing error"),
        }
    }

    #[test]
    fn test_error_mapper_timeout_error() {
        let mapper = MoonshotErrorMapper;
        let mapped = mapper.map_timeout_error(std::time::Duration::from_secs(60));

        match mapped {
            ProviderError::Timeout { provider, .. } => {
                assert_eq!(provider, "moonshot");
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    // ==================== Get Error Mapper Tests ====================

    #[tokio::test]
    async fn test_get_error_mapper() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        let _mapper = provider.get_error_mapper();
        // Just verify it doesn't panic
    }

    // ==================== Clone/Debug Tests ====================

    #[tokio::test]
    async fn test_provider_clone() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        let cloned = provider.clone();

        assert_eq!(provider.name(), cloned.name());
        assert_eq!(provider.models().len(), cloned.models().len());
    }

    #[tokio::test]
    async fn test_provider_debug() {
        let provider = MoonshotProvider::new(create_test_config()).await.unwrap();
        let debug_str = format!("{:?}", provider);

        assert!(debug_str.contains("MoonshotProvider"));
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

        assert!(debug_str.contains("MoonshotConfig"));
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["api_key"], "test_api_key");
        assert_eq!(json["api_base"], "https://api.moonshot.cn/v1");
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

        let config: MoonshotConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, "my_key");
        assert_eq!(config.api_base, "https://custom.api.com");
        assert_eq!(config.timeout_seconds, 120);
        assert_eq!(config.max_retries, 5);
    }

    // ==================== Static Capabilities Constant Tests ====================

    #[test]
    fn test_moonshot_capabilities_constant() {
        assert!(MOONSHOT_CAPABILITIES.contains(&ProviderCapability::ChatCompletion));
        assert!(MOONSHOT_CAPABILITIES.contains(&ProviderCapability::ChatCompletionStream));
        assert!(MOONSHOT_CAPABILITIES.contains(&ProviderCapability::FunctionCalling));
        assert_eq!(MOONSHOT_CAPABILITIES.len(), 3);
    }
}
