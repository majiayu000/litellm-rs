//! Mistral AI Provider (Refactored)
//!
//! Mistral AI model integration using the base infrastructure.
//! This implementation eliminates the need for common_utils.rs.

use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

// Use base infrastructure instead of common_utils
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

// Re-export submodules (these remain as-is for now)
pub mod chat;
pub mod embedding;

// Static capabilities
const MISTRAL_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::Embeddings,
];

/// Mistral provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MistralConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://api.mistral.ai/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for MistralConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.mistral.ai/v1".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

impl ProviderConfig for MistralConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("Mistral")
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

/// Mistral error type (simplified using ProviderError)
pub type MistralError = ProviderError;

/// Mistral error mapper
pub struct MistralErrorMapper;

impl ErrorMapper<MistralError> for MistralErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> MistralError {
        HttpErrorMapper::map_status_code("mistral", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> MistralError {
        HttpErrorMapper::parse_json_error("mistral", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> MistralError {
        ProviderError::network("mistral", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> MistralError {
        ProviderError::response_parsing("mistral", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> MistralError {
        ProviderError::timeout(
            "mistral",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Mistral provider implementation (refactored)
#[derive(Debug, Clone)]
pub struct MistralProvider {
    config: MistralConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl MistralProvider {
    /// Create a new Mistral provider instance
    pub async fn new(config: MistralConfig) -> Result<Self, MistralError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("mistral", e))?;

        // Create base HTTP client using our infrastructure
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
        let models = vec![
            ModelInfo {
                id: "mistral-tiny".to_string(),
                name: "Mistral Tiny".to_string(),
                provider: "mistral".to_string(),
                max_context_length: 32000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0002),
                output_cost_per_1k_tokens: Some(0.0006),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "mistral-small".to_string(),
                name: "Mistral Small".to_string(),
                provider: "mistral".to_string(),
                max_context_length: 32000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.001),
                output_cost_per_1k_tokens: Some(0.003),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "mistral-medium".to_string(),
                name: "Mistral Medium".to_string(),
                provider: "mistral".to_string(),
                max_context_length: 32000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0027),
                output_cost_per_1k_tokens: Some(0.0081),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "mistral-large".to_string(),
                name: "Mistral Large".to_string(),
                provider: "mistral".to_string(),
                max_context_length: 32000,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.008),
                output_cost_per_1k_tokens: Some(0.024),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "mistral-embed".to_string(),
                name: "Mistral Embed".to_string(),
                provider: "mistral".to_string(),
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

    /// Check if model is an embedding model
    fn is_embedding_model(&self, model: &str) -> bool {
        model.contains("embed")
    }
}

impl LLMProvider for MistralProvider {
    fn name(&self) -> &'static str {
        "mistral"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        MISTRAL_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "top_p",
            "max_tokens",
            "stream",
            "stop",
            "random_seed",
            "tools",
            "tool_choice",
            "response_format",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        // Mistral uses OpenAI-compatible parameters, so mostly pass-through
        let mut mapped = HashMap::new();

        for (key, value) in params {
            match key.as_str() {
                // Mistral uses 'random_seed' instead of 'seed'
                "seed" => mapped.insert("random_seed".to_string(), value),
                // Direct pass-through for standard parameters
                "temperature" | "top_p" | "max_tokens" | "stream" | "stop" | "tools"
                | "tool_choice" | "response_format" => mapped.insert(key, value),
                // Skip unsupported parameters
                _ => None,
            };
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        // Use the OpenAI transformer from base_provider
        let mut body = OpenAIRequestTransformer::transform_chat_request(&request);

        // Mistral-specific adjustments
        if let Some(seed) = body.get("seed").cloned()
            && let Some(obj) = body.as_object_mut()
        {
            obj.remove("seed");
            obj.insert("random_seed".to_string(), seed);
        }

        Ok(body)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Parse Mistral response (OpenAI-compatible format)
        serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("mistral", e.to_string()))
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(MistralErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Mistral chat request: model={}", request.model);

        if self.is_embedding_model(&request.model) {
            return Err(ProviderError::invalid_request(
                "mistral",
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
            .map_err(|e| ProviderError::network("mistral", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("mistral", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("mistral", e.to_string()))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("Mistral streaming chat request: model={}", request.model);

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
            .map_err(|e| ProviderError::network("mistral", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("mistral", status, &body));
        }

        Ok(crate::core::providers::base::create_provider_sse_stream(
            response, "mistral",
        ))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        debug!("Mistral embedding request: model={}", request.model);

        let body = serde_json::json!({
            "model": request.model,
            "input": request.input,
            "encoding_format": request.encoding_format,
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
            .map_err(|e| ProviderError::network("mistral", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("mistral", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("mistral", e.to_string()))
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
                debug!("Mistral health check failed: status={}", response.status());
                HealthStatus::Unhealthy
            }
            Err(e) => {
                debug!("Mistral health check error: {}", e);
                HealthStatus::Unhealthy
            }
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
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

    fn create_test_config() -> MistralConfig {
        MistralConfig {
            api_key: "test_api_key".to_string(),
            ..Default::default()
        }
    }

    // ==================== Provider Creation Tests ====================

    #[tokio::test]
    async fn test_mistral_provider_creation() {
        let config = MistralConfig {
            api_key: "test_key".to_string(),
            ..Default::default()
        };

        let provider = MistralProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(LLMProvider::name(&provider), "mistral");
        assert!(
            provider
                .capabilities()
                .contains(&ProviderCapability::ChatCompletionStream)
        );
    }

    #[tokio::test]
    async fn test_mistral_provider_creation_custom_base() {
        let config = MistralConfig {
            api_key: "test_key".to_string(),
            api_base: "https://custom.mistral.ai/v1".to_string(),
            ..Default::default()
        };

        let provider = MistralProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_mistral_provider_creation_no_api_key() {
        let config = MistralConfig::default();
        let provider = MistralProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_mistral_provider_creation_empty_api_key() {
        let config = MistralConfig {
            api_key: "".to_string(),
            ..Default::default()
        };

        let provider = MistralProvider::new(config).await;
        assert!(provider.is_err());
    }

    // ==================== Config Validation Tests ====================

    #[test]
    fn test_mistral_config_validation() {
        let mut config = MistralConfig::default();
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
    fn test_mistral_config_default() {
        let config = MistralConfig::default();

        assert_eq!(config.api_key, "");
        assert_eq!(config.api_base, "https://api.mistral.ai/v1");
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_mistral_config_provider_config_trait() {
        let config = create_test_config();

        assert_eq!(config.api_key(), Some("test_api_key"));
        assert_eq!(config.api_base(), Some("https://api.mistral.ai/v1"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(30));
        assert_eq!(config.max_retries(), 3);
    }

    #[test]
    fn test_mistral_config_validation_max_retries_boundary() {
        let mut config = create_test_config();

        config.max_retries = 10;
        assert!(config.validate().is_ok());

        config.max_retries = 11;
        assert!(config.validate().is_err());
    }

    // ==================== Provider Capabilities Tests ====================

    #[tokio::test]
    async fn test_provider_name() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();
        assert_eq!(provider.name(), "mistral");
    }

    #[tokio::test]
    async fn test_provider_capabilities() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::ToolCalling));
        assert!(caps.contains(&ProviderCapability::Embeddings));
        assert_eq!(caps.len(), 4);
    }

    #[tokio::test]
    async fn test_provider_models() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "mistral-tiny"));
        assert!(models.iter().any(|m| m.id == "mistral-small"));
        assert!(models.iter().any(|m| m.id == "mistral-medium"));
        assert!(models.iter().any(|m| m.id == "mistral-large"));
        assert!(models.iter().any(|m| m.id == "mistral-embed"));
    }

    #[tokio::test]
    async fn test_provider_models_have_pricing() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        for model in models {
            assert_eq!(model.provider, "mistral");
            assert!(model.input_cost_per_1k_tokens.is_some());
            assert!(model.output_cost_per_1k_tokens.is_some());
        }
    }

    // ==================== Supported Params Tests ====================

    #[tokio::test]
    async fn test_get_supported_openai_params() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();
        let params = provider.get_supported_openai_params("mistral-large");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"top_p"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"stream"));
        assert!(params.contains(&"stop"));
        assert!(params.contains(&"random_seed"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"tool_choice"));
        assert!(params.contains(&"response_format"));
    }

    // ==================== Map OpenAI Params Tests ====================

    #[tokio::test]
    async fn test_map_openai_params_seed_to_random_seed() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("seed".to_string(), serde_json::json!(42));

        let mapped = provider
            .map_openai_params(params, "mistral-large")
            .await
            .unwrap();

        assert!(!mapped.contains_key("seed"));
        assert!(mapped.contains_key("random_seed"));
        assert_eq!(mapped.get("random_seed").unwrap(), &serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_map_openai_params_passthrough() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.7));
        params.insert("max_tokens".to_string(), serde_json::json!(100));
        params.insert("top_p".to_string(), serde_json::json!(0.9));

        let mapped = provider
            .map_openai_params(params, "mistral-large")
            .await
            .unwrap();

        assert_eq!(mapped.get("temperature").unwrap(), &serde_json::json!(0.7));
        assert_eq!(mapped.get("max_tokens").unwrap(), &serde_json::json!(100));
        assert_eq!(mapped.get("top_p").unwrap(), &serde_json::json!(0.9));
    }

    #[tokio::test]
    async fn test_map_openai_params_unsupported_filtered() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let mut params = HashMap::new();
        params.insert("unsupported_param".to_string(), serde_json::json!("value"));
        params.insert("temperature".to_string(), serde_json::json!(0.5));

        let mapped = provider
            .map_openai_params(params, "mistral-large")
            .await
            .unwrap();

        assert!(!mapped.contains_key("unsupported_param"));
        assert!(mapped.contains_key("temperature"));
    }

    // ==================== Transform Request Tests ====================

    #[tokio::test]
    async fn test_transform_request_basic() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "mistral-large".to_string(),
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
        assert_eq!(transformed["model"], "mistral-large");
        assert!(transformed["messages"].is_array());
    }

    #[tokio::test]
    async fn test_transform_request_with_seed() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "mistral-large".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            seed: Some(42),
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.transform_request(request, context).await;

        assert!(result.is_ok());
        let transformed = result.unwrap();
        // Seed should be transformed to random_seed
        assert!(transformed.get("seed").is_none() || transformed["random_seed"].is_number());
    }

    // ==================== Is Embedding Model Tests ====================

    #[tokio::test]
    async fn test_is_embedding_model() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        assert!(provider.is_embedding_model("mistral-embed"));
        assert!(provider.is_embedding_model("text-embedding-model"));
        assert!(!provider.is_embedding_model("mistral-large"));
        assert!(!provider.is_embedding_model("mistral-small"));
    }

    // ==================== Cost Calculation Tests ====================

    #[tokio::test]
    async fn test_calculate_cost_known_model() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("mistral-large", 1000, 500).await;
        assert!(matches!(cost, Ok(v) if v >= 0.0));
    }

    #[tokio::test]
    async fn test_calculate_cost_embed_model() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("mistral-embed", 1000, 0).await;
        assert!(matches!(cost, Ok(v) if v >= 0.0));
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("unknown-model", 1000, 500).await;
        assert!(matches!(cost, Ok(v) if v >= 0.0));
    }

    #[tokio::test]
    async fn test_calculate_cost_zero_tokens() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();

        let cost = provider.calculate_cost("mistral-large", 0, 0).await;
        assert!(cost.is_ok());
        assert!((cost.unwrap() - 0.0).abs() < 0.0001);
    }

    // ==================== Error Mapper Tests ====================

    #[test]
    fn test_error_mapper_authentication() {
        let mapper = MistralErrorMapper;
        let error = mapper.map_http_error(401, "Unauthorized");

        match error {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "mistral");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_error_mapper_rate_limit() {
        let mapper = MistralErrorMapper;
        let error = mapper.map_http_error(429, "Rate limit exceeded");

        match error {
            ProviderError::RateLimit { provider, .. } => {
                assert_eq!(provider, "mistral");
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_error_mapper_network_error() {
        let mapper = MistralErrorMapper;
        let error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let mapped = mapper.map_network_error(&error);

        match mapped {
            ProviderError::Network { provider, .. } => {
                assert_eq!(provider, "mistral");
            }
            _ => panic!("Expected Network error"),
        }
    }

    #[test]
    fn test_error_mapper_parsing_error() {
        let mapper = MistralErrorMapper;
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid JSON");
        let mapped = mapper.map_parsing_error(&error);

        match mapped {
            ProviderError::ResponseParsing { provider, .. } => {
                assert_eq!(provider, "mistral");
            }
            _ => panic!("Expected ResponseParsing error"),
        }
    }

    #[test]
    fn test_error_mapper_timeout_error() {
        let mapper = MistralErrorMapper;
        let mapped = mapper.map_timeout_error(std::time::Duration::from_secs(30));

        match mapped {
            ProviderError::Timeout { provider, .. } => {
                assert_eq!(provider, "mistral");
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    // ==================== Get Error Mapper Tests ====================

    #[tokio::test]
    async fn test_get_error_mapper() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();
        let _mapper = provider.get_error_mapper();
        // Just verify it doesn't panic
    }

    // ==================== Clone/Debug Tests ====================

    #[tokio::test]
    async fn test_provider_clone() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();
        let cloned = provider.clone();

        assert_eq!(provider.name(), cloned.name());
        assert_eq!(provider.models().len(), cloned.models().len());
    }

    #[tokio::test]
    async fn test_provider_debug() {
        let provider = MistralProvider::new(create_test_config()).await.unwrap();
        let debug_str = format!("{:?}", provider);

        assert!(debug_str.contains("MistralProvider"));
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

        assert!(debug_str.contains("MistralConfig"));
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["api_key"], "test_api_key");
        assert_eq!(json["api_base"], "https://api.mistral.ai/v1");
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

        let config: MistralConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, "my_key");
        assert_eq!(config.api_base, "https://custom.api.com");
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.max_retries, 5);
    }
}
