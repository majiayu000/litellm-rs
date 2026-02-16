//! Cohere Provider Implementation
//!
//! Main provider implementation integrating all Cohere capabilities:
//! - Chat completions (Command models)
//! - Embeddings (embed models)
//! - Reranking (rerank models)

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

use super::chat::CohereChatHandler;
use super::config::CohereConfig;
use super::embed::CohereEmbeddingHandler;
use super::rerank::{CohereRerankHandler, RerankRequest, RerankResponse};
use super::streaming::create_cohere_stream;
use crate::core::providers::base_provider::{
    BaseHttpClient, BaseProviderConfig, CostCalculator, HeaderBuilder, HttpErrorMapper,
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

// Static capabilities
const COHERE_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::Embeddings,
];

/// Cohere error mapper
pub struct CohereErrorMapper;

impl ErrorMapper<ProviderError> for CohereErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        HttpErrorMapper::map_status_code("cohere", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> ProviderError {
        HttpErrorMapper::parse_json_error("cohere", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::network("cohere", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::response_parsing("cohere", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> ProviderError {
        ProviderError::timeout(
            "cohere",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Cohere provider implementation
#[derive(Debug, Clone)]
pub struct CohereProvider {
    config: CohereConfig,
    client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl CohereProvider {
    /// Create a new Cohere provider instance
    pub async fn new(config: CohereConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("cohere", e))?;

        let base_config = BaseProviderConfig {
            api_key: Some(config.api_key.clone()),
            api_base: Some(config.api_base.clone()),
            timeout: Some(config.timeout_seconds),
            max_retries: Some(config.max_retries),
            headers: None,
            organization: None,
            api_version: None,
        };

        let client = BaseHttpClient::new(base_config)?;

        let models = Self::create_model_registry();

        Ok(Self {
            config,
            client,
            models,
        })
    }

    /// Create provider with API key
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = CohereConfig::new(api_key);
        Self::new(config).await
    }

    /// Create the model registry with all supported models
    fn create_model_registry() -> Vec<ModelInfo> {
        vec![
            // Command models (Chat)
            ModelInfo {
                id: "command-r-plus".to_string(),
                name: "Command R+".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 128000,
                max_output_length: Some(4096),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.003),
                output_cost_per_1k_tokens: Some(0.015),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "command-r".to_string(),
                name: "Command R".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 128000,
                max_output_length: Some(4096),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0005),
                output_cost_per_1k_tokens: Some(0.0015),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "command".to_string(),
                name: "Command".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 4096,
                max_output_length: Some(4096),
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
                id: "command-light".to_string(),
                name: "Command Light".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 4096,
                max_output_length: Some(4096),
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0003),
                output_cost_per_1k_tokens: Some(0.0006),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            // Embedding models
            ModelInfo {
                id: "embed-english-v3.0".to_string(),
                name: "Embed English v3.0".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 512,
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
            ModelInfo {
                id: "embed-multilingual-v3.0".to_string(),
                name: "Embed Multilingual v3.0".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 512,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: true, // Supports images
                input_cost_per_1k_tokens: Some(0.0001),
                output_cost_per_1k_tokens: Some(0.0),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "embed-english-light-v3.0".to_string(),
                name: "Embed English Light v3.0".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 512,
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
            ModelInfo {
                id: "embed-multilingual-light-v3.0".to_string(),
                name: "Embed Multilingual Light v3.0".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 512,
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
            // Rerank models
            ModelInfo {
                id: "rerank-english-v3.0".to_string(),
                name: "Rerank English v3.0".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 4096,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.002),
                output_cost_per_1k_tokens: Some(0.0),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "rerank-multilingual-v3.0".to_string(),
                name: "Rerank Multilingual v3.0".to_string(),
                provider: "cohere".to_string(),
                max_context_length: 4096,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.002),
                output_cost_per_1k_tokens: Some(0.0),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
        ]
    }

    /// Check if model is an embedding model
    fn is_embedding_model(&self, model: &str) -> bool {
        model.contains("embed")
    }

    /// Check if model is a rerank model
    fn is_rerank_model(&self, model: &str) -> bool {
        model.contains("rerank")
    }

    /// Get config reference
    pub fn config(&self) -> &CohereConfig {
        &self.config
    }

    /// Execute a rerank request
    pub async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, ProviderError> {
        debug!("Cohere rerank request: model={}", request.model);

        let body = CohereRerankHandler::transform_request(&request)?;

        let url = self.config.rerank_endpoint();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("cohere", e.to_string()))?;

        let response = self
            .client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("cohere", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("cohere", status, body));
        }

        let response_json: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("cohere", e.to_string()))?;

        CohereRerankHandler::transform_response(response_json)
    }
}

#[async_trait]
impl LLMProvider for CohereProvider {
    type Config = CohereConfig;
    type Error = ProviderError;
    type ErrorMapper = CohereErrorMapper;

    fn name(&self) -> &'static str {
        "cohere"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        COHERE_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        if self.is_embedding_model(model) {
            CohereEmbeddingHandler::get_supported_params()
        } else {
            CohereChatHandler::get_supported_params()
        }
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        Ok(CohereChatHandler::map_openai_params(params))
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        CohereChatHandler::transform_request(&request, &self.config)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response_json: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("cohere", e.to_string()))?;

        CohereChatHandler::transform_response(response_json, model)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        CohereErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Cohere chat request: model={}", request.model);

        if self.is_embedding_model(&request.model) {
            return Err(ProviderError::invalid_request(
                "cohere",
                "Use embeddings endpoint for embedding models",
            ));
        }

        if self.is_rerank_model(&request.model) {
            return Err(ProviderError::invalid_request(
                "cohere",
                "Use rerank endpoint for rerank models",
            ));
        }

        let body = self.transform_request(request.clone(), context).await?;

        let url = self.config.chat_endpoint();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("cohere", e.to_string()))?;

        let response = self
            .client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("cohere", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("cohere", status, body));
        }

        let response_json: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("cohere", e.to_string()))?;

        CohereChatHandler::transform_response(response_json, &request.model)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Cohere streaming chat request: model={}", request.model);

        let mut body = self.transform_request(request.clone(), context).await?;
        body["stream"] = serde_json::json!(true);

        let url = self.config.chat_endpoint();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("cohere", e.to_string()))?;

        let response = self
            .client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("cohere", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("cohere", status, body));
        }

        // Create stream using unified SSE infrastructure
        let use_v2 = matches!(self.config.api_version, super::config::CohereApiVersion::V2);
        let stream = create_cohere_stream(response.bytes_stream(), &request.model, use_v2);

        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        debug!("Cohere embedding request: model={}", request.model);

        let body = CohereEmbeddingHandler::transform_request(&request, &self.config)?;

        let url = self.config.embed_endpoint();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("cohere", e.to_string()))?;

        let response = self
            .client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("cohere", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("cohere", status, body));
        }

        let response_json: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("cohere", e.to_string()))?;

        // Get input count for usage estimation
        let input_count = match &request.input {
            crate::core::types::embedding::EmbeddingInput::Text(_) => 1,
            crate::core::types::embedding::EmbeddingInput::Array(arr) => arr.len(),
        };

        CohereEmbeddingHandler::transform_response(response_json, &request.model, input_count)
    }

    async fn health_check(&self) -> HealthStatus {
        let url = self.config.models_endpoint();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .build_reqwest();

        match headers {
            Ok(headers) => match self.client.inner().get(&url).headers(headers).send().await {
                Ok(response) if response.status().is_success() => HealthStatus::Healthy,
                Ok(response) => {
                    debug!("Cohere health check failed: status={}", response.status());
                    HealthStatus::Unhealthy
                }
                Err(e) => {
                    debug!("Cohere health check error: {}", e);
                    HealthStatus::Unhealthy
                }
            },
            Err(_) => HealthStatus::Unhealthy,
        }
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
            .ok_or_else(|| ProviderError::model_not_found("cohere", model.to_string()))?;

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

    fn create_test_config() -> CohereConfig {
        CohereConfig::new("test_api_key")
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let provider = CohereProvider::new(create_test_config()).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "cohere");
    }

    #[tokio::test]
    async fn test_provider_with_api_key() {
        let provider = CohereProvider::with_api_key("test_key").await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_provider_creation_no_api_key() {
        let config = CohereConfig::default();
        let provider = CohereProvider::new(config).await;
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_provider_capabilities() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::Embeddings));
        assert!(caps.contains(&ProviderCapability::ToolCalling));
    }

    #[tokio::test]
    async fn test_provider_models() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "command-r-plus"));
        assert!(models.iter().any(|m| m.id == "command-r"));
        assert!(models.iter().any(|m| m.id == "embed-english-v3.0"));
        assert!(models.iter().any(|m| m.id == "rerank-english-v3.0"));
    }

    #[tokio::test]
    async fn test_is_embedding_model() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();

        assert!(provider.is_embedding_model("embed-english-v3.0"));
        assert!(provider.is_embedding_model("embed-multilingual-v3.0"));
        assert!(!provider.is_embedding_model("command-r-plus"));
    }

    #[tokio::test]
    async fn test_is_rerank_model() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();

        assert!(provider.is_rerank_model("rerank-english-v3.0"));
        assert!(provider.is_rerank_model("rerank-multilingual-v3.0"));
        assert!(!provider.is_rerank_model("command-r"));
    }

    #[tokio::test]
    async fn test_get_supported_openai_params_chat() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();
        let params = provider.get_supported_openai_params("command-r-plus");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
    }

    #[tokio::test]
    async fn test_get_supported_openai_params_embed() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();
        let params = provider.get_supported_openai_params("embed-english-v3.0");

        assert!(params.contains(&"encoding_format"));
        assert!(params.contains(&"dimensions"));
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();

        let cost = provider
            .calculate_cost("command-r-plus", 1000, 500)
            .await
            .unwrap();

        // command-r-plus: $0.003 input, $0.015 output per 1k
        // (1000/1000 * 0.003) + (500/1000 * 0.015) = 0.003 + 0.0075 = 0.0105
        assert!((cost - 0.0105).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();

        let result = provider.calculate_cost("unknown-model", 1000, 500).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transform_request() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();

        let request = ChatRequest {
            model: "command-r-plus".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.transform_request(request, context).await;

        assert!(result.is_ok());
        let transformed = result.unwrap();
        assert_eq!(transformed["model"], "command-r-plus");
        assert!((transformed["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
        assert_eq!(transformed["max_tokens"], 100);
    }

    #[tokio::test]
    async fn test_provider_clone() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();
        let cloned = provider.clone();

        assert_eq!(provider.name(), cloned.name());
        assert_eq!(provider.models().len(), cloned.models().len());
    }

    #[tokio::test]
    async fn test_error_mapper() {
        let provider = CohereProvider::new(create_test_config()).await.unwrap();
        let mapper = provider.get_error_mapper();

        let error = mapper.map_http_error(401, "Unauthorized");
        assert_eq!(error.provider(), "cohere");

        let error = mapper.map_http_error(429, "Rate limited");
        assert_eq!(error.provider(), "cohere");
    }
}
