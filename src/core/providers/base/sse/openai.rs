use serde_json::Value;

use super::SSETransformer;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice};
use crate::core::types::thinking::ThinkingDelta;

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
            let reasoning = delta
                .get("reasoning_content")
                .and_then(Value::as_str)
                .filter(|reasoning| !reasoning.is_empty())
                .or_else(|| {
                    delta
                        .get("reasoning")
                        .and_then(Value::as_str)
                        .filter(|reasoning| !reasoning.is_empty())
                });
            if let Some(reasoning) = reasoning {
                delta_obj.thinking = Some(ThinkingDelta {
                    content: Some(reasoning.to_string()),
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
