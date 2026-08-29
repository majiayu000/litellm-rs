//! Streaming Module for Bedrock
//!
//! Handles AWS Event Stream parsing and streaming responses

mod event_stream;

use crate::core::providers::bedrock::model_config::{BedrockApiType, BedrockModelFamily};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::ChatChunk;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::Value;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use event_stream::{EventStreamHeader, EventStreamMessage, HeaderValue};

/// Bedrock streaming response
pub struct BedrockStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, ProviderError>> + Send>>,
    buffer: Vec<u8>,
    model_family: BedrockModelFamily,
    api_type: BedrockApiType,
    completion_id: String,
    request_model_id: String,
    created: i64,
}

impl BedrockStream {
    /// Create a new Bedrock stream
    pub fn new(
        stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
        model_family: BedrockModelFamily,
        api_type: BedrockApiType,
        request_model_id: impl Into<String>,
    ) -> Self {
        let mapped_stream = stream
            .map(|result| result.map_err(|e| ProviderError::network("bedrock", e.to_string())));

        Self {
            inner: Box::pin(mapped_stream),
            buffer: Vec::new(),
            model_family,
            api_type,
            completion_id: format!("bedrock-{}", uuid::Uuid::new_v4()),
            request_model_id: request_model_id.into(),
            created: chrono::Utc::now().timestamp(),
        }
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
        Self::take_event_message(&mut self.buffer).map(|message| {
            message.and_then(|message| {
                Self::check_stream_error(&message)?;
                self.parse_chunk(&message.payload)
            })
        })
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

    fn chunk(
        &self,
        delta: crate::core::types::responses::ChatDelta,
        finish_reason: Option<crate::core::types::responses::FinishReason>,
    ) -> ChatChunk {
        use crate::core::types::responses::ChatStreamChoice;

        ChatChunk {
            id: self.completion_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: self.created,
            model: self.request_model_id.clone(),
            choices: vec![ChatStreamChoice {
                index: 0,
                delta,
                finish_reason,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        }
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
        use crate::core::types::responses::ChatDelta;

        if let Some(tool_call) = Self::parse_converse_tool_start(value) {
            return Ok(Some(self.chunk(
                ChatDelta {
                    role: None,
                    content: None,
                    thinking: None,
                    tool_calls: Some(vec![tool_call]),
                    function_call: None,
                    audio: None,
                },
                None,
            )));
        }

        if let Some(tool_call) = Self::parse_converse_tool_input(value) {
            return Ok(Some(self.chunk(
                ChatDelta {
                    role: None,
                    content: None,
                    thinking: None,
                    tool_calls: Some(vec![tool_call]),
                    function_call: None,
                    audio: None,
                },
                None,
            )));
        }

        if let Some(content) = value
            .get("contentBlockDelta")
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
        {
            return Ok(Some(self.chunk(
                ChatDelta {
                    role: None,
                    content: Some(content.to_string()),
                    thinking: None,
                    tool_calls: None,
                    function_call: None,
                    audio: None,
                },
                None,
            )));
        }

        if let Some(message_stop) = value.get("messageStop") {
            let stop_reason = message_stop
                .get("stopReason")
                .or_else(|| value.get("stopReason"))
                .and_then(|reason| reason.as_str());
            let finish_reason = Self::parse_converse_finish_reason(stop_reason);

            return Ok(Some(self.chunk(
                ChatDelta {
                    role: None,
                    content: None,
                    thinking: None,
                    tool_calls: None,
                    function_call: None,
                    audio: None,
                },
                Some(finish_reason),
            )));
        }

        Ok(None)
    }

    /// Parse Claude streaming chunk
    fn parse_claude_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::ChatDelta;

        // Claude uses specific event types
        let event_type = value.get("type").and_then(|v| v.as_str());

        match event_type {
            Some("content_block_delta") => {
                let delta = value
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                Ok(Some(self.chunk(
                    ChatDelta {
                        role: None,
                        content: Some(delta.to_string()),
                        thinking: None,
                        tool_calls: None,
                        function_call: None,
                        audio: None,
                    },
                    None,
                )))
            }
            Some("message_stop") => Ok(Some(self.chunk(
                ChatDelta {
                    role: None,
                    content: None,
                    thinking: None,
                    tool_calls: None,
                    function_call: None,
                    audio: None,
                },
                Some(crate::core::types::responses::FinishReason::Stop),
            ))),
            _ => Ok(None),
        }
    }

    /// Parse Nova streaming chunk
    fn parse_nova_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::ChatDelta;

        if let Some(content) = value
            .get("contentBlockDelta")
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
        {
            Ok(Some(self.chunk(
                ChatDelta {
                    role: None,
                    content: Some(content.to_string()),
                    thinking: None,
                    tool_calls: None,
                    function_call: None,
                    audio: None,
                },
                None,
            )))
        } else {
            Ok(None)
        }
    }

    /// Parse Titan streaming chunk
    fn parse_titan_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::ChatDelta;

        if let Some(content) = value.get("outputText").and_then(|t| t.as_str()) {
            let finish_reason = if value.get("completionReason").is_some() {
                Some(crate::core::types::responses::FinishReason::Stop)
            } else {
                None
            };

            Ok(Some(self.chunk(
                ChatDelta {
                    role: None,
                    content: Some(content.to_string()),
                    thinking: None,
                    tool_calls: None,
                    function_call: None,
                    audio: None,
                },
                finish_reason,
            )))
        } else {
            Ok(None)
        }
    }

    /// Parse generic streaming chunk
    fn parse_generic_chunk(&self, value: &Value) -> Result<Option<ChatChunk>, ProviderError> {
        use crate::core::types::responses::ChatDelta;

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
            Ok(Some(self.chunk(
                ChatDelta {
                    role: None,
                    content: content.map(str::to_string),
                    thinking: None,
                    tool_calls: openai_tool_calls,
                    function_call: None,
                    audio: None,
                },
                openai_finish_reason,
            )))
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
