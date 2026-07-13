//! Azure OpenAI Provider
//!
//! Azure OpenAI integration for LiteLLM using LLMProvider trait

pub mod assistants;
pub mod batches;
pub mod chat;
pub mod client;
pub mod config;
pub mod embed;
pub mod error;
pub mod image;
pub mod responses;
pub mod utils;

// Re-export core utilities
pub use crate::core::providers::unified_provider::ProviderError;
pub use client::{AzureClient, AzureConfigFactory, AzureRateLimitInfo};
pub use config::{AzureConfig, AzureModelInfo};
pub use error::{
    AzureErrorMapper, azure_ad_error, azure_api_error, azure_config_error, azure_deployment_error,
    azure_header_error,
};
pub use utils::{AzureEndpointType, AzureUtils};

// Use the new unified cost calculation system
pub use crate::core::cost::providers::azure::{
    AzureCostCalculator, cost_per_token, get_azure_model_pricing,
};

// Re-export assistant functionality
pub use assistants::{AzureAssistantHandler, AzureAssistantUtils};

// Re-export batch functionality
pub use batches::{AzureBatchHandler, AzureBatchUtils};

// Re-export chat functionality
pub use chat::{AzureChatHandler, AzureChatUtils};

// Re-export embedding functionality
pub use embed::{AzureEmbeddingHandler, AzureEmbeddingUtils};

// Re-export image functionality
pub use image::{AzureImageHandler, AzureImageUtils};

// Re-export response processing functionality
pub use responses::{AzureResponseHandler, AzureResponseProcessor, AzureResponseUtils};

use futures::Stream;
use reqwest::Method;
use serde_json::Value;
use std::pin::Pin;

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

use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

/// Main Azure OpenAI provider - complete implementation
#[derive(Debug, Clone)]
pub struct AzureOpenAIProvider {
    config: AzureConfig,
    chat_handler: AzureChatHandler,
    embedding_handler: AzureEmbeddingHandler,
    image_handler: AzureImageHandler,
    cost_calculator: AzureCostCalculator,
}

impl AzureOpenAIProvider {
    /// Create new Azure OpenAI provider
    pub fn new(config: AzureConfig) -> Result<Self, ProviderError> {
        let chat_handler = AzureChatHandler::new(config)?;
        let config = chat_handler.policy_client().get_config().clone();
        let embedding_handler = AzureEmbeddingHandler::new(config.clone())?;
        let image_handler = AzureImageHandler::new(config.clone())?;
        let cost_calculator = AzureCostCalculator::new();

        Ok(Self {
            config,
            chat_handler,
            embedding_handler,
            image_handler,
            cost_calculator,
        })
    }

    /// Create from configuration
    pub fn from_config(config: AzureConfig) -> Result<Self, ProviderError> {
        Self::new(config)
    }

    /// Get Azure configuration
    pub fn get_azure_config(&self) -> &AzureConfig {
        &self.config
    }

    /// Get cost calculator
    pub fn get_cost_calculator(&self) -> &AzureCostCalculator {
        &self.cost_calculator
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = AzureConfig::new();
        Self::new(config)
    }

    /// Create with API key
    pub fn with_api_key(
        api_key: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let config = AzureConfig::new()
            .with_api_key(api_key.into())
            .with_azure_endpoint(endpoint.into());
        Self::new(config)
    }
}

fn build_azure_models_health_url(azure_endpoint: &str, api_version: &str) -> String {
    let base = azure_endpoint.trim_end_matches('/');
    let resource_base = base
        .split_once("/openai/deployments/")
        .map(|(resource_base, _)| resource_base)
        .unwrap_or(base);

    if resource_base.ends_with("/openai") {
        format!("{}/models?api-version={}", resource_base, api_version)
    } else {
        format!(
            "{}/openai/models?api-version={}",
            resource_base, api_version
        )
    }
}

// Azure error mapper is now re-exported from common_utils

/// Implement the unified LLMProvider trait for AzureOpenAIProvider
impl LLMProvider for AzureOpenAIProvider {
    fn name(&self) -> &'static str {
        "azure_openai"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        static CAPABILITIES: &[ProviderCapability] = &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
            ProviderCapability::FunctionCalling,
            ProviderCapability::ToolCalling,
        ];
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        // Return empty for now - Azure uses deployment names
        &[]
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
            "stream",
            "functions",
            "function_call",
            "tools",
            "tool_choice",
        ]
    }

    async fn map_openai_params(
        &self,
        params: std::collections::HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, ProviderError> {
        // Azure OpenAI API is largely compatible with OpenAI, minimal mapping needed
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        self.chat_handler.transform_request(&request)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let response_json: Value = serde_json::from_slice(raw_response)?;
        self.chat_handler.transform_response(response_json, model)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(AzureErrorMapper)
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        // Basic cost calculation for Azure OpenAI models
        let cost = match model {
            "gpt-35-turbo" => {
                (input_tokens as f64 * 0.0015 + output_tokens as f64 * 0.002) / 1000.0
            }
            "gpt-4" => (input_tokens as f64 * 0.03 + output_tokens as f64 * 0.06) / 1000.0,
            "gpt-4-turbo" => (input_tokens as f64 * 0.01 + output_tokens as f64 * 0.03) / 1000.0,
            _ => 0.0,
        };
        Ok(cost)
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
        self.chat_handler
            .create_chat_completion_stream(request, context)
            .await
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        self.embedding_handler
            .create_embeddings(request, context)
            .await
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.image_handler.generate_image(request, context).await
    }

    async fn health_check(&self) -> HealthStatus {
        let client = self.chat_handler.policy_client();
        let config = client.get_config();
        let endpoint = match config.get_effective_azure_endpoint() {
            Some(endpoint) => endpoint,
            None => return HealthStatus::Unhealthy,
        };
        if config.api_version.is_empty() {
            return HealthStatus::Unhealthy;
        }

        let api_key = match config.get_effective_api_key().await {
            Some(api_key) => api_key,
            None => return HealthStatus::Unhealthy,
        };

        let url = build_azure_models_health_url(&endpoint, &config.api_version);
        let mut request = match client.request(Method::GET, &url) {
            Ok(request) => request.header("api-key", api_key),
            Err(_) => return HealthStatus::Unhealthy,
        };
        for (key, value) in &config.custom_headers {
            request = request.header(key.as_str(), value.as_str());
        }

        match request.send().await {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::Degraded,
            Err(_) => HealthStatus::Unhealthy,
        }
    }
}

// ProviderConfig implementation is in common_utils.rs

/// Azure provider factory
pub struct AzureProviderFactory;

impl AzureProviderFactory {
    /// Create provider with default configuration
    pub fn create_default() -> Result<AzureOpenAIProvider, ProviderError> {
        let config = AzureConfig::new();
        AzureOpenAIProvider::new(config)
    }

    /// Create provider with custom configuration
    pub fn create_with_config(config: AzureConfig) -> Result<AzureOpenAIProvider, ProviderError> {
        AzureOpenAIProvider::new(config)
    }

    /// Create provider from environment variables
    pub fn create_from_env() -> Result<AzureOpenAIProvider, ProviderError> {
        AzureOpenAIProvider::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    fn test_config(endpoint: String) -> AzureConfig {
        AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint(endpoint)
            .with_endpoint_access(crate::core::net::ProviderEndpointAccess::PrivateNetwork)
            .with_api_version("2024-02-01".to_string())
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

    #[test]
    fn test_supports_dynamic_deployment_names() {
        let provider = match AzureOpenAIProvider::new(test_config(
            "https://test.openai.azure.com".to_string(),
        )) {
            Ok(provider) => provider,
            Err(error) => panic!("provider should be created: {error}"),
        };

        assert!(provider.supports_model("customer-gpt4o-prod"));
        assert!(!provider.supports_model("   "));
    }

    #[test]
    fn test_health_url_strips_deployment_base() {
        let url = build_azure_models_health_url(
            "https://test.openai.azure.com/openai/deployments/prod",
            "2024-02-01",
        );

        assert_eq!(
            url,
            "https://test.openai.azure.com/openai/models?api-version=2024-02-01"
        );
    }

    #[tokio::test]
    async fn test_health_check_success_requires_endpoint_success() {
        let api_base = match health_response_base_url("200 OK").await {
            Ok(url) => url,
            Err(error) => panic!("test server should start: {error}"),
        };
        let provider = match AzureOpenAIProvider::new(test_config(api_base)) {
            Ok(provider) => provider,
            Err(error) => panic!("provider should be created: {error}"),
        };

        assert_eq!(provider.health_check().await, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_health_check_degrades_on_endpoint_failure() {
        let api_base = match health_response_base_url("500 Internal Server Error").await {
            Ok(url) => url,
            Err(error) => panic!("test server should start: {error}"),
        };
        let provider = match AzureOpenAIProvider::new(test_config(api_base)) {
            Ok(provider) => provider,
            Err(error) => panic!("provider should be created: {error}"),
        };

        assert_eq!(provider.health_check().await, HealthStatus::Degraded);
    }
}
