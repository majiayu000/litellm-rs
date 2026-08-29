use serde_json::Value;

use super::SSETransformer;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice, FinishReason, Usage};

/// Cohere SSE Transformer
///
/// Handles Cohere's streaming format with v1/v2 API version support.
#[derive(Debug, Clone)]
pub struct CohereTransformer {
    model: String,
    response_id: String,
    /// true = v2, false = v1
    use_v2: bool,
}

impl CohereTransformer {
    pub fn new(model: impl Into<String>, use_v2: bool) -> Self {
        Self {
            model: model.into(),
            response_id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            use_v2,
        }
    }

    fn parse_cohere_finish_reason(reason: &str) -> FinishReason {
        match reason.to_lowercase().as_str() {
            "stop" | "complete" | "end_turn" => FinishReason::Stop,
            "length" | "max_tokens" => FinishReason::Length,
            "tool_calls" | "tool_use" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            _ => FinishReason::Stop,
        }
    }
}

impl SSETransformer for CohereTransformer {
    fn provider_name(&self) -> &'static str {
        "cohere"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let json: Value = serde_json::from_str(data).map_err(|e| {
            ProviderError::response_parsing("cohere", format!("Failed to parse Cohere SSE: {}", e))
        })?;

        let event_type = json
            .get("type")
            .or_else(|| json.get("event"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let created = chrono::Utc::now().timestamp();

        if self.use_v2 {
            match event_type {
                "content-delta" => {
                    let text = json
                        .get("delta")
                        .and_then(|d| d.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|c| {
                            c.get("text")
                                .and_then(|t| t.as_str())
                                .or_else(|| c.as_str())
                        })
                        .unwrap_or("");

                    if text.is_empty() {
                        return Ok(None);
                    }

                    Ok(Some(ChatChunk {
                        id: self.response_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: self.model.clone(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: None,
                                content: Some(text.to_string()),
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                                audio: None,
                                annotations: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                        system_fingerprint: None,
                    }))
                }
                "message-end" => {
                    let data_field = json.get("data").or(json.get("delta"));
                    let finish_reason = data_field
                        .and_then(|d| d.get("delta"))
                        .and_then(|d| d.get("finish_reason"))
                        .and_then(|f| f.as_str())
                        .unwrap_or("stop");

                    let usage = data_field
                        .and_then(|d| d.get("delta"))
                        .and_then(|d| d.get("usage"))
                        .and_then(|u| u.get("tokens"))
                        .map(|tokens| {
                            let prompt = tokens
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            let completion = tokens
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            Usage {
                                prompt_tokens: prompt,
                                completion_tokens: completion,
                                total_tokens: prompt + completion,
                                prompt_tokens_details: None,
                                completion_tokens_details: None,
                                thinking_usage: None,
                            }
                        });

                    Ok(Some(ChatChunk {
                        id: self.response_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: self.model.clone(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: None,
                                content: None,
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                                audio: None,
                                annotations: None,
                            },
                            finish_reason: Some(Self::parse_cohere_finish_reason(finish_reason)),
                            logprobs: None,
                        }],
                        usage,
                        system_fingerprint: None,
                    }))
                }
                // message-start, content-start, content-end, tool-call-*, citation-* - skip
                _ => Ok(None),
            }
        } else {
            // v1 format
            match event_type {
                "text-generation" => {
                    let text = json.get("text").and_then(|t| t.as_str()).unwrap_or("");
                    Ok(Some(ChatChunk {
                        id: self.response_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: self.model.clone(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: None,
                                content: Some(text.to_string()),
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                                audio: None,
                                annotations: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                        system_fingerprint: None,
                    }))
                }
                "stream-end" => {
                    let finish_reason = json
                        .get("finish_reason")
                        .and_then(|f| f.as_str())
                        .unwrap_or("stop");
                    Ok(Some(ChatChunk {
                        id: self.response_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: self.model.clone(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: None,
                                content: None,
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                                audio: None,
                                annotations: None,
                            },
                            finish_reason: Some(Self::parse_cohere_finish_reason(finish_reason)),
                            logprobs: None,
                        }],
                        usage: None,
                        system_fingerprint: None,
                    }))
                }
                // stream-start, citation-generation, tool-calls-generation - skip
                _ => Ok(None),
            }
        }
    }
}
