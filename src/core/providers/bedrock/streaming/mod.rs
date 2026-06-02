//! Streaming Module for Bedrock
//!
//! Handles AWS Event Stream parsing and streaming responses

use crate::core::providers::bedrock::model_config::{BedrockApiType, BedrockModelFamily};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::ChatChunk;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::Value;
use std::pin::Pin;
use std::task::{Context, Poll};

/// AWS Event Stream message
#[derive(Debug)]
pub struct EventStreamMessage {
    pub headers: Vec<EventStreamHeader>,
    pub payload: Bytes,
}

/// Event stream header
#[derive(Debug)]
pub struct EventStreamHeader {
    pub name: String,
    pub value: HeaderValue,
}

/// Header value types
#[derive(Debug)]
pub enum HeaderValue {
    String(String),
    ByteArray(Vec<u8>),
    Boolean(bool),
    Byte(i8),
    Short(i16),
    Integer(i32),
    Long(i64),
    UUID(String),
    Timestamp(i64),
}

/// Bedrock streaming response
pub struct BedrockStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, ProviderError>> + Send>>,
    buffer: Vec<u8>,
    model_family: BedrockModelFamily,
    api_type: BedrockApiType,
}

impl BedrockStream {
    /// Create a new Bedrock stream
    pub fn new(
        stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
        model_family: BedrockModelFamily,
        api_type: BedrockApiType,
    ) -> Self {
        let mapped_stream = stream
            .map(|result| result.map_err(|e| ProviderError::network("bedrock", e.to_string())));

        Self {
            inner: Box::pin(mapped_stream),
            buffer: Vec::new(),
            model_family,
            api_type,
        }
    }

    /// Parse event stream message from bytes
    fn parse_event_message(data: &[u8]) -> Result<EventStreamMessage, ProviderError> {
        if data.len() < 16 {
            return Err(ProviderError::response_parsing(
                "bedrock",
                "Invalid event stream message",
            ));
        }

        // Parse prelude (12 bytes)
        let total_length = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let headers_length = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        // let prelude_crc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        if data.len() < total_length {
            return Err(ProviderError::response_parsing(
                "bedrock",
                "Incomplete event stream message",
            ));
        }

        // Parse headers
        let mut headers = Vec::new();
        let mut offset = 12;
        let headers_end = 12 + headers_length;

        while offset < headers_end {
            if offset + 1 > data.len() {
                break;
            }

            let name_length = data[offset] as usize;
            offset += 1;

            if offset + name_length > data.len() {
                break;
            }

            let name = String::from_utf8_lossy(&data[offset..offset + name_length]).to_string();
            offset += name_length;

            if offset >= data.len() {
                break;
            }

            let header_type = data[offset];
            offset += 1;

            let value = match header_type {
                5 | 7 => {
                    // String type
                    if offset + 2 > data.len() {
                        break;
                    }
                    let string_length =
                        u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                    offset += 2;
                    if offset + string_length > data.len() {
                        break;
                    }
                    let string_value =
                        String::from_utf8_lossy(&data[offset..offset + string_length]).to_string();
                    offset += string_length;
                    HeaderValue::String(string_value)
                }
                _ => {
                    // Skip unknown header types
                    HeaderValue::String(String::new())
                }
            };

            headers.push(EventStreamHeader { name, value });
        }

        // Extract payload
        let payload_start = headers_end;
        let payload_end = total_length - 4; // Exclude message CRC
        let payload = if payload_start < payload_end && payload_end <= data.len() {
            Bytes::copy_from_slice(&data[payload_start..payload_end])
        } else {
            Bytes::new()
        };

        Ok(EventStreamMessage { headers, payload })
    }

    fn header_value<'a>(message: &'a EventStreamMessage, name: &str) -> Option<&'a str> {
        message.headers.iter().find_map(|header| {
            (header.name == name)
                .then_some(&header.value)
                .and_then(|value| match value {
                    HeaderValue::String(value) => Some(value.as_str()),
                    _ => None,
                })
        })
    }

    fn stream_exception_from_payload(value: &Value) -> Option<(String, String)> {
        let object = value.as_object()?;

        for (code, detail) in object {
            if code.ends_with("Exception") || code.ends_with("exception") {
                let message = detail
                    .get("message")
                    .and_then(Value::as_str)
                    .or_else(|| detail.as_str())
                    .unwrap_or("");
                return Some((code.clone(), message.to_string()));
            }
        }

        None
    }

    fn stream_error(code: &str, message: &str) -> ProviderError {
        let details = if message.is_empty() {
            format!("Bedrock stream error: {code}")
        } else {
            format!("Bedrock stream error {code}: {message}")
        };

        if code.eq_ignore_ascii_case("validationException") {
            ProviderError::invalid_request("bedrock", details)
        } else {
            ProviderError::api_error("bedrock", 500, details)
        }
    }

    fn check_stream_error(message: &EventStreamMessage) -> Result<(), ProviderError> {
        let message_type = Self::header_value(message, ":message-type");
        let exception_type = Self::header_value(message, ":exception-type");

        if matches!(message_type, Some("exception" | "error")) || exception_type.is_some() {
            let payload = serde_json::from_slice::<Value>(&message.payload).ok();
            let payload_message = payload
                .as_ref()
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let code = exception_type.unwrap_or("streamException");
            return Err(Self::stream_error(code, payload_message));
        }

        if let Ok(payload) = serde_json::from_slice::<Value>(&message.payload)
            && let Some((code, message)) = Self::stream_exception_from_payload(&payload)
        {
            return Err(Self::stream_error(&code, &message));
        }

        Ok(())
    }

    /// Parse chunk based on model family
    fn parse_chunk(&self, payload: &[u8]) -> Result<Option<ChatChunk>, ProviderError> {
        let json_str = String::from_utf8_lossy(payload);
        let mut value: Value = serde_json::from_str(&json_str)
            .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

        match &self.api_type {
            BedrockApiType::Converse | BedrockApiType::ConverseStream => {
                return self.parse_converse_chunk(&value);
            }
            BedrockApiType::Invoke | BedrockApiType::InvokeStream => {
                value = Self::decode_invoke_stream_payload(value)?;
            }
        }

        // Parse invoke-style streams based on model family.
        match &self.model_family {
            BedrockModelFamily::Claude => self.parse_claude_chunk(&value),
            BedrockModelFamily::Nova => self.parse_nova_chunk(&value),
            BedrockModelFamily::TitanText => self.parse_titan_chunk(&value),
            _ => {
                // Generic parsing for other models
                self.parse_generic_chunk(&value)
            }
        }
    }

    fn decode_invoke_stream_payload(value: Value) -> Result<Value, ProviderError> {
        let Some(encoded) = value
            .get("chunk")
            .and_then(|chunk| chunk.get("bytes"))
            .and_then(Value::as_str)
        else {
            return Ok(value);
        };

        use base64::Engine as _;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;
        serde_json::from_slice(&decoded)
            .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))
    }

    fn parse_buffered_chunk(&mut self) -> Option<Result<Option<ChatChunk>, ProviderError>> {
        if self.buffer.len() < 16 {
            return None;
        }

        let total_length = u32::from_be_bytes([
            self.buffer[0],
            self.buffer[1],
            self.buffer[2],
            self.buffer[3],
        ]) as usize;
        if total_length < 16 {
            self.buffer.clear();
            return Some(Err(ProviderError::response_parsing(
                "bedrock",
                "invalid Bedrock event stream frame length",
            )));
        }

        if self.buffer.len() < total_length {
            return None;
        }

        let message_data = self.buffer[..total_length].to_vec();
        self.buffer.drain(..total_length);
        Some(
            Self::parse_event_message(&message_data).and_then(|message| {
                Self::check_stream_error(&message)?;
                self.parse_chunk(&message.payload)
            }),
        )
    }

    fn parse_converse_finish_reason(
        stop_reason: Option<&str>,
    ) -> crate::core::types::responses::FinishReason {
        use crate::core::types::responses::FinishReason;

        match stop_reason {
            Some("tool_use") => FinishReason::ToolCalls,
            Some("max_tokens") => FinishReason::Length,
            Some("model_context_window_exceeded") => FinishReason::Length,
            Some("stop_sequence") => FinishReason::StopSequence,
            Some("content_filtered") | Some("guardrail_intervened") => FinishReason::ContentFilter,
            Some("malformed_model_output") | Some("malformed_tool_use") => FinishReason::Refusal,
            _ => FinishReason::Stop,
        }
    }

    fn parse_openai_finish_reason(reason: &str) -> crate::core::types::responses::FinishReason {
        use crate::core::types::responses::FinishReason;

        match reason {
            "length" => FinishReason::Length,
            "tool_calls" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            "stop_sequence" => FinishReason::StopSequence,
            _ => FinishReason::Stop,
        }
    }

    fn parse_openai_tool_call_deltas(
        value: &Value,
    ) -> Option<Vec<crate::core::types::responses::ToolCallDelta>> {
        use crate::core::types::responses::{FunctionCallDelta, ToolCallDelta};

        let calls = value.as_array()?;
        let tool_calls = calls
            .iter()
            .map(|call| {
                let function = call.get("function").map(|function| FunctionCallDelta {
                    name: function
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    arguments: function
                        .get("arguments")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });

                ToolCallDelta {
                    index: call
                        .get("index")
                        .and_then(Value::as_u64)
                        .and_then(|index| u32::try_from(index).ok())
                        .unwrap_or(0),
                    id: call.get("id").and_then(Value::as_str).map(str::to_string),
                    tool_type: call.get("type").and_then(Value::as_str).map(str::to_string),
                    function,
                }
            })
            .collect::<Vec<_>>();

        (!tool_calls.is_empty()).then_some(tool_calls)
    }

    fn converse_content_block_index(event: &Value) -> u32 {
        event
            .get("contentBlockIndex")
            .and_then(Value::as_u64)
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0)
    }

    fn parse_converse_tool_start(
        event: &Value,
    ) -> Option<crate::core::types::responses::ToolCallDelta> {
        use crate::core::types::responses::{FunctionCallDelta, ToolCallDelta};

        let content_block_start = event.get("contentBlockStart")?;
        let tool_use = content_block_start.get("start")?.get("toolUse")?;
        let tool_use = tool_use.get("tool_use").unwrap_or(tool_use);

        Some(ToolCallDelta {
            index: Self::converse_content_block_index(content_block_start),
            id: Some(tool_use.get("toolUseId")?.as_str()?.to_string()),
            tool_type: Some("function".to_string()),
            function: Some(FunctionCallDelta {
                name: Some(tool_use.get("name")?.as_str()?.to_string()),
                arguments: None,
            }),
        })
    }

    fn parse_converse_tool_input(
        event: &Value,
    ) -> Option<crate::core::types::responses::ToolCallDelta> {
        use crate::core::types::responses::{FunctionCallDelta, ToolCallDelta};

        let content_block_delta = event.get("contentBlockDelta")?;
        let tool_use = content_block_delta.get("delta")?.get("toolUse")?;
        let tool_use = tool_use.get("tool_use").unwrap_or(tool_use);
        let input = tool_use.get("input")?.as_str()?;

        Some(ToolCallDelta {
            index: Self::converse_content_block_index(content_block_delta),
            id: tool_use
                .get("toolUseId")
                .and_then(Value::as_str)
                .map(str::to_string),
            tool_type: None,
            function: Some(FunctionCallDelta {
                name: tool_use
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                arguments: Some(input.to_string()),
            }),
        })
    }

    /// Parse ConverseStream event chunks.
    fn parse_converse_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::{ChatDelta, ChatStreamChoice};

        if let Some(tool_call) = Self::parse_converse_tool_start(value) {
            return Ok(Some(ChatChunk {
                id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: String::new(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta {
                        tool_calls: Some(vec![tool_call]),
                        ..Default::default()
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            }));
        }

        if let Some(tool_call) = Self::parse_converse_tool_input(value) {
            return Ok(Some(ChatChunk {
                id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: String::new(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta {
                        tool_calls: Some(vec![tool_call]),
                        ..Default::default()
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            }));
        }

        if let Some(content) = value
            .get("contentBlockDelta")
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
        {
            return Ok(Some(ChatChunk {
                id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: String::new(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta {
                        content: Some(content.to_string()),
                        ..Default::default()
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            }));
        }

        if let Some(message_stop) = value.get("messageStop") {
            let stop_reason = message_stop
                .get("stopReason")
                .or_else(|| value.get("stopReason"))
                .and_then(|reason| reason.as_str());
            let finish_reason = Self::parse_converse_finish_reason(stop_reason);

            return Ok(Some(ChatChunk {
                id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: String::new(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta::default(),
                    finish_reason: Some(finish_reason),
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            }));
        }

        Ok(None)
    }

    /// Parse Claude streaming chunk
    fn parse_claude_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::{ChatDelta, ChatStreamChoice};

        // Claude uses specific event types
        let event_type = value.get("type").and_then(|v| v.as_str());

        match event_type {
            Some("content_block_delta") => {
                let delta = value
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                Ok(Some(ChatChunk {
                    id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp(),
                    model: String::new(),
                    choices: vec![ChatStreamChoice {
                        index: 0,
                        delta: ChatDelta {
                            content: Some(delta.to_string()),
                            ..Default::default()
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    usage: None,
                    system_fingerprint: None,
                }))
            }
            Some("message_stop") => Ok(Some(ChatChunk {
                id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: String::new(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta::default(),
                    finish_reason: Some(crate::core::types::responses::FinishReason::Stop),
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            })),
            _ => Ok(None),
        }
    }

    /// Parse Nova streaming chunk
    fn parse_nova_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::{ChatDelta, ChatStreamChoice};

        if let Some(content) = value
            .get("contentBlockDelta")
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
        {
            Ok(Some(ChatChunk {
                id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: String::new(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta {
                        content: Some(content.to_string()),
                        ..Default::default()
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            }))
        } else {
            Ok(None)
        }
    }

    /// Parse Titan streaming chunk
    fn parse_titan_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::{ChatDelta, ChatStreamChoice};

        if let Some(content) = value.get("outputText").and_then(|t| t.as_str()) {
            Ok(Some(ChatChunk {
                id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: String::new(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta {
                        content: Some(content.to_string()),
                        ..Default::default()
                    },
                    finish_reason: if value.get("completionReason").is_some() {
                        Some(crate::core::types::responses::FinishReason::Stop)
                    } else {
                        None
                    },
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            }))
        } else {
            Ok(None)
        }
    }

    /// Parse generic streaming chunk
    fn parse_generic_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::{ChatDelta, ChatStreamChoice};

        let openai_choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());
        let openai_delta = openai_choice.and_then(|choice| choice.get("delta"));
        let openai_content = openai_delta
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
            .or_else(|| {
                openai_choice
                    .and_then(|choice| choice.get("text"))
                    .and_then(Value::as_str)
            });
        let openai_tool_calls = openai_delta
            .and_then(|delta| delta.get("tool_calls"))
            .and_then(Self::parse_openai_tool_call_deltas);
        let openai_finish_reason = openai_choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .map(Self::parse_openai_finish_reason);

        // Try to find content in common locations
        let content = openai_content.or_else(|| {
            value
                .get("completion")
                .or_else(|| value.get("generation"))
                .or_else(|| value.get("text"))
                .and_then(|t| t.as_str())
                .or_else(|| {
                    value
                        .get("outputs")
                        .and_then(Value::as_array)
                        .and_then(|outputs| outputs.first())
                        .and_then(|output| output.get("text"))
                        .and_then(Value::as_str)
                })
                .or_else(|| {
                    value
                        .get("results")
                        .and_then(Value::as_array)
                        .and_then(|results| results.first())
                        .and_then(|result| result.get("outputText"))
                        .and_then(Value::as_str)
                })
        });

        if content.is_some() || openai_tool_calls.is_some() || openai_finish_reason.is_some() {
            Ok(Some(ChatChunk {
                id: format!("bedrock-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: String::new(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta {
                        content: content.map(str::to_string),
                        tool_calls: openai_tool_calls,
                        ..Default::default()
                    },
                    finish_reason: openai_finish_reason,
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            }))
        } else {
            Ok(None)
        }
    }
}

impl Stream for BedrockStream {
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(parsed) = self.parse_buffered_chunk() {
                match parsed {
                    Ok(Some(chunk)) => return Poll::Ready(Some(Ok(chunk))),
                    Ok(None) => continue,
                    Err(e) => return Poll::Ready(Some(Err(e))),
                }
            }

            match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    self.buffer.extend_from_slice(&bytes);
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(None) => {
                    if self.buffer.is_empty() {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(Err(ProviderError::response_parsing(
                        "bedrock",
                        "incomplete Bedrock event stream frame",
                    ))));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
