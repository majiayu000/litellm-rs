use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Mutex;

use super::SSETransformer;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice, Usage};
use crate::core::types::thinking::ThinkingDelta;

#[cfg(test)]
#[path = "openai_stream_tests.rs"]
mod stream_tests;

const INVALIDATED_USAGE_STREAM_TYPE: &str = "chat.completion.usage_invalidated";

/// A later explicit declaration invalidated previously reported token counts.
pub(crate) fn invalidates_stream_usage(error: &ProviderError) -> bool {
    matches!(
        error,
        ProviderError::Streaming { stream_type, .. }
            if stream_type == INVALIDATED_USAGE_STREAM_TYPE
    )
}

/// OpenAI-compatible SSE Transformer (can be reused by many providers)
#[derive(Debug)]
pub struct OpenAICompatibleTransformer {
    provider: &'static str,
    /// Only the streaming entry point advances lifecycle state; stand-alone
    /// chunk conversion remains usable without constructing an entire stream.
    choices: Mutex<BTreeMap<u32, bool>>,
    error_usage: Mutex<Option<ChatChunk>>,
    valid_usage_seen: Mutex<bool>,
}

impl Clone for OpenAICompatibleTransformer {
    fn clone(&self) -> Self {
        // A transformer clone belongs to a new request, never to the stream
        // whose completion state is currently being observed.
        Self::new(self.provider)
    }
}

impl OpenAICompatibleTransformer {
    pub fn new(provider: &'static str) -> Self {
        Self {
            provider,
            choices: Mutex::new(BTreeMap::new()),
            error_usage: Mutex::new(None),
            valid_usage_seen: Mutex::new(false),
        }
    }

    fn lifecycle_error(&self, index: Option<u32>, message: &str) -> ProviderError {
        ProviderError::streaming_error(
            self.provider,
            "chat.completion",
            index.map(u64::from),
            None,
            message,
        )
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

        // Usage must satisfy the raw protocol invariants before it can release
        // an admission estimate. Keep valid detail fields during deserialization;
        // invalid usage must not discard the accompanying output.
        let usage = match json_value.get("usage") {
            None | Some(Value::Null) => None,
            Some(v) if crate::core::providers::shared::strict_openai_chat_usage(v).is_none() => {
                tracing::warn!("{}: ignoring untrusted usage in SSE chunk", self.provider);
                None
            }
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

    fn transform_stream_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let mut valid_usage_seen = self.valid_usage_seen.lock().map_err(|_| {
            self.lifecycle_error(None, "OpenAI-compatible usage state lock poisoned")
        })?;
        // Inspect an explicit declaration before choice parsing can reject the
        // frame. A structural error must not hide a simultaneous usage retraction.
        if *valid_usage_seen
            && let Ok(value) = serde_json::from_str::<Value>(data)
            && let Some(usage) = value.get("usage")
            && !usage.is_null()
            && (crate::core::providers::shared::strict_openai_chat_usage(usage).is_none()
                || serde_json::from_value::<Usage>(usage.clone()).is_err())
        {
            return Err(ProviderError::streaming_error(
                self.provider,
                INVALIDATED_USAGE_STREAM_TYPE,
                None,
                None,
                "explicit invalid usage followed valid stream usage",
            ));
        }
        let chunk = self.transform_chunk(data)?;
        if let Some(chunk) = &chunk {
            if chunk.usage.is_some() {
                *valid_usage_seen = true;
            }
            drop(valid_usage_seen);
            let mut choices = self.choices.lock().map_err(|_| {
                self.lifecycle_error(None, "OpenAI-compatible stream state lock poisoned")
            })?;
            for choice in &chunk.choices {
                if choices.get(&choice.index) == Some(&true) {
                    // Usage is independently validated by transform_chunk.
                    // Preserve it without accepting content from an event that
                    // violates the choice lifecycle.
                    if chunk.usage.is_some() {
                        let mut usage_only = chunk.clone();
                        usage_only.choices.clear();
                        *self.error_usage.lock().map_err(|_| {
                            self.lifecycle_error(
                                None,
                                "OpenAI-compatible usage state lock poisoned",
                            )
                        })? = Some(usage_only);
                    }
                    return Err(self.lifecycle_error(
                        Some(choice.index),
                        "received a choice after its terminal finish_reason",
                    ));
                }
                choices.insert(choice.index, choice.finish_reason.is_some());
            }
        }
        Ok(chunk)
    }

    fn take_stream_error_chunk(&self) -> Option<ChatChunk> {
        self.error_usage.lock().ok()?.take()
    }

    fn finish_stream(&self) -> Result<Option<ChatChunk>, ProviderError> {
        let choices = self.choices.lock().map_err(|_| {
            self.lifecycle_error(None, "OpenAI-compatible stream state lock poisoned")
        })?;
        if choices.is_empty() {
            return Err(self.lifecycle_error(None, "stream ended without a completed choice"));
        }
        if let Some((&index, _)) = choices.iter().find(|(_, finished)| !**finished) {
            return Err(self.lifecycle_error(
                Some(index),
                "stream ended before the choice's terminal finish_reason",
            ));
        }
        // Some compatible providers end the HTTP body after final choices
        // without a [DONE] sentinel. Every observed choice must still finish.
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_cached_usage_must_fit_prompt() {
        for (details, valid) in [
            (serde_json::json!({"cached_tokens":100}), false),
            (
                serde_json::json!({"cached_tokens":1,"cache_read_tokens":100}),
                false,
            ),
            (serde_json::json!({"cache_creation_tokens":100}), false),
            (
                serde_json::json!({"cached_tokens":2,"cache_creation_tokens":2}),
                false,
            ),
            (
                serde_json::json!({"cached_tokens":3,"cache_read_tokens":1,"cache_creation_tokens":2,"audio_tokens":1}),
                true,
            ),
        ] {
            let transformer = OpenAICompatibleTransformer::new("test");
            let wire = serde_json::json!({
                "id":"cache-bound", "created":1, "model":"test",
                "choices":[{"index":0,"delta":{"content":"kept"}}],
                "usage":{"prompt_tokens":3,"completion_tokens":0,"total_tokens":3,
                    "prompt_tokens_details":details}
            });
            let chunk = transformer
                .transform_chunk(&wire.to_string())
                .unwrap()
                .unwrap();
            assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("kept"));
            if valid {
                let preserved = chunk.usage.unwrap().prompt_tokens_details.unwrap();
                assert_eq!(serde_json::to_value(preserved).unwrap(), details);
            } else {
                assert!(chunk.usage.is_none(), "{details}");
            }
        }
    }

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
