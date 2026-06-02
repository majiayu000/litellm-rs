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
            "stop_sequence" => FinishReason::StopSequence,
            "refusal" => FinishReason::Refusal,
            "pause_turn" => FinishReason::PauseTurn,
            _ => FinishReason::Stop,
        }
    }

    fn empty_delta() -> ChatDelta {
        ChatDelta::default()
    }

    fn chunk_with_choice(
        &self,
        created: i64,
        delta: ChatDelta,
        finish_reason: Option<FinishReason>,
        usage: Option<Usage>,
    ) -> ChatChunk {
        ChatChunk {
            id: String::new(),
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
                            role: Some(MessageRole::Assistant),
                            ..Default::default()
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
                        let id = content_block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        let name = content_block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
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
                    "thinking" | "redacted_thinking" => {
                        let mut delta = Self::empty_delta();
                        delta.thinking = Some(ThinkingDelta::start());
                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    "text" => Ok(None),
                    _ => {
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
                    "thinking_delta" => {
                        let thinking = delta_json
                            .get("thinking")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let mut delta = Self::empty_delta();
                        delta.thinking = Some(ThinkingDelta::new(thinking));
                        Ok(Some(self.chunk_with_choice(created, delta, None, None)))
                    }
                    "signature_delta" => {
                        let signature = delta_json
                            .get("signature")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
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
            "content_block_stop" | "ping" => Ok(None),
            _ => {
                warn!(
                    provider = "anthropic",
                    event_type, "Ignoring unknown Anthropic SSE event type"
                );
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_delta_extracts_cache_tokens() {
        let t = AnthropicTransformer::new("claude-3-5-sonnet");
        let event = serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {
                "input_tokens": 12,
                "output_tokens": 50,
                "cache_creation_input_tokens": 1000,
                "cache_read_input_tokens": 2000
            }
        });
        let chunk = t.transform_chunk(&event.to_string()).unwrap().unwrap();
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
        let t = AnthropicTransformer::new("claude-3-5-sonnet");
        let event = serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"input_tokens": 12, "output_tokens": 50}
        });
        let chunk = t.transform_chunk(&event.to_string()).unwrap().unwrap();
        let usage = chunk.usage.as_ref().unwrap();
        assert!(usage.prompt_tokens_details.is_none());
    }
}
