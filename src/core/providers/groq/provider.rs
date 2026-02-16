//! Main Groq Provider Implementation
//!
//! Implements the LLMProvider trait for Groq's ultra-fast LPU-based inference.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::GroqConfig;
use super::error::GroqError;
use super::model_info::{get_available_models, get_model_info, is_reasoning_model};
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header, streaming_client};
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

/// Static capabilities for Groq provider
const GROQ_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

/// Groq provider implementation
#[derive(Debug, Clone)]
pub struct GroqProvider {
    config: GroqConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl GroqProvider {
    /// Create a new Groq provider instance
    pub async fn new(config: GroqConfig) -> Result<Self, GroqError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| GroqError::configuration("groq", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            GroqError::configuration("groq", format!("Failed to create pool manager: {}", e))
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
                // Note: Vision is supported through multimodal input, not a separate capability

                ModelInfo {
                    id: info.model_id.to_string(),
                    name: info.display_name.to_string(),
                    provider: "groq".to_string(),
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
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, GroqError> {
        let config = GroqConfig::from_env().with_api_key(api_key);
        Self::new(config).await
    }

    /// Check if response format requires fake streaming
    pub(crate) fn should_fake_stream(&self, request: &ChatRequest) -> bool {
        // Groq doesn't support response_format while streaming
        request.response_format.is_some() && request.stream
    }

    /// Transform messages for Groq API
    fn transform_messages(&self, request: &mut ChatRequest) {
        // Remove null function_call from assistant messages (Groq doesn't support it)
        for message in request.messages.iter_mut() {
            if message.role == MessageRole::Assistant {
                // Function call handling would go here if needed
                // Currently Groq supports tools differently
            }
        }
    }

    /// Handle response_format with tool calling
    fn handle_response_format(&self, _request: &mut ChatRequest) {
        // Groq supports JSON mode directly through response_format
        // No need to convert to tools
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, GroqError> {
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
            .map_err(|e| GroqError::network("groq", e.to_string()))?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| GroqError::network("groq", e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            GroqError::api_error("groq", 500, format!("Failed to parse response: {}", e))
        })
    }

    /// Speech-to-text transcription
    pub async fn transcribe_audio(
        &self,
        file: Vec<u8>,
        model: Option<String>,
        language: Option<String>,
        response_format: Option<String>,
    ) -> Result<super::stt::TranscriptionResponse, GroqError> {
        let request = super::stt::SpeechToTextRequest {
            file,
            model: model.unwrap_or_else(|| "whisper-large-v3-turbo".to_string()),
            language,
            prompt: None,
            response_format,
            temperature: None,
            timestamp_granularities: None,
        };

        // Create multipart form
        let form = super::stt::create_multipart_form(request)?;

        let url = format!("{}/audio/transcriptions", self.config.get_api_base());
        let mut headers = Vec::new();
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(("Authorization".to_string(), format!("Bearer {}", api_key)));
        }

        // Use streaming_client for connection pooling in multipart requests
        let client = streaming_client();
        let mut req = client.post(&url);
        for (key, value) in headers {
            req = req.header(key, value);
        }

        let response = req
            .multipart(form)
            .send()
            .await
            .map_err(|e| GroqError::network("groq", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(match status {
                400 => GroqError::invalid_request(
                    "groq",
                    body.unwrap_or_else(|| "Invalid audio format or parameters".to_string()),
                ),
                401 => GroqError::authentication("groq", "Invalid API key"),
                413 => GroqError::invalid_request("groq", "Audio file too large (max 25MB)"),
                429 => GroqError::rate_limit("groq", None),
                _ => GroqError::api_error(
                    "groq",
                    status,
                    format!("Transcription failed: {}", status),
                ),
            });
        }

        let response_text = response.text().await.map_err(|e| {
            GroqError::api_error("groq", 500, format!("Failed to read response: {}", e))
        })?;

        // Try to parse as JSON first
        if let Ok(json_response) =
            serde_json::from_str::<super::stt::TranscriptionResponse>(&response_text)
        {
            Ok(json_response)
        } else {
            // If not JSON, assume it's plain text response
            Ok(super::stt::TranscriptionResponse {
                text: response_text,
                task: Some("transcribe".to_string()),
                language: None,
                duration: None,
                words: None,
                segments: None,
            })
        }
    }
}

#[async_trait]
impl LLMProvider for GroqProvider {
    type Config = GroqConfig;
    type Error = GroqError;
    type ErrorMapper = crate::core::traits::error_mapper::DefaultErrorMapper;

    fn name(&self) -> &'static str {
        "groq"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        GROQ_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        let is_reasoning = is_reasoning_model(model);

        if is_reasoning {
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
                "reasoning_effort",
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
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        // Groq uses the same parameters as OpenAI
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
            .map_err(|e| GroqError::invalid_request("groq", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        // Parse response
        let chat_response: ChatResponse = serde_json::from_slice(raw_response).map_err(|e| {
            GroqError::api_error("groq", 500, format!("Failed to parse response: {}", e))
        })?;

        Ok(chat_response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        crate::core::traits::error_mapper::DefaultErrorMapper
    }

    async fn chat_completion(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Groq chat request: model={}", request.model);

        // Check if fake streaming is needed
        if self.should_fake_stream(&request) {
            request.stream = false;
        }

        // Transform and execute
        let request_json = serde_json::to_value(&request)
            .map_err(|e| GroqError::invalid_request("groq", e.to_string()))?;

        let response = self
            .execute_request("/chat/completions", request_json)
            .await?;

        serde_json::from_value(response).map_err(|e| {
            GroqError::api_error("groq", 500, format!("Failed to parse chat response: {}", e))
        })
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Groq streaming request: model={}", request.model);

        // Check if fake streaming is needed
        if self.should_fake_stream(&request) {
            // Execute non-streaming request and convert to fake stream
            request.stream = false;
            let response = self.chat_completion(request, context).await?;
            return super::streaming::create_fake_stream(response).await;
        }

        // Execute streaming request
        request.stream = true;

        // Get API configuration
        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| GroqError::authentication("groq", "API key is required"))?;

        // Execute streaming request using the global connection pool
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let client = streaming_client();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| GroqError::network("groq", e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok();
            return Err(match status {
                400 => GroqError::invalid_request(
                    "groq",
                    body.unwrap_or_else(|| "Bad request".to_string()),
                ),
                401 => GroqError::authentication("groq", "Invalid API key"),
                429 => GroqError::rate_limit("groq", None),
                _ => GroqError::api_error(
                    "groq",
                    status,
                    format!("Stream request failed: {}", status),
                ),
            });
        }

        // Create SSE stream
        let stream = super::streaming::GroqStream::new(response.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(GroqError::not_supported(
            "groq",
            "Groq does not support embeddings. Use text generation models instead.",
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
            GroqError::model_not_found("groq", format!("Unknown model: {}", model))
        })?;

        let input_cost = (input_tokens as f64) * (model_info.input_cost_per_million / 1_000_000.0);
        let output_cost =
            (output_tokens as f64) * (model_info.output_cost_per_million / 1_000_000.0);
        Ok(input_cost + output_cost)
    }
}
