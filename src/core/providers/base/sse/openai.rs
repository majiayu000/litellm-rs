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

            // Prefer the upstream choice index: with n>1 each chunk carries a
            // single choice with its real index, so the array position is wrong.
            let index = choice
                .get("index")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(index as u32);

            let logprobs = match choice.get("logprobs") {
                None | Some(Value::Null) => None,
                Some(v) => match serde_json::from_value(v.clone()) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        tracing::error!(
                            "{}: failed to parse 'logprobs' in SSE chunk: {} (raw: {})",
                            self.provider,
                            e,
                            crate::utils::truncate_string(&v.to_string(), 200)
                        );
                        None
                    }
                },
            };

            stream_choices.push(ChatStreamChoice {
                index,
                delta: delta_obj,
                finish_reason,
                logprobs,
            });
        }

        // Parse usage (optional). A parse failure must not drop the chunk, but
        // usage feeds billing, so it must not be silently discarded either.
        let usage = match json_value.get("usage") {
            None | Some(Value::Null) => None,
            Some(v) => match serde_json::from_value(v.clone()) {
                Ok(parsed) => Some(parsed),
                Err(e) => {
                    tracing::error!(
                        "{}: failed to parse 'usage' in SSE chunk: {} (raw: {})",
                        self.provider,
                        e,
                        crate::utils::truncate_string(&v.to_string(), 200)
                    );
                    None
                }
            },
        };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preserves_upstream_choice_index() {
        let transformer = OpenAICompatibleTransformer::new("test");
        // n>1: upstream sends one choice per chunk carrying its real index.
        let chunk = r#"{
            "id": "id",
            "object": "chat.completion.chunk",
            "created": 1,
            "model": "gpt-4",
            "choices": [{"index": 2, "delta": {"content": "x"}, "finish_reason": null}]
        }"#;
        let result = transformer.transform_chunk(chunk).unwrap().unwrap();
        assert_eq!(result.choices[0].index, 2);
    }

    #[test]
    fn test_missing_index_falls_back_to_position() {
        let transformer = OpenAICompatibleTransformer::new("test");
        let chunk = r#"{
            "id": "id",
            "object": "chat.completion.chunk",
            "created": 1,
            "model": "gpt-4",
            "choices": [{"delta": {"content": "x"}, "finish_reason": null}]
        }"#;
        let result = transformer.transform_chunk(chunk).unwrap().unwrap();
        assert_eq!(result.choices[0].index, 0);
    }

    #[test]
    fn test_malformed_logprobs_does_not_drop_usage() {
        let transformer = OpenAICompatibleTransformer::new("test");
        // logprobs is the wrong shape; it must not fail the chunk or drop usage.
        let chunk = r#"{
            "id": "id",
            "object": "chat.completion.chunk",
            "created": 1,
            "model": "gpt-4",
            "choices": [{"index": 0, "delta": {"content": "x"}, "logprobs": 42}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 7, "total_tokens": 12}
        }"#;
        let result = transformer.transform_chunk(chunk).unwrap().unwrap();
        assert!(result.choices[0].logprobs.is_none());
        let usage = result
            .usage
            .expect("usage must survive a logprobs parse error");
        assert_eq!(usage.total_tokens, 12);
    }
}
