//! Main Oobabooga Provider Implementation
//!
//! Implements the LLMProvider trait for Oobabooga text-generation-webui's OpenAI-compatible API.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::OobaboogaConfig;
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header_owned};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::types::GenericErrorMapper;
use crate::core::traits::{
    provider::ProviderConfig as _, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    message::MessageContent,
    message::MessageRole,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{
        ChatChoice, ChatChunk, ChatResponse, EmbeddingData, EmbeddingResponse, FinishReason, Usage,
    },
    tools::FunctionCall,
    tools::ToolCall,
};

/// Static capabilities for Oobabooga provider
const OOBABOOGA_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::Embeddings,
];

/// Oobabooga provider implementation
#[derive(Debug, Clone)]
pub struct OobaboogaProvider {
    config: OobaboogaConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl OobaboogaProvider {
    /// Create a new Oobabooga provider instance
    pub async fn new(config: OobaboogaConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("oobabooga", e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                "oobabooga",
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

    /// Create provider with custom API base
    pub async fn with_base_url(base_url: impl Into<String>) -> Result<Self, ProviderError> {
        let config = OobaboogaConfig {
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
        let auth_headers = self.config.build_auth_headers();
        let headers: Vec<_> = auth_headers
            .into_iter()
            .map(|(k, v)| header_owned(k, v))
            .collect();

        let response = self
            .pool_manager
            .execute_request(url, method, headers, body)
            .await
            .map_err(|e| {
                let error_msg = e.to_string();
                if error_msg.contains("Connection refused") || error_msg.contains("connect error") {
                    ProviderError::network(
                        "oobabooga",
                        "Failed to connect to text-generation-webui server. Is it running?"
                            .to_string(),
                    )
                } else if error_msg.contains("timed out") || error_msg.contains("timeout") {
                    ProviderError::Timeout {
                        provider: "oobabooga",
                        message: error_msg,
                    }
                } else {
                    ProviderError::network("oobabooga", error_msg)
                }
            })?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("oobabooga", e.to_string()))?;

        serde_json::from_slice(&response_bytes).map_err(|e| {
            ProviderError::api_error("oobabooga", 500, format!("Failed to parse response: {}", e))
        })
    }

    /// Build OpenAI-compatible chat request from ChatRequest
    fn build_chat_request(
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
                            crate::core::types::content::ContentPart::Image {
                                source,
                                detail,
                                ..
                            } => {
                                // Convert base64 image to URL format
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
                            crate::core::types::content::ContentPart::ImageUrl { image_url } => {
                                let mut img_obj = serde_json::json!({"url": &image_url.url});
                                if let Some(d) = &image_url.detail {
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
            "model": request.model.strip_prefix("oobabooga/").unwrap_or(&request.model),
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
    fn parse_chat_response(
        &self,
        response: serde_json::Value,
        model: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Check for error in response
        if let Some(error) = response.get("error") {
            let error_msg = error.as_str().unwrap_or("Unknown error");
            return Err(ProviderError::api_error(
                "oobabooga",
                500,
                error_msg.to_string(),
            ));
        }

        let choices = response
            .get("choices")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                ProviderError::api_error(
                    "oobabooga",
                    500,
                    "Missing choices in response".to_string(),
                )
            })?;

        let mut chat_choices = Vec::new();

        for (i, choice) in choices.iter().enumerate() {
            let message = choice.get("message").ok_or_else(|| {
                ProviderError::api_error("oobabooga", 500, "Missing message in choice".to_string())
            })?;

            let content = message
                .get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());

            // Parse tool calls if present
            let tool_calls = if let Some(tcs) = message.get("tool_calls").and_then(|v| v.as_array())
            {
                let calls: Vec<_> = tcs
                    .iter()
                    .map(|tc| {
                        let func = tc
                            .get("function")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({}));
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
                .unwrap_or(&format!("oobabooga-{}", uuid::Uuid::new_v4()))
                .to_string(),
            object: "chat.completion".to_string(),
            created: response
                .get("created")
                .and_then(|c| c.as_i64())
                .unwrap_or_else(|| chrono::Utc::now().timestamp()),
            model: format!(
                "oobabooga/{}",
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
impl LLMProvider for OobaboogaProvider {
    type Config = OobaboogaConfig;
    type Error = ProviderError;
    type ErrorMapper = GenericErrorMapper;

    fn name(&self) -> &'static str {
        "oobabooga"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        OOBABOOGA_CAPABILITIES
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
        // Oobabooga uses OpenAI-compatible API, so params pass through mostly unchanged
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
            ProviderError::api_error("oobabooga", 500, format!("Failed to parse response: {}", e))
        })?;

        self.parse_chat_response(response, model)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        GenericErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        debug!("Oobabooga chat request: model={}", request.model);

        let model = request.model.clone();
        let request_body = self.build_chat_request(&request, false)?;

        let url = self
            .config
            .get_chat_endpoint()
            .map_err(|e| ProviderError::configuration("oobabooga", e))?;
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
        debug!("Oobabooga streaming request: model={}", request.model);

        let request_body = self.build_chat_request(&request, true)?;

        // Use reqwest directly for streaming
        let url = self
            .config
            .get_chat_endpoint()
            .map_err(|e| ProviderError::configuration("oobabooga", e))?;

        let mut req = reqwest::Client::new().post(&url);

        // Add auth headers
        for (key, value) in self.config.build_auth_headers() {
            req = req.header(&key, &value);
        }

        let response = req.json(&request_body).send().await.map_err(|e| {
            let error_msg = e.to_string();
            if error_msg.contains("Connection refused") || error_msg.contains("connect error") {
                ProviderError::network(
                    "oobabooga",
                    "Failed to connect to text-generation-webui server. Is it running?".to_string(),
                )
            } else if error_msg.contains("timed out") || error_msg.contains("timeout") {
                ProviderError::Timeout {
                    provider: "oobabooga",
                    message: error_msg,
                }
            } else {
                ProviderError::network("oobabooga", error_msg)
            }
        })?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error("oobabooga", status, body));
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
                    "oobabooga",
                    "chat",
                    None,
                    None,
                    e.to_string(),
                ))),
            }
        });

        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        debug!("Oobabooga embeddings request: model={}", request.model);

        let model = request
            .model
            .strip_prefix("oobabooga/")
            .unwrap_or(&request.model);

        // Build input array
        let input: Vec<String> = match &request.input {
            crate::core::types::embedding::EmbeddingInput::Text(text) => vec![text.clone()],
            crate::core::types::embedding::EmbeddingInput::Array(texts) => texts.clone(),
        };

        let body = serde_json::json!({
            "input": input,
        });

        let url = self
            .config
            .get_embeddings_endpoint()
            .map_err(|e| ProviderError::configuration("oobabooga", e))?;
        let response = self
            .execute_request(&url, HttpMethod::POST, Some(body))
            .await?;

        // Check for error in response
        if let Some(error) = response.get("error") {
            let error_msg = error.as_str().unwrap_or("Unknown error");
            return Err(ProviderError::api_error(
                "oobabooga",
                500,
                error_msg.to_string(),
            ));
        }

        // Parse OpenAI-compatible embeddings response
        let data_arr = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| {
                ProviderError::api_error(
                    "oobabooga",
                    500,
                    "Missing data in embeddings response".to_string(),
                )
            })?;

        let data: Vec<EmbeddingData> = data_arr
            .iter()
            .enumerate()
            .map(|(i, emb)| {
                let embedding: Vec<f32> = emb
                    .get("embedding")
                    .and_then(|e| e.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect()
                    })
                    .unwrap_or_default();

                EmbeddingData {
                    object: "embedding".to_string(),
                    embedding,
                    index: i as u32,
                }
            })
            .collect();

        // Calculate token usage from embedding dimensions
        let num_tokens = data.first().map(|d| d.embedding.len()).unwrap_or(0);

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data,
            model: format!("oobabooga/{}", model),
            usage: Some(Usage {
                prompt_tokens: num_tokens as u32,
                completion_tokens: 0,
                total_tokens: num_tokens as u32,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
            embeddings: None,
        })
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to list models as a health check
        let url = match self.config.get_models_endpoint() {
            Ok(url) => url,
            Err(_) => return HealthStatus::Unhealthy,
        };

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
        // Oobabooga is local/free, so cost is always 0
        Ok(0.0)
    }
}

// Additional utility methods
impl OobaboogaProvider {
    /// Check if Oobabooga server is running
    pub async fn is_server_running(&self) -> bool {
        let url = match self.config.get_models_endpoint() {
            Ok(url) => url,
            Err(_) => return false,
        };
        self.execute_request(&url, HttpMethod::GET, None)
            .await
            .is_ok()
    }

    /// List available models from Oobabooga server
    pub async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let url = self
            .config
            .get_models_endpoint()
            .map_err(|e| ProviderError::configuration("oobabooga", e))?;
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
                provider: "oobabooga".to_string(),
                max_context_length: 4096, // Default, actual value depends on model
                max_output_length: None,
                supports_streaming: true,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.0), // Oobabooga is free
                output_cost_per_1k_tokens: Some(0.0), // Oobabooga is free
                currency: "USD".to_string(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                    ProviderCapability::Embeddings,
                ],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            })
            .collect();

        Ok(())
    }
}
