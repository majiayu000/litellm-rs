use std::collections::HashMap;

use serde_json::Value;

use crate::core::providers::ChatContinuationResponse;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::anthropic_continuation::{
    AnthropicRedactedData, AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    ChatMessageExtensions,
};
use crate::core::types::{
    chat::ChatMessage,
    message::{MessageContent, MessageRole},
    responses::{ChatChoice, ChatResponse, FinishReason},
    thinking::ThinkingContent,
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
        let continuation = self
            .transform_chat_response_with_continuation(response, tool_name_map)?
            .into_parts();
        let (mut response, extensions) = continuation;
        // Preserve the historical single-signature field only for callers of
        // the legacy response path. The typed continuation wrapper keeps the
        // opaque signature solely in its secret-safe sidecar.
        for (choice, extension) in response.choices.iter_mut().zip(&extensions) {
            let Some(ThinkingContent::Text { signature, .. }) = &mut choice.message.thinking else {
                continue;
            };
            *signature = extension.anthropic_thinking().and_then(|thinking| {
                thinking
                    .blocks()
                    .iter()
                    .rev()
                    .find_map(|block| match block {
                        AnthropicThinkingBlock::Thinking { signature, .. } => {
                            Some(signature.expose().to_string())
                        }
                        AnthropicThinkingBlock::RedactedThinking { .. } => None,
                    })
            });
        }
        Ok(response)
    }

    pub(crate) fn transform_chat_response_with_continuation(
        &self,
        response: Value,
        tool_name_map: &HashMap<String, String>,
    ) -> Result<ChatContinuationResponse, ProviderError> {
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
        let mut continuation_blocks = Vec::new();
        let mut tool_calls = Vec::new();

        for (block_index, item) in content.iter().enumerate() {
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
                        .ok_or_else(|| {
                            anthropic_parse_error(format!(
                                "choice 0 block {block_index} thinking text is missing"
                            ))
                        })?;
                    let signature = item
                        .get("signature")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| {
                            anthropic_parse_error(format!(
                                "choice 0 block {block_index} thinking signature is missing"
                            ))
                        })?;
                    continuation_blocks.push(AnthropicThinkingBlock::Thinking {
                        thinking: thinking.to_string(),
                        signature: AnthropicSignature::try_from(signature).map_err(|_| {
                            anthropic_parse_error(format!(
                                "choice 0 block {block_index} thinking signature is empty"
                            ))
                        })?,
                    });
                }
                Some("redacted_thinking") => {
                    let data = item
                        .get("data")
                        .and_then(|value| value.as_str())
                        .ok_or_else(|| {
                            anthropic_parse_error(format!(
                                "choice 0 block {block_index} redacted thinking data is missing"
                            ))
                        })?;
                    continuation_blocks.push(AnthropicThinkingBlock::RedactedThinking {
                        data: AnthropicRedactedData::try_from(data).map_err(|_| {
                            anthropic_parse_error(format!(
                                "choice 0 block {block_index} redacted thinking data is empty"
                            ))
                        })?,
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

        let continuation = AnthropicThinkingContent::new(continuation_blocks);
        let thinking = if let Some(text) = continuation.as_text() {
            Some(ThinkingContent::Text {
                text: text.into_owned(),
                signature: None,
            })
        } else if continuation.has_redacted_block() {
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

        let response = ChatResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model,
            choices: vec![choice],
            usage,
            system_fingerprint: None,
        };
        let extension = if continuation.blocks().is_empty() {
            ChatMessageExtensions::new()
        } else {
            ChatMessageExtensions::new().with_anthropic_thinking(continuation)
        };
        ChatContinuationResponse::new(response, vec![extension])
    }
}
