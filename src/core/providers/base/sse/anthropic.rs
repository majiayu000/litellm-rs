use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;
use tracing::warn;

use super::SSETransformer;
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
}

impl Clone for AnthropicTransformer {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            tool_name_map: self.tool_name_map.clone(),
            message_id: Mutex::new(None),
            thinking_state: Mutex::new(AnthropicThinkingStreamState::default()),
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
        }
    }

    pub fn with_tool_name_map(mut self, tool_name_map: HashMap<String, String>) -> Self {
        self.tool_name_map = tool_name_map;
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
                let index = Self::required_index(&json, "content block start")?;
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
                let index = Self::required_index(&json, "content block delta")?;
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
                        delta_json.get("citation").ok_or_else(|| {
                            ProviderError::response_parsing(
                                "anthropic",
                                "No citation in Anthropic citations_delta",
                            )
                        })?;
                        Ok(None)
                    }
                    "thinking_delta" => {
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
                    self.with_thinking_state(AnthropicThinkingStreamState::begin_pending_stop)?;
                } else {
                    self.with_thinking_state(|state| {
                        state.ensure_message_open(0, "message_delta")
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
                self.with_thinking_state(AnthropicThinkingStreamState::end_message)?;
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
                let index = Self::required_index(&json, "content block stop")?;
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
        self.with_thinking_state(|state| state.ensure_stream_complete())?;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_from_event(transformer: &AnthropicTransformer, event: Value) -> ChatChunk {
        match transformer.transform_chunk(&event.to_string()) {
            Ok(Some(chunk)) => chunk,
            Ok(None) => panic!("expected Anthropic SSE event to produce a chunk"),
            Err(error) => panic!("unexpected Anthropic SSE error: {error}"),
        }
    }

    #[test]
    fn test_message_delta_extracts_cache_tokens() {
        let transformer = AnthropicTransformer::new("claude-3-5-sonnet");
        let chunk = chunk_from_event(
            &transformer,
            serde_json::json!({
                "type":"message_delta", "delta":{"stop_reason":"end_turn"},
                "usage":{
                    "input_tokens":12, "output_tokens":50,
                    "cache_creation_input_tokens":1000,
                    "cache_read_input_tokens":2000
                }
            }),
        );
        let usage = chunk.usage.as_ref().expect("usage must be present");
        assert_eq!(usage.prompt_tokens, 12);
        assert_eq!(usage.completion_tokens, 50);
        let details = usage
            .prompt_tokens_details
            .as_ref()
            .expect("cache token details must be present");
        assert_eq!(details.cache_creation_tokens, Some(1000));
        assert_eq!(details.cache_read_tokens, Some(2000));
        assert_eq!(details.cached_tokens, Some(2000));
    }

    #[test]
    fn test_message_delta_no_cache_tokens_yields_none_details() {
        let transformer = AnthropicTransformer::new("claude-3-5-sonnet");
        let chunk = chunk_from_event(
            &transformer,
            serde_json::json!({
                "type":"message_delta", "delta":{"stop_reason":"end_turn"},
                "usage":{"input_tokens":12,"output_tokens":50}
            }),
        );
        assert!(
            chunk
                .usage
                .as_ref()
                .expect("usage must be present")
                .prompt_tokens_details
                .is_none()
        );
    }

    #[test]
    fn test_chunks_after_message_start_keep_message_id() {
        let transformer = AnthropicTransformer::new("claude-3-5-sonnet");
        assert_eq!(
            chunk_from_event(
                &transformer,
                serde_json::json!({"type":"message_start","message":{"id":"msg_123"}})
            )
            .id,
            "msg_123"
        );
        transformer
            .transform_chunk(
                &serde_json::json!({
                    "type":"content_block_start", "index":0,
                    "content_block":{"type":"text","text":""}
                })
                .to_string(),
            )
            .expect("text start must be valid");
        assert_eq!(
            chunk_from_event(
                &transformer,
                serde_json::json!({
                    "type":"content_block_delta", "index":0,
                    "delta":{"type":"text_delta","text":"hello"}
                })
            )
            .id,
            "msg_123"
        );
        transformer
            .transform_chunk(
                &serde_json::json!({"type":"content_block_stop","index":0}).to_string(),
            )
            .expect("text stop must be valid");
        assert_eq!(
            chunk_from_event(
                &transformer,
                serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}})
            )
            .id,
            "msg_123"
        );
        assert_eq!(
            chunk_from_event(&transformer, serde_json::json!({"type":"message_stop"})).id,
            "msg_123"
        );
    }

    #[test]
    fn test_cloned_transformers_keep_independent_message_ids() {
        let stream_a = AnthropicTransformer::new("claude-3-5-sonnet");
        let stream_b = stream_a.clone();
        assert_eq!(
            chunk_from_event(
                &stream_a,
                serde_json::json!({"type":"message_start","message":{"id":"msg_a"}})
            )
            .id,
            "msg_a"
        );
        assert_eq!(
            chunk_from_event(
                &stream_b,
                serde_json::json!({"type":"message_start","message":{"id":"msg_b"}})
            )
            .id,
            "msg_b"
        );
    }

    #[test]
    fn issue_761_stream_restores_original_tool_names() -> Result<(), crate::ProviderError> {
        let transformer = AnthropicTransformer::new("claude-3-5-sonnet").with_tool_name_map(
            HashMap::from([("weather_lookup".to_string(), "weather.lookup".to_string())]),
        );
        let chunk = transformer.transform_chunk(
            &serde_json::json!({
                "type":"content_block_start", "index":0,
                "content_block":{
                    "type":"tool_use", "id":"toolu_123",
                    "name":"weather_lookup", "input":{}
                }
            })
            .to_string(),
        )?;
        let name = chunk
            .as_ref()
            .and_then(|chunk| chunk.choices.first())
            .and_then(|choice| choice.delta.tool_calls.as_ref())
            .and_then(|calls| calls.first())
            .and_then(|call| call.function.as_ref())
            .and_then(|function| function.name.as_deref());
        assert_eq!(name, Some("weather.lookup"));
        Ok(())
    }
}

#[cfg(test)]
#[path = "anthropic_lifecycle_tests.rs"]
mod lifecycle_tests;
