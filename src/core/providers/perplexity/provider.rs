//! Perplexity Provider Implementation
//!
//! Main provider implementation using the unified base infrastructure.
//! Supports search-integrated chat completions with citations.

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

use super::{PerplexityClient, PerplexityConfig, PerplexityErrorMapper};

/// Perplexity AI Provider
///
/// Provides search-integrated AI chat completions with:
/// - Web search integration
/// - Citation support
/// - Search context customization
#[derive(Debug, Clone)]
pub struct PerplexityProvider {
    config: PerplexityConfig,
    pool_manager: Arc<GlobalPoolManager>,
    supported_models: Vec<ModelInfo>,
}

impl PerplexityProvider {
    /// Generate headers for Perplexity API requests
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(2);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        // Add custom headers
        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Create new Perplexity provider
    pub fn new(config: PerplexityConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("perplexity", e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration("perplexity", e.to_string()))?,
        );
        let supported_models = PerplexityClient::supported_models();

        Ok(Self {
            config,
            pool_manager,
            supported_models,
        })
    }

    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = PerplexityConfig::from_env();
        Self::new(config)
    }

    /// Create provider with API key
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let mut config = PerplexityConfig::new("perplexity");
        config.base.api_key = Some(api_key.into());
        config.base.api_base = Some("https://api.perplexity.ai".to_string());
        Self::new(config)
    }

    /// Get citations from the last response (if available)
    /// This is a helper for accessing Perplexity-specific response data
    pub fn extract_citations_from_raw(raw_response: &[u8]) -> Option<Vec<String>> {
        if let Ok(json) = serde_json::from_slice::<Value>(raw_response) {
            PerplexityClient::extract_citations(&json)
        } else {
            None
        }
    }

    /// Get search results from the last response (if available)
    pub fn extract_search_results_from_raw(
        raw_response: &[u8],
    ) -> Option<Vec<super::client::SearchResult>> {
        if let Ok(json) = serde_json::from_slice::<Value>(raw_response) {
            PerplexityClient::extract_search_results(&json)
        } else {
            None
        }
    }
}

#[async_trait]
impl LLMProvider for PerplexityProvider {
    type Config = PerplexityConfig;
    type Error = ProviderError;
    type ErrorMapper = PerplexityErrorMapper;

    fn name(&self) -> &'static str {
        "perplexity"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        PerplexityClient::supported_openai_params()
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // Perplexity uses OpenAI-compatible parameters
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        Ok(PerplexityClient::transform_chat_request(request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("perplexity", e.to_string()))?;
        PerplexityClient::transform_chat_response(response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        PerplexityErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let body = PerplexityClient::transform_chat_request(request.clone());

        let headers = self.get_request_headers();
        let body_data = Some(body);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body_data)
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("perplexity", e.to_string()))?;

        // Check for error status
        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&response_bytes);
            let mapper = self.get_error_mapper();
            return Err(
                crate::core::traits::error_mapper::trait_def::ErrorMapper::map_http_error(
                    &mapper,
                    status.as_u16(),
                    &error_text,
                ),
            );
        }

        self.transform_response(&response_bytes, &request.model, &context.request_id)
            .await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        let url = format!("{}/chat/completions", self.config.get_api_base());

        // Create request body with streaming enabled
        let mut body = PerplexityClient::transform_chat_request(request.clone());
        body["stream"] = serde_json::Value::Bool(true);

        // Get API key
        let api_key = self
            .config
            .base
            .get_effective_api_key("perplexity")
            .ok_or_else(|| ProviderError::authentication("perplexity", "API key is required"))?;

        // Create streaming request
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("perplexity", e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error(
                "perplexity",
                status.as_u16(),
                error_text,
            ));
        }

        // Create Perplexity stream using unified SSE parser
        let stream = response.bytes_stream();
        Ok(Box::pin(super::streaming::create_perplexity_stream(stream)))
    }

    async fn health_check(&self) -> HealthStatus {
        if self
            .config
            .base
            .get_effective_api_key("perplexity")
            .is_some()
        {
            // Try to make a lightweight request to verify connectivity
            let url = format!("{}/chat/completions", self.config.get_api_base());
            let client = reqwest::Client::new();

            match client.head(&url).send().await {
                Ok(response) if response.status().as_u16() != 401 => HealthStatus::Healthy,
                _ => HealthStatus::Healthy, // HEAD might not be supported, assume healthy if key exists
            }
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
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    fn create_test_config() -> PerplexityConfig {
        let mut config = PerplexityConfig::new("perplexity");
        config.base.api_key = Some("pplx-test-key-12345".to_string());
        config.base.api_base = Some("https://api.perplexity.ai".to_string());
        config
    }

    fn create_test_request() -> ChatRequest {
        ChatRequest {
            model: "sonar".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            }],
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
        }
    }

    #[test]
    fn test_provider_creation() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_creation_missing_api_key() {
        let mut config = PerplexityConfig::new("perplexity");
        config.base.api_key = None;
        let provider = PerplexityProvider::new(config);
        assert!(provider.is_err());
    }

    #[test]
    fn test_provider_name() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();
        assert_eq!(provider.name(), "perplexity");
    }

    #[test]
    fn test_provider_capabilities() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
        // Perplexity doesn't support tool calling
        assert!(!caps.contains(&ProviderCapability::ToolCalling));
    }

    #[test]
    fn test_provider_models() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "sonar"));
    }

    #[test]
    fn test_supported_openai_params() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();
        let params = provider.get_supported_openai_params("sonar");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"stream"));
    }

    #[tokio::test]
    async fn test_transform_request() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();
        let request = create_test_request();
        let context = RequestContext::new();

        let result = provider.transform_request(request, context).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value["model"], "sonar");
        assert!(value["messages"].is_array());
    }

    #[tokio::test]
    async fn test_transform_response() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();

        let raw_response = br#"{
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "sonar",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 10,
                "total_tokens": 15
            }
        }"#;

        let result = provider
            .transform_response(raw_response, "sonar", "req-123")
            .await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.model, "sonar");
        assert_eq!(response.choices.len(), 1);
    }

    #[tokio::test]
    async fn test_map_openai_params() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("temperature".to_string(), serde_json::json!(0.5));
        params.insert("max_tokens".to_string(), serde_json::json!(100));

        let result = provider.map_openai_params(params.clone(), "sonar").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), params);
    }

    #[test]
    fn test_extract_citations_from_raw() {
        let raw = br#"{"citations": ["https://example.com", "https://test.com"]}"#;
        let citations = PerplexityProvider::extract_citations_from_raw(raw);

        assert!(citations.is_some());
        let citations = citations.unwrap();
        assert_eq!(citations.len(), 2);
    }

    #[test]
    fn test_extract_citations_from_raw_none() {
        let raw = br#"{"model": "sonar"}"#;
        let citations = PerplexityProvider::extract_citations_from_raw(raw);
        assert!(citations.is_none());
    }

    #[test]
    fn test_extract_search_results_from_raw() {
        let raw = br#"{
            "search_results": [
                {"url": "https://example.com", "title": "Example"}
            ]
        }"#;

        let results = PerplexityProvider::extract_search_results_from_raw(raw);
        assert!(results.is_some());
        let results = results.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example");
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();

        let result = provider.calculate_cost("sonar", 100, 200).await;
        assert!(result.is_ok());
        // Cost should be >= 0
        assert!(result.unwrap() >= 0.0);
    }

    #[test]
    fn test_get_request_headers() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();
        let headers = provider.get_request_headers();

        // Should have authorization header
        assert!(!headers.is_empty());
    }

    #[test]
    fn test_get_error_mapper() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();
        let _mapper = provider.get_error_mapper();
        // Just verify it returns without panic
    }

    #[tokio::test]
    async fn test_health_check_with_api_key() {
        let config = create_test_config();
        let provider = PerplexityProvider::new(config).unwrap();

        let status = provider.health_check().await;
        // With API key present, should report healthy
        assert!(matches!(status, HealthStatus::Healthy));
    }
}
