//! Sambanova AI Provider
//!
//! High-performance inference provider using Sambanova's custom AI chips.
//! This implementation is OpenAI-compatible with minimal transformation needed.
//!
//! Reference: <https://docs.sambanova.ai/cloud/api-reference/>

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HttpErrorMapper, OpenAIRequestTransformer, UrlBuilder,
    apply_headers, get_pricing_db, header, header_static,
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

// Static capabilities for Sambanova
const SAMBANOVA_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::Embeddings,
];

/// Sambanova provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SambanovaConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://api.sambanova.ai/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for SambanovaConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.sambanova.ai/v1".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

impl ProviderConfig for SambanovaConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("Sambanova")
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

/// Sambanova error type (using unified ProviderError)
pub type SambanovaError = ProviderError;

/// Sambanova error mapper
pub struct SambanovaErrorMapper;

impl ErrorMapper<SambanovaError> for SambanovaErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> SambanovaError {
        HttpErrorMapper::map_status_code("sambanova", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> SambanovaError {
        HttpErrorMapper::parse_json_error("sambanova", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> SambanovaError {
        ProviderError::network("sambanova", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> SambanovaError {
        ProviderError::response_parsing("sambanova", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> SambanovaError {
        ProviderError::timeout(
            "sambanova",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Sambanova provider implementation
///
/// Sambanova uses custom AI chips (RDU) for high-performance inference.
/// The API is OpenAI-compatible, so minimal transformation is needed.
#[derive(Debug, Clone)]
pub struct SambanovaProvider {
    config: SambanovaConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl SambanovaProvider {
    /// Create a new Sambanova provider instance
    pub async fn new(config: SambanovaConfig) -> Result<Self, SambanovaError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("sambanova", e))?;

        // Create base HTTP client
        let base_config = BaseConfig {
            api_key: Some(config.api_key.clone()),
            api_base: Some(config.api_base.clone()),
            timeout: config.timeout_seconds,
            max_retries: config.max_retries,
            headers: HashMap::new(),
            organization: None,
            api_version: None,
        };

        let base_client = BaseHttpClient::new(base_config)?;

        // Define supported models with pricing
        // Note: Sambanova pricing varies - these are placeholder values
        let models = vec![
            ModelInfo {
                id: "Meta-Llama-3.1-8B-Instruct".to_string(),
                name: "Llama 3.1 8B Instruct".to_string(),
                provider: "sambanova".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0001),
                output_cost_per_1k_tokens: Some(0.0002),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "Meta-Llama-3.1-70B-Instruct".to_string(),
                name: "Llama 3.1 70B Instruct".to_string(),
                provider: "sambanova".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0005),
                output_cost_per_1k_tokens: Some(0.001),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "Meta-Llama-3.1-405B-Instruct".to_string(),
                name: "Llama 3.1 405B Instruct".to_string(),
                provider: "sambanova".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.001),
                output_cost_per_1k_tokens: Some(0.002),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "sambanova-embed".to_string(),
                name: "Sambanova Embed".to_string(),
                provider: "sambanova".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0001),
                output_cost_per_1k_tokens: Some(0.0),
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

    /// Create provider with just API key using default configuration
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, SambanovaError> {
        let config = SambanovaConfig {
            api_key: api_key.into(),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Check if model is an embedding model
    fn is_embedding_model(&self, model: &str) -> bool {
        model.contains("embed")
    }

    /// Check if model supports function calling
    fn supports_function_calling(&self, model: &str) -> bool {
        // Most Llama models on Sambanova support function calling
        model.contains("Instruct") || model.contains("Chat")
    }
}

#[async_trait]
impl LLMProvider for SambanovaProvider {
    type Config = SambanovaConfig;
    type Error = SambanovaError;
    type ErrorMapper = SambanovaErrorMapper;

    fn name(&self) -> &'static str {
        "sambanova"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        SAMBANOVA_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        // Base parameters supported by all Sambanova models
        let base_params: &[&str] = &[
            "max_completion_tokens",
            "max_tokens",
            "response_format",
            "stop",
            "stream",
            "stream_options",
            "temperature",
            "top_p",
            "top_k",
        ];

        // Add tool-related params if model supports function calling
        if self.supports_function_calling(model) {
            &[
                "max_completion_tokens",
                "max_tokens",
                "response_format",
                "stop",
                "stream",
                "stream_options",
                "temperature",
                "top_p",
                "top_k",
                "tools",
                "tool_choice",
                "parallel_tool_calls",
            ]
        } else {
            base_params
        }
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        let mut mapped = HashMap::new();

        for (key, value) in params {
            match key.as_str() {
                // Map max_completion_tokens to max_tokens for Sambanova
                "max_completion_tokens" => {
                    mapped.insert("max_tokens".to_string(), value);
                }
                // Direct pass-through for standard parameters
                "temperature"
                | "top_p"
                | "top_k"
                | "max_tokens"
                | "stream"
                | "stop"
                | "stream_options"
                | "tools"
                | "tool_choice"
                | "response_format"
                | "parallel_tool_calls" => {
                    mapped.insert(key, value);
                }
                // Skip unsupported parameters
                _ => {}
            }
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        // Use OpenAI transformer since Sambanova is OpenAI-compatible
        let body = OpenAIRequestTransformer::transform_chat_request(&request);
        Ok(body)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        // Parse response (OpenAI-compatible format)
        serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("sambanova", e.to_string()))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        SambanovaErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Sambanova chat request: model={}", request.model);

        if self.is_embedding_model(&request.model) {
            return Err(ProviderError::invalid_request(
                "sambanova",
                "Use embeddings endpoint for embedding models".to_string(),
            ));
        }

        let body = self.transform_request(request, context).await?;

        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/chat/completions")
            .build();

        let headers = vec![
            header("Authorization", format!("Bearer {}", self.config.api_key)),
            header_static("Content-Type", "application/json"),
        ];

        let response = apply_headers(self.base_client.inner().post(&url), headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("sambanova", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("sambanova", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("sambanova", e.to_string()))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Sambanova streaming chat request: model={}", request.model);

        let mut body = self.transform_request(request, context).await?;
        body["stream"] = serde_json::json!(true);

        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/chat/completions")
            .build();

        let headers = vec![
            header("Authorization", format!("Bearer {}", self.config.api_key)),
            header_static("Content-Type", "application/json"),
        ];

        let response = apply_headers(self.base_client.inner().post(&url), headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("sambanova", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("sambanova", status, &body));
        }

        Ok(crate::core::providers::base::create_provider_sse_stream(
            response,
            "sambanova",
        ))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        debug!("Sambanova embedding request: model={}", request.model);

        let body = serde_json::json!({
            "model": request.model,
            "input": request.input,
        });

        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/embeddings")
            .build();

        let headers = vec![
            header("Authorization", format!("Bearer {}", self.config.api_key)),
            header_static("Content-Type", "application/json"),
        ];

        let response = apply_headers(self.base_client.inner().post(&url), headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("sambanova", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("sambanova", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("sambanova", e.to_string()))
    }

    async fn health_check(&self) -> HealthStatus {
        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/models")
            .build();

        match apply_headers(
            self.base_client.inner().get(&url),
            vec![header(
                "Authorization",
                format!("Bearer {}", self.config.api_key),
            )],
        )
        .send()
        .await
        {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(response) => {
                debug!(
                    "Sambanova health check failed: status={}",
                    response.status()
                );
                HealthStatus::Unhealthy
            }
            Err(e) => {
                debug!("Sambanova health check error: {}", e);
                HealthStatus::Unhealthy
            }
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        let usage = crate::core::providers::base::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };
        Ok(get_pricing_db().calculate(model, &usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    fn create_test_config() -> SambanovaConfig {
        SambanovaConfig {
            api_key: "test_api_key".to_string(),
            ..Default::default()
        }
    }

    // ==================== Provider Creation Tests ====================

    #[tokio::test]
    async fn test_sambanova_provider_creation() {
        let config = SambanovaConfig {
            api_key: "test_key".to_string(),
            ..Default::default()
        };

        let provider = SambanovaProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(LLMProvider::name(&provider), "sambanova");
        assert!(
            provider
                .capabilities()
                .contains(&ProviderCapability::ChatCompletionStream)
        );
    }

    #[tokio::test]
    async fn test_sambanova_provider_creation_custom_base() {
        let config = SambanovaConfig {
            api_key: "test_key".to_string(),
            api_base: "https://custom.sambanova.ai/v1".to_string(),
            ..Default::default()
        };

        let provider = SambanovaProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_sambanova_provider_creation_no_api_key() {
        let config = SambanovaConfig::default();
        let provider = SambanovaProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_sambanova_provider_creation_empty_api_key() {
        let config = SambanovaConfig {
            api_key: "".to_string(),
            ..Default::default()
        };

        let provider = SambanovaProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_sambanova_with_api_key() {
        let provider = SambanovaProvider::with_api_key("test_key").await;
        assert!(provider.is_ok());
    }

    // ==================== Config Validation Tests ====================

    #[test]
    fn test_sambanova_config_validation() {
        let mut config = SambanovaConfig::default();
        assert!(config.validate().is_err()); // No API key

        config.api_key = "test_key".to_string();
        assert!(config.validate().is_ok());

        config.timeout_seconds = 0;
        assert!(config.validate().is_err()); // Invalid timeout

        config.timeout_seconds = 30;
        config.max_retries = 11;
        assert!(config.validate().is_err()); // Too many retries
    }

    #[test]
    fn test_sambanova_config_default() {
        let config = SambanovaConfig::default();

        assert_eq!(config.api_key, "");
        assert_eq!(config.api_base, "https://api.sambanova.ai/v1");
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_sambanova_config_provider_config_trait() {
        let config = create_test_config();

        assert_eq!(config.api_key(), Some("test_api_key"));
        assert_eq!(config.api_base(), Some("https://api.sambanova.ai/v1"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(30));
        assert_eq!(config.max_retries(), 3);
    }

    // ==================== Provider Capabilities Tests ====================

    #[tokio::test]
    async fn test_provider_name() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();
        assert_eq!(provider.name(), "sambanova");
    }

    #[tokio::test]
    async fn test_provider_capabilities() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::ToolCalling));
        assert!(caps.contains(&ProviderCapability::Embeddings));
        assert_eq!(caps.len(), 4);
    }

    #[tokio::test]
    async fn test_provider_models() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "Meta-Llama-3.1-8B-Instruct"));
        assert!(models.iter().any(|m| m.id == "Meta-Llama-3.1-70B-Instruct"));
        assert!(
            models
                .iter()
                .any(|m| m.id == "Meta-Llama-3.1-405B-Instruct")
        );
    }

    #[tokio::test]
    async fn test_provider_models_have_pricing() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        for model in models {
            assert_eq!(model.provider, "sambanova");
            assert!(model.input_cost_per_1k_tokens.is_some());
            assert!(model.output_cost_per_1k_tokens.is_some());
        }
    }

    // ==================== Supported Params Tests ====================

    #[tokio::test]
    async fn test_get_supported_openai_params_instruct() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();
        let params = provider.get_supported_openai_params("Meta-Llama-3.1-70B-Instruct");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"top_p"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"stream"));
        assert!(params.contains(&"stop"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"tool_choice"));
    }

    // ==================== Map OpenAI Params Tests ====================

    #[tokio::test]
    async fn test_map_openai_params_max_completion_tokens() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("max_completion_tokens".to_string(), serde_json::json!(100));

        let mapped = provider
            .map_openai_params(params, "Meta-Llama-3.1-70B-Instruct")
            .await
            .unwrap();

        assert!(!mapped.contains_key("max_completion_tokens"));
        assert!(mapped.contains_key("max_tokens"));
        assert_eq!(mapped.get("max_tokens").unwrap(), &serde_json::json!(100));
    }

    #[tokio::test]
    async fn test_map_openai_params_passthrough() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.7));
        params.insert("max_tokens".to_string(), serde_json::json!(100));
        params.insert("top_p".to_string(), serde_json::json!(0.9));

        let mapped = provider
            .map_openai_params(params, "Meta-Llama-3.1-70B-Instruct")
            .await
            .unwrap();

        assert_eq!(mapped.get("temperature").unwrap(), &serde_json::json!(0.7));
        assert_eq!(mapped.get("max_tokens").unwrap(), &serde_json::json!(100));
        assert_eq!(mapped.get("top_p").unwrap(), &serde_json::json!(0.9));
    }

    #[tokio::test]
    async fn test_map_openai_params_unsupported_filtered() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("unsupported_param".to_string(), serde_json::json!("value"));
        params.insert("temperature".to_string(), serde_json::json!(0.5));

        let mapped = provider
            .map_openai_params(params, "Meta-Llama-3.1-70B-Instruct")
            .await
            .unwrap();

        assert!(!mapped.contains_key("unsupported_param"));
        assert!(mapped.contains_key("temperature"));
    }

    // ==================== Transform Request Tests ====================

    #[tokio::test]
    async fn test_transform_request_basic() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "Meta-Llama-3.1-70B-Instruct".to_string(),
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
        assert_eq!(transformed["model"], "Meta-Llama-3.1-70B-Instruct");
        assert!(transformed["messages"].is_array());
    }

    // ==================== Is Embedding Model Tests ====================

    #[tokio::test]
    async fn test_is_embedding_model() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        assert!(provider.is_embedding_model("sambanova-embed"));
        assert!(provider.is_embedding_model("text-embedding-model"));
        assert!(!provider.is_embedding_model("Meta-Llama-3.1-70B-Instruct"));
    }

    // ==================== Supports Function Calling Tests ====================

    #[tokio::test]
    async fn test_supports_function_calling() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        assert!(provider.supports_function_calling("Meta-Llama-3.1-70B-Instruct"));
        assert!(provider.supports_function_calling("SomeModel-Chat"));
        assert!(!provider.supports_function_calling("sambanova-embed"));
    }

    // ==================== Cost Calculation Tests ====================

    #[tokio::test]
    async fn test_calculate_cost_known_model() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        let cost = provider
            .calculate_cost("Meta-Llama-3.1-70B-Instruct", 1000, 500)
            .await;
        assert!(matches!(cost, Ok(v) if v >= 0.0));
    }

    #[tokio::test]
    async fn test_calculate_cost_embed_model() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("sambanova-embed", 1000, 0).await;
        assert!(matches!(cost, Ok(v) if v >= 0.0));
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("unknown-model", 1000, 500).await;
        assert!(matches!(cost, Ok(v) if v >= 0.0));
    }

    #[tokio::test]
    async fn test_calculate_cost_zero_tokens() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();

        let cost = provider
            .calculate_cost("Meta-Llama-3.1-70B-Instruct", 0, 0)
            .await;
        assert!(cost.is_ok());
        assert!((cost.unwrap() - 0.0).abs() < 0.0001);
    }

    // ==================== Error Mapper Tests ====================

    #[test]
    fn test_error_mapper_authentication() {
        let mapper = SambanovaErrorMapper;
        let error = mapper.map_http_error(401, "Unauthorized");

        match error {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "sambanova");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_error_mapper_rate_limit() {
        let mapper = SambanovaErrorMapper;
        let error = mapper.map_http_error(429, "Rate limit exceeded");

        match error {
            ProviderError::RateLimit { provider, .. } => {
                assert_eq!(provider, "sambanova");
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_error_mapper_network_error() {
        let mapper = SambanovaErrorMapper;
        let error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let mapped = mapper.map_network_error(&error);

        match mapped {
            ProviderError::Network { provider, .. } => {
                assert_eq!(provider, "sambanova");
            }
            _ => panic!("Expected Network error"),
        }
    }

    #[test]
    fn test_error_mapper_parsing_error() {
        let mapper = SambanovaErrorMapper;
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid JSON");
        let mapped = mapper.map_parsing_error(&error);

        match mapped {
            ProviderError::ResponseParsing { provider, .. } => {
                assert_eq!(provider, "sambanova");
            }
            _ => panic!("Expected ResponseParsing error"),
        }
    }

    #[test]
    fn test_error_mapper_timeout_error() {
        let mapper = SambanovaErrorMapper;
        let mapped = mapper.map_timeout_error(std::time::Duration::from_secs(30));

        match mapped {
            ProviderError::Timeout { provider, .. } => {
                assert_eq!(provider, "sambanova");
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    // ==================== Get Error Mapper Tests ====================

    #[tokio::test]
    async fn test_get_error_mapper() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();
        let _mapper = provider.get_error_mapper();
        // Just verify it doesn't panic
    }

    // ==================== Clone/Debug Tests ====================

    #[tokio::test]
    async fn test_provider_clone() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();
        let cloned = provider.clone();

        assert_eq!(provider.name(), cloned.name());
        assert_eq!(provider.models().len(), cloned.models().len());
    }

    #[tokio::test]
    async fn test_provider_debug() {
        let provider = SambanovaProvider::new(create_test_config()).await.unwrap();
        let debug_str = format!("{:?}", provider);

        assert!(debug_str.contains("SambanovaProvider"));
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

        assert!(debug_str.contains("SambanovaConfig"));
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["api_key"], "test_api_key");
        assert_eq!(json["api_base"], "https://api.sambanova.ai/v1");
        assert_eq!(json["timeout_seconds"], 30);
        assert_eq!(json["max_retries"], 3);
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "api_key": "my_key",
            "api_base": "https://custom.api.com",
            "timeout_seconds": 60,
            "max_retries": 5
        }"#;

        let config: SambanovaConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, "my_key");
        assert_eq!(config.api_base, "https://custom.api.com");
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.max_retries, 5);
    }
}
