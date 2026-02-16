//! Main NVIDIA NIM Provider Implementation
//!
//! Implements the LLMProvider trait for NVIDIA NIM's inference microservices.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::NvidiaNimConfig;
use super::model_info::{
    get_available_models, get_model_info, get_supported_params, supports_tools,
};
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

/// Static capabilities for NVIDIA NIM provider
const NVIDIA_NIM_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::Embeddings,
];

/// NVIDIA NIM provider implementation
#[derive(Debug, Clone)]
pub struct NvidiaNimProvider {
    config: NvidiaNimConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl NvidiaNimProvider {
    /// Create a new NVIDIA NIM provider instance
    pub async fn new(config: NvidiaNimConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("nvidia_nim", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                "nvidia_nim",
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
                    provider: "nvidia_nim".to_string(),
                    max_context_length: info.max_context_length as u32,
                    max_output_length: Some(info.max_output_length as u32),
                    supports_streaming: info.supports_streaming,
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
        let config = NvidiaNimConfig::from_env().with_api_key(api_key.into());
        Self::new(config).await
    }

    /// Execute an HTTP request to NVIDIA NIM API
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
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
            .map_err(|e| ProviderError::network("nvidia_nim", e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();
            return Err(match status {
                400 => ProviderError::invalid_request("nvidia_nim", body_text),
                401 => ProviderError::authentication("nvidia_nim", "Invalid API key"),
                404 => ProviderError::model_not_found("nvidia_nim", "Model not found"),
                429 => ProviderError::rate_limit_simple("nvidia_nim", "Rate limit exceeded"),
                _ => ProviderError::api_error(
                    "nvidia_nim",
                    status,
                    format!("HTTP {}: {}", status, body_text),
                ),
            });
        }

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("nvidia_nim", e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::response_parsing(
                "nvidia_nim",
                format!("Failed to parse response: {}", e),
            )
        })
    }

    /// Map OpenAI parameters to NVIDIA NIM format
    fn map_params(&self, params: &mut serde_json::Value, model: &str) {
        let supported = get_supported_params(model);

        // Filter out unsupported parameters
        if let Some(obj) = params.as_object_mut() {
            let keys_to_remove: Vec<String> = obj
                .keys()
                .filter(|k| !supported.contains(&k.as_str()) && *k != "messages" && *k != "model")
                .cloned()
                .collect();

            for key in keys_to_remove {
                obj.remove(&key);
            }

            // Map max_completion_tokens to max_tokens if present
            if let Some(max_completion) = obj.remove("max_completion_tokens") {
                if !obj.contains_key("max_tokens") {
                    obj.insert("max_tokens".to_string(), max_completion);
                }
            }
        }
    }
}

#[async_trait]
impl LLMProvider for NvidiaNimProvider {
    type Config = NvidiaNimConfig;
    type Error = ProviderError;
    type ErrorMapper = crate::core::traits::error_mapper::types::GenericErrorMapper;

    fn name(&self) -> &'static str {
        "nvidia_nim"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        NVIDIA_NIM_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        get_supported_params(model)
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, serde_json::Value>,
        model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        let supported = get_supported_params(model);

        // Filter unsupported params and map max_completion_tokens
        let mut result = HashMap::new();
        for (key, value) in params.drain() {
            if key == "max_completion_tokens" {
                result.insert("max_tokens".to_string(), value);
            } else if supported.contains(&key.as_str()) {
                result.insert(key, value);
            }
        }

        Ok(result)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        let mut request_json = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("nvidia_nim", e.to_string()))?;

        // Map parameters based on model
        self.map_params(&mut request_json, &request.model);

        // Remove tools if model doesn't support them
        if !supports_tools(&request.model) {
            if let Some(obj) = request_json.as_object_mut() {
                obj.remove("tools");
                obj.remove("tool_choice");
                obj.remove("parallel_tool_calls");
            }
        }

        Ok(request_json)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::response_parsing(
                "nvidia_nim",
                format!("Failed to parse response: {}", e),
            )
        })
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        crate::core::traits::error_mapper::types::GenericErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("NVIDIA NIM chat request: model={}", request.model);

        // Transform request
        let request_json = self.transform_request(request, context).await?;

        // Execute request
        let response = self
            .execute_request("/chat/completions", request_json)
            .await?;

        // Parse response
        serde_json::from_value(response).map_err(|e| {
            ProviderError::response_parsing(
                "nvidia_nim",
                format!("Failed to parse chat response: {}", e),
            )
        })
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("NVIDIA NIM streaming request: model={}", request.model);

        // Ensure streaming is enabled
        request.stream = true;

        // Get API configuration
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ProviderError::authentication("nvidia_nim", "API key is required"))?;

        // Build request JSON
        let mut request_json = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("nvidia_nim", e.to_string()))?;

        self.map_params(&mut request_json, &request.model);

        // Remove tools if not supported
        if !supports_tools(&request.model) {
            if let Some(obj) = request_json.as_object_mut() {
                obj.remove("tools");
                obj.remove("tool_choice");
            }
        }

        // Execute streaming request using reqwest directly for SSE
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_json)
            .send()
            .await
            .map_err(|e| ProviderError::network("nvidia_nim", e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(match status {
                400 => ProviderError::invalid_request(
                    "nvidia_nim",
                    body.unwrap_or_else(|| "Bad request".to_string()),
                ),
                401 => ProviderError::authentication("nvidia_nim", "Invalid API key"),
                429 => ProviderError::rate_limit_simple("nvidia_nim", "Rate limit exceeded"),
                _ => ProviderError::api_error(
                    "nvidia_nim",
                    status,
                    format!("Stream request failed: {}", status),
                ),
            });
        }

        // Create SSE stream
        let stream = NvidiaNimStream::new(response.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        debug!("NVIDIA NIM embedding request: model={}", request.model);

        // Transform request
        let request_json = serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("nvidia_nim", e.to_string()))?;

        // Execute request
        let response = self.execute_request("/embeddings", request_json).await?;

        // Parse response
        serde_json::from_value(response).map_err(|e| {
            ProviderError::response_parsing(
                "nvidia_nim",
                format!("Failed to parse embedding response: {}", e),
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
        let model_info = get_model_info(model)
            .ok_or_else(|| ProviderError::model_not_found("nvidia_nim", model))?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}

// ==================== Streaming Support ====================

use bytes::Bytes;

/// NVIDIA NIM streaming response parser
pub struct NvidiaNimStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    buffer: String,
}

impl NvidiaNimStream {
    pub fn new(stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static) -> Self {
        Self {
            inner: Box::pin(stream),
            buffer: String::new(),
        }
    }
}

impl Stream for NvidiaNimStream {
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            // Try to parse a complete SSE message from buffer
            if let Some(pos) = self.buffer.find("\n\n") {
                let message = self.buffer[..pos].to_string();
                self.buffer = self.buffer[pos + 2..].to_string();

                // Parse SSE message
                for line in message.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            return std::task::Poll::Ready(None);
                        }

                        match serde_json::from_str::<ChatChunk>(data) {
                            Ok(chunk) => return std::task::Poll::Ready(Some(Ok(chunk))),
                            Err(e) => {
                                return std::task::Poll::Ready(Some(Err(
                                    ProviderError::api_error(
                                        "nvidia_nim",
                                        500,
                                        format!("Failed to parse chunk: {}", e),
                                    ),
                                )));
                            }
                        }
                    }
                }
            }

            // Need more data
            match self.inner.as_mut().poll_next(cx) {
                std::task::Poll::Ready(Some(Ok(bytes))) => {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        self.buffer.push_str(&text);
                    }
                }
                std::task::Poll::Ready(Some(Err(e))) => {
                    return std::task::Poll::Ready(Some(Err(ProviderError::network(
                        "nvidia_nim",
                        e.to_string(),
                    ))));
                }
                std::task::Poll::Ready(None) => {
                    return std::task::Poll::Ready(None);
                }
                std::task::Poll::Pending => {
                    return std::task::Poll::Pending;
                }
            }
        }
    }
}
