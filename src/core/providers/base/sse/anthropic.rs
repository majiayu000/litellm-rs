use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;
use tracing::warn;

use super::SSETransformer;
use crate::core::providers::anthropic::http_annotations::HttpAnnotationSender;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::message::MessageRole;
use crate::core::types::responses::{
    ChatChunk, ChatDelta, ChatStreamChoice, FinishReason, FunctionCallDelta, PromptTokensDetails,
    ToolCallDelta, Usage,
};
use crate::core::types::thinking::ThinkingDelta;

#[path = "anthropic_state.rs"]
mod state;

use state::{ActiveContentBlock, AnthropicThinkingStreamState, DeltaDisposition};

/// Anthropic SSE Transformer
///
/// Handles Anthropic's event-based SSE format with message_start, content_block_delta,
/// message_delta, and message_stop events.
#[derive(Debug)]
pub struct AnthropicTransformer {
    model: String,
    tool_name_map: HashMap<String, String>,
    message_id: Mutex<Option<String>>,
    thinking_state: Mutex<AnthropicThinkingStreamState>,
    http_annotation_sender: Option<HttpAnnotationSender>,
}

impl Clone for AnthropicTransformer {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            tool_name_map: self.tool_name_map.clone(),
            message_id: Mutex::new(None),
            thinking_state: Mutex::new(AnthropicThinkingStreamState::default()),
            http_annotation_sender: self.http_annotation_sender.clone(),
        }
    }
}

impl AnthropicTransformer {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            tool_name_map: HashMap::new(),
            message_id: Mutex::new(None),
            thinking_state: Mutex::new(AnthropicThinkingStreamState::default()),
            http_annotation_sender: None,
        }
    }

    pub fn with_tool_name_map(mut self, tool_name_map: HashMap<String, String>) -> Self {
        self.tool_name_map = tool_name_map;
        self
    }

    pub(crate) fn with_http_annotation_sender(
        mut self,
        sender: Option<HttpAnnotationSender>,
    ) -> Self {
        self.http_annotation_sender = sender;
        self
    }

    fn restore_tool_name(&self, name: &str) -> String {
        self.tool_name_map
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    fn set_message_id(&self, message_id: String) {
        let mut guard = match self.message_id.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(message_id);
    }

    fn current_message_id(&self) -> String {
        let guard = match self.message_id.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "anthropic-stream".to_string())
    }

    fn clear_message_id(&self) {
        let mut guard = match self.message_id.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = None;
    }

    fn with_thinking_state<T>(
        &self,
        operation: impl FnOnce(&mut AnthropicThinkingStreamState) -> Result<T, ProviderError>,
    ) -> Result<T, ProviderError> {
        let mut state = match self.thinking_state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        operation(&mut state)
    }

    fn required_index(json: &Value, event: &str) -> Result<u32, ProviderError> {
        let index = json.get("index").and_then(Value::as_u64).ok_or_else(|| {
            ProviderError::response_parsing(
                "anthropic",
                format!("No content index in Anthropic {event}"),
            )
        })?;
        u32::try_from(index).map_err(|_| {
            ProviderError::response_parsing(
                "anthropic",
                format!("Anthropic {event} content index {index} exceeds u32"),
            )
        })
    }

    fn parse_anthropic_finish_reason(reason: &str) -> FinishReason {
        match reason {
            "end_turn" => FinishReason::Stop,
            "max_tokens" => FinishReason::Length,
            "tool_use" => FinishReason::ToolCalls,
            "stop_sequence" => FinishReason::StopSequence,
            "refusal" => FinishReason::Refusal,
            "pause_turn" => FinishReason::PauseTurn,
            _ => FinishReason::Stop,
        }
    }

    fn empty_delta() -> ChatDelta {
        ChatDelta {
            role: None,
            content: None,
            thinking: None,
            tool_calls: None,
            function_call: None,
            audio: None,
        }
    }

    fn chunk_with_choice(
        &self,
        created: i64,
        delta: ChatDelta,
        finish_reason: Option<FinishReason>,
        usage: Option<Usage>,
    ) -> ChatChunk {
        ChatChunk {
            id: self.current_message_id(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: self.model.clone(),
            choices: vec![ChatStreamChoice {
                index: 0,
                delta,
                finish_reason,
                logprobs: None,
            }],
            usage,
            system_fingerprint: None,
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
                self.with_thinking_state(AnthropicThinkingStreamState::begin_message)?;
                let message_id = json
                    .get("message")
                    .and_then(|m| m.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("anthropic-stream")
                    .to_string();
                self.set_message_id(message_id.clone());

                Ok(Some(ChatChunk {
                    id: message_id,
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: self.model.clone(),
                    choices: vec![ChatStreamChoice {
                        index: 0,
                        delta: ChatDelta {
                            role: Some(MessageRole::Assistant),
                            content: None,
                            thinking: None,
                            tool_calls: None,
                            function_call: None,
                            audio: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    usage: None,
                    system_fingerprint: None,
                }))
            }
            "content_block_start" => {
                let index = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let content_block = json.get("content_block").ok_or_else(|| {
                    ProviderError::response_parsing(
                        "anthropic",
                        "No content_block in content_block_start".to_string(),
                    )
                })?;
                let block_type = content_block
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                match block_type {
                    "tool_use" => {
                        self.with_thinking_state(|state| {
                            state.begin_content(index, ActiveContentBlock::ToolUse)
                        })?;
                        let id = content_block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        let name = content_block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(|name| self.restore_tool_name(name));
                        let arguments = content_block.get("input").and_then(|input| {
                            if input.is_null()
                                || input
                                    .as_object()
                                    .map(|object| object.is_empty())
                                    .unwrap_or(false)
                            {
                                None
                            } else {
                                Some(input.to_string())
                            }
                        });

                        let mut delta = Self::empty_delta();
                        delta.tool_calls = Some(vec![ToolCallDelta {
                            index,
                            id,
                            tool_type: Some("function".to_string()),
                            function: Some(FunctionCallDelta { name, arguments }),
                        }]);

                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    "thinking" => {
                        let index = Self::required_index(&json, "thinking block start")?;
                        let thinking = content_block
                            .get("thinking")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let signature = content_block
                            .get("signature")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        self.with_thinking_state(|state| {
                            state.begin_thinking(index, thinking, signature)
                        })?;
                        let mut delta = Self::empty_delta();
                        delta.thinking = Some(ThinkingDelta::start());
                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    "redacted_thinking" => {
                        let index = Self::required_index(&json, "redacted thinking block start")?;
                        let data = content_block
                            .get("data")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        self.with_thinking_state(|state| state.begin_redacted(index, data))?;
                        let mut delta = Self::empty_delta();
                        delta.thinking = Some(ThinkingDelta::start());
                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    "text" => {
                        self.with_thinking_state(|state| {
                            state.begin_content(index, ActiveContentBlock::Text)
                        })?;
                        Ok(None)
                    }
                    _ => {
                        self.with_thinking_state(|state| {
                            state.begin_content(index, ActiveContentBlock::Ignored)
                        })?;
                        warn!(
                            provider = "anthropic",
                            block_type, "Ignoring unknown Anthropic content block start"
                        );
                        Ok(None)
                    }
                }
            }
            "content_block_delta" => {
                let index = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let delta_json = json.get("delta").ok_or_else(|| {
                    ProviderError::response_parsing(
                        "anthropic",
                        "No delta in content_block_delta".to_string(),
                    )
                })?;
                let delta_type = delta_json
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let disposition =
                    self.with_thinking_state(|state| state.validate_delta_kind(index, delta_type))?;
                if disposition == DeltaDisposition::Ignore {
                    return Ok(None);
                }

                match delta_type {
                    "text_delta" => {
                        let text = delta_json
                            .get("text")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");
                        let mut delta = Self::empty_delta();
                        delta.content = Some(text.to_string());
                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    "input_json_delta" => {
                        let partial_json = delta_json
                            .get("partial_json")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let mut delta = Self::empty_delta();
                        delta.tool_calls = Some(vec![ToolCallDelta {
                            index,
                            id: None,
                            tool_type: Some("function".to_string()),
                            function: Some(FunctionCallDelta {
                                name: None,
                                arguments: Some(partial_json.to_string()),
                            }),
                        }]);
                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    "citations_delta" => {
                        let citation = delta_json.get("citation").cloned().ok_or_else(|| {
                            ProviderError::response_parsing(
                                "anthropic",
                                "No citation in Anthropic citations_delta",
                            )
                        })?;
                        let Some(sender) = &self.http_annotation_sender else {
                            return Ok(None);
                        };
                        sender.send(0, citation)?;
                        Ok(Some(ChatChunk::provider_annotation_marker(
                            self.current_message_id(),
                            created,
                            self.model.clone(),
                            0,
                        )))
                    }
                    "thinking_delta" => {
                        let index = Self::required_index(&json, "thinking delta")?;
                        let thinking = delta_json
                            .get("thinking")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        self.with_thinking_state(|state| state.append_thinking(index, thinking))?;
                        let mut delta = Self::empty_delta();
                        delta.thinking = Some(ThinkingDelta::new(thinking));
                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    "signature_delta" => {
                        let index = Self::required_index(&json, "signature delta")?;
                        let signature = delta_json
                            .get("signature")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        self.with_thinking_state(|state| state.append_signature(index, signature))?;
                        let mut delta = Self::empty_delta();
                        delta.thinking = Some(ThinkingDelta {
                            signature: Some(signature.to_string()),
                            ..Default::default()
                        });
                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    _ => {
                        warn!(
                            provider = "anthropic",
                            delta_type, "Ignoring unknown Anthropic content block delta"
                        );
                        Ok(None)
                    }
                }
            }
            "message_delta" => {
                let finish_reason = json
                    .get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(|r| r.as_str())
                    .map(Self::parse_anthropic_finish_reason);
                if finish_reason.is_some() {
                    self.with_thinking_state(|state| {
                        state.ensure_complete("message_delta stop_reason")
                    })?;
                }

                let usage = json.get("usage").map(|u| {
                    let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let output =
                        u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let cache_creation_tokens = u
                        .get("cache_creation_input_tokens")
                        .and_then(|v| v.as_u64())
                        .map(|t| t as u32);
                    let cache_read_tokens = u
                        .get("cache_read_input_tokens")
                        .and_then(|v| v.as_u64())
                        .map(|t| t as u32);
                    let prompt_tokens_details =
                        if cache_creation_tokens.is_some() || cache_read_tokens.is_some() {
                            Some(PromptTokensDetails {
                                cached_tokens: cache_read_tokens,
                                cache_creation_tokens,
                                cache_read_tokens,
                                audio_tokens: None,
                            })
                        } else {
                            None
                        };
                    Usage {
                        prompt_tokens: input,
                        completion_tokens: output,
                        total_tokens: input + output,
                        completion_tokens_details: None,
                        prompt_tokens_details,
                        thinking_usage: None,
                    }
                });

                Ok(Some(self.chunk_with_choice(
                    created,
                    Self::empty_delta(),
                    finish_reason,
                    usage,
                )))
            }
            "message_stop" => {
                self.with_thinking_state(|state| state.ensure_complete("message_stop"))?;
                let message_id = self.current_message_id();
                self.clear_message_id();
                Ok(Some(ChatChunk {
                    id: message_id,
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: self.model.clone(),
                    choices: vec![],
                    usage: None,
                    system_fingerprint: None,
                }))
            }
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
            "content_block_stop" => {
                let Some(index) = json
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|index| u32::try_from(index).ok())
                else {
                    return Ok(None);
                };
                if self.with_thinking_state(|state| state.complete(index))? {
                    let mut delta = Self::empty_delta();
                    delta.thinking = Some(ThinkingDelta::complete());
                    Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                } else {
                    Ok(None)
                }
            }
            "ping" => Ok(None),
            _ => {
                warn!(
                    provider = "anthropic",
                    event_type, "Ignoring unknown Anthropic SSE event type"
                );
                Ok(None)
            }
        }
    }

    fn finish_stream(&self) -> Result<Option<ChatChunk>, ProviderError> {
        self.with_thinking_state(|state| state.ensure_complete("stream termination"))?;
        Ok(None)
    }
}

#[cfg(test)]
#[path = "anthropic_lifecycle_tests.rs"]
mod lifecycle_tests;
