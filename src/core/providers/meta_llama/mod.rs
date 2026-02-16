//! Meta Llama Provider
//!
//! Meta's Llama models integration for LiteLLM.
//! This provider supports Llama API's OpenAI-compatible endpoint at <https://api.llama.com/compat/v1>
//!
//! ## Features
//! - OpenAI-compatible API interface
//! - Support for all Llama model variants (Llama 3.1, 3.2, etc.)
//! - Function calling and tool support
//! - JSON schema response format
//! - Streaming support
//!
//! ## Documentation
//! - API Docs: <https://llama.developer.meta.com/docs/features/compatibility/>

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info};

pub mod chat;
pub mod common_utils;

// Use the new unified cost calculation system
use crate::core::cost::CostCalculator;
use crate::core::cost::providers::generic::StubCostCalculator;

// Re-export main components
pub use chat::{LlamaChatHandler, LlamaChatTransformation};
pub use common_utils::{LlamaClient, LlamaConfig, LlamaUtils};

// Import unified error system
use crate::core::providers::unified_provider::ProviderError;

// use super::base_llm::{BaseLLMProvider, BaseLLMError};
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

// Static capabilities for Meta Llama provider
const LLAMA_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

// For now, use a lazy static or instance method for models since they contain owned strings
// TODO: Refactor to use static string slices later

/// Meta Llama provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaProviderConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://api.llama.com/compat/v1>)
    pub api_base: Option<String>,
    /// Organization ID
    pub organization_id: Option<String>,
    /// Request timeout in seconds
    pub timeout: Option<u64>,
    /// Maximum retries for failed requests
    pub max_retries: Option<u32>,
    /// Custom headers
    pub headers: Option<HashMap<String, String>>,
    /// Supported models
    pub supported_models: Vec<String>,
    /// Provider metadata
    pub metadata: HashMap<String, String>,
    /// Cost calculator
    #[serde(skip)]
    pub cost_calculator: Option<StubCostCalculator>,
}

impl Default for LlamaProviderConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: Some("https://api.llama.com/compat/v1".to_string()),
            organization_id: None,
            timeout: Some(30),
            max_retries: Some(3),
            headers: None,
            supported_models: vec![
                // LLaMA 4 series (2025 - Latest)
                "llama4-scout".to_string(),
                "llama4-maverick".to_string(),
                // LLaMA 3.3 series
                "llama3.3-70b".to_string(),
                // LLaMA 3.2 series
                "llama3.2-1b".to_string(),
                "llama3.2-3b".to_string(),
                "llama3.2-11b-vision".to_string(),
                "llama3.2-90b-vision".to_string(),
                // LLaMA 3.1 series
                "llama3.1-8b".to_string(),
                "llama3.1-70b".to_string(),
                "llama3.1-405b".to_string(),
            ],
            metadata: HashMap::new(),
            cost_calculator: None,
        }
    }
}

/// Meta Llama provider implementation
#[derive(Debug, Clone)]
pub struct LlamaProvider {
    config: Arc<LlamaProviderConfig>,
    client: Arc<LlamaClient>,
    chat_handler: Arc<LlamaChatHandler>,
    cost_calculator: StubCostCalculator,
    models: Vec<ModelInfo>,
}

impl LlamaProvider {
    /// Create a new Llama provider instance
    pub fn new(config: LlamaProviderConfig) -> Result<Self, ProviderError> {
        let llama_config = LlamaConfig::from_provider_config(&config)?;
        let client = LlamaClient::new(llama_config.clone())?;
        let chat_handler = LlamaChatHandler::new(llama_config.clone())?;
        let cost_calculator = config
            .cost_calculator
            .clone()
            .unwrap_or_else(|| StubCostCalculator::new("meta_llama".to_string()));

        let models = vec![
            // ==================== LLaMA 4 Series (2025 - Latest) ====================
            ModelInfo {
                id: "llama4-scout".to_string(),
                name: "Llama 4 Scout".to_string(),
                provider: "meta".to_string(),
                max_context_length: 10_000_000, // 10M tokens (industry-leading)
                max_output_length: Some(128000),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true, // Native multimodal
                input_cost_per_1k_tokens: Some(0.00008), // $0.08/1M input
                output_cost_per_1k_tokens: Some(0.0003), // $0.30/1M output
                currency: "USD".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                    ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "llama4-maverick".to_string(),
                name: "Llama 4 Maverick".to_string(),
                provider: "meta".to_string(),
                max_context_length: 1_000_000, // 1M tokens
                max_output_length: Some(128000),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.00020), // $0.20/1M input
                output_cost_per_1k_tokens: Some(0.0006), // $0.60/1M output
                currency: "USD".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                    ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            // ==================== LLaMA 3.3 Series ====================
            ModelInfo {
                id: "llama3.3-70b".to_string(),
                name: "Llama 3.3 70B".to_string(),
                provider: "meta".to_string(),
                max_context_length: 128000,
                max_output_length: Some(32000),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0006),
                output_cost_per_1k_tokens: Some(0.0006),
                currency: "USD".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                    ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            // ==================== LLaMA 3.1 Series ====================
            ModelInfo {
                id: "llama3.1-405b".to_string(),
                name: "Llama 3.1 405B".to_string(),
                provider: "meta".to_string(),
                max_context_length: 128000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.002),
                output_cost_per_1k_tokens: Some(0.002),
                currency: "USD".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                    ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "llama3.1-70b".to_string(),
                name: "Llama 3.1 70B".to_string(),
                provider: "meta".to_string(),
                max_context_length: 128000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.001),
                output_cost_per_1k_tokens: Some(0.001),
                currency: "USD".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                    ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
        ];

        Ok(Self {
            config: Arc::new(config),
            client: Arc::new(client),
            chat_handler: Arc::new(chat_handler),
            cost_calculator,
            models,
        })
    }

    /// Get the API base URL
    pub fn get_api_base(&self) -> String {
        self.config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.llama.com/compat/v1".to_string())
    }

    /// Check if a model is supported
    pub fn is_model_supported(&self, model: &str) -> bool {
        self.config
            .supported_models
            .iter()
            .any(|m| m == model || model.contains(m))
    }

    /// Get provider capabilities
    pub fn get_capabilities(&self) -> &'static [ProviderCapability] {
        LLAMA_CAPABILITIES
    }
}

#[async_trait]
impl LLMProvider for LlamaProvider {
    type Config = LlamaProviderConfig;
    type Error = ProviderError;
    type ErrorMapper = UnifiedErrorMapper;

    fn name(&self) -> &'static str {
        "meta"
    }
    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Llama chat completion request: model={}", request.model);

        // Validate model support
        if !self.is_model_supported(&request.model) {
            return Err(ProviderError::model_not_found(
                "meta",
                request.model.clone(),
            ));
        }

        // Transform request to Llama format
        let llama_request = self.chat_handler.transform_request(request)?;

        // Make API call
        let response = self
            .client
            .chat_completion(
                llama_request,
                Some(&self.config.api_key),
                self.config.api_base.as_deref(),
                self.config.headers.clone(),
            )
            .await?;

        // Transform response back to standard format
        let chat_response = self.chat_handler.transform_response(response)?;

        info!(
            "Llama chat completion successful: model={}",
            chat_response.model
        );
        Ok(chat_response)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Llama streaming chat request: model={}", request.model);

        // Validate model support
        if !self.is_model_supported(&request.model) {
            return Err(ProviderError::model_not_found(
                "meta",
                request.model.clone(),
            ));
        }

        // Clone Arc references for the stream to own
        let client = Arc::clone(&self.client);
        let config = Arc::clone(&self.config);
        let chat_handler = Arc::clone(&self.chat_handler);

        // Transform request and enable streaming
        let mut llama_request = chat_handler.transform_request(request)?;
        if let serde_json::Value::Object(ref mut obj) = llama_request {
            obj.insert("stream".to_string(), serde_json::Value::Bool(true));
        }

        // Get stream from client using owned data
        let api_key = Some(config.api_key.clone());
        let api_base = config.api_base.clone();
        let headers = config.headers.clone();

        let json_stream = client
            .chat_completion_stream(
                llama_request,
                api_key.as_deref(),
                api_base.as_deref(),
                headers,
            )
            .await?;

        // Convert JsonValue stream to ChatChunk stream
        use futures::stream::StreamExt;
        let chunk_stream = json_stream.map(|result| {
            result.map(|json| crate::core::types::responses::ChatChunk {
                id: json
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                object: "chat.completion.chunk".to_string(),
                created: json.get("created").and_then(|v| v.as_i64()).unwrap_or(0),
                model: json
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                choices: vec![],
                usage: None,
                system_fingerprint: None,
            })
        });

        Ok(Box::pin(chunk_stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        // Llama API doesn't directly support embeddings through the OpenAI-compatible endpoint
        Err(ProviderError::not_implemented("meta", "embeddings"))
    }

    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        // Llama models don't support image generation, only vision understanding
        Err(ProviderError::not_implemented("meta", "image generation"))
    }

    async fn health_check(&self) -> HealthStatus {
        // Perform a simple API call to check health
        match self.client.check_health().await {
            Ok(status) => status,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        LLAMA_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "messages",
            "model",
            "max_tokens",
            "temperature",
            "top_p",
            "n",
            "stream",
            "stop",
            "presence_penalty",
            "frequency_penalty",
            "user",
            "seed",
            "response_format",
            "tools",
            "tool_choice",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Llama API is OpenAI-compatible, so no mapping needed
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        self.chat_handler.transform_request(request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response: Value = serde_json::from_slice(raw_response)?;
        self.chat_handler
            .transform_response(response)
            .map_err(|e| ProviderError::serialization("meta", e.to_string()))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        UnifiedErrorMapper
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        use crate::core::cost::types::UsageTokens;
        let usage = UsageTokens::new(input_tokens, output_tokens);
        let cost = self.cost_calculator.calculate_cost("", &usage).await?;
        Ok(cost.total_cost)
    }
}

impl ProviderConfig for LlamaProviderConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("API key is required for Llama provider".to_string());
        }

        if let Some(timeout) = self.timeout {
            if timeout == 0 {
                return Err("Timeout must be greater than 0".to_string());
            }
        }

        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        if self.api_key.is_empty() {
            None
        } else {
            Some(&self.api_key)
        }
    }

    fn api_base(&self) -> Option<&str> {
        self.api_base.as_deref()
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout.unwrap_or(30))
    }

    fn max_retries(&self) -> u32 {
        self.max_retries.unwrap_or(3)
    }
}

/// Unified error mapper for ProviderError - no conversion needed since we use ProviderError directly
#[derive(Debug, Clone)]
pub struct UnifiedErrorMapper;

impl ErrorMapper<ProviderError> for UnifiedErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        ProviderError::api_error("meta", status_code, response_body.to_string())
    }

    fn map_json_error(&self, _error_response: &serde_json::Value) -> ProviderError {
        ProviderError::response_parsing("meta", "Failed to parse JSON error response")
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::network("meta", error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LlamaProviderConfig::default();
        assert_eq!(config.api_base.unwrap(), "https://api.llama.com/compat/v1");
        assert_eq!(config.timeout.unwrap(), 30);
        assert!(!config.supported_models.is_empty());
    }

    #[test]
    fn test_model_support() {
        let config = LlamaProviderConfig {
            api_key: "test-api-key-1234567890123456".to_string(),
            ..Default::default()
        };
        let provider = LlamaProvider::new(config).unwrap();

        // LLaMA 4 series (2025)
        assert!(provider.is_model_supported("llama4-scout"));
        assert!(provider.is_model_supported("llama4-maverick"));
        // LLaMA 3.x series
        assert!(provider.is_model_supported("llama3.3-70b"));
        assert!(provider.is_model_supported("llama3.1-8b"));
        assert!(provider.is_model_supported("llama3.2-11b-vision"));
        // Non-supported models
        assert!(!provider.is_model_supported("gpt-4"));
    }

    #[test]
    fn test_capabilities() {
        let config = LlamaProviderConfig {
            api_key: "test-api-key-1234567890123456".to_string(),
            ..Default::default()
        };
        let provider = LlamaProvider::new(config).unwrap();
        let capabilities = provider.get_capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
        assert!(!capabilities.contains(&ProviderCapability::ImageGeneration));
    }
}
