use std::collections::HashMap;

use serde_json::Value;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatMessage,
    message::{MessageContent, MessageRole},
    responses::{ChatChoice, ChatResponse, FinishReason},
    thinking::{AnthropicThinkingBlock, ThinkingContent},
    tools::{FunctionCall, ToolCall},
};

use super::{AnthropicClient, anthropic_parse_error, request_utils, usage};

fn parse_anthropic_stop_reason(reason: &str) -> FinishReason {
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

impl AnthropicClient {
    /// Response
    #[cfg(test)]
    pub(super) fn transform_chat_response(
        &self,
        response: Value,
    ) -> Result<ChatResponse, ProviderError> {
        self.transform_chat_response_with_tool_name_map(response, &HashMap::new())
    }

    pub(crate) fn transform_chat_response_with_tool_name_map(
        &self,
        response: Value,
        tool_name_map: &HashMap<String, String>,
    ) -> Result<ChatResponse, ProviderError> {
        // Extract basic information
        let id = response
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let model = response
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Handle content
        let content = response
            .get("content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anthropic_parse_error("Missing or invalid content array"))?;

        let mut message_content = String::new();
        let mut thinking_parts = Vec::new();
        let mut thinking_signature = None;
        let mut has_redacted_thinking = false;
        let mut anthropic_thinking_blocks = Vec::new();
        let mut tool_calls = Vec::new();

        for item in content {
            match item.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        message_content.push_str(text);
                    }
                }
                Some("tool_use") => {
                    if let (Some(id), Some(name), Some(input)) = (
                        item.get("id").and_then(|v| v.as_str()),
                        item.get("name").and_then(|v| v.as_str()),
                        item.get("input"),
                    ) {
                        tool_calls.push(ToolCall {
                            id: id.to_string(),
                            tool_type: "function".to_string(),
                            function: FunctionCall {
                                name: request_utils::restore_tool_name(name, tool_name_map),
                                arguments: input.to_string(),
                            },
                        });
                    }
                }
                Some("thinking") => {
                    let thinking = item
                        .get("thinking")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| anthropic_parse_error("Thinking block missing thinking"))?;
                    let signature = item
                        .get("signature")
                        .and_then(|value| value.as_str())
                        .map(str::to_string);
                    thinking_parts.push(thinking.to_string());
                    if signature.is_some() {
                        thinking_signature = signature.clone();
                    }
                    anthropic_thinking_blocks.push(AnthropicThinkingBlock::Thinking {
                        thinking: thinking.to_string(),
                        signature,
                    });
                }
                Some("redacted_thinking") => {
                    let data = item
                        .get("data")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| {
                            anthropic_parse_error("Redacted thinking block missing data")
                        })?;
                    has_redacted_thinking = true;
                    anthropic_thinking_blocks.push(AnthropicThinkingBlock::RedactedThinking {
                        data: data.to_string(),
                    });
                }
                Some("refusal") => {
                    if let Some(refusal) = item.get("refusal").and_then(|r| r.as_str()) {
                        message_content.push_str(refusal);
                    }
                }
                _ => {}
            }
        }

        let thinking = if !tool_calls.is_empty() && !anthropic_thinking_blocks.is_empty() {
            Some(ThinkingContent::AnthropicBlocks {
                blocks: anthropic_thinking_blocks,
            })
        } else if !thinking_parts.is_empty() {
            Some(ThinkingContent::Text {
                text: thinking_parts.join(""),
                signature: thinking_signature,
            })
        } else if has_redacted_thinking {
            Some(ThinkingContent::Redacted { token_count: None })
        } else {
            None
        };

        // Build message
        let message = ChatMessage {
            role: MessageRole::Assistant,
            content: if message_content.is_empty() {
                None
            } else {
                Some(MessageContent::Text(message_content))
            },
            thinking,
            audio: None,
            name: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            tool_call_id: None,
            function_call: None,
        };

        // Build choice
        let choice = ChatChoice {
            index: 0,
            message,
            finish_reason: response
                .get("stop_reason")
                .and_then(|r| r.as_str())
                .map(parse_anthropic_stop_reason),
            logprobs: None,
        };

        let usage = response.get("usage").map(usage::build_usage);

        Ok(ChatResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model,
            choices: vec![choice],
            usage,
            system_fingerprint: None,
        })
    }
}
