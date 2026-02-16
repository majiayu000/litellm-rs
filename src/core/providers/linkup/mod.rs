//! Linkup Provider
//!
//! Linkup API integration providing search-augmented generation.

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

// Static capabilities
const LINKUP_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
];

/// Linkup provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkupConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://api.linkup.so/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for LinkupConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.linkup.so/v1".to_string(),
            timeout_seconds: 60,
            max_retries: 3,
        }
    }
}

impl ProviderConfig for LinkupConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("Linkup API key is required".to_string());
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

/// Linkup error type (using unified ProviderError)
pub type LinkupError = ProviderError;

/// Linkup error mapper
pub struct LinkupErrorMapper;

impl ErrorMapper<LinkupError> for LinkupErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> LinkupError {
        HttpErrorMapper::map_status_code("linkup", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> LinkupError {
        HttpErrorMapper::parse_json_error("linkup", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> LinkupError {
        ProviderError::network("linkup", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> LinkupError {
        ProviderError::response_parsing("linkup", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> LinkupError {
        ProviderError::timeout(
            "linkup",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Linkup provider implementation
#[derive(Debug, Clone)]
pub struct LinkupProvider {
    config: LinkupConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl LinkupProvider {
    /// Create a new Linkup provider instance
    pub async fn new(config: LinkupConfig) -> Result<Self, LinkupError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("linkup", e))?;

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

        let models = vec![
            ModelInfo {
                id: "linkup-search-gpt-4".to_string(),
                name: "Linkup Search GPT-4".to_string(),
                provider: "linkup".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.03),
                output_cost_per_1k_tokens: Some(0.06),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "linkup-search-gpt-3.5".to_string(),
                name: "Linkup Search GPT-3.5".to_string(),
                provider: "linkup".to_string(),
                max_context_length: 4096,
                max_output_length: None,
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0015),
                output_cost_per_1k_tokens: Some(0.002),
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
}

#[async_trait]
impl LLMProvider for LinkupProvider {
    type Config = LinkupConfig;
    type Error = LinkupError;
    type ErrorMapper = LinkupErrorMapper;

    fn name(&self) -> &'static str {
        "linkup"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        LINKUP_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["temperature", "max_tokens", "stream"]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        let mut mapped = HashMap::new();
        for (key, value) in params {
            match key.as_str() {
                "temperature" | "max_tokens" | "stream" => {
                    mapped.insert(key, value);
                }
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
        Ok(OpenAIRequestTransformer::transform_chat_request(&request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("linkup", e.to_string()))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        LinkupErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Linkup chat request: model={}", request.model);

        let body = self.transform_request(request, context).await?;

        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/chat/completions")
            .build();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("linkup", e.to_string()))?;

        let response = self
            .base_client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("linkup", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("linkup", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("linkup", e.to_string()))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Linkup streaming chat request: model={}", request.model);

        let mut body = self.transform_request(request, context).await?;
        body["stream"] = serde_json::json!(true);

        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/chat/completions")
            .build();

        let headers = HeaderBuilder::new()
            .with_bearer_token(&self.config.api_key)
            .with_content_type("application/json")
            .build_reqwest()
            .map_err(|e| ProviderError::invalid_request("linkup", e.to_string()))?;

        let response = self
            .base_client
            .inner()
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("linkup", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("linkup", status, body));
        }

        use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEParser};
        use futures::StreamExt;

        let transformer = OpenAICompatibleTransformer::new("linkup");
        let parser = UnifiedSSEParser::new(transformer);

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
                    Err(e) => Some(Err(ProviderError::network("linkup", e.to_string()))),
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
        Err(ProviderError::not_implemented(
            "linkup",
            "Linkup does not support embeddings".to_string(),
        ))
    }

    async fn health_check(&self) -> HealthStatus {
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
                        debug!("Linkup health check failed: status={}", response.status());
                        HealthStatus::Unhealthy
                    }
                    Err(e) => {
                        debug!("Linkup health check error: {}", e);
                        HealthStatus::Unhealthy
                    }
                }
            }
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
            .ok_or_else(|| ProviderError::model_not_found("linkup", model.to_string()))?;

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

    fn create_test_config() -> LinkupConfig {
        LinkupConfig {
            api_key: "test_api_key".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let config = create_test_config();
        let provider = LinkupProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "linkup");
    }

    #[test]
    fn test_config_validation() {
        let mut config = LinkupConfig::default();
        assert!(config.validate().is_err());

        config.api_key = "test_key".to_string();
        assert!(config.validate().is_ok());
    }
}
