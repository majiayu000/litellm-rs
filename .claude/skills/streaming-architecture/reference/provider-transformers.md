## Provider-Specific Transformers

### OpenAI Transformer

```rust
pub struct OpenAITransformer;

impl SSETransformer for OpenAITransformer {
    fn transform(&self, event: &SSEEvent) -> Result<Option<ChatChunk>, StreamError> {
        // Handle [DONE] marker
        if event.data == "[DONE]" {
            return Ok(None);
        }

        // Parse OpenAI chunk format
        let chunk: ChatChunk = serde_json::from_str(&event.data)
            .map_err(|e| StreamError::Parse {
                provider: self.provider_name(),
                message: e.to_string(),
            })?;

        Ok(Some(chunk))
    }

    fn is_done(&self, event: &SSEEvent) -> bool {
        event.data == "[DONE]"
    }

    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn handle_error(&self, event: &SSEEvent) -> Option<StreamError> {
        if event.data.contains("\"error\"") {
            if let Ok(error) = serde_json::from_str::<serde_json::Value>(&event.data) {
                if let Some(msg) = error.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
                    return Some(StreamError::ProviderError {
                        provider: self.provider_name(),
                        message: msg.to_string(),
                    });
                }
            }
        }
        None
    }
}
```

### Anthropic Transformer

```rust
pub struct AnthropicTransformer {
    /// Accumulated content for multi-part responses
    accumulated_content: std::cell::RefCell<String>,
}

impl AnthropicTransformer {
    pub fn new() -> Self {
        Self {
            accumulated_content: std::cell::RefCell::new(String::new()),
        }
    }
}

impl SSETransformer for AnthropicTransformer {
    fn transform(&self, event: &SSEEvent) -> Result<Option<ChatChunk>, StreamError> {
        // Anthropic uses event types
        let event_type = event.event_type.as_deref().unwrap_or("");

        match event_type {
            "message_start" => {
                // Initialize message, extract ID
                let data: serde_json::Value = serde_json::from_str(&event.data)
                    .map_err(|e| StreamError::Parse {
                        provider: self.provider_name(),
                        message: e.to_string(),
                    })?;

                let id = data.get("message")
                    .and_then(|m| m.get("id"))
                    .and_then(|i| i.as_str())
                    .unwrap_or("msg_anthropic")
                    .to_string();

                Ok(Some(ChatChunk {
                    id,
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp(),
                    model: data.get("message")
                        .and_then(|m| m.get("model"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("claude")
                        .to_string(),
                    choices: vec![],
                    usage: None,
                    system_fingerprint: None,
                }))
            }

            "content_block_delta" => {
                let data: serde_json::Value = serde_json::from_str(&event.data)
                    .map_err(|e| StreamError::Parse {
                        provider: self.provider_name(),
                        message: e.to_string(),
                    })?;

                let delta_text = data.get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                // Convert to OpenAI chunk format
                Ok(Some(ChatChunk {
                    id: "".to_string(),
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp(),
                    model: "".to_string(),
                    choices: vec![crate::core::types::responses::ChunkChoice {
                        index: 0,
                        delta: crate::core::types::responses::ChunkDelta {
                            role: None,
                            content: Some(delta_text.to_string()),
                            tool_calls: None,
                            function_call: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    usage: None,
                    system_fingerprint: None,
                }))
            }

            "message_stop" => {
                Ok(None)
            }

            "message_delta" => {
                // Final message with usage info
                let data: serde_json::Value = serde_json::from_str(&event.data)
                    .map_err(|e| StreamError::Parse {
                        provider: self.provider_name(),
                        message: e.to_string(),
                    })?;

                let finish_reason = data.get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(|r| r.as_str())
                    .map(|r| match r {
                        "end_turn" => crate::core::types::responses::FinishReason::Stop,
                        "max_tokens" => crate::core::types::responses::FinishReason::Length,
                        "tool_use" => crate::core::types::responses::FinishReason::ToolCalls,
                        _ => crate::core::types::responses::FinishReason::Stop,
                    });

                Ok(Some(ChatChunk {
                    id: "".to_string(),
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp(),
                    model: "".to_string(),
                    choices: vec![crate::core::types::responses::ChunkChoice {
                        index: 0,
                        delta: crate::core::types::responses::ChunkDelta {
                            role: None,
                            content: None,
                            tool_calls: None,
                            function_call: None,
                        },
                        finish_reason,
                        logprobs: None,
                    }],
                    usage: data.get("usage").and_then(|u| {
                        Some(crate::core::types::responses::Usage {
                            prompt_tokens: u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
                            completion_tokens: u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
                            total_tokens: 0,
                            prompt_tokens_details: None,
                            completion_tokens_details: None,
                            thinking_usage: None,
                        })
                    }),
                    system_fingerprint: None,
                }))
            }

            "error" => {
                Err(StreamError::ProviderError {
                    provider: self.provider_name(),
                    message: event.data.clone(),
                })
            }

            _ => Ok(None),
        }
    }

    fn is_done(&self, event: &SSEEvent) -> bool {
        event.event_type.as_deref() == Some("message_stop")
    }

    fn provider_name(&self) -> &'static str {
        "anthropic"
    }

    fn handle_error(&self, event: &SSEEvent) -> Option<StreamError> {
        if event.event_type.as_deref() == Some("error") {
            return Some(StreamError::ProviderError {
                provider: self.provider_name(),
                message: event.data.clone(),
            });
        }
        None
    }
}
```

### Google Gemini Transformer

```rust
pub struct GeminiTransformer;

impl SSETransformer for GeminiTransformer {
    fn transform(&self, event: &SSEEvent) -> Result<Option<ChatChunk>, StreamError> {
        // Gemini uses a different format
        let data: serde_json::Value = serde_json::from_str(&event.data)
            .map_err(|e| StreamError::Parse {
                provider: self.provider_name(),
                message: e.to_string(),
            })?;

        // Extract text from candidates[0].content.parts[0].text
        let text = data.get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .and_then(|p| p.first())
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str());

        let finish_reason = data.get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
            .and_then(|c| c.get("finishReason"))
            .and_then(|r| r.as_str())
            .map(|r| match r {
                "STOP" => crate::core::types::responses::FinishReason::Stop,
                "MAX_TOKENS" => crate::core::types::responses::FinishReason::Length,
                "SAFETY" => crate::core::types::responses::FinishReason::ContentFilter,
                _ => crate::core::types::responses::FinishReason::Stop,
            });

        Ok(Some(ChatChunk {
            id: "".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: "gemini".to_string(),
            choices: vec![crate::core::types::responses::ChunkChoice {
                index: 0,
                delta: crate::core::types::responses::ChunkDelta {
                    role: None,
                    content: text.map(|t| t.to_string()),
                    tool_calls: None,
                    function_call: None,
                },
                finish_reason,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        }))
    }

    fn is_done(&self, event: &SSEEvent) -> bool {
        serde_json::from_str::<serde_json::Value>(&event.data)
            .ok()
            .and_then(|d| d.get("candidates"))
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
            .and_then(|c| c.get("finishReason"))
            .is_some()
    }

    fn provider_name(&self) -> &'static str {
        "google"
    }

    fn handle_error(&self, event: &SSEEvent) -> Option<StreamError> {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&event.data) {
            if let Some(error) = data.get("error") {
                return Some(StreamError::ProviderError {
                    provider: self.provider_name(),
                    message: error.to_string(),
                });
            }
        }
        None
    }
}
```
