//! Main Together AI Provider Implementation
//!
//! Implements the LLMProvider trait for Together AI's high-performance inference.
//! Together AI is OpenAI-compatible and supports chat completions, embeddings, and rerank.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::TogetherConfig;
use super::error::{TogetherError, TogetherErrorMapper};
use super::model_info::{get_available_models, get_model_info, is_function_calling_model};
use super::rerank::{RerankRequest, RerankResponse};
use crate::core::providers::base::{GlobalPoolManager, HttpErrorMapper, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    provider::ProviderConfig as _, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    message::MessageRole,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse},
};

/// Static capabilities for Together AI provider
const TOGETHER_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::Embeddings,
];

/// Together AI provider implementation
#[derive(Debug, Clone)]
pub struct TogetherProvider {
    config: TogetherConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl TogetherProvider {
    /// Create a new Together AI provider instance
    pub async fn new(config: TogetherConfig) -> Result<Self, TogetherError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("together", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                "together",
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Build model list from static configuration
        let models = get_available_models()
            .iter()
            .filter_map(|id| get_model_info(id))
            .filter(|info| !info.is_embedding && !info.is_rerank) // Only chat models
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
                    provider: "together".to_string(),
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
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, TogetherError> {
        let config = TogetherConfig::from_env().with_api_key(api_key);
        Self::new(config).await
    }

    /// Check if response format requires special handling
    pub(crate) fn should_handle_response_format(&self, request: &ChatRequest) -> bool {
        // Together AI supports response_format only for certain models with function calling
        if let Some(ref format) = request.response_format
            && format.format_type == "json_object"
        {
            return !is_function_calling_model(&request.model);
        }
        false
    }

    /// Transform messages for Together AI API
    fn transform_messages(&self, request: &mut ChatRequest) {
        // Remove null function_call from assistant messages
        for message in request.messages.iter_mut() {
            if message.role == MessageRole::Assistant {
                // Function call handling would go here if needed
            }
        }
    }

    /// Handle response_format - remove for models that don't support it
    fn handle_response_format(&self, request: &mut ChatRequest) {
        // Check if model supports function calling / response_format
        if let Some(ref format) = request.response_format
            && format.format_type == "text"
        {
            // Remove text format as it's the default
            request.response_format = None;
        }
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, TogetherError> {
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
            .map_err(|e| ProviderError::network("together", e.to_string()))?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("together", e.to_string()))?;

        if !status.is_success() {
            let error_body = String::from_utf8_lossy(&response_bytes);
            return Err(match status.as_u16() {
                400 => ProviderError::invalid_request("together", error_body.to_string()),
                401 => ProviderError::authentication("together", "Invalid API key"),
                404 => ProviderError::model_not_found("together", "Model not found"),
                429 => ProviderError::rate_limit("together", None),
                _ => HttpErrorMapper::map_status_code("together", status.as_u16(), &error_body),
            });
        }

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::api_error("together", 500, format!("Failed to parse response: {}", e))
        })
    }

    /// Execute a rerank request
    pub async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, TogetherError> {
        let api_key = self.config.get_api_key().ok_or_else(|| {
            ProviderError::authentication("together", "API key is required".to_string())
        })?;

        let url = format!("{}/rerank", self.config.get_api_base());

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network("together", e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(match status.as_u16() {
                400 => ProviderError::invalid_request(
                    "together",
                    format!("Bad request: {}", error_body),
                ),
                401 => ProviderError::authentication("together", "Invalid API key"),
                429 => ProviderError::rate_limit("together", None),
                _ => HttpErrorMapper::map_status_code("together", status.as_u16(), &error_body),
            });
        }

        let response_text = response.text().await.map_err(|e| {
            ProviderError::api_error("together", 500, format!("Failed to read response: {}", e))
        })?;

        serde_json::from_str(&response_text).map_err(|e| {
            ProviderError::api_error("together", 500, format!("Failed to parse response: {}", e))
        })
    }
}

#[async_trait]
impl LLMProvider for TogetherProvider {
    type Config = TogetherConfig;
    type Error = TogetherError;
    type ErrorMapper = TogetherErrorMapper;

    fn name(&self) -> &'static str {
        "together"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        TOGETHER_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        let supports_function_calling = is_function_calling_model(model);

        if supports_function_calling {
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
                "user",
                "logprobs",
                "top_logprobs",
                "repetition_penalty",
            ]
        } else {
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
                "seed",
                "user",
                "logprobs",
                "top_logprobs",
                "repetition_penalty",
            ]
        }
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        // Together AI uses the same parameters as OpenAI
        Ok(params)
    }

    async fn transform_request(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        // Transform messages
        self.transform_messages(&mut request);

        // Handle response_format
        self.handle_response_format(&mut request);

        // Convert to JSON value
        serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("together", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        // Parse response
        let chat_response: ChatResponse = serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::api_error("together", 500, format!("Failed to parse response: {}", e))
        })?;

        Ok(chat_response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        TogetherErrorMapper
    }

    async fn chat_completion(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Together AI chat request: model={}", request.model);

        // Check if model supports function calling
        if self.should_handle_response_format(&request) {
            // Remove response_format for models that don't support it
            debug!(
                "Removing response_format for model {} (not supported)",
                request.model
            );
            request.response_format = None;
        }

        // Transform and execute
        let request_json = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("together", e.to_string()))?;

        let response = self
            .execute_request("/chat/completions", request_json)
            .await?;

        serde_json::from_value(response).map_err(|e| {
            ProviderError::api_error(
                "together",
                500,
                format!("Failed to parse chat response: {}", e),
            )
        })
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Together AI streaming request: model={}", request.model);

        // Check if model supports function calling
        if self.should_handle_response_format(&request) {
            // Execute non-streaming request and convert to fake stream
            debug!(
                "Using fake stream for model {} (response_format not supported)",
                request.model
            );
            request.response_format = None;
            request.stream = false;
            let response = self.chat_completion(request, context).await?;
            return super::streaming::create_fake_stream(response).await;
        }

        // Execute streaming request
        request.stream = true;

        // Get API configuration
        let api_key = self.config.get_api_key().ok_or_else(|| {
            ProviderError::authentication("together", "API key is required".to_string())
        })?;

        // Execute streaming request using reqwest directly for SSE
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network("together", e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(match status {
                400 => ProviderError::invalid_request(
                    "together",
                    body.unwrap_or_else(|| "Bad request".to_string()),
                ),
                401 => ProviderError::authentication("together", "Invalid API key"),
                429 => ProviderError::rate_limit("together", None),
                _ => HttpErrorMapper::map_status_code(
                    "together",
                    status,
                    &format!("Stream request failed: {}", status),
                ),
            });
        }

        // Create SSE stream
        let stream = super::streaming::create_together_stream(response.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        debug!("Together AI embeddings request: model={}", request.model);

        let request_json = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("together", e.to_string()))?;

        let response = self.execute_request("/embeddings", request_json).await?;

        serde_json::from_value(response).map_err(|e| {
            ProviderError::api_error(
                "together",
                500,
                format!("Failed to parse embeddings response: {}", e),
            )
        })
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
            ProviderError::model_not_found("together", format!("Unknown model: {}", model))
        })?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}
