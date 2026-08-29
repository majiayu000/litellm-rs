use serde_json::Value;

use super::SSETransformer;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::message::MessageRole;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice};

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
                                "assistant" => Some(MessageRole::Assistant),
                                "user" => Some(MessageRole::User),
                                "system" => Some(MessageRole::System),
                                "tool" => Some(MessageRole::Tool),
                                _ => None,
                            });

                    // Handle content - could be string or array (Claude reasoning)
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
                        audio: None,
                        annotations: None,
                    }
                } else {
                    ChatDelta {
                        role: None,
                        content: None,
                        thinking: None,
                        tool_calls: None,
                        function_call: None,
                        audio: None,
                        annotations: None,
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
