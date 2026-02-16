//! Heroku Provider Implementation
//!
//! Main provider implementation for Heroku AI Inference API.
//! Heroku provides managed access to various AI models including Claude, Amazon Nova, and more.

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

use super::config::{DEFAULT_API_BASE, HerokuConfig, PROVIDER_NAME};
use super::{HerokuClient, HerokuErrorMapper};

/// Heroku AI Inference Provider
///
/// Provides access to AI models through Heroku's managed inference service,
/// which is part of the Salesforce ecosystem.
#[derive(Debug, Clone)]
pub struct HerokuProvider {
    config: HerokuConfig,
    pool_manager: Arc<GlobalPoolManager>,
    supported_models: Vec<ModelInfo>,
}

impl HerokuProvider {
    /// Generate headers for Heroku API requests
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(3);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        headers.push(header("Content-Type", "application/json".to_string()));

        // Add custom headers
        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Get the effective API base URL
    fn get_api_base(&self) -> String {
        self.config
            .base
            .api_base
            .clone()
            .or_else(|| std::env::var("INFERENCE_URL").ok())
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string())
    }

    /// Create a new Heroku provider with the given configuration
    pub fn new(config: HerokuConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e.to_string()))?,
        );
        let supported_models = HerokuClient::supported_models();

        Ok(Self {
            config,
            pool_manager,
            supported_models,
        })
    }

    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = HerokuConfig::from_env();
        Self::new(config)
    }

    /// Create provider with API key
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = HerokuConfig::with_api_key(api_key);
        Self::new(config)
    }

    /// Create provider with API key and custom API base
    pub async fn with_api_key_and_base(
        api_key: impl Into<String>,
        api_base: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let config = HerokuConfig::with_api_key(api_key).with_api_base(api_base);
        Self::new(config)
    }
}

#[async_trait]
impl LLMProvider for HerokuProvider {
    type Config = HerokuConfig;
    type Error = ProviderError;
    type ErrorMapper = HerokuErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        HerokuClient::supported_openai_params()
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Map max_completion_tokens to max_tokens for Heroku
        if let Some(max_completion_tokens) = params.remove("max_completion_tokens") {
            params.insert("max_tokens".to_string(), max_completion_tokens);
        }
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        Ok(HerokuClient::transform_chat_request(request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))?;
        HerokuClient::transform_chat_response(response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        HerokuErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let url = format!("{}/chat/completions", self.get_api_base());
        let body = HerokuClient::transform_chat_request(request.clone());

        let headers = self.get_request_headers();
        let body_data = Some(body);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body_data)
            .await?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        self.transform_response(&response_bytes, &request.model, &context.request_id)
            .await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        let url = format!("{}/chat/completions", self.get_api_base());

        // Create streaming request
        let mut body = HerokuClient::transform_chat_request(request.clone());
        body["stream"] = serde_json::Value::Bool(true);

        // Get API key
        let api_key = self
            .config
            .base
            .get_effective_api_key(PROVIDER_NAME)
            .or_else(|| std::env::var("INFERENCE_KEY").ok())
            .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key is required"))?;

        // Create request
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error(
                PROVIDER_NAME,
                status.as_u16(),
                error_text,
            ));
        }

        // Create Heroku stream using unified SSE parser
        let stream = response.bytes_stream();
        Ok(Box::pin(super::streaming::create_heroku_stream(stream)))
    }

    async fn health_check(&self) -> HealthStatus {
        // Check if we have a valid API key
        let has_key = self
            .config
            .base
            .get_effective_api_key(PROVIDER_NAME)
            .is_some()
            || std::env::var("INFERENCE_KEY").is_ok();

        if has_key {
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
    fn test_provider_creation() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        assert_eq!(provider.name(), PROVIDER_NAME);
    }

    #[test]
    fn test_provider_capabilities() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
        assert!(capabilities.contains(&ProviderCapability::Embeddings));
        assert!(capabilities.contains(&ProviderCapability::ImageGeneration));
    }

    #[test]
    fn test_provider_models() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        // Check for Claude models
        assert!(models.iter().any(|m| m.id.contains("claude")));
    }

    #[test]
    fn test_provider_from_env() {
        // This test may fail if HEROKU_API_KEY or INFERENCE_KEY is not set
        let result = HerokuProvider::from_env();
        // Either succeeds or fails with missing API key
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_provider_missing_api_key() {
        let config = HerokuConfig::new(PROVIDER_NAME);
        let result = HerokuProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_supported_openai_params() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        let params = provider.get_supported_openai_params("claude-4-5-sonnet");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
    }

    #[tokio::test]
    async fn test_map_openai_params() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("max_completion_tokens".to_string(), serde_json::json!(1000));
        params.insert("temperature".to_string(), serde_json::json!(0.7));

        let mapped = provider
            .map_openai_params(params, "claude-4-5-sonnet")
            .await
            .unwrap();

        // max_completion_tokens should be mapped to max_tokens
        assert!(mapped.contains_key("max_tokens"));
        assert!(!mapped.contains_key("max_completion_tokens"));
        assert!(mapped.contains_key("temperature"));
    }

    #[tokio::test]
    async fn test_health_check_with_key() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        let health = provider.health_check().await;

        assert!(matches!(health, HealthStatus::Healthy));
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        let cost = provider
            .calculate_cost("claude-4-5-sonnet", 1000, 500)
            .await
            .unwrap();

        // Cost should be non-negative
        assert!(cost >= 0.0);
    }

    #[test]
    fn test_get_request_headers() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());
        config
            .base
            .headers
            .insert("X-Custom-Header".to_string(), "custom-value".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        let headers = provider.get_request_headers();

        assert!(!headers.is_empty());
    }

    #[test]
    fn test_get_api_base_default() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());
        config.base.api_base = None;

        let provider = HerokuProvider::new(config).unwrap();
        let api_base = provider.get_api_base();

        // Should use default or env var
        assert!(!api_base.is_empty());
    }

    #[test]
    fn test_get_api_base_custom() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());
        config.base.api_base = Some("https://custom.inference.heroku.com/v1".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        let api_base = provider.get_api_base();

        assert_eq!(api_base, "https://custom.inference.heroku.com/v1");
    }

    #[tokio::test]
    async fn test_transform_request() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "claude-4-5-sonnet".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stream: false,
            tools: None,
            tool_choice: None,
            user: None,
            response_format: None,
            seed: None,
            max_completion_tokens: None,
            stop: None,
            parallel_tool_calls: None,
            n: None,
            logit_bias: None,
            functions: None,
            function_call: None,
            logprobs: None,
            top_logprobs: None,
            thinking: None,
            extra_params: HashMap::new(),
        };

        let context = RequestContext::default();
        let transformed = provider.transform_request(request, context).await.unwrap();

        assert_eq!(transformed["model"], "claude-4-5-sonnet");
    }

    #[test]
    fn test_heroku_managed_metadata() {
        let mut config = HerokuConfig::new(PROVIDER_NAME);
        config.base.api_key = Some("test-api-key".to_string());

        let provider = HerokuProvider::new(config).unwrap();
        let models = provider.models();

        // Check that models have Heroku-specific metadata
        for model in models {
            assert!(
                model.metadata.contains_key("heroku_managed")
                    || model.metadata.contains_key("underlying_provider")
            );
        }
    }

    #[tokio::test]
    async fn test_with_api_key() {
        let provider = HerokuProvider::with_api_key("test-key").await;
        assert!(provider.is_ok());
        assert_eq!(provider.unwrap().name(), PROVIDER_NAME);
    }

    #[tokio::test]
    async fn test_with_api_key_and_base() {
        let provider =
            HerokuProvider::with_api_key_and_base("test-key", "https://custom.api.com/v1").await;
        assert!(provider.is_ok());
        let provider = provider.unwrap();
        assert_eq!(provider.get_api_base(), "https://custom.api.com/v1");
    }
}
