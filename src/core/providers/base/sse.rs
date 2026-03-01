//! Unified SSE (Server-Sent Events) Parser
//!
//! A centralized SSE parsing system that eliminates code duplication across providers.
//! All providers can use this parser and only need to implement the transformation logic.

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice, FinishReason};
use crate::core::{providers::unified_provider::ProviderError, types::thinking::ThinkingDelta};

/// SSE Event Types
#[derive(Debug, Clone, PartialEq)]
pub enum SSEEventType {
    Data,
    Event,
    Id,
    Retry,
    Comment,
}

/// Parsed SSE Event
#[derive(Debug, Clone)]
pub struct SSEEvent {
    pub event_type: Option<String>,
    pub data: String,
    pub id: Option<String>,
    pub retry: Option<u64>,
}

impl SSEEvent {
    /// Parse SSE event from a line
    pub fn from_line(line: &str) -> Option<Self> {
        if line.is_empty() || line.starts_with(':') {
            return None;
        }

        if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = line[colon_pos + 1..].trim_start();

            match field {
                "data" => Some(SSEEvent {
                    event_type: None,
                    data: value.to_string(),
                    id: None,
                    retry: None,
                }),
                "event" => Some(SSEEvent {
                    event_type: Some(value.to_string()),
                    data: String::new(),
                    id: None,
                    retry: None,
                }),
                "id" => Some(SSEEvent {
                    event_type: None,
                    data: String::new(),
                    id: Some(value.to_string()),
                    retry: None,
                }),
                "retry" => {
                    if let Ok(retry_ms) = value.parse::<u64>() {
                        Some(SSEEvent {
                            event_type: None,
                            data: String::new(),
                            id: None,
                            retry: Some(retry_ms),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

/// Trait for provider-specific SSE transformation
pub trait SSETransformer: Send + Sync {
    /// Provider name for error reporting
    fn provider_name(&self) -> &'static str;

    /// Check if this is the end-of-stream marker
    fn is_end_marker(&self, data: &str) -> bool {
        data.trim() == "[DONE]"
    }

    /// Transform raw SSE data into ChatChunk
    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError>;

    /// Parse finish reason from string (provider-specific)
    fn parse_finish_reason(&self, reason: &str) -> Option<FinishReason> {
        match reason {
            "stop" => Some(FinishReason::Stop),
            "length" | "max_tokens" => Some(FinishReason::Length),
            "tool_calls" | "function_call" => Some(FinishReason::ToolCalls),
            "content_filter" => Some(FinishReason::ContentFilter),
            _ => None,
        }
    }
}

/// Unified SSE Parser
pub struct UnifiedSSEParser<T: SSETransformer> {
    transformer: T,
    buffer: String,
    current_event: Option<SSEEvent>,
}

impl<T: SSETransformer> UnifiedSSEParser<T> {
    /// Create new SSE parser with a transformer
    pub fn new(transformer: T) -> Self {
        Self {
            transformer,
            buffer: String::new(),
            current_event: None,
        }
    }

    /// Process raw bytes into SSE events
    ///
    /// Optimized to minimize allocations:
    /// - Uses `from_utf8_lossy` which returns `Cow<str>` (borrowed when valid UTF-8)
    /// - Processes lines without collecting into intermediate `Vec<String>`
    /// - Only allocates for incomplete lines that need to be buffered
    pub fn process_bytes(&mut self, bytes: &[u8]) -> Result<Vec<ChatChunk>, ProviderError> {
        // Append new bytes to buffer - from_utf8_lossy avoids allocation for valid UTF-8
        let text = String::from_utf8_lossy(bytes);
        self.buffer.push_str(&text);

        let mut chunks = Vec::new();

        // Find the last newline position
        let last_newline = self.buffer.rfind('\n');

        if let Some(pos) = last_newline {
            // Extract complete part and remaining incomplete part
            let complete_part = self.buffer[..=pos].to_string();
            let incomplete_part = self.buffer[pos + 1..].to_string();

            // Update buffer with incomplete part before processing
            // (This avoids borrow issues)
            self.buffer = incomplete_part;

            // Process complete lines
            for line in complete_part.lines() {
                if let Some(chunk) = self.process_line(line)? {
                    chunks.push(chunk);
                }
            }
        }
        // If no newline found, keep buffering (no action needed)

        Ok(chunks)
    }

    /// Process a single SSE line
    fn process_line(&mut self, line: &str) -> Result<Option<ChatChunk>, ProviderError> {
        // Empty line signals end of event
        if line.is_empty() {
            if let Some(event) = self.current_event.take() {
                return self.process_event(event);
            }
            return Ok(None);
        }

        // Parse SSE field
        if let Some(event) = SSEEvent::from_line(line) {
            // For data fields, accumulate or merge
            if !event.data.is_empty() {
                if let Some(ref mut current) = self.current_event {
                    // Append to existing data
                    if !current.data.is_empty() {
                        current.data.push('\n');
                    }
                    current.data.push_str(&event.data);
                } else {
                    self.current_event = Some(event);
                }
            } else if event.event_type.is_some() || event.id.is_some() || event.retry.is_some() {
                // Merge other fields
                if let Some(ref mut current) = self.current_event {
                    if event.event_type.is_some() {
                        current.event_type = event.event_type;
                    }
                    if event.id.is_some() {
                        current.id = event.id;
                    }
                    if event.retry.is_some() {
                        current.retry = event.retry;
                    }
                } else {
                    self.current_event = Some(event);
                }
            }
        }

        Ok(None)
    }

    /// Process a complete SSE event
    fn process_event(&self, event: SSEEvent) -> Result<Option<ChatChunk>, ProviderError> {
        // Skip empty events
        if event.data.is_empty() {
            return Ok(None);
        }

        // Check for end marker
        if self.transformer.is_end_marker(&event.data) {
            return Ok(None);
        }

        // Transform to ChatChunk
        self.transformer.transform_chunk(&event.data)
    }
}

/// Streaming wrapper that uses UnifiedSSEParser
///
/// Uses `VecDeque` for buffered chunks to enable O(1) pop_front instead of O(n) Vec::remove(0).
pub struct UnifiedSSEStream<S, T>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin,
    T: SSETransformer + Clone,
{
    inner: S,
    parser: UnifiedSSEParser<T>,
    chunk_buffer: VecDeque<ChatChunk>,
}

impl<S, T> UnifiedSSEStream<S, T>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin,
    T: SSETransformer + Clone,
{
    pub fn new(stream: S, transformer: T) -> Self {
        Self {
            inner: stream,
            parser: UnifiedSSEParser::new(transformer),
            chunk_buffer: VecDeque::new(),
        }
    }
}

impl<S, T> Stream for UnifiedSSEStream<S, T>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin,
    T: SSETransformer + Clone + Unpin,
{
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // Return buffered chunks first - O(1) with VecDeque
        if let Some(chunk) = this.chunk_buffer.pop_front() {
            return Poll::Ready(Some(Ok(chunk)));
        }

        // Poll inner stream for more data
        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                match this.parser.process_bytes(&bytes) {
                    Ok(chunks) => {
                        if chunks.is_empty() {
                            // No chunks yet, poll again
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        } else {
                            // Buffer chunks and return first one
                            this.chunk_buffer.extend(chunks);
                            if let Some(chunk) = this.chunk_buffer.pop_front() {
                                Poll::Ready(Some(Ok(chunk)))
                            } else {
                                cx.waker().wake_by_ref();
                                Poll::Pending
                            }
                        }
                    }
                    Err(e) => Poll::Ready(Some(Err(e))),
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(ProviderError::network(
                this.parser.transformer.provider_name(),
                format!("Stream error: {}", e),
            )))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// OpenAI-compatible SSE Transformer (can be reused by many providers)
#[derive(Debug, Clone)]
pub struct OpenAICompatibleTransformer {
    provider: &'static str,
}

impl OpenAICompatibleTransformer {
    pub fn new(provider: &'static str) -> Self {
        Self { provider }
    }
}

impl SSETransformer for OpenAICompatibleTransformer {
    fn provider_name(&self) -> &'static str {
        self.provider
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        // Parse JSON
        let json_value: Value = serde_json::from_str(data).map_err(|e| {
            ProviderError::response_parsing(
                self.provider,
                format!("Failed to parse SSE JSON: {}", e),
            )
        })?;

        // Extract fields
        let id = json_value
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("stream-chunk")
            .to_string();

        let model = json_value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let created = json_value
            .get("created")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp()) as u64;

        // Parse choices
        let choices = json_value
            .get("choices")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                ProviderError::response_parsing(
                    self.provider,
                    "No choices in SSE chunk".to_string(),
                )
            })?;

        let mut stream_choices = Vec::new();

        for (index, choice) in choices.iter().enumerate() {
            let delta = choice.get("delta").ok_or_else(|| {
                ProviderError::response_parsing(self.provider, "No delta in choice".to_string())
            })?;

            let mut delta_obj: ChatDelta = serde_json::from_value(delta.clone()).map_err(|e| {
                ProviderError::response_parsing(
                    self.provider,
                    format!("Failed to parse delta: {}", e),
                )
            })?;
            if let Some(Value::String(reasoning_content)) = delta.get("reasoning_content") {
                delta_obj.thinking = Some(ThinkingDelta {
                    content: Some(reasoning_content.clone()),
                    ..Default::default()
                });
            }

            let finish_reason = choice
                .get("finish_reason")
                .and_then(|v| v.as_str())
                .and_then(|s| self.parse_finish_reason(s));

            let logprobs = choice
                .get("logprobs")
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            stream_choices.push(ChatStreamChoice {
                index: index as u32,
                delta: delta_obj,
                finish_reason,
                logprobs,
            });
        }

        // Parse usage (optional)
        let usage = json_value
            .get("usage")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        Ok(Some(ChatChunk {
            id,
            object: "chat.completion.chunk".to_string(),
            created: created as i64,
            model,
            choices: stream_choices,
            usage,
            system_fingerprint: json_value
                .get("system_fingerprint")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }))
    }
}

/// Anthropic SSE Transformer
///
/// Handles Anthropic's event-based SSE format with message_start, content_block_delta,
/// message_delta, and message_stop events.
#[derive(Debug, Clone)]
pub struct AnthropicTransformer {
    model: String,
}

impl AnthropicTransformer {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
        }
    }

    fn parse_anthropic_finish_reason(reason: &str) -> FinishReason {
        match reason {
            "end_turn" => FinishReason::Stop,
            "max_tokens" => FinishReason::Length,
            "tool_use" => FinishReason::ToolCalls,
            _ => FinishReason::Stop,
        }
    }
}

impl SSETransformer for AnthropicTransformer {
    fn provider_name(&self) -> &'static str {
        "anthropic"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let json: Value = serde_json::from_str(data).map_err(|e| {
            ProviderError::response_parsing(
                "anthropic",
                format!("Failed to parse Anthropic SSE: {}", e),
            )
        })?;

        let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let created = chrono::Utc::now().timestamp();

        match event_type {
            "message_start" => {
                let message_id = json
                    .get("message")
                    .and_then(|m| m.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("anthropic-stream")
                    .to_string();

                Ok(Some(ChatChunk {
                    id: message_id,
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: self.model.clone(),
                    choices: vec![ChatStreamChoice {
                        index: 0,
                        delta: ChatDelta {
                            role: Some(crate::core::types::message::MessageRole::Assistant),
                            content: None,
                            thinking: None,
                            tool_calls: None,
                            function_call: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    usage: None,
                    system_fingerprint: None,
                }))
            }
            "content_block_delta" => {
                let text = json
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                Ok(Some(ChatChunk {
                    id: String::new(),
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: self.model.clone(),
                    choices: vec![ChatStreamChoice {
                        index: 0,
                        delta: ChatDelta {
                            role: None,
                            content: Some(text.to_string()),
                            thinking: None,
                            tool_calls: None,
                            function_call: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    usage: None,
                    system_fingerprint: None,
                }))
            }
            "message_delta" => {
                let finish_reason = json
                    .get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(|r| r.as_str())
                    .map(Self::parse_anthropic_finish_reason);

                let usage = json.get("usage").map(|u| {
                    let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let output =
                        u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    crate::core::types::responses::Usage {
                        prompt_tokens: input,
                        completion_tokens: output,
                        total_tokens: input + output,
                        completion_tokens_details: None,
                        prompt_tokens_details: None,
                        thinking_usage: None,
                    }
                });

                Ok(Some(ChatChunk {
                    id: String::new(),
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: self.model.clone(),
                    choices: vec![ChatStreamChoice {
                        index: 0,
                        delta: ChatDelta {
                            role: None,
                            content: None,
                            thinking: None,
                            tool_calls: None,
                            function_call: None,
                        },
                        finish_reason,
                        logprobs: None,
                    }],
                    usage,
                    system_fingerprint: None,
                }))
            }
            "message_stop" => Ok(Some(ChatChunk {
                id: String::new(),
                object: "chat.completion.chunk".to_string(),
                created,
                model: self.model.clone(),
                choices: vec![],
                usage: None,
                system_fingerprint: None,
            })),
            "error" => {
                let msg = json
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown streaming error");
                Err(ProviderError::streaming_error(
                    "anthropic",
                    "chat",
                    None,
                    None,
                    msg.to_string(),
                ))
            }
            // content_block_start, content_block_stop, ping — skip
            _ => Ok(None),
        }
    }
}

/// Gemini SSE Transformer
///
/// Handles Gemini's streaming format with candidates/parts structure.
#[derive(Debug, Clone)]
pub struct GeminiTransformer {
    model: String,
    chunk_id: String,
}

impl GeminiTransformer {
    pub fn new(model: impl Into<String>) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Self {
            model: model.into(),
            chunk_id: format!("gemini-stream-{}", nanos),
        }
    }
}

impl SSETransformer for GeminiTransformer {
    fn provider_name(&self) -> &'static str {
        "gemini"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let json: Value = serde_json::from_str(data).map_err(|e| {
            ProviderError::response_parsing("gemini", format!("Failed to parse Gemini SSE: {}", e))
        })?;

        // Error response
        if let Some(error) = json.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown Gemini error");
            return Err(ProviderError::api_error(
                "gemini",
                error.get("code").and_then(|c| c.as_u64()).unwrap_or(500) as u16,
                msg.to_string(),
            ));
        }

        let empty_arr = vec![];
        let candidates = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .unwrap_or(&empty_arr);

        if candidates.is_empty() {
            // Usage-only chunk
            let usage = json.get("usageMetadata").map(|u| {
                let prompt = u
                    .get("promptTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let completion = u
                    .get("candidatesTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let total = u
                    .get("totalTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                crate::core::types::responses::Usage {
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    total_tokens: total,
                    prompt_tokens_details: None,
                    completion_tokens_details: None,
                    thinking_usage: None,
                }
            });
            if usage.is_some() {
                return Ok(Some(ChatChunk {
                    id: self.chunk_id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp(),
                    model: self.model.clone(),
                    choices: vec![],
                    usage,
                    system_fingerprint: None,
                }));
            }
            return Ok(None);
        }

        let mut choices = Vec::new();
        for (index, candidate) in candidates.iter().enumerate() {
            let empty_parts = vec![];
            let parts = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
                .unwrap_or(&empty_parts);

            let mut text_parts = Vec::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text);
                }
            }
            let delta_content = text_parts.join("");

            let finish_reason = candidate
                .get("finishReason")
                .and_then(|r| r.as_str())
                .map(|r| match r {
                    "STOP" => FinishReason::Stop,
                    "MAX_TOKENS" => FinishReason::Length,
                    "SAFETY" | "RECITATION" => FinishReason::ContentFilter,
                    _ => FinishReason::Stop,
                });

            choices.push(ChatStreamChoice {
                index: index as u32,
                delta: ChatDelta {
                    role: if !delta_content.is_empty() || finish_reason.is_some() {
                        Some(crate::core::types::message::MessageRole::Assistant)
                    } else {
                        None
                    },
                    content: if delta_content.is_empty() {
                        None
                    } else {
                        Some(delta_content)
                    },
                    thinking: None,
                    function_call: None,
                    tool_calls: None,
                },
                finish_reason,
                logprobs: None,
            });
        }

        let usage = json.get("usageMetadata").map(|u| {
            let prompt = u
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let completion = u
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let total = u
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            crate::core::types::responses::Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: total,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }
        });

        if choices.is_empty() && usage.is_none() {
            return Ok(None);
        }

        Ok(Some(ChatChunk {
            id: self.chunk_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: self.model.clone(),
            choices,
            usage,
            system_fingerprint: None,
        }))
    }
}

/// Cohere SSE Transformer
///
/// Handles Cohere's streaming format with v1/v2 API version support.
#[derive(Debug, Clone)]
pub struct CohereTransformer {
    model: String,
    response_id: String,
    /// true = v2, false = v1
    use_v2: bool,
}

impl CohereTransformer {
    pub fn new(model: impl Into<String>, use_v2: bool) -> Self {
        Self {
            model: model.into(),
            response_id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            use_v2,
        }
    }

    fn parse_cohere_finish_reason(reason: &str) -> FinishReason {
        match reason.to_lowercase().as_str() {
            "stop" | "complete" | "end_turn" => FinishReason::Stop,
            "length" | "max_tokens" => FinishReason::Length,
            "tool_calls" | "tool_use" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            _ => FinishReason::Stop,
        }
    }
}

impl SSETransformer for CohereTransformer {
    fn provider_name(&self) -> &'static str {
        "cohere"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let json: Value = serde_json::from_str(data).map_err(|e| {
            ProviderError::response_parsing("cohere", format!("Failed to parse Cohere SSE: {}", e))
        })?;

        let event_type = json
            .get("type")
            .or_else(|| json.get("event"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let created = chrono::Utc::now().timestamp();

        if self.use_v2 {
            match event_type {
                "content-delta" => {
                    let text = json
                        .get("delta")
                        .and_then(|d| d.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|c| {
                            c.get("text")
                                .and_then(|t| t.as_str())
                                .or_else(|| c.as_str())
                        })
                        .unwrap_or("");

                    if text.is_empty() {
                        return Ok(None);
                    }

                    Ok(Some(ChatChunk {
                        id: self.response_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: self.model.clone(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: None,
                                content: Some(text.to_string()),
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                        system_fingerprint: None,
                    }))
                }
                "message-end" => {
                    let data_field = json.get("data").or(json.get("delta"));
                    let finish_reason = data_field
                        .and_then(|d| d.get("delta"))
                        .and_then(|d| d.get("finish_reason"))
                        .and_then(|f| f.as_str())
                        .unwrap_or("stop");

                    let usage = data_field
                        .and_then(|d| d.get("delta"))
                        .and_then(|d| d.get("usage"))
                        .and_then(|u| u.get("tokens"))
                        .map(|tokens| {
                            let prompt = tokens
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            let completion = tokens
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            crate::core::types::responses::Usage {
                                prompt_tokens: prompt,
                                completion_tokens: completion,
                                total_tokens: prompt + completion,
                                prompt_tokens_details: None,
                                completion_tokens_details: None,
                                thinking_usage: None,
                            }
                        });

                    Ok(Some(ChatChunk {
                        id: self.response_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: self.model.clone(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: None,
                                content: None,
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                            },
                            finish_reason: Some(Self::parse_cohere_finish_reason(finish_reason)),
                            logprobs: None,
                        }],
                        usage,
                        system_fingerprint: None,
                    }))
                }
                // message-start, content-start, content-end, tool-call-*, citation-* — skip
                _ => Ok(None),
            }
        } else {
            // v1 format
            match event_type {
                "text-generation" => {
                    let text = json.get("text").and_then(|t| t.as_str()).unwrap_or("");
                    Ok(Some(ChatChunk {
                        id: self.response_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: self.model.clone(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: None,
                                content: Some(text.to_string()),
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                        system_fingerprint: None,
                    }))
                }
                "stream-end" => {
                    let finish_reason = json
                        .get("finish_reason")
                        .and_then(|f| f.as_str())
                        .unwrap_or("stop");
                    Ok(Some(ChatChunk {
                        id: self.response_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: self.model.clone(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: None,
                                content: None,
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                            },
                            finish_reason: Some(Self::parse_cohere_finish_reason(finish_reason)),
                            logprobs: None,
                        }],
                        usage: None,
                        system_fingerprint: None,
                    }))
                }
                // stream-start, citation-generation, tool-calls-generation — skip
                _ => Ok(None),
            }
        }
    }
}

/// Databricks SSE Transformer
///
/// OpenAI-compatible format with additional support for array content (Claude-style).
#[derive(Debug, Clone)]
pub struct DatabricksTransformer;

impl SSETransformer for DatabricksTransformer {
    fn provider_name(&self) -> &'static str {
        "databricks"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let json: Value = serde_json::from_str(data).map_err(|e| {
            ProviderError::response_parsing(
                "databricks",
                format!("Failed to parse SSE JSON: {}", e),
            )
        })?;

        let id = json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("chunk")
            .to_string();
        let created = json
            .get("created")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp());
        let model = json
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut choices = Vec::new();
        if let Some(choices_arr) = json.get("choices").and_then(|v| v.as_array()) {
            for choice in choices_arr {
                let index = choice.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                let delta = if let Some(delta_obj) = choice.get("delta") {
                    let role =
                        delta_obj
                            .get("role")
                            .and_then(|v| v.as_str())
                            .and_then(|r| match r {
                                "assistant" => {
                                    Some(crate::core::types::message::MessageRole::Assistant)
                                }
                                "user" => Some(crate::core::types::message::MessageRole::User),
                                "system" => Some(crate::core::types::message::MessageRole::System),
                                "tool" => Some(crate::core::types::message::MessageRole::Tool),
                                _ => None,
                            });

                    // Handle content — could be string or array (Claude reasoning)
                    let content = match delta_obj.get("content") {
                        Some(Value::String(s)) => Some(s.clone()),
                        Some(Value::Array(arr)) => {
                            let mut text = String::new();
                            for item in arr {
                                if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                                    text.push_str(t);
                                }
                            }
                            if text.is_empty() { None } else { Some(text) }
                        }
                        _ => None,
                    };

                    ChatDelta {
                        role,
                        content,
                        thinking: None,
                        tool_calls: None,
                        function_call: None,
                    }
                } else {
                    ChatDelta {
                        role: None,
                        content: None,
                        thinking: None,
                        tool_calls: None,
                        function_call: None,
                    }
                };

                let finish_reason = choice
                    .get("finish_reason")
                    .and_then(|v| v.as_str())
                    .and_then(|r| self.parse_finish_reason(r));

                choices.push(ChatStreamChoice {
                    index,
                    delta,
                    finish_reason,
                    logprobs: None,
                });
            }
        }

        Ok(Some(ChatChunk {
            id,
            object: "chat.completion.chunk".to_string(),
            created,
            model,
            choices,
            usage: None,
            system_fingerprint: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_event_parsing() {
        let event = SSEEvent::from_line("data: test data").unwrap();
        assert_eq!(event.data, "test data");

        let event = SSEEvent::from_line("event: message").unwrap();
        assert_eq!(event.event_type, Some("message".to_string()));

        let event = SSEEvent::from_line("id: 123").unwrap();
        assert_eq!(event.id, Some("123".to_string()));

        let event = SSEEvent::from_line("retry: 5000").unwrap();
        assert_eq!(event.retry, Some(5000));

        // Comments should be ignored
        assert!(SSEEvent::from_line(": comment").is_none());
    }

    #[test]
    fn test_openai_transformer() {
        let transformer = OpenAICompatibleTransformer::new("test");

        // Test end marker
        assert!(transformer.is_end_marker("[DONE]"));
        assert!(!transformer.is_end_marker("data: {\"test\": 1}"));

        // Test JSON transformation
        let json_data = r#"{
            "id": "test-id",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello"},
                "finish_reason": null
            }]
        }"#;

        let result = transformer.transform_chunk(json_data).unwrap().unwrap();
        assert_eq!(result.id, "test-id");
        assert_eq!(result.model, "gpt-4");
        assert_eq!(result.choices[0].delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_openai_transformer_reasoning_content_to_thinking() {
        let transformer = OpenAICompatibleTransformer::new("test");

        let json_data = r#"{
            "id": "test-id-reasoning",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "Answer",
                    "reasoning_content": "chain-of-thought"
                },
                "finish_reason": null
            }]
        }"#;

        let result = transformer.transform_chunk(json_data).unwrap().unwrap();
        assert_eq!(
            result.choices[0]
                .delta
                .thinking
                .as_ref()
                .and_then(|t| t.content.as_ref())
                .map(String::as_str),
            Some("chain-of-thought")
        );
    }

    #[test]
    fn test_sse_parser_multiline() {
        let transformer = OpenAICompatibleTransformer::new("test");
        let mut parser = UnifiedSSEParser::new(transformer);

        // Simulate receiving data in chunks
        let chunk1 = b"data: {\"id\": \"test\", ";
        let chunk2 = b"\"choices\": [{\"delta\": {\"content\": \"Hi\"}, \"index\": 0}], ";
        let chunk3 = b"\"model\": \"gpt-4\", \"created\": 123}\n\n";

        let results1 = parser.process_bytes(chunk1).unwrap();
        assert!(results1.is_empty()); // Not complete yet

        let results2 = parser.process_bytes(chunk2).unwrap();
        assert!(results2.is_empty()); // Still not complete

        let results3 = parser.process_bytes(chunk3).unwrap();
        assert_eq!(results3.len(), 1); // Now we have a complete event
        assert_eq!(results3[0].choices[0].delta.content, Some("Hi".to_string()));
    }
}
