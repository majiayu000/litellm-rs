//! Google PSE (Programmable Search Engine) Provider
//!
//! Google Programmable Search Engine integration for search-augmented generation.

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HttpErrorMapper, OpenAIRequestTransformer, UrlBuilder,
    apply_headers, get_pricing_db, header_static,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    error_mapper::trait_def::ErrorMapper, provider::ProviderConfig,
    provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    message::MessageRole,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChoice, ChatChunk, ChatResponse, EmbeddingResponse, FinishReason, Usage},
};

// Static capabilities
const GOOGLE_PSE_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::ChatCompletion];

/// Google PSE provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooglePSEConfig {
    /// API key for authentication
    pub api_key: String,
    /// Search Engine ID
    pub search_engine_id: String,
    /// API base URL (defaults to <https://www.googleapis.com/customsearch/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for GooglePSEConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            search_engine_id: String::new(),
            api_base: "https://www.googleapis.com/customsearch/v1".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

impl ProviderConfig for GooglePSEConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("Google PSE")?;
        if self.search_engine_id.is_empty() {
            return Err("Google PSE Search Engine ID is required".to_string());
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

/// Google PSE error type (using unified ProviderError)
pub type GooglePSEError = ProviderError;

/// Google PSE error mapper
pub struct GooglePSEErrorMapper;

impl ErrorMapper<GooglePSEError> for GooglePSEErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> GooglePSEError {
        HttpErrorMapper::map_status_code("google_pse", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> GooglePSEError {
        HttpErrorMapper::parse_json_error("google_pse", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> GooglePSEError {
        ProviderError::network("google_pse", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> GooglePSEError {
        ProviderError::response_parsing("google_pse", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> GooglePSEError {
        ProviderError::timeout(
            "google_pse",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Google PSE provider implementation
#[derive(Debug, Clone)]
pub struct GooglePSEProvider {
    config: GooglePSEConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl GooglePSEProvider {
    /// Create a new Google PSE provider instance
    pub async fn new(config: GooglePSEConfig) -> Result<Self, GooglePSEError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("google_pse", e))?;

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

        let models = vec![ModelInfo {
            id: "google-pse-search".to_string(),
            name: "Google PSE Search".to_string(),
            provider: "google_pse".to_string(),
            max_context_length: 1024,
            max_output_length: None,
            supports_streaming: false,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_1k_tokens: Some(0.005),
            output_cost_per_1k_tokens: Some(0.0),
            currency: "USD".to_string(),
            capabilities: vec![],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        }];

        Ok(Self {
            config,
            base_client,
            models,
        })
    }
}

#[async_trait]
impl LLMProvider for GooglePSEProvider {
    type Config = GooglePSEConfig;
    type Error = GooglePSEError;
    type ErrorMapper = GooglePSEErrorMapper;

    fn name(&self) -> &'static str {
        "google_pse"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        GOOGLE_PSE_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["num_results", "search_type"]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        let mut mapped = HashMap::new();
        for (key, value) in params {
            match key.as_str() {
                "num_results" | "search_type" => {
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
            .map_err(|e| ProviderError::response_parsing("google_pse", e.to_string()))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        GooglePSEErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Google PSE search request: model={}", request.model);

        let query = if let Some(last_message) = request.messages.last() {
            if let Some(crate::core::types::message::MessageContent::Text(text)) =
                &last_message.content
            {
                text.clone()
            } else {
                return Err(ProviderError::invalid_request(
                    "google_pse",
                    "Last message must contain text content".to_string(),
                ));
            }
        } else {
            return Err(ProviderError::invalid_request(
                "google_pse",
                "Request must contain at least one message".to_string(),
            ));
        };

        let url = UrlBuilder::new(&self.config.api_base)
            .with_query("key", &self.config.api_key)
            .with_query("cx", &self.config.search_engine_id)
            .with_query("q", &query)
            .build();

        let headers = vec![header_static("Content-Type", "application/json")];

        let response = apply_headers(self.base_client.inner().get(&url), headers)
            .send()
            .await
            .map_err(|e| ProviderError::network("google_pse", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code(
                "google_pse",
                status,
                &body,
            ));
        }

        let search_response: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("google_pse", e.to_string()))?;

        let content = format!(
            "Search results: {}",
            serde_json::to_string_pretty(&search_response).unwrap_or_default()
        );

        Ok(ChatResponse {
            id: format!("pse-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: request.model.clone(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(crate::core::types::message::MessageContent::Text(content)),
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
            system_fingerprint: None,
        })
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        Err(ProviderError::not_implemented(
            "google_pse",
            "Google PSE does not support streaming".to_string(),
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_implemented(
            "google_pse",
            "Google PSE does not support embeddings".to_string(),
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        let url = UrlBuilder::new(&self.config.api_base)
            .with_query("key", &self.config.api_key)
            .with_query("cx", &self.config.search_engine_id)
            .with_query("q", "test")
            .build();

        match self.base_client.inner().get(&url).send().await {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(response) => {
                debug!(
                    "Google PSE health check failed: status={}",
                    response.status()
                );
                HealthStatus::Unhealthy
            }
            Err(e) => {
                debug!("Google PSE health check error: {}", e);
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

    fn create_test_config() -> GooglePSEConfig {
        GooglePSEConfig {
            api_key: "test_api_key".to_string(),
            search_engine_id: "test_engine_id".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let config = create_test_config();
        let provider = GooglePSEProvider::new(config).await;
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "google_pse");
    }

    #[test]
    fn test_config_validation() {
        let mut config = GooglePSEConfig::default();
        assert!(config.validate().is_err());

        config.api_key = "test_key".to_string();
        assert!(config.validate().is_err());

        config.search_engine_id = "test_engine_id".to_string();
        assert!(config.validate().is_ok());
    }
}
