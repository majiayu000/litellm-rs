//! Nebius Provider Implementation
//!
//! Main provider implementation for Nebius AI cloud platform

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, get_pricing_db, header, header_owned,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    provider::ProviderConfig, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse},
};

use super::{NebiusClient, NebiusConfig, NebiusErrorMapper};

/// Nebius provider for Nebius AI cloud platform
#[derive(Debug, Clone)]
pub struct NebiusProvider {
    config: NebiusConfig,
    pool_manager: Arc<GlobalPoolManager>,
    supported_models: Vec<ModelInfo>,
}

impl NebiusProvider {
    /// Generate headers for Nebius API requests
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(3);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        // Add custom headers (including x-folder-id if set)
        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Create new Nebius provider
    pub fn new(config: NebiusConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("nebius", e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration("nebius", e.to_string()))?,
        );
        let supported_models = NebiusClient::supported_models();

        Ok(Self {
            config,
            pool_manager,
            supported_models,
        })
    }

    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = NebiusConfig::from_env();
        Self::new(config)
    }
}

#[async_trait]
impl LLMProvider for NebiusProvider {
    type Config = NebiusConfig;
    type Error = ProviderError;
    type ErrorMapper = NebiusErrorMapper;

    fn name(&self) -> &'static str {
        "nebius"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
            ProviderCapability::Embeddings,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        NebiusClient::supported_openai_params()
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        Ok(NebiusClient::transform_chat_request(request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("nebius", e.to_string()))?;
        NebiusClient::transform_chat_response(response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        NebiusErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let url = format!(
            "{}/chat/completions",
            self.config.base.get_effective_api_base("nebius")
        );
        let body = NebiusClient::transform_chat_request(request.clone());

        let headers = self.get_request_headers();
        let body_data = Some(body);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body_data)
            .await?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("nebius", e.to_string()))?;

        self.transform_response(&response_bytes, &request.model, &context.request_id)
            .await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        let url = format!(
            "{}/chat/completions",
            self.config.base.get_effective_api_base("nebius")
        );

        // Create streaming request
        let mut body = NebiusClient::transform_chat_request(request.clone());
        body["stream"] = serde_json::Value::Bool(true);

        // Get API key
        let api_key = self
            .config
            .base
            .get_effective_api_key("nebius")
            .ok_or_else(|| ProviderError::authentication("nebius", "API key is required"))?;

        // Build request with headers
        let mut request_builder = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json");

        // Add custom headers
        for (key, value) in &self.config.base.headers {
            request_builder = request_builder.header(key, value);
        }

        let response = request_builder
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("nebius", e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error(
                "nebius",
                status.as_u16(),
                error_text,
            ));
        }

        // Create Nebius stream using unified SSE parser
        let stream = response.bytes_stream();
        Ok(Box::pin(super::streaming::create_nebius_stream(stream)))
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.base.get_effective_api_key("nebius").is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
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

    #[test]
    fn test_provider_name() {
        let mut config = NebiusConfig::new("nebius");
        config.base.api_key = Some("test-key".to_string());
        let provider = NebiusProvider::new(config).unwrap();
        assert_eq!(provider.name(), "nebius");
    }

    #[test]
    fn test_provider_capabilities() {
        let mut config = NebiusConfig::new("nebius");
        config.base.api_key = Some("test-key".to_string());
        let provider = NebiusProvider::new(config).unwrap();
        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        assert!(caps.contains(&ProviderCapability::ToolCalling));
    }

    #[test]
    fn test_provider_models() {
        let mut config = NebiusConfig::new("nebius");
        config.base.api_key = Some("test-key".to_string());
        let provider = NebiusProvider::new(config).unwrap();
        let models = provider.models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_provider_supports_model() {
        let mut config = NebiusConfig::new("nebius");
        config.base.api_key = Some("test-key".to_string());
        let provider = NebiusProvider::new(config).unwrap();
        assert!(provider.supports_model("meta-llama/Meta-Llama-3.1-8B-Instruct"));
    }

    #[test]
    fn test_provider_supports_streaming() {
        let mut config = NebiusConfig::new("nebius");
        config.base.api_key = Some("test-key".to_string());
        let provider = NebiusProvider::new(config).unwrap();
        assert!(provider.supports_streaming());
    }

    #[test]
    fn test_provider_without_api_key() {
        let config = NebiusConfig::new("nebius");
        let result = NebiusProvider::new(config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_check_with_key() {
        let mut config = NebiusConfig::new("nebius");
        config.base.api_key = Some("test-key".to_string());
        let provider = NebiusProvider::new(config).unwrap();
        let status = provider.health_check().await;
        assert_eq!(status, HealthStatus::Healthy);
    }
}
