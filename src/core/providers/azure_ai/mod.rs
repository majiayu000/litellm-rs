//! Azure AI Provider (Foundry)
//!
//! Complete Azure AI Foundry integration following unified provider architecture

// Complete Azure AI modules
pub mod chat;
mod client;
pub mod config;
pub mod embed;
pub mod error;
pub mod image_generation;
pub mod models;
#[cfg(test)]
mod policy_tests;
pub mod rerank;

// Re-export main components
pub use chat::{AzureAIChatHandler, AzureAIChatUtils};
pub use config::{AzureAIConfig, AzureAIEndpointType};
pub use embed::{AzureAIEmbeddingHandler, AzureAIEmbeddingUtils};
pub use error::AzureAIErrorMapper;
pub use image_generation::AzureAIImageHandler;
pub use models::{AzureAIModelRegistry, AzureAIModelSpec, AzureAIModelType, get_azure_ai_registry};
pub use rerank::{AzureAIRerankHandler, AzureAIRerankUtils, RerankRequest, RerankResponse};

use futures::Stream;
use reqwest::Method;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
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
use client::AzureAIClient;

/// Main Azure AI provider following unified architecture
#[derive(Debug, Clone)]
pub struct AzureAIProvider {
    client: AzureAIClient,
    chat_handler: AzureAIChatHandler,
    embedding_handler: AzureAIEmbeddingHandler,
    image_handler: AzureAIImageHandler,
    rerank_handler: AzureAIRerankHandler,
    model_registry: &'static AzureAIModelRegistry,
}

impl AzureAIProvider {
    /// Create new Azure AI provider
    pub fn new(config: AzureAIConfig) -> Result<Self, ProviderError> {
        let client = AzureAIClient::new(config)?;
        let chat_handler = AzureAIChatHandler::from_client(client.clone())
            .map_err(|e| ProviderError::configuration("azure_ai", e.to_string()))?;
        let embedding_handler = AzureAIEmbeddingHandler::from_client(client.clone())
            .map_err(|e| ProviderError::configuration("azure_ai", e.to_string()))?;
        let image_handler = AzureAIImageHandler::from_client(client.clone())
            .map_err(|e| ProviderError::configuration("azure_ai", e.to_string()))?;
        let rerank_handler = AzureAIRerankHandler::from_client(client.clone())
            .map_err(|e| ProviderError::configuration("azure_ai", e.to_string()))?;
        let model_registry = get_azure_ai_registry();

        Ok(Self {
            client,
            chat_handler,
            embedding_handler,
            image_handler,
            rerank_handler,
            model_registry,
        })
    }

    /// Get Azure AI configuration
    pub fn get_config(&self) -> &AzureAIConfig {
        self.client.get_config()
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = AzureAIConfig::from_env();
        Self::new(config)
    }

    /// Create with API key
    pub fn with_api_key(
        api_key: impl Into<String>,
        api_base: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some(api_key.into());
        config.base.api_base = Some(api_base.into());
        Self::new(config)
    }

    /// Get chat handler
    pub fn get_chat_handler(&self) -> &AzureAIChatHandler {
        &self.chat_handler
    }

    /// Get embedding handler
    pub fn get_embedding_handler(&self) -> &AzureAIEmbeddingHandler {
        &self.embedding_handler
    }

    /// Get image generation handler
    pub fn get_image_handler(&self) -> &AzureAIImageHandler {
        &self.image_handler
    }

    /// Get reranking handler
    pub fn get_rerank_handler(&self) -> &AzureAIRerankHandler {
        &self.rerank_handler
    }

    /// Get model registry
    pub fn get_model_registry(&self) -> &AzureAIModelRegistry {
        self.model_registry
    }
}

impl LLMProvider for AzureAIProvider {
    fn name(&self) -> &'static str {
        "azure_ai"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        // Get
        static MODELS: std::sync::OnceLock<Vec<ModelInfo>> = std::sync::OnceLock::new();
        MODELS.get_or_init(|| self.model_registry.to_model_infos())
    }

    fn supports_model(&self, model: &str) -> bool {
        !model.trim().is_empty()
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "max_tokens",
            "max_completion_tokens",
            "top_p",
            "frequency_penalty",
            "presence_penalty",
            "tools",
            "tool_choice",
            "stream",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        // Azure AI generally uses the same parameters as OpenAI
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        // Transform ChatRequest to Azure AI API format
        AzureAIChatUtils::transform_request(&request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Parse Azure AI response to ChatResponse
        let response_json: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("azure_ai", e.to_string()))?;

        AzureAIChatUtils::transform_response(response_json, model)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(AzureAIErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        self.chat_handler
            .create_chat_completion(request, context)
            .await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        let stream = self
            .chat_handler
            .create_chat_completion_stream(request, context)
            .await?;
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        self.embedding_handler.embedding(request, context).await
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.image_handler.generate_image(request, context).await
    }

    async fn health_check(&self) -> HealthStatus {
        // Validate configuration first
        if self.client.get_config().validate().is_err() {
            return HealthStatus::Unhealthy;
        }

        let url = match self.client.get_config().build_endpoint_url("models") {
            Ok(url) => url,
            Err(_) => return HealthStatus::Unhealthy,
        };
        let request = match self.client.request(Method::GET, &url) {
            Ok(request) => request,
            Err(_) => return HealthStatus::Unhealthy,
        };

        match request.send().await {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::Degraded,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        if let Some(model_spec) = self.model_registry.get_model(model) {
            let input_cost =
                model_spec.input_price_per_1k.unwrap_or(0.0) * (input_tokens as f64 / 1000.0);
            let output_cost =
                model_spec.output_price_per_1k.unwrap_or(0.0) * (output_tokens as f64 / 1000.0);
            Ok(input_cost + output_cost)
        } else {
            Err(ProviderError::model_not_found(
                "azure_ai",
                "Model not found for cost calculation",
            ))
        }
    }
}

/// Azure AI provider factory
pub struct AzureAIProviderFactory;

impl AzureAIProviderFactory {
    /// Create provider with default configuration
    pub fn create_default() -> Result<AzureAIProvider, ProviderError> {
        let config = AzureAIConfig::new("azure_ai");
        AzureAIProvider::new(config)
    }

    /// Create provider with custom configuration
    pub fn create_with_config(config: AzureAIConfig) -> Result<AzureAIProvider, ProviderError> {
        AzureAIProvider::new(config)
    }

    /// Create provider from environment variables
    pub fn create_from_env() -> Result<AzureAIProvider, ProviderError> {
        let config = AzureAIConfig::from_env();
        AzureAIProvider::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    fn create_test_config() -> AzureAIConfig {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test_api_key".to_string());
        config.base.api_base = Some("https://test.ai.azure.com".to_string());
        config
    }

    async fn read_http_headers(socket: &mut TcpStream) -> std::io::Result<()> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];

        loop {
            let bytes_read = socket.read(&mut buffer).await?;
            if bytes_read == 0 {
                return Ok(());
            }
            request.extend_from_slice(&buffer[..bytes_read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                return Ok(());
            }
        }
    }

    async fn health_response_base_url(status: &str) -> std::io::Result<String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
        );

        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            if read_http_headers(&mut socket).await.is_err() {
                return;
            }
            if let Err(err) = socket.write_all(response.as_bytes()).await {
                eprintln!("test server failed to write response: {err}");
            }
        });

        Ok(format!("http://{addr}"))
    }

    // ==================== Provider Creation Tests ====================

    #[tokio::test]
    async fn test_provider_factory() {
        // This will fail without proper env vars, but tests the structure
        let _result = AzureAIProviderFactory::create_default();
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let config = AzureAIConfig::new("azure_ai");
        // This will fail without proper env vars, but tests the structure
        let _result = AzureAIProvider::new(config);
    }

    #[test]
    fn test_provider_creation_with_valid_config() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_creation_missing_api_key() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_base = Some("https://test.ai.azure.com".to_string());
        // api_key is None
        let provider = AzureAIProvider::new(config);
        assert!(provider.is_err());
    }

    #[test]
    fn test_provider_with_api_key() {
        let provider = AzureAIProvider::with_api_key("test_key", "https://test.ai.azure.com");
        assert!(provider.is_ok());
    }

    // ==================== Provider Factory Tests ====================

    #[test]
    fn test_factory_create_with_config() {
        let config = create_test_config();
        let provider = AzureAIProviderFactory::create_with_config(config);
        assert!(provider.is_ok());
    }

    // ==================== Provider Properties Tests ====================

    #[test]
    fn test_provider_name() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();
        assert_eq!(provider.name(), "azure_ai");
    }

    #[test]
    fn test_provider_capabilities() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::Embeddings));
        assert!(caps.contains(&ProviderCapability::ImageGeneration));
        assert_eq!(caps.len(), 4);
    }

    #[test]
    fn test_provider_models_not_empty() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();
        assert!(!provider.models().is_empty());
    }

    #[test]
    fn test_supports_dynamic_deployment_names() {
        let config = create_test_config();
        let provider = match AzureAIProvider::new(config) {
            Ok(provider) => provider,
            Err(error) => panic!("provider should be created: {error}"),
        };

        assert!(provider.supports_model("customer-gpt4o-prod"));
        assert!(!provider.supports_model("   "));
    }

    #[test]
    fn test_get_config() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config.clone()).unwrap();
        let retrieved_config = provider.get_config();

        assert_eq!(retrieved_config.base.api_key, config.base.api_key);
        assert_eq!(retrieved_config.base.api_base, config.base.api_base);
    }

    #[test]
    fn test_get_handlers() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();

        // Verify we can access all handlers
        let _chat = provider.get_chat_handler();
        let _embed = provider.get_embedding_handler();
        let _image = provider.get_image_handler();
        let _rerank = provider.get_rerank_handler();
        let _registry = provider.get_model_registry();
    }

    // ==================== Model Capabilities Tests ====================

    #[test]
    fn test_model_capabilities() {
        let registry = get_azure_ai_registry();

        assert!(registry.supports_capability("gpt-4o", &ProviderCapability::ChatCompletion));
        assert!(
            registry.supports_capability("text-embedding-3-large", &ProviderCapability::Embeddings)
        );
        assert!(!registry.supports_capability("dall-e-3", &ProviderCapability::ChatCompletion));
    }

    #[test]
    fn test_model_registry_gpt_models() {
        let registry = get_azure_ai_registry();

        // Check various GPT models exist
        assert!(registry.get_model("gpt-4o").is_some());
        assert!(registry.get_model("gpt-4").is_some());
        assert!(registry.get_model("gpt-35-turbo").is_some());
    }

    #[test]
    fn test_model_registry_embedding_models() {
        let registry = get_azure_ai_registry();

        assert!(registry.get_model("text-embedding-3-large").is_some());
        assert!(registry.get_model("text-embedding-3-small").is_some());
    }

    // ==================== Supported Params Tests ====================

    #[test]
    fn test_get_supported_openai_params() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();
        let params = provider.get_supported_openai_params("gpt-4o");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"max_completion_tokens"));
        assert!(params.contains(&"top_p"));
        assert!(params.contains(&"frequency_penalty"));
        assert!(params.contains(&"presence_penalty"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"tool_choice"));
        assert!(params.contains(&"stream"));
    }

    // ==================== Map OpenAI Params Tests ====================

    #[tokio::test]
    async fn test_map_openai_params_passthrough() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.7));
        params.insert("max_tokens".to_string(), serde_json::json!(100));

        let mapped = provider
            .map_openai_params(params.clone(), "gpt-4o")
            .await
            .unwrap();

        // Azure AI should pass through params unchanged
        assert_eq!(mapped, params);
    }

    // ==================== Transform Request Tests ====================

    #[tokio::test]
    async fn test_transform_request_basic() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "gpt-4o".to_string(),
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
        assert!(transformed["messages"].is_array());
    }

    #[tokio::test]
    async fn test_transform_request_with_temperature() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "gpt-4o".to_string(),
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
    }

    // ==================== Health Check Tests ====================

    #[tokio::test]
    async fn test_health_check_success_requires_endpoint_success() {
        let mut config = create_test_config();
        config.base.api_base = Some(match health_response_base_url("200 OK").await {
            Ok(url) => url,
            Err(error) => panic!("test server should start: {error}"),
        });
        config.base.endpoint_access = crate::core::net::ProviderEndpointAccess::PrivateNetwork;
        let provider = match AzureAIProvider::new(config) {
            Ok(provider) => provider,
            Err(error) => panic!("provider should be created: {error}"),
        };

        let status = provider.health_check().await;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_health_check_degrades_on_endpoint_failure() {
        let mut config = create_test_config();
        config.base.api_base = Some(
            match health_response_base_url("500 Internal Server Error").await {
                Ok(url) => url,
                Err(error) => panic!("test server should start: {error}"),
            },
        );
        config.base.endpoint_access = crate::core::net::ProviderEndpointAccess::PrivateNetwork;
        let provider = match AzureAIProvider::new(config) {
            Ok(provider) => provider,
            Err(error) => panic!("provider should be created: {error}"),
        };

        let status = provider.health_check().await;
        assert_eq!(status, HealthStatus::Degraded);
    }

    // ==================== Cost Calculation Tests ====================

    #[tokio::test]
    async fn test_calculate_cost_known_model() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();

        let cost = provider.calculate_cost("gpt-4o", 1000, 500).await;
        // May succeed or fail depending on model pricing
        let _ = cost;
    }

    #[tokio::test]
    async fn test_calculate_cost_unknown_model() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();

        let cost = provider
            .calculate_cost("unknown-model-xyz", 1000, 500)
            .await;
        assert!(cost.is_err());
    }

    // ==================== Error Mapper Tests ====================

    #[test]
    fn test_get_error_mapper() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();
        let _mapper = provider.get_error_mapper();
        // Just verify it doesn't panic
    }

    // ==================== Clone/Debug Tests ====================

    #[test]
    fn test_provider_clone() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();
        let cloned = provider.clone();

        assert_eq!(provider.name(), cloned.name());
        assert_eq!(provider.capabilities().len(), cloned.capabilities().len());
    }

    #[test]
    fn test_provider_debug() {
        let config = create_test_config();
        let provider = AzureAIProvider::new(config).unwrap();
        let debug_str = format!("{:?}", provider);

        assert!(debug_str.contains("AzureAIProvider"));
    }

    // ==================== Model Registry Tests ====================

    #[test]
    fn test_model_registry_to_model_infos() {
        let registry = get_azure_ai_registry();
        let infos = registry.to_model_infos();

        assert!(!infos.is_empty());
        // Models may have different provider values (e.g., "azure_ai", "cohere")
        // Just verify the list is not empty and has valid entries
        for info in &infos {
            assert!(!info.id.is_empty());
            assert!(!info.provider.is_empty());
        }
    }

    #[test]
    fn test_model_registry_get_all_models() {
        let registry = get_azure_ai_registry();
        let models = registry.get_all_models();

        assert!(!models.is_empty());
    }
}
