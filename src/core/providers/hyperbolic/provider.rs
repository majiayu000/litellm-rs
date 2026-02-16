//! Main Hyperbolic Provider Implementation
//!
//! Implements the LLMProvider trait for Hyperbolic's OpenAI-compatible API.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::HyperbolicConfig;
use super::error::HyperbolicError;
use super::model_info::{get_available_models, get_model_info};
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header};
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

/// Static capabilities for Hyperbolic provider
const HYPERBOLIC_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// Hyperbolic provider implementation
#[derive(Debug, Clone)]
pub struct HyperbolicProvider {
    config: HyperbolicConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl HyperbolicProvider {
    /// Create a new Hyperbolic provider instance
    pub async fn new(config: HyperbolicConfig) -> Result<Self, HyperbolicError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| HyperbolicError::configuration("hyperbolic", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            HyperbolicError::configuration(
                "hyperbolic",
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
                    provider: "hyperbolic".to_string(),
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
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, HyperbolicError> {
        let config = HyperbolicConfig::from_env().with_api_key(api_key);
        Self::new(config).await
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, HyperbolicError> {
        let url = format!("{}{}", self.config.get_api_base(), endpoint);

        let mut headers = Vec::with_capacity(2);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| HyperbolicError::network("hyperbolic", e.to_string()))?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| HyperbolicError::network("hyperbolic", e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            HyperbolicError::api_error(
                "hyperbolic",
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }
}

#[async_trait]
impl LLMProvider for HyperbolicProvider {
    type Config = HyperbolicConfig;
    type Error = HyperbolicError;
    type ErrorMapper = crate::core::traits::error_mapper::DefaultErrorMapper;

    fn name(&self) -> &'static str {
        "hyperbolic"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        HYPERBOLIC_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        // Reference: https://docs.hyperbolic.xyz/docs/rest-api
        &[
            "messages",
            "model",
            "stream",
            "temperature",
            "top_p",
            "max_tokens",
            "frequency_penalty",
            "presence_penalty",
            "stop",
            "n",
            "tools",
            "tool_choice",
            "response_format",
            "seed",
            "user",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        // Hyperbolic uses the same parameters as OpenAI
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        // Convert to JSON value
        serde_json::to_value(&request)
            .map_err(|e| HyperbolicError::invalid_request("hyperbolic", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        // Parse response
        let chat_response: ChatResponse = serde_json::from_slice(raw_response).map_err(|e| {
            HyperbolicError::api_error(
                "hyperbolic",
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
        debug!("Hyperbolic chat request: model={}", request.model);

        // Transform and execute
        let request_json = serde_json::to_value(&request)
            .map_err(|e| HyperbolicError::invalid_request("hyperbolic", e.to_string()))?;

        let response = self
            .execute_request("/chat/completions", request_json)
            .await?;

        serde_json::from_value(response).map_err(|e| {
            HyperbolicError::api_error(
                "hyperbolic",
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
        Err(HyperbolicError::not_supported(
            "hyperbolic",
            "Streaming is not yet implemented for Hyperbolic provider",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(HyperbolicError::not_supported(
            "hyperbolic",
            "Hyperbolic does not support embeddings through this endpoint.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple health check - try to get models list
        let url = format!("{}/models", self.config.get_api_base());
        let mut headers = Vec::with_capacity(1);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

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
            HyperbolicError::model_not_found("hyperbolic", format!("Unknown model: {}", model))
        })?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}
