use serde_json::Value;

use super::SSETransformer;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::message::MessageRole;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice, FinishReason, Usage};

/// Gemini SSE Transformer
///
/// Handles Gemini's streaming format with candidates/parts structure.
#[derive(Debug, Clone)]
pub struct GeminiTransformer {
    model: String,
    chunk_id: String,
}

impl GeminiTransformer {
    pub fn new(model: impl Into<String>) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Self {
            model: model.into(),
            chunk_id: format!("gemini-stream-{}", nanos),
        }
    }
}

impl SSETransformer for GeminiTransformer {
    fn provider_name(&self) -> &'static str {
        "gemini"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let json: Value = serde_json::from_str(data).map_err(|e| {
            ProviderError::response_parsing("gemini", format!("Failed to parse Gemini SSE: {}", e))
        })?;

        // Error response
        if let Some(error) = json.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown Gemini error");
            return Err(ProviderError::api_error(
                "gemini",
                error.get("code").and_then(|c| c.as_u64()).unwrap_or(500) as u16,
                msg.to_string(),
            ));
        }

        let empty_arr = vec![];
        let candidates = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .unwrap_or(&empty_arr);

        if candidates.is_empty() {
            // Usage-only chunk
            let usage = json.get("usageMetadata").map(|u| {
                let prompt = u
                    .get("promptTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let completion = u
                    .get("candidatesTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let total = u
                    .get("totalTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                Usage {
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    total_tokens: total,
                    prompt_tokens_details: None,
                    completion_tokens_details: None,
                    thinking_usage: None,
                }
            });
            if usage.is_some() {
                return Ok(Some(ChatChunk {
                    id: self.chunk_id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp(),
                    model: self.model.clone(),
                    choices: vec![],
                    usage,
                    system_fingerprint: None,
                }));
            }
            return Ok(None);
        }

        let mut choices = Vec::new();
        for (index, candidate) in candidates.iter().enumerate() {
            let empty_parts = vec![];
            let parts = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
                .unwrap_or(&empty_parts);

            let mut text_parts = Vec::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text);
                }
            }
            let delta_content = text_parts.join("");

            let finish_reason = candidate
                .get("finishReason")
                .and_then(|r| r.as_str())
                .map(|r| match r {
                    "STOP" => FinishReason::Stop,
                    "MAX_TOKENS" => FinishReason::Length,
                    "SAFETY" | "RECITATION" => FinishReason::ContentFilter,
                    _ => FinishReason::Stop,
                });

            choices.push(ChatStreamChoice {
                index: index as u32,
                delta: ChatDelta {
                    role: if !delta_content.is_empty() || finish_reason.is_some() {
                        Some(MessageRole::Assistant)
                    } else {
                        None
                    },
                    content: if delta_content.is_empty() {
                        None
                    } else {
                        Some(delta_content)
                    },
                    ..Default::default()
                },
                finish_reason,
                logprobs: None,
            });
        }

        let usage = json.get("usageMetadata").map(|u| {
            let prompt = u
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let completion = u
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let total = u
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: total,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }
        });

        if choices.is_empty() && usage.is_none() {
            return Ok(None);
        }

        Ok(Some(ChatChunk {
            id: self.chunk_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: self.model.clone(),
            choices,
            usage,
            system_fingerprint: None,
        }))
    }
}
