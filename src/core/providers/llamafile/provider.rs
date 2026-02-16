//! Main Llamafile Provider Implementation
//!
//! Implements the LLMProvider trait for Llamafile's OpenAI-compatible local inference server.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::LlamafileConfig;
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    provider::ProviderConfig as _, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    message::MessageContent,
    message::MessageRole,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChoice, ChatChunk, ChatResponse, FinishReason, Usage},
    tools::FunctionCall,
    tools::ToolCall,
};

/// Provider name constant
const PROVIDER_NAME: &str = "llamafile";
pub(crate) const LLAMAFILE_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
];

/// Llamafile provider implementation
#[derive(Debug, Clone)]
pub struct LlamafileProvider {
    pub(crate) config: LlamafileConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl LlamafileProvider {
    /// Create a new Llamafile provider instance
    pub async fn new(config: LlamafileConfig) -> Result<Self, ProviderError> {
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

        // Initialize with empty models
        let models = Vec::new();

        Ok(Self {
            config,
            pool_manager,
            models,
        })
    }

    /// Create provider with default configuration
    pub async fn default_local() -> Result<Self, ProviderError> {
        Self::new(LlamafileConfig::default()).await
    }

    /// Create provider with custom API base
    pub async fn with_base_url(base_url: impl Into<String>) -> Result<Self, ProviderError> {
        let config = LlamafileConfig {
            api_base: Some(base_url.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        url: &str,
        method: HttpMethod,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut headers = Vec::with_capacity(2);

        // Add auth header (Llamafile typically doesn't require it, but support it anyway)
        let api_key = self.config.get_api_key();
        if api_key != "fake-api-key" {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(url, method, headers, body)
            .await
            .map_err(|e| {
                let error_msg = e.to_string();
                if error_msg.contains("Connection refused") || error_msg.contains("connect error") {
                    ProviderError::provider_unavailable(
                        PROVIDER_NAME,
                        "Failed to connect to llamafile server. Is llamafile running?",
                    )
                } else if error_msg.contains("timed out") || error_msg.contains("timeout") {
                    ProviderError::timeout(PROVIDER_NAME, error_msg)
                } else {
                    ProviderError::network(PROVIDER_NAME, error_msg)
                }
            })?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::api_error(
                PROVIDER_NAME,
                500,
                format!("Failed to parse response: {}", e),
            )
        })
    }

    /// Build OpenAI-compatible chat request from ChatRequest
    pub(crate) fn build_chat_request(
        &self,
        request: &ChatRequest,
        stream: bool,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut messages = Vec::new();

        for msg in &request.messages {
            let role = match &msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
                MessageRole::Function => "function",
            };

            let mut message = serde_json::json!({
                "role": role,
            });

            // Handle content
            match &msg.content {
                Some(MessageContent::Text(text)) => {
                    message["content"] = serde_json::json!(text);
                }
                Some(MessageContent::Parts(parts)) => {
                    // Convert to OpenAI multimodal format
                    let content_parts: Vec<_> = parts
                        .iter()
                        .filter_map(|part| match part {
                            crate::core::types::content::ContentPart::Text { text } => {
                                Some(serde_json::json!({"type": "text", "text": text}))
                            }
                            crate::core::types::content::ContentPart::ImageUrl { image_url } => {
                                // Image URL format
                                let mut img_obj = serde_json::json!({"url": &image_url.url});
                                if let Some(d) = &image_url.detail {
                                    img_obj["detail"] = serde_json::json!(d);
                                }
                                Some(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": img_obj
                                }))
                            }
                            crate::core::types::content::ContentPart::Image {
                                source,
                                detail,
                                ..
                            } => {
                                // Base64 image format
                                let url =
                                    format!("data:{};base64,{}", source.media_type, source.data);
                                let mut img_obj = serde_json::json!({"url": url});
                                if let Some(d) = detail {
                                    img_obj["detail"] = serde_json::json!(d);
                                }
                                Some(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": img_obj
                                }))
                            }
                            // Skip unsupported content types
                            _ => None,
                        })
                        .collect();
                    message["content"] = serde_json::json!(content_parts);
                }
                None => {
                    message["content"] = serde_json::json!("");
                }
            }

            // Handle tool calls for assistant messages
            if let Some(tool_calls) = &msg.tool_calls {
                let openai_tool_calls: Vec<_> = tool_calls
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.arguments
                            }
                        })
                    })
                    .collect();
                message["tool_calls"] = serde_json::json!(openai_tool_calls);
            }

            // Handle tool call id for tool messages
            if msg.role == MessageRole::Tool {
                if let Some(name) = &msg.name {
                    message["name"] = serde_json::json!(name);
                }
            }

            messages.push(message);
        }

        // Build the request body (OpenAI format)
        let mut body = serde_json::json!({
            "model": request.model.strip_prefix("llamafile/").unwrap_or(&request.model),
            "messages": messages,
            "stream": stream,
        });

        // Add optional parameters
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(stop) = &request.stop {
            body["stop"] = serde_json::json!(stop);
        }
        if let Some(freq_penalty) = request.frequency_penalty {
            body["frequency_penalty"] = serde_json::json!(freq_penalty);
        }
        if let Some(pres_penalty) = request.presence_penalty {
            body["presence_penalty"] = serde_json::json!(pres_penalty);
        }
        if let Some(seed) = request.seed {
            body["seed"] = serde_json::json!(seed);
        }

        Ok(body)
    }

    /// Parse OpenAI-compatible chat response into ChatResponse
    pub(crate) fn parse_chat_response(
        &self,
        response: serde_json::Value,
        model: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let choices = response
            .get("choices")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                ProviderError::api_error(PROVIDER_NAME, 500, "Missing choices in response")
            })?;

        let mut chat_choices = Vec::new();

        for (i, choice) in choices.iter().enumerate() {
            let message = choice.get("message").ok_or_else(|| {
                ProviderError::api_error(PROVIDER_NAME, 500, "Missing message in choice")
            })?;

            let content = message
                .get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());

            // Parse tool calls if present
            let tool_calls = if let Some(tcs) = message.get("tool_calls").and_then(|v| v.as_array())
            {
                let empty_obj = serde_json::json!({});
                let calls: Vec<_> = tcs
                    .iter()
                    .map(|tc| {
                        let func = tc.get("function").unwrap_or(&empty_obj);
                        ToolCall {
                            id: tc
                                .get("id")
                                .and_then(|id| id.as_str())
                                .unwrap_or("")
                                .to_string(),
                            tool_type: "function".to_string(),
                            function: FunctionCall {
                                name: func
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                arguments: func
                                    .get("arguments")
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            },
                        }
                    })
                    .collect();
                if calls.is_empty() { None } else { Some(calls) }
            } else {
                None
            };

            // Determine finish reason
            let finish_reason_str = choice
                .get("finish_reason")
                .and_then(|r| r.as_str())
                .unwrap_or("stop");
            let finish_reason = match finish_reason_str {
                "stop" => FinishReason::Stop,
                "length" => FinishReason::Length,
                "tool_calls" => FinishReason::ToolCalls,
                "content_filter" => FinishReason::ContentFilter,
                "function_call" => FinishReason::FunctionCall,
                _ => FinishReason::Stop,
            };

            chat_choices.push(ChatChoice {
                index: i as u32,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: content.map(MessageContent::Text),
                    thinking: None,
                    tool_calls,
                    function_call: None,
                    name: None,
                    tool_call_id: None,
                },
                finish_reason: Some(finish_reason),
                logprobs: None,
            });
        }

        // Build usage info
        let usage = response.get("usage").map(|usage_obj| Usage {
            prompt_tokens: usage_obj
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: usage_obj
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: usage_obj
                .get("total_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        Ok(ChatResponse {
            id: response
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or(&format!("llamafile-{}", uuid::Uuid::new_v4()))
                .to_string(),
            object: "chat.completion".to_string(),
            created: response
                .get("created")
                .and_then(|c| c.as_i64())
                .unwrap_or_else(|| chrono::Utc::now().timestamp()),
            model: format!(
                "llamafile/{}",
                response
                    .get("model")
                    .and_then(|m| m.as_str())
                    .unwrap_or(model)
            ),
            system_fingerprint: response
                .get("system_fingerprint")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            choices: chat_choices,
            usage,
        })
    }
}

#[async_trait]
impl LLMProvider for LlamafileProvider {
    type Config = LlamafileConfig;
    type Error = ProviderError;
    type ErrorMapper = crate::core::traits::error_mapper::DefaultErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        LLAMAFILE_CAPABILITIES
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
            "seed",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        // Llamafile uses OpenAI-compatible API, so params pass through mostly unchanged
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        self.build_chat_request(&request, request.stream)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response: serde_json::Value = serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::api_error(
                PROVIDER_NAME,
                500,
                format!("Failed to parse response: {}", e),
            )
        })?;

        self.parse_chat_response(response, model)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        crate::core::traits::error_mapper::DefaultErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Llamafile chat request: model={}", request.model);

        let model = request.model.clone();
        let request_body = self.build_chat_request(&request, false)?;

        let url = self.config.get_chat_endpoint();
        let response = self
            .execute_request(&url, HttpMethod::POST, Some(request_body))
            .await?;

        self.parse_chat_response(response, &model)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        debug!("Llamafile streaming request: model={}", request.model);

        let request_body = self.build_chat_request(&request, true)?;

        // Use reqwest directly for streaming
        let url = self.config.get_chat_endpoint();

        let api_key = self.config.get_api_key();
        let mut req = reqwest::Client::new().post(&url);

        // Add auth header if not fake
        if api_key != "fake-api-key" {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                let error_msg = e.to_string();
                if error_msg.contains("Connection refused") || error_msg.contains("connect error") {
                    ProviderError::provider_unavailable(
                        PROVIDER_NAME,
                        "Failed to connect to llamafile server. Is llamafile running?",
                    )
                } else if error_msg.contains("timed out") || error_msg.contains("timeout") {
                    ProviderError::timeout(PROVIDER_NAME, error_msg)
                } else {
                    ProviderError::network(PROVIDER_NAME, error_msg)
                }
            })?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.ok().unwrap_or_default();
            return Err(Self::map_http_error(status, &body));
        }

        // Create SSE stream using the OpenAI streaming format
        use futures::StreamExt;
        let byte_stream = response.bytes_stream();

        let stream = byte_stream.filter_map(|result| async move {
            match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    // Parse SSE format
                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                return None;
                            }
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(choices) =
                                    json.get("choices").and_then(|c| c.as_array())
                                {
                                    if let Some(choice) = choices.first() {
                                        let delta = choice
                                            .get("delta")
                                            .cloned()
                                            .unwrap_or_else(|| serde_json::json!({}));
                                        let content = delta
                                            .get("content")
                                            .and_then(|c| c.as_str())
                                            .map(|s| s.to_string());

                                        let finish_reason = choice
                                            .get("finish_reason")
                                            .and_then(|r| r.as_str())
                                            .map(|r| match r {
                                                "stop" => FinishReason::Stop,
                                                "length" => FinishReason::Length,
                                                "tool_calls" => FinishReason::ToolCalls,
                                                _ => FinishReason::Stop,
                                            });

                                        return Some(Ok(ChatChunk {
                                            id: json
                                                .get("id")
                                                .and_then(|id| id.as_str())
                                                .unwrap_or("")
                                                .to_string(),
                                            object: "chat.completion.chunk".to_string(),
                                            created: json
                                                .get("created")
                                                .and_then(|c| c.as_i64())
                                                .unwrap_or(0),
                                            model: json
                                                .get("model")
                                                .and_then(|m| m.as_str())
                                                .unwrap_or("")
                                                .to_string(),
                                            choices: vec![
                                                crate::core::types::responses::ChatStreamChoice {
                                                    index: 0,
                                                    delta:
                                                        crate::core::types::responses::ChatDelta {
                                                            role: None,
                                                            content,
                                                            thinking: None,
                                                            tool_calls: None,
                                                            function_call: None,
                                                        },
                                                    finish_reason,
                                                    logprobs: None,
                                                },
                                            ],
                                            system_fingerprint: None,
                                            usage: None,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                    None
                }
                Err(e) => Some(Err(ProviderError::streaming_error(
                    PROVIDER_NAME,
                    "chat",
                    None,
                    None,
                    e.to_string(),
                ))),
            }
        });

        Ok(Box::pin(stream))
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to list models as a health check
        let url = self.config.get_models_endpoint();

        match self.execute_request(&url, HttpMethod::GET, None).await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // Llamafile is local/free, so cost is always 0
        Ok(0.0)
    }
}

// Additional utility methods
impl LlamafileProvider {
    /// Map HTTP status codes to provider errors
    fn map_http_error(status: u16, body: &str) -> ProviderError {
        let message = if body.is_empty() {
            format!("HTTP error {}", status)
        } else {
            body.to_string()
        };

        // Check for specific error patterns
        let message_lower = message.to_lowercase();
        if message_lower.contains("model") && message_lower.contains("not found") {
            return ProviderError::model_not_found(PROVIDER_NAME, message);
        }
        if message_lower.contains("context length") || message_lower.contains("too long") {
            return ProviderError::context_length_exceeded(PROVIDER_NAME, 0, 0);
        }

        match status {
            400 => ProviderError::invalid_request(PROVIDER_NAME, message),
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            403 => ProviderError::authentication(PROVIDER_NAME, "Access forbidden"),
            404 => ProviderError::model_not_found(PROVIDER_NAME, message),
            408 | 504 => ProviderError::timeout(PROVIDER_NAME, message),
            429 => ProviderError::rate_limit(PROVIDER_NAME, None),
            500 => ProviderError::api_error(PROVIDER_NAME, 500, message),
            502 | 503 => ProviderError::provider_unavailable(PROVIDER_NAME, message),
            _ => ProviderError::api_error(PROVIDER_NAME, status, message),
        }
    }

    /// Check if Llamafile server is running
    pub async fn is_server_running(&self) -> bool {
        let url = self.config.get_models_endpoint();
        self.execute_request(&url, HttpMethod::GET, None)
            .await
            .is_ok()
    }

    /// List available models from Llamafile server
    pub async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let url = self.config.get_models_endpoint();
        let response = self.execute_request(&url, HttpMethod::GET, None).await?;

        let models = response
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        m.get("id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }

    /// Refresh model list from server
    pub async fn refresh_models(&mut self) -> Result<(), ProviderError> {
        let model_ids = self.list_models().await?;

        self.models = model_ids
            .into_iter()
            .map(|id| ModelInfo {
                id: id.clone(),
                name: id.clone(),
                provider: "llamafile".to_string(),
                max_context_length: 4096, // Default, actual value depends on model
                max_output_length: None,
                supports_streaming: true,
                supports_tools: false, // Llamafile basic support
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0), // Llamafile is free
                output_cost_per_1k_tokens: Some(0.0), // Llamafile is free
                currency: "USD".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            })
            .collect();

        Ok(())
    }
}
