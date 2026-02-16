//! Dashscope (Alibaba Cloud) AI Provider
//!
//! Dashscope provides access to Alibaba's Qwen series models with an OpenAI-compatible API.
//! API Base: <https://dashscope.aliyuncs.com/compatible-mode/v1>

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
const DASHSCOPE_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::FunctionCalling,
];

/// Dashscope provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashscopeConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://dashscope.aliyuncs.com/compatible-mode/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for DashscopeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            timeout_seconds: 60,
            max_retries: 3,
        }
    }
}

impl ProviderConfig for DashscopeConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("Dashscope API key is required".to_string());
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

/// Dashscope error type (simplified using ProviderError)
pub type DashscopeError = ProviderError;

/// Dashscope error mapper
pub struct DashscopeErrorMapper;

impl ErrorMapper<DashscopeError> for DashscopeErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> DashscopeError {
        HttpErrorMapper::map_status_code("dashscope", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> DashscopeError {
        HttpErrorMapper::parse_json_error("dashscope", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> DashscopeError {
        ProviderError::network("dashscope", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> DashscopeError {
        ProviderError::response_parsing("dashscope", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> DashscopeError {
        ProviderError::timeout(
            "dashscope",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Dashscope provider implementation
#[derive(Debug, Clone)]
pub struct DashscopeProvider {
    config: DashscopeConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl DashscopeProvider {
    /// Create a new Dashscope provider instance
    pub async fn new(config: DashscopeConfig) -> Result<Self, DashscopeError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("dashscope", e))?;

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

        // Define supported models with pricing (CNY per 1k tokens)
        let models = vec![
            ModelInfo {
                id: "qwen-turbo".to_string(),
                name: "Qwen Turbo".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 131072,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0008),
                output_cost_per_1k_tokens: Some(0.002),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen-plus".to_string(),
                name: "Qwen Plus".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 131072,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.004),
                output_cost_per_1k_tokens: Some(0.012),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen-max".to_string(),
                name: "Qwen Max".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 32768,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.02),
                output_cost_per_1k_tokens: Some(0.06),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen-max-longcontext".to_string(),
                name: "Qwen Max Long Context".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 1000000,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.02),
                output_cost_per_1k_tokens: Some(0.06),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen-vl-plus".to_string(),
                name: "Qwen VL Plus".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 32768,
                max_output_length: Some(2048),
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.008),
                output_cost_per_1k_tokens: Some(0.008),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen-vl-max".to_string(),
                name: "Qwen VL Max".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 32768,
                max_output_length: Some(2048),
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.02),
                output_cost_per_1k_tokens: Some(0.02),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen2.5-72b-instruct".to_string(),
                name: "Qwen 2.5 72B Instruct".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 131072,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.004),
                output_cost_per_1k_tokens: Some(0.012),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen2.5-32b-instruct".to_string(),
                name: "Qwen 2.5 32B Instruct".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 131072,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0035),
                output_cost_per_1k_tokens: Some(0.007),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen2.5-14b-instruct".to_string(),
                name: "Qwen 2.5 14B Instruct".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 131072,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.002),
                output_cost_per_1k_tokens: Some(0.006),
                currency: "CNY".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen2.5-7b-instruct".to_string(),
                name: "Qwen 2.5 7B Instruct".to_string(),
                provider: "dashscope".to_string(),
                max_context_length: 131072,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.001),
                output_cost_per_1k_tokens: Some(0.002),
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

    /// Build the complete URL for the chat completions endpoint
    fn build_chat_url(&self) -> String {
        let base = &self.config.api_base;
        if base.ends_with("/chat/completions") {
            base.clone()
        } else if base.ends_with('/') {
            format!("{}chat/completions", base)
        } else {
            format!("{}/chat/completions", base)
        }
    }
}

#[async_trait]
impl LLMProvider for DashscopeProvider {
    type Config = DashscopeConfig;
    type Error = DashscopeError;
    type ErrorMapper = DashscopeErrorMapper;

    fn name(&self) -> &'static str {
        "dashscope"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        DASHSCOPE_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Dashscope chat request: model={}", request.model);

        // Transform request (Dashscope uses OpenAI-compatible format but needs content list to string conversion)
        let body = self.transform_request(request, context).await?;

        // Build URL based on config
        let url = self.build_chat_url();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("dashscope", e.to_string()))?;

        let response = self
            .base_client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("dashscope", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("dashscope", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("dashscope", e.to_string()))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Dashscope streaming chat request: model={}", request.model);

        // Transform request
        let mut body = self.transform_request(request, context).await?;
        body["stream"] = serde_json::json!(true);

        // Build URL based on config
        let url = self.build_chat_url();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("dashscope", e.to_string()))?;

        let response = self
            .base_client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("dashscope", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("dashscope", status, body));
        }

        // Parse SSE stream using shared infrastructure
        use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEParser};
        use futures::StreamExt;

        let transformer = OpenAICompatibleTransformer::new("dashscope");
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
                    Err(e) => Some(Err(ProviderError::network("dashscope", e.to_string()))),
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
        // Dashscope does support embeddings via text-embedding-v2/v3
        // For now, return not supported - can be implemented later
        Err(ProviderError::not_supported("dashscope", "embeddings"))
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
                        debug!(
                            "Dashscope health check failed: status={}",
                            response.status()
                        );
                        HealthStatus::Unhealthy
                    }
                    Err(e) => {
                        debug!("Dashscope health check error: {}", e);
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
            "seed",
            "top_k", // Qwen-specific
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Dashscope is OpenAI-compatible, pass-through most parameters
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        // Use the OpenAI transformer from base_provider
        // Note: Dashscope doesn't support content in list format, so we need to convert
        let mut body = OpenAIRequestTransformer::transform_chat_request(&request);

        // Convert content list to string if needed (Dashscope requirement)
        if let Some(messages) = body.get_mut("messages") {
            if let Some(messages_array) = messages.as_array_mut() {
                for message in messages_array {
                    if let Some(content) = message.get("content") {
                        if content.is_array() {
                            // Convert array content to string
                            if let Some(content_array) = content.as_array() {
                                let text_parts: Vec<String> = content_array
                                    .iter()
                                    .filter_map(|part| {
                                        if let Some(text) = part.get("text") {
                                            text.as_str().map(|s| s.to_string())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                message["content"] = serde_json::json!(text_parts.join("\n"));
                            }
                        }
                    }
                }
            }
        }

        Ok(body)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        // Parse Dashscope response (OpenAI-compatible format)
        serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("dashscope", e.to_string()))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        DashscopeErrorMapper
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
            .ok_or_else(|| ProviderError::model_not_found("dashscope", model.to_string()))?;

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

    fn create_test_config() -> DashscopeConfig {
        DashscopeConfig {
            api_key: "test_api_key".to_string(),
            ..Default::default()
        }
    }

    // ==================== Provider Creation Tests ====================

    #[tokio::test]
    async fn test_dashscope_provider_creation() {
        let config = DashscopeConfig {
            api_key: "test_key".to_string(),
            ..Default::default()
        };

        let provider = DashscopeProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "dashscope");
        assert!(
            provider
                .capabilities()
                .contains(&ProviderCapability::ChatCompletionStream)
        );
    }

    #[tokio::test]
    async fn test_dashscope_provider_creation_custom_base() {
        let config = DashscopeConfig {
            api_key: "test_key".to_string(),
            api_base: "https://custom.aliyuncs.com/v1".to_string(),
            ..Default::default()
        };

        let provider = DashscopeProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_dashscope_provider_creation_no_api_key() {
        let config = DashscopeConfig::default();
        let provider = DashscopeProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_dashscope_provider_creation_empty_api_key() {
        let config = DashscopeConfig {
            api_key: "".to_string(),
            ..Default::default()
        };

        let provider = DashscopeProvider::new(config).await;
        assert!(provider.is_err());
    }

    // ==================== Config Validation Tests ====================

    #[test]
    fn test_dashscope_config_validation() {
        let mut config = DashscopeConfig::default();
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
    fn test_dashscope_config_default() {
        let config = DashscopeConfig::default();

        assert_eq!(config.api_key, "");
        assert_eq!(
            config.api_base,
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_dashscope_config_provider_config_trait() {
        let config = create_test_config();

        assert_eq!(config.api_key(), Some("test_api_key"));
        assert_eq!(
            config.api_base(),
            Some("https://dashscope.aliyuncs.com/compatible-mode/v1")
        );
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
        assert_eq!(config.max_retries(), 3);
    }

    #[test]
    fn test_dashscope_config_validation_max_retries_boundary() {
        let mut config = create_test_config();

        config.max_retries = 10;
        assert!(config.validate().is_ok());

        config.max_retries = 11;
        assert!(config.validate().is_err());
    }

    // ==================== Provider Capabilities Tests ====================

    #[tokio::test]
    async fn test_provider_name() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        assert_eq!(provider.name(), "dashscope");
    }

    #[tokio::test]
    async fn test_provider_capabilities() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::FunctionCalling));
        assert_eq!(caps.len(), 3);
    }

    #[tokio::test]
    async fn test_provider_models() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "qwen-turbo"));
        assert!(models.iter().any(|m| m.id == "qwen-plus"));
        assert!(models.iter().any(|m| m.id == "qwen-max"));
        assert!(models.iter().any(|m| m.id == "qwen-vl-plus"));
    }

    #[tokio::test]
    async fn test_provider_models_have_pricing() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        for model in models {
            assert_eq!(model.provider, "dashscope");
            assert_eq!(model.currency, "CNY");
            assert!(model.input_cost_per_1k_tokens.is_some());
            assert!(model.output_cost_per_1k_tokens.is_some());
        }
    }

    #[tokio::test]
    async fn test_provider_models_context_lengths() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        let qwen_turbo = models.iter().find(|m| m.id == "qwen-turbo").unwrap();
        assert_eq!(qwen_turbo.max_context_length, 131072);

        let qwen_max = models.iter().find(|m| m.id == "qwen-max").unwrap();
        assert_eq!(qwen_max.max_context_length, 32768);

        let qwen_max_long = models
            .iter()
            .find(|m| m.id == "qwen-max-longcontext")
            .unwrap();
        assert_eq!(qwen_max_long.max_context_length, 1000000);
    }

    // ==================== URL Building Tests ====================

    #[tokio::test]
    async fn test_build_chat_url_default() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let url = provider.build_chat_url();
        assert_eq!(
            url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
    }

    #[tokio::test]
    async fn test_build_chat_url_with_trailing_slash() {
        let config = DashscopeConfig {
            api_key: "test_key".to_string(),
            api_base: "https://dashscope.aliyuncs.com/compatible-mode/v1/".to_string(),
            ..Default::default()
        };
        let provider = DashscopeProvider::new(config).await.unwrap();
        let url = provider.build_chat_url();
        assert_eq!(
            url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
    }

    #[tokio::test]
    async fn test_build_chat_url_already_complete() {
        let config = DashscopeConfig {
            api_key: "test_key".to_string(),
            api_base: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
                .to_string(),
            ..Default::default()
        };
        let provider = DashscopeProvider::new(config).await.unwrap();
        let url = provider.build_chat_url();
        assert_eq!(
            url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
    }

    // ==================== Supported Params Tests ====================

    #[tokio::test]
    async fn test_get_supported_openai_params() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let params = provider.get_supported_openai_params("qwen-turbo");

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
        assert!(params.contains(&"seed"));
        assert!(params.contains(&"top_k")); // Qwen-specific
    }

    // ==================== Map OpenAI Params Tests ====================

    #[tokio::test]
    async fn test_map_openai_params_passthrough() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.7));
        params.insert("max_tokens".to_string(), serde_json::json!(100));
        params.insert("top_p".to_string(), serde_json::json!(0.9));

        let mapped = provider
            .map_openai_params(params.clone(), "qwen-turbo")
            .await
            .unwrap();

        // Dashscope is OpenAI-compatible, should pass through
        assert_eq!(mapped, params);
    }

    // ==================== Transform Request Tests ====================

    #[tokio::test]
    async fn test_transform_request_basic() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "qwen-turbo".to_string(),
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
        assert_eq!(transformed["model"], "qwen-turbo");
        assert!(transformed["messages"].is_array());
    }

    #[tokio::test]
    async fn test_transform_request_with_temperature() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "qwen-plus".to_string(),
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
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();

        let request = crate::core::types::embedding::EmbeddingRequest {
            model: "qwen-turbo".to_string(),
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
    async fn test_calculate_cost_qwen_turbo() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("qwen-turbo", 1000, 500).await;
        assert!(cost.is_ok());

        let cost_value = cost.unwrap();
        // qwen-turbo: 0.0008 CNY input, 0.002 CNY output per 1k
        // (1000/1000 * 0.0008) + (500/1000 * 0.002) = 0.0008 + 0.001 = 0.0018
        assert!((cost_value - 0.0018).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_calculate_cost_qwen_max() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("qwen-max", 1000, 500).await;
        assert!(cost.is_ok());

        let cost_value = cost.unwrap();
        // qwen-max: 0.02 CNY input, 0.06 CNY output per 1k
        // (1000/1000 * 0.02) + (500/1000 * 0.06) = 0.02 + 0.03 = 0.05
        assert!((cost_value - 0.05).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("unknown-model", 1000, 500).await;
        assert!(cost.is_err());
    }

    #[tokio::test]
    async fn test_calculate_cost_zero_tokens() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("qwen-turbo", 0, 0).await;
        assert!(cost.is_ok());
        assert!((cost.unwrap() - 0.0).abs() < 0.0001);
    }

    // ==================== Error Mapper Tests ====================

    #[test]
    fn test_error_mapper_authentication() {
        let mapper = DashscopeErrorMapper;
        let error = mapper.map_http_error(401, "Unauthorized");

        match error {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "dashscope");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_error_mapper_rate_limit() {
        let mapper = DashscopeErrorMapper;
        let error = mapper.map_http_error(429, "Rate limit exceeded");

        match error {
            ProviderError::RateLimit { provider, .. } => {
                assert_eq!(provider, "dashscope");
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_error_mapper_network_error() {
        let mapper = DashscopeErrorMapper;
        let error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let mapped = mapper.map_network_error(&error);

        match mapped {
            ProviderError::Network { provider, .. } => {
                assert_eq!(provider, "dashscope");
            }
            _ => panic!("Expected Network error"),
        }
    }

    #[test]
    fn test_error_mapper_parsing_error() {
        let mapper = DashscopeErrorMapper;
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid JSON");
        let mapped = mapper.map_parsing_error(&error);

        match mapped {
            ProviderError::ResponseParsing { provider, .. } => {
                assert_eq!(provider, "dashscope");
            }
            _ => panic!("Expected ResponseParsing error"),
        }
    }

    #[test]
    fn test_error_mapper_timeout_error() {
        let mapper = DashscopeErrorMapper;
        let mapped = mapper.map_timeout_error(std::time::Duration::from_secs(60));

        match mapped {
            ProviderError::Timeout { provider, .. } => {
                assert_eq!(provider, "dashscope");
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    // ==================== Get Error Mapper Tests ====================

    #[tokio::test]
    async fn test_get_error_mapper() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let _mapper = provider.get_error_mapper();
        // Just verify it doesn't panic
    }

    // ==================== Clone/Debug Tests ====================

    #[tokio::test]
    async fn test_provider_clone() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let cloned = provider.clone();

        assert_eq!(provider.name(), cloned.name());
        assert_eq!(provider.models().len(), cloned.models().len());
    }

    #[tokio::test]
    async fn test_provider_debug() {
        let provider = DashscopeProvider::new(create_test_config()).await.unwrap();
        let debug_str = format!("{:?}", provider);

        assert!(debug_str.contains("DashscopeProvider"));
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

        assert!(debug_str.contains("DashscopeConfig"));
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["api_key"], "test_api_key");
        assert_eq!(
            json["api_base"],
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
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

        let config: DashscopeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, "my_key");
        assert_eq!(config.api_base, "https://custom.api.com");
        assert_eq!(config.timeout_seconds, 120);
        assert_eq!(config.max_retries, 5);
    }

    // ==================== Static Capabilities Constant Tests ====================

    #[test]
    fn test_dashscope_capabilities_constant() {
        assert!(DASHSCOPE_CAPABILITIES.contains(&ProviderCapability::ChatCompletion));
        assert!(DASHSCOPE_CAPABILITIES.contains(&ProviderCapability::ChatCompletionStream));
        assert!(DASHSCOPE_CAPABILITIES.contains(&ProviderCapability::FunctionCalling));
        assert_eq!(DASHSCOPE_CAPABILITIES.len(), 3);
    }
}
