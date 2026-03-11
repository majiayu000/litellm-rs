//! Streaming response handler implementation

use super::types::{ChatCompletionChunk, ChatCompletionChunkChoice, ChatCompletionDelta, Event};
use crate::core::models::openai::Usage;
use crate::core::types::message::MessageRole;
use crate::utils::error::gateway_error::Result;
use bytes::Bytes;
use futures::stream::{Stream, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;
use uuid::Uuid;

/// Streaming response handler
pub struct StreamingHandler {
    /// Request ID for tracking
    request_id: String,
    /// Model being used
    pub(crate) model: String,
    /// Whether this is the first chunk
    pub(crate) is_first_chunk: bool,
    /// Accumulated content for final usage calculation
    pub(crate) accumulated_content: String,
}

impl StreamingHandler {
    /// Create a new streaming handler
    pub fn new(model: String) -> Self {
        Self {
            request_id: format!("chatcmpl-{}", Uuid::new_v4()),
            model,
            is_first_chunk: true,
            accumulated_content: String::new(),
        }
    }

    /// Create a streaming response from a provider stream for Actix-web
    pub fn create_sse_stream<S>(mut self, provider_stream: S) -> impl Stream<Item = Result<Bytes>>
    where
        S: Stream<Item = Result<String>> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            tokio::pin!(provider_stream);

            while let Some(chunk_result) = provider_stream.next().await {
                match chunk_result {
                    Ok(chunk_data) => {
                        match self.process_chunk(&chunk_data).await {
                            Ok(Some(event)) => {
                                if tx.send(Ok(event.to_bytes())).await.is_err() {
                                    break;
                                }
                            }
                            Ok(None) => continue, // Skip empty chunks
                            Err(e) => {
                                error!("Error processing chunk: {}", e);
                                let error_event = Event::default()
                                    .event("error")
                                    .data(&json!({"error": e.to_string()}).to_string());
                                let _ = tx.send(Ok(error_event.to_bytes())).await;
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Provider stream error: {}", e);
                        let error_event = Event::default()
                            .event("error")
                            .data(&json!({"error": e.to_string()}).to_string());
                        let _ = tx.send(Ok(error_event.to_bytes())).await;
                        break;
                    }
                }
            }

            // Send final chunk with usage information
            if let Ok(final_event) = self.create_final_chunk().await {
                let _ = tx.send(Ok(final_event.to_bytes())).await;
            }

            // Send done event
            let done_event = Event::default().data("[DONE]");
            let _ = tx.send(Ok(done_event.to_bytes())).await;
        });

        ReceiverStream::new(rx)
    }

    /// Process a single chunk from the provider
    async fn process_chunk(&mut self, chunk_data: &str) -> Result<Option<Event>> {
        // Parse provider-specific chunk format
        let content = self.extract_content_from_chunk(chunk_data)?;

        if content.is_empty() {
            return Ok(None);
        }

        self.accumulated_content.push_str(&content);

        let chunk = ChatCompletionChunk {
            id: self.request_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: self.model.clone(),
            system_fingerprint: None,
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionDelta {
                    role: if self.is_first_chunk {
                        Some(MessageRole::Assistant)
                    } else {
                        None
                    },
                    content: Some(content),
                    tool_calls: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
        };

        self.is_first_chunk = false;

        let event = Event::default().data(&serde_json::to_string(&chunk)?);

        Ok(Some(event))
    }

    /// Extract content from provider-specific chunk format
    pub(crate) fn extract_content_from_chunk(&self, chunk_data: &str) -> Result<String> {
        // Handle different provider formats
        if chunk_data.starts_with("data: ") {
            let data = chunk_data.strip_prefix("data: ").unwrap_or(chunk_data);

            if data.trim() == "[DONE]" {
                return Ok(String::new());
            }

            // Parse JSON chunk
            if let Ok(json_chunk) = serde_json::from_str::<serde_json::Value>(data) {
                // OpenAI format
                if let Some(choices) = json_chunk.get("choices").and_then(|c| c.as_array())
                    && let Some(choice) = choices.first()
                    && let Some(delta) = choice.get("delta")
                    && let Some(content) = delta.get("content").and_then(|c| c.as_str())
                {
                    return Ok(content.to_string());
                }

                // Anthropic format
                if let Some(delta) = json_chunk.get("delta")
                    && let Some(text) = delta.get("text").and_then(|t| t.as_str())
                {
                    return Ok(text.to_string());
                }

                // Generic text field
                if let Some(text) = json_chunk.get("text").and_then(|t| t.as_str()) {
                    return Ok(text.to_string());
                }
            }
        }

        // Fallback: treat as plain text
        Ok(chunk_data.to_string())
    }

    /// Create the final chunk with usage information
    async fn create_final_chunk(&self) -> Result<Event> {
        // Calculate actual token counts using the token counter
        let token_counter = crate::utils::ai::counter::token_counter::TokenCounter::new();
        let completion_tokens = token_counter
            .count_completion_tokens(&self.model, &self.accumulated_content)
            .map(|estimate| estimate.input_tokens)
            .unwrap_or_else(|_| self.estimate_token_count(&self.accumulated_content));

        // For prompt tokens, we'd need the original request context
        // For now, use a reasonable estimate based on typical chat requests
        let prompt_tokens = self.estimate_prompt_tokens();
        let total_tokens = prompt_tokens + completion_tokens;

        let usage = Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        };

        let final_chunk = ChatCompletionChunk {
            id: self.request_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: self.model.clone(),
            system_fingerprint: None,
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionDelta {
                    role: None,
                    content: None,
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Some(usage),
        };

        let event = Event::default().data(&serde_json::to_string(&final_chunk)?);

        Ok(event)
    }

    /// Estimate token count from text (simplified)
    pub(crate) fn estimate_token_count(&self, text: &str) -> u32 {
        // Very rough estimation: ~4 characters per token
        (text.len() as f64 / 4.0).ceil() as u32
    }

    /// Estimate prompt tokens based on typical chat requests
    fn estimate_prompt_tokens(&self) -> u32 {
        // This is a rough estimate since we don't have the original request
        // In a real implementation, we'd store the original prompt tokens
        // For now, use a reasonable default based on typical usage
        match self.model.as_str() {
            m if m.contains("gpt-4") => 150,
            m if m.contains("gpt-3.5") => 100,
            m if m.contains("claude") => 200,
            m if m.contains("gemini") => 120,
            _ => 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== StreamingHandler Creation Tests ====================

    #[test]
    fn test_streaming_handler_new() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        assert_eq!(handler.model, "gpt-4");
        assert!(handler.is_first_chunk);
        assert!(handler.accumulated_content.is_empty());
        assert!(handler.request_id.starts_with("chatcmpl-"));
    }

    #[test]
    fn test_streaming_handler_new_different_models() {
        let handler1 = StreamingHandler::new("gpt-3.5-turbo".to_string());
        assert_eq!(handler1.model, "gpt-3.5-turbo");

        let handler2 = StreamingHandler::new("claude-3-opus".to_string());
        assert_eq!(handler2.model, "claude-3-opus");

        let handler3 = StreamingHandler::new("gemini-pro".to_string());
        assert_eq!(handler3.model, "gemini-pro");
    }

    #[test]
    fn test_streaming_handler_unique_request_ids() {
        let handler1 = StreamingHandler::new("gpt-4".to_string());
        let handler2 = StreamingHandler::new("gpt-4".to_string());

        assert_ne!(handler1.request_id, handler2.request_id);
    }

    // ==================== Token Estimation Tests ====================

    #[test]
    fn test_estimate_token_count_empty() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        assert_eq!(handler.estimate_token_count(""), 0);
    }

    #[test]
    fn test_estimate_token_count_short_text() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        // "Hi" = 2 chars => 2/4 = 0.5 => ceil = 1
        assert_eq!(handler.estimate_token_count("Hi"), 1);
    }

    #[test]
    fn test_estimate_token_count_medium_text() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        // "Hello world" = 11 chars => 11/4 = 2.75 => ceil = 3
        assert_eq!(handler.estimate_token_count("Hello world"), 3);
    }

    #[test]
    fn test_estimate_token_count_long_text() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        // 100 chars => 100/4 = 25
        let text = "a".repeat(100);
        assert_eq!(handler.estimate_token_count(&text), 25);
    }

    #[test]
    fn test_estimate_token_count_unicode() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        // Unicode characters are counted by byte length in Rust's len()
        let text = "你好世界"; // 4 Chinese chars = 12 bytes
        let estimated = handler.estimate_token_count(text);
        assert!(estimated > 0);
    }

    #[test]
    fn test_estimate_prompt_tokens_gpt4() {
        let handler = StreamingHandler::new("gpt-4-turbo".to_string());
        assert_eq!(handler.estimate_prompt_tokens(), 150);
    }

    #[test]
    fn test_estimate_prompt_tokens_gpt35() {
        let handler = StreamingHandler::new("gpt-3.5-turbo".to_string());
        assert_eq!(handler.estimate_prompt_tokens(), 100);
    }

    #[test]
    fn test_estimate_prompt_tokens_claude() {
        let handler = StreamingHandler::new("claude-3-sonnet".to_string());
        assert_eq!(handler.estimate_prompt_tokens(), 200);
    }

    #[test]
    fn test_estimate_prompt_tokens_gemini() {
        let handler = StreamingHandler::new("gemini-pro".to_string());
        assert_eq!(handler.estimate_prompt_tokens(), 120);
    }

    #[test]
    fn test_estimate_prompt_tokens_unknown() {
        let handler = StreamingHandler::new("unknown-model".to_string());
        assert_eq!(handler.estimate_prompt_tokens(), 100);
    }

    // ==================== Content Extraction Tests ====================

    #[test]
    fn test_extract_content_openai_format() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_extract_content_openai_format_with_role() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{"role":"assistant","content":"World"}}]}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert_eq!(result, "World");
    }

    #[test]
    fn test_extract_content_anthropic_format() {
        let handler = StreamingHandler::new("claude-3".to_string());
        let chunk = r#"data: {"delta":{"text":"Bonjour"}}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert_eq!(result, "Bonjour");
    }

    #[test]
    fn test_extract_content_generic_text_field() {
        let handler = StreamingHandler::new("custom-model".to_string());
        let chunk = r#"data: {"text":"Generic text"}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert_eq!(result, "Generic text");
    }

    #[test]
    fn test_extract_content_done_signal() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = "data: [DONE]";
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_content_done_signal_with_whitespace() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = "data:   [DONE]  ";
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_content_plain_text_fallback() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = "Just plain text";
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert_eq!(result, "Just plain text");
    }

    #[test]
    fn test_extract_content_empty_delta() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{}}]}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        // Empty delta returns the original chunk when no content is found
        assert!(!result.is_empty()); // Falls through to raw JSON string
    }

    #[test]
    fn test_extract_content_empty_choices() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[]}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        // No choices means fallback
        assert!(!result.is_empty());
    }

    #[test]
    fn test_extract_content_multiple_choices() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk =
            r#"data: {"choices":[{"delta":{"content":"First"}},{"delta":{"content":"Second"}}]}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        // Should extract first choice only
        assert_eq!(result, "First");
    }

    #[test]
    fn test_extract_content_special_characters() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{"content":"Hello\nWorld\t!"}}]}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert_eq!(result, "Hello\nWorld\t!");
    }

    #[test]
    fn test_extract_content_unicode_content() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{"content":"こんにちは世界"}}]}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert_eq!(result, "こんにちは世界");
    }

    #[test]
    fn test_extract_content_empty_string() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = "";
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        assert!(result.is_empty());
    }

    // ==================== Accumulated Content Tests ====================

    #[tokio::test]
    async fn test_process_chunk_accumulates_content() {
        let mut handler = StreamingHandler::new("gpt-4".to_string());

        let chunk1 = r#"data: {"choices":[{"delta":{"content":"Hello "}}]}"#;
        let _ = handler.process_chunk(chunk1).await;
        assert_eq!(handler.accumulated_content, "Hello ");

        let chunk2 = r#"data: {"choices":[{"delta":{"content":"World"}}]}"#;
        let _ = handler.process_chunk(chunk2).await;
        assert_eq!(handler.accumulated_content, "Hello World");
    }

    #[tokio::test]
    async fn test_process_chunk_sets_first_chunk_flag() {
        let mut handler = StreamingHandler::new("gpt-4".to_string());
        assert!(handler.is_first_chunk);

        let chunk = r#"data: {"choices":[{"delta":{"content":"Test"}}]}"#;
        let _ = handler.process_chunk(chunk).await;
        assert!(!handler.is_first_chunk);
    }

    #[tokio::test]
    async fn test_process_chunk_empty_returns_none() {
        let mut handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = "data: [DONE]";
        let result = handler.process_chunk(chunk).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_process_chunk_returns_event() {
        let mut handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        let result = handler.process_chunk(chunk).await.unwrap();
        assert!(result.is_some());
    }

    // ==================== ChatCompletionChunk Format Tests ====================

    #[tokio::test]
    async fn test_process_chunk_returns_valid_json() {
        let mut handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{"content":"Test"}}]}"#;
        let result = handler.process_chunk(chunk).await.unwrap();

        if let Some(event) = result {
            let bytes = event.to_bytes();
            let event_str = String::from_utf8_lossy(&bytes);
            // Should contain valid JSON data
            assert!(event_str.contains("data:"));
            assert!(event_str.contains("chat.completion.chunk"));
        } else {
            panic!("Expected Some event");
        }
    }

    #[tokio::test]
    async fn test_first_chunk_includes_role() {
        let mut handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#;

        // First chunk should have role
        assert!(handler.is_first_chunk);
        let result = handler.process_chunk(chunk).await.unwrap();

        if let Some(event) = result {
            let bytes = event.to_bytes();
            let event_str = String::from_utf8_lossy(&bytes);
            // First chunk includes role
            assert!(event_str.contains("assistant") || event_str.contains("role"));
        }

        // After first chunk, is_first_chunk should be false
        assert!(!handler.is_first_chunk);
    }

    // ==================== Final Chunk Tests ====================

    #[tokio::test]
    async fn test_create_final_chunk() {
        let mut handler = StreamingHandler::new("gpt-4".to_string());
        handler.accumulated_content = "Hello world".to_string();

        let result = handler.create_final_chunk().await;
        assert!(result.is_ok());

        let event = result.unwrap();
        let bytes = event.to_bytes();
        let event_str = String::from_utf8_lossy(&bytes);

        assert!(event_str.contains("finish_reason"));
        assert!(event_str.contains("stop"));
        assert!(event_str.contains("usage"));
    }

    #[tokio::test]
    async fn test_create_final_chunk_includes_token_counts() {
        let mut handler = StreamingHandler::new("gpt-4".to_string());
        handler.accumulated_content = "This is a test response with some content.".to_string();

        let result = handler.create_final_chunk().await.unwrap();
        let bytes = result.to_bytes();
        let event_str = String::from_utf8_lossy(&bytes);

        // Should include token usage fields
        assert!(event_str.contains("prompt_tokens"));
        assert!(event_str.contains("completion_tokens"));
        assert!(event_str.contains("total_tokens"));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_extract_content_malformed_json() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = "data: {invalid json}";
        let result = handler.extract_content_from_chunk(chunk);
        // Should fall back to raw text
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_content_null_content() {
        let handler = StreamingHandler::new("gpt-4".to_string());
        let chunk = r#"data: {"choices":[{"delta":{"content":null}}]}"#;
        let result = handler.extract_content_from_chunk(chunk).unwrap();
        // null content should not be extracted as string
        assert!(!result.contains("null") || result.contains("{"));
    }

    #[test]
    fn test_handler_with_empty_model() {
        let handler = StreamingHandler::new(String::new());
        assert!(handler.model.is_empty());
        assert_eq!(handler.estimate_prompt_tokens(), 100); // Falls to default
    }

    #[tokio::test]
    async fn test_process_multiple_chunks_in_sequence() {
        let mut handler = StreamingHandler::new("claude-3".to_string());

        let chunks = vec![
            r#"data: {"delta":{"text":"Hello"}}"#,
            r#"data: {"delta":{"text":" "}}"#,
            r#"data: {"delta":{"text":"World"}}"#,
            r#"data: {"delta":{"text":"!"}}"#,
        ];

        for chunk in chunks {
            let _ = handler.process_chunk(chunk).await;
        }

        assert_eq!(handler.accumulated_content, "Hello World!");
    }
}
