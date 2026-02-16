//! Main Novita Provider Implementation
//!
//! Implements the LLMProvider trait for Novita AI's OpenAI-compatible API.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::NovitaConfig;
use super::model_info::{get_available_models, get_model_info};
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    provider::ProviderConfig as _, provider::llm_provider::trait_definition::LLMProvider,
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

/// Provider name constant
const PROVIDER_NAME: &str = "novita";

/// Static capabilities for Novita provider
const NOVITA_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// Novita provider implementation
#[derive(Debug, Clone)]
pub struct NovitaProvider {
    config: NovitaConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl NovitaProvider {
    /// Create a new Novita provider instance
    pub async fn new(config: NovitaConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                PROVIDER_NAME,
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build model list from static configuration
        let models = get_available_models()
            .iter()
            .filter_map(|id| get_model_info(id))
            .map(|info| {
                let mut capabilities = vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ];
                if info.supports_tools {
                    capabilities.push(ProviderCapability::ToolCalling);
                }

                ModelInfo {
                    id: info.model_id.to_string(),
                    name: info.display_name.to_string(),
                    provider: PROVIDER_NAME.to_string(),
                    max_context_length: info.max_context_length,
                    max_output_length: Some(info.max_output_length),
                    supports_streaming: true,
                    supports_tools: info.supports_tools,
                    supports_multimodal: info.supports_multimodal,
                    input_cost_per_1k_tokens: Some(info.input_cost_per_million / 1000.0),
                    output_cost_per_1k_tokens: Some(info.output_cost_per_million / 1000.0),
                    currency: "USD".to_string(),
                    capabilities,
                    created_at: None,
                    updated_at: None,
                    metadata: HashMap::new(),
                }
            })
            .collect();

        Ok(Self {
            config,
            pool_manager,
            models,
        })
    }

    /// Create provider with API key only
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = NovitaConfig::from_env().with_api_key(api_key.into());
        Self::new(config).await
    }

    /// Execute an HTTP request with Novita-specific headers
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let url = format!("{}{}", self.config.get_api_base(), endpoint);

        let mut headers = Vec::with_capacity(4);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));
        // Novita-specific header as per Python implementation
        headers.push(header("X-Novita-Source", "litellm".to_string()));

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&response_bytes);
            return Err(Self::map_http_error(status.as_u16(), &error_text));
        }

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::api_error(
                PROVIDER_NAME,
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }

    /// Map HTTP status codes to provider errors
    fn map_http_error(status: u16, body: &str) -> ProviderError {
        let message = if body.is_empty() {
            format!("HTTP error {}", status)
        } else {
            body.to_string()
        };

        match status {
            400 => ProviderError::invalid_request(PROVIDER_NAME, message),
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            403 => ProviderError::authentication(PROVIDER_NAME, "Access forbidden"),
            404 => ProviderError::model_not_found(PROVIDER_NAME, "Model not found"),
            429 => ProviderError::rate_limit(PROVIDER_NAME, None),
            500 => ProviderError::api_error(PROVIDER_NAME, 500, "Internal server error"),
            502 => ProviderError::provider_unavailable(PROVIDER_NAME, "Bad gateway"),
            503 => ProviderError::provider_unavailable(PROVIDER_NAME, "Service unavailable"),
            _ => ProviderError::api_error(PROVIDER_NAME, status, message),
        }
    }
}

#[async_trait]
impl LLMProvider for NovitaProvider {
    type Config = NovitaConfig;
    type Error = ProviderError;
    type ErrorMapper = crate::core::traits::error_mapper::DefaultErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        NOVITA_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "top_p",
            "max_tokens",
            "max_completion_tokens",
            "stream",
            "stop",
            "frequency_penalty",
            "presence_penalty",
            "n",
            "response_format",
            "seed",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "user",
            "logprobs",
            "top_logprobs",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        // Novita uses the same parameters as OpenAI
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        // Convert to JSON value
        serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request(PROVIDER_NAME, e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        // Parse response
        let chat_response: ChatResponse = serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::api_error(
                PROVIDER_NAME,
                500,
                format!("Failed to parse response: {}", e),
            )
        })?;

        Ok(chat_response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        crate::core::traits::error_mapper::DefaultErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Novita chat request: model={}", request.model);

        // Transform and execute
        let request_json = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request(PROVIDER_NAME, e.to_string()))?;

        let response = self
            .execute_request("/chat/completions", request_json)
            .await?;

        serde_json::from_value(response).map_err(|e| {
            ProviderError::api_error(
                PROVIDER_NAME,
                500,
                format!("Failed to parse chat response: {}", e),
            )
        })
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        // Streaming would require SSE handling similar to Groq
        Err(ProviderError::not_implemented(
            PROVIDER_NAME,
            "Streaming is not yet implemented for Novita provider",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_supported(
            PROVIDER_NAME,
            "Novita does not support embeddings through this endpoint.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple health check - try to get models list
        let url = format!("{}/models", self.config.get_api_base());
        let mut headers = Vec::with_capacity(2);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("X-Novita-Source", "litellm".to_string()));

        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None::<serde_json::Value>)
            .await
        {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        let model_info = get_model_info(model).ok_or_else(|| {
            ProviderError::model_not_found(PROVIDER_NAME, format!("Unknown model: {}", model))
        })?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}
