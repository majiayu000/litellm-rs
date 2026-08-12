//! Main Ollama Provider Implementation
//!
//! Implements the LLMProvider trait for Ollama's local inference server.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::OllamaConfig;
use super::error::{
    inline_image_data, parse_http_json_response, parse_tool_arguments, response_format_value,
    should_send_tools,
};
use super::model_info::{OllamaModelInfo, OllamaShowResponse, OllamaTagsResponse, get_model_info};
use super::streaming::OllamaStream;
use crate::core::providers::base::{
    BaseConfig, GlobalPoolManager, HttpErrorMapper, HttpMethod, header,
};
use crate::core::providers::shared::MessageTransformer;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
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

/// Static capabilities for Ollama provider
const OLLAMA_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::Embeddings,
    ProviderCapability::ToolCalling,
];

/// Ollama provider implementation
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    config: OllamaConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl OllamaProvider {
    const REQUEST_INTEGER_OPTIONS: [&'static str; 2] = ["num_ctx", "num_predict"];
    const REQUEST_FLOAT_OPTIONS: [&'static str; 1] = ["repeat_penalty"];

    fn merge_request_options(
        &self,
        options: &mut serde_json::Map<String, serde_json::Value>,
        request: &ChatRequest,
    ) -> Result<(), ProviderError> {
        for name in Self::REQUEST_INTEGER_OPTIONS {
            let Some(value) = request.extra_params.get(name) else {
                continue;
            };
            let integer = value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
                .or_else(|| {
                    value.as_f64().and_then(|value| {
                        (value.is_finite()
                            && value.fract() == 0.0
                            && value >= i64::MIN as f64
                            && value <= i64::MAX as f64)
                            .then_some(value as i64)
                    })
                });
            let valid = match (name, integer) {
                ("num_ctx", Some(value)) => value > 0 && u32::try_from(value).is_ok(),
                (_, Some(_)) => true,
                (_, None) => false,
            };
            if !valid {
                return Err(ProviderError::invalid_request(
                    "ollama",
                    format!("native Ollama option {name} must be a valid integer"),
                ));
            }
            let integer = integer.expect("validated integer option");
            if name == "num_ctx"
                && self
                    .config
                    .num_ctx
                    .is_some_and(|configured_max| integer as u64 > u64::from(configured_max))
            {
                return Err(ProviderError::invalid_request(
                    "ollama",
                    format!(
                        "native Ollama option num_ctx exceeds the configured maximum of {}",
                        self.config.num_ctx.expect("checked configured num_ctx")
                    ),
                ));
            }
            options.insert(name.to_string(), serde_json::json!(integer));
        }

        for name in Self::REQUEST_FLOAT_OPTIONS {
            let Some(value) = request.extra_params.get(name) else {
                continue;
            };
            let valid = value
                .as_f64()
                .is_some_and(|value| value.is_finite() && (value as f32).is_finite());
            if !valid {
                return Err(ProviderError::invalid_request(
                    "ollama",
                    format!("native Ollama option {name} must be a finite number"),
                ));
            }
            options.insert(name.to_string(), value.clone());
        }

        Ok(())
    }

    /// Create a new Ollama provider instance
    pub async fn new(config: OllamaConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("ollama", e))?;

        let api_base = config.get_api_base();
        let endpoint_access = config.resolved_endpoint_access(&api_base);
        let pool_manager = Arc::new(
            GlobalPoolManager::new_for_provider(
                "ollama",
                BaseConfig {
                    api_key: config.get_api_key(),
                    api_base: Some(api_base),
                    endpoint_access,
                    timeout: config.timeout,
                    max_retries: config.max_retries,
                    ..Default::default()
                },
            )
            .map_err(|e| {
                ProviderError::configuration(
                    "ollama",
                    format!("Failed to create pool manager: {}", e),
                )
            })?,
        );

        let models = config
            .models
            .iter()
            .map(|model| get_model_info(model).into())
            .collect();

        Ok(Self {
            config,
            pool_manager,
            models,
        })
    }

    /// Create provider with custom API base
    pub async fn with_base_url(base_url: impl Into<String>) -> Result<Self, ProviderError> {
        let config = OllamaConfig {
            api_base: Some(base_url.into()),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Create provider with default configuration (localhost:11434)
    pub async fn default_local() -> Result<Self, ProviderError> {
        Self::new(OllamaConfig::default()).await
    }

    /// Execute an HTTP request
    async fn execute_request(
        &self,
        url: &str,
        method: HttpMethod,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut headers = Vec::with_capacity(2);

        // Add auth header if API key is set
        if let Some(api_key) = &self.config.get_api_key() {
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
                    ProviderError::network(
                        "ollama",
                        format!(
                            "Failed to connect to Ollama server at {}. Is Ollama running?",
                            self.config.get_api_base()
                        ),
                    )
                } else if error_msg.contains("timed out") || error_msg.contains("timeout") {
                    ProviderError::Timeout {
                        provider: "ollama",
                        message: error_msg,
                    }
                } else {
                    ProviderError::network("ollama", error_msg)
                }
            })?;

        let status = response.status().as_u16();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("ollama", e.to_string()))?;
        parse_http_json_response(status, &response_bytes)
    }

    /// List available models from Ollama server
    pub async fn list_models(&self) -> Result<Vec<OllamaModelInfo>, ProviderError> {
        let url = self.config.get_tags_endpoint();
        let response = self.execute_request(&url, HttpMethod::GET, None).await?;

        let tags: OllamaTagsResponse = serde_json::from_value(response).map_err(|e| {
            ProviderError::api_error("ollama", 500, format!("Failed to parse models list: {}", e))
        })?;

        Ok(tags.models.into_iter().map(|m| m.into()).collect())
    }

    /// Get detailed model information
    pub async fn show_model(&self, model: &str) -> Result<OllamaShowResponse, ProviderError> {
        let url = self.config.get_show_endpoint();
        let body = serde_json::json!({ "name": model });

        let response = self
            .execute_request(&url, HttpMethod::POST, Some(body))
            .await?;

        serde_json::from_value(response).map_err(|e| {
            ProviderError::api_error("ollama", 500, format!("Failed to parse model info: {}", e))
        })
    }

    /// Build Ollama chat request from ChatRequest
    fn build_chat_request(
        &self,
        request: &ChatRequest,
        stream: bool,
    ) -> Result<serde_json::Value, ProviderError> {
        if request.n.is_some_and(|n| n != 1) {
            return Err(ProviderError::invalid_request(
                "ollama",
                "native Ollama supports exactly one choice",
            ));
        }
        let tool_names: HashMap<&str, &str> = request
            .messages
            .iter()
            .filter_map(|message| message.tool_calls.as_ref())
            .flatten()
            .map(|call| (call.id.as_str(), call.function.name.as_str()))
            .collect();
        let mut messages = self
            .config
            .system
            .as_ref()
            .map(|system| vec![serde_json::json!({"role": "system", "content": system})])
            .unwrap_or_default();

        for msg in &request.messages {
            if self.config.system.is_some() && msg.role == MessageRole::System {
                continue;
            }
            let role = match &msg.role {
                MessageRole::System | MessageRole::Developer => "system",
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
                    // Handle multimodal content
                    let mut images = Vec::new();
                    let mut text_parts = Vec::new();

                    for part in parts {
                        match part {
                            crate::core::types::content::ContentPart::Text { text } => {
                                text_parts.push(text.clone());
                            }
                            crate::core::types::content::ContentPart::ImageUrl { image_url } => {
                                images.push(inline_image_data(&image_url.url)?);
                            }
                            crate::core::types::content::ContentPart::Image { source, .. } => {
                                // Base64 encoded image
                                images.push(source.data.clone());
                            }
                            // Skip unsupported content types (Audio, Document, ToolResult, ToolUse)
                            _ => {}
                        }
                    }

                    message["content"] = serde_json::json!(text_parts.join("\n"));
                    if !images.is_empty() {
                        message["images"] = serde_json::json!(images);
                    }
                }
                None => {
                    message["content"] = serde_json::json!("");
                }
            }

            // Handle tool calls for assistant messages
            if let Some(tool_calls) = &msg.tool_calls {
                let ollama_tool_calls = tool_calls
                    .iter()
                    .map(|tc| {
                        let arguments = parse_tool_arguments(&tc.function.arguments)?;
                        Ok(serde_json::json!({
                            "id": tc.id,
                            "function": {
                                "name": tc.function.name,
                                "arguments": arguments
                            }
                        }))
                    })
                    .collect::<Result<Vec<_>, ProviderError>>()?;
                message["tool_calls"] = serde_json::json!(ollama_tool_calls);
            }

            if msg.role == MessageRole::Tool {
                if let Some(id) = msg.tool_call_id.as_deref() {
                    message["tool_call_id"] = serde_json::json!(id);
                    let name = msg
                        .name
                        .as_deref()
                        .or_else(|| tool_names.get(id).copied())
                        .ok_or_else(|| {
                            ProviderError::invalid_request(
                                "ollama",
                                format!("tool result references unknown call ID: {id}"),
                            )
                        })?;
                    message["tool_name"] = serde_json::json!(name);
                } else if let Some(name) = &msg.name {
                    message["tool_name"] = serde_json::json!(name);
                }
            }

            messages.push(message);
        }

        // Build the request body
        let mut body = serde_json::json!({
            "model": request.model.strip_prefix("ollama/").unwrap_or(&request.model),
            "messages": messages,
            "stream": stream,
        });

        // Add options from request parameters
        let mut options = self.config.build_options();
        if let serde_json::Value::Object(ref mut opts) = options {
            self.merge_request_options(opts, request)?;
            if let Some(temp) = request.temperature {
                opts.insert("temperature".to_string(), serde_json::json!(temp));
            }
            if let Some(top_p) = request.top_p {
                opts.insert("top_p".to_string(), serde_json::json!(top_p));
            }
            if let Some(max_tokens) = request.max_completion_tokens.or(request.max_tokens) {
                opts.insert("num_predict".to_string(), serde_json::json!(max_tokens));
            }
            if let Some(stop) = &request.stop {
                opts.insert("stop".to_string(), serde_json::json!(stop));
            }
            if let Some(freq_penalty) = request.frequency_penalty {
                opts.insert(
                    "frequency_penalty".to_string(),
                    serde_json::json!(freq_penalty),
                );
            }
            if let Some(pres_penalty) = request.presence_penalty {
                opts.insert(
                    "presence_penalty".to_string(),
                    serde_json::json!(pres_penalty),
                );
            }
            if let Some(seed) = request.seed {
                opts.insert("seed".to_string(), serde_json::json!(seed));
            }
        }
        body["options"] = options;

        // Add tools if present
        if should_send_tools(request.tool_choice.as_ref())?
            && let Some(tools) = &request.tools
        {
            let ollama_tools: Vec<_> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.function.name,
                            "description": t.function.description,
                            "parameters": t.function.parameters
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(ollama_tools);
        }

        // Add response format if set
        if let Some(format) = response_format_value(request.response_format.as_ref())? {
            body["format"] = format;
        }

        // Add keep_alive if set in config
        if let Some(keep_alive) = &self.config.keep_alive {
            body["keep_alive"] = serde_json::json!(keep_alive);
        }

        Ok(body)
    }

    /// Parse Ollama chat response into ChatResponse
    fn parse_chat_response(
        &self,
        response: serde_json::Value,
        model: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let message = response.get("message").ok_or_else(|| {
            ProviderError::api_error("ollama", 500, "Missing message in response".to_string())
        })?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        // Parse thinking content if present
        let thinking = message
            .get("thinking")
            .and_then(|t| t.as_str())
            .map(crate::core::types::thinking::ThinkingContent::text);

        // Parse tool calls if present
        let tool_calls = if let Some(tcs) = message.get("tool_calls").and_then(|v| v.as_array()) {
            let calls: Vec<_> = tcs
                .iter()
                .map(|tc| {
                    let func = tc
                        .get("function")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));
                    ToolCall {
                        id: format!("call_{}", uuid::Uuid::new_v4()),
                        tool_type: "function".to_string(),
                        function: FunctionCall {
                            name: func
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("")
                                .to_string(),
                            arguments: func
                                .get("arguments")
                                .map(|a| a.to_string())
                                .unwrap_or_default(),
                        },
                    }
                })
                .collect();
            if calls.is_empty() { None } else { Some(calls) }
        } else {
            None
        };

        // Determine finish reason
        let finish_reason = if tool_calls.is_some() {
            FinishReason::ToolCalls
        } else {
            response
                .get("done_reason")
                .and_then(|reason| reason.as_str())
                .and_then(MessageTransformer::parse_finish_reason)
                .unwrap_or(FinishReason::Stop)
        };

        // Build usage info
        let usage = Usage {
            prompt_tokens: response
                .get("prompt_eval_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: response
                .get("eval_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: response
                .get("prompt_eval_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32
                + response
                    .get("eval_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        };

        Ok(ChatResponse {
            id: format!("ollama-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: format!(
                "ollama/{}",
                response
                    .get("model")
                    .and_then(|m| m.as_str())
                    .unwrap_or(model)
            ),
            system_fingerprint: None,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: content.map(MessageContent::Text),
                    thinking,
                    audio: None,
                    tool_calls,
                    function_call: None,
                    name: None,
                    tool_call_id: None,
                },
                finish_reason: Some(finish_reason),
                logprobs: None,
            }],
            usage: Some(usage),
        })
    }
}

impl LLMProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        OLLAMA_CAPABILITIES
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
            // Ollama-specific params exposed as OpenAI-compatible
            "num_ctx",
            "num_predict",
            "repeat_penalty",
        ]
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // Map max_tokens to num_predict (Ollama's equivalent)
        if let Some(max_tokens) = params.remove("max_tokens") {
            params.insert("num_predict".to_string(), max_tokens);
        }
        if let Some(max_completion_tokens) = params.remove("max_completion_tokens") {
            params.insert("num_predict".to_string(), max_completion_tokens);
        }

        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        self.build_chat_request(&request, request.stream)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let response: serde_json::Value = serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::api_error("ollama", 500, format!("Failed to parse response: {}", e))
        })?;

        self.parse_chat_response(response, model)
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(GenericErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Ollama chat request: model={}", request.model);

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
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("Ollama streaming request: model={}", request.model);

        let request_body = self.build_chat_request(&request, true)?;

        let url = self.config.get_chat_endpoint();
        let mut headers = vec![header("Content-Type", "application/json".to_string())];
        if let Some(api_key) = self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        let response = self
            .pool_manager
            .execute_streaming_request(&url, headers, request_body, "ollama")
            .await
            .map_err(|error| {
                let error_msg = error.to_string();
                if error_msg.contains("Connection refused") || error_msg.contains("connect error") {
                    ProviderError::network(
                        "ollama",
                        format!(
                            "Failed to connect to Ollama server at {}. Is Ollama running?",
                            self.config.get_api_base()
                        ),
                    )
                } else {
                    error
                }
            })?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body =
                crate::core::providers::base::connection_pool::read_streaming_error_body(response)
                    .await
                    .map_err(|err| err.into_provider_error("ollama"))?;
            return Err(HttpErrorMapper::map_status_code("ollama", status, &body));
        }

        // Create NDJSON stream
        let stream = OllamaStream::new(response.bytes_stream());
        Ok(Box::pin(stream))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        debug!("Ollama embeddings request: model={}", request.model);

        let model = request
            .model
            .strip_prefix("ollama/")
            .unwrap_or(&request.model);

        // Build input array
        let input = match request.input {
            crate::core::types::embedding::EmbeddingInput::Text(text) => vec![text],
            crate::core::types::embedding::EmbeddingInput::Array(texts) => texts,
        };
        let expected_embedding_count = input.len();

        let body = serde_json::json!({
            "model": model,
            "input": input,
        });

        let url = self.config.get_embeddings_endpoint();
        let response = self
            .execute_request(&url, HttpMethod::POST, Some(body))
            .await?;

        // Parse Ollama embeddings response
        // Ollama returns: { "embeddings": [[...], [...]] }
        let embeddings = response
            .get("embeddings")
            .and_then(|e| e.as_array())
            .ok_or_else(|| {
                ProviderError::response_parsing("ollama", "Missing embeddings in response")
            })?;

        if embeddings.len() != expected_embedding_count {
            return Err(ProviderError::response_parsing(
                "ollama",
                format!(
                    "Ollama returned {} embeddings for {expected_embedding_count} inputs",
                    embeddings.len()
                ),
            ));
        }

        let mut expected_dimension = None;
        let data: Vec<EmbeddingData> = embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| -> Result<EmbeddingData, ProviderError> {
                let values = emb
                    .as_array()
                    .ok_or_else(|| {
                        ProviderError::response_parsing(
                            "ollama",
                            format!("Ollama embedding at index {i} is not an array"),
                        )
                    })?;
                if values.is_empty() {
                    return Err(ProviderError::response_parsing(
                        "ollama",
                        format!("Ollama embedding at index {i} is empty"),
                    ));
                }
                if expected_dimension.is_some_and(|dimension| dimension != values.len()) {
                    return Err(ProviderError::response_parsing(
                        "ollama",
                        format!("Ollama embedding at index {i} has an inconsistent dimension"),
                    ));
                }
                expected_dimension.get_or_insert(values.len());
                let embedding = values
                    .iter()
                    .enumerate()
                    .map(|(coordinate, value)| {
                        value
                            .as_f64()
                            .filter(|value| value.is_finite() && (*value as f32).is_finite())
                            .map(|value| value as f32)
                            .ok_or_else(|| {
                                ProviderError::response_parsing(
                                    "ollama",
                                    format!(
                                        "Ollama embedding at index {i} has an invalid coordinate at {coordinate}"
                                    ),
                                )
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(EmbeddingData {
                    object: "embedding".to_string(),
                    embedding,
                    index: i as u32,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prompt_tokens = response
            .get("prompt_eval_count")
            .and_then(serde_json::Value::as_u64)
            .map(|count| u32::try_from(count).unwrap_or(u32::MAX))
            .unwrap_or(0);

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data,
            model: format!("ollama/{}", model),
            usage: Some(Usage {
                prompt_tokens,
                completion_tokens: 0,
                total_tokens: prompt_tokens,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
            embeddings: None,
        })
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to list models as a health check
        match self.list_models().await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        // Ollama is local/free, so cost is always 0
        Ok(0.0)
    }
}

// Additional utility methods
impl OllamaProvider {
    /// Check if Ollama server is running
    pub async fn is_server_running(&self) -> bool {
        self.list_models().await.is_ok()
    }

    /// Get model info from server
    pub async fn get_model_info(&self, model: &str) -> Result<OllamaModelInfo, ProviderError> {
        // First try to get detailed info from show endpoint
        match self.show_model(model).await {
            Ok(show_response) => {
                let mut info = get_model_info(model);

                // Enrich with server data
                if let Some(ctx_len) = show_response.get_context_length() {
                    info.max_context_length = Some(ctx_len);
                }
                if show_response.supports_tools() {
                    info.supports_tools = true;
                }
                if let Some(details) = show_response.details {
                    info.family = details.family;
                    info.parameter_size = details.parameter_size;
                    info.quantization = details.quantization_level;
                }

                Ok(info)
            }
            Err(_) => {
                // Fall back to inferred info
                Ok(get_model_info(model))
            }
        }
    }

    /// Refresh model list from server
    pub async fn refresh_models(&mut self) -> Result<(), ProviderError> {
        let ollama_models = self.list_models().await?;

        self.models = ollama_models.into_iter().map(Into::into).collect();

        Ok(())
    }
}
