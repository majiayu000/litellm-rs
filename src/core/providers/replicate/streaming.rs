//! Replicate Streaming Support
//!
//! Streaming support for Replicate predictions.
//! Replicate uses a polling-based approach for predictions, but also supports
//! Server-Sent Events (SSE) for some models.

use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

use crate::core::providers::base::sse::{SSETransformer, UnifiedSSEStream};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice};

/// Replicate SSE transformer for streaming predictions
#[derive(Debug, Clone)]
pub struct ReplicateTransformer {
    provider_name: &'static str,
}

impl ReplicateTransformer {
    /// Create a new Replicate transformer
    pub fn new() -> Self {
        Self {
            provider_name: "replicate",
        }
    }
}

impl Default for ReplicateTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl SSETransformer for ReplicateTransformer {
    fn provider_name(&self) -> &'static str {
        self.provider_name
    }

    fn is_end_marker(&self, data: &str) -> bool {
        data.trim() == "[DONE]" || data.trim() == "done"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        // Replicate streaming format:
        // The output is streamed as text chunks
        // Each chunk contains just the text delta

        // Skip empty lines
        if data.trim().is_empty() {
            return Ok(None);
        }

        // Check for done marker
        if self.is_end_marker(data) {
            return Ok(None);
        }

        // Try to parse as JSON first (some models return JSON)
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            // Check if it's an output array update
            if let Some(output) = json.get("output") {
                if let Some(arr) = output.as_array() {
                    // Join array elements
                    let text: String = arr
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join("");

                    if !text.is_empty() {
                        return Ok(Some(create_chat_chunk(&text)));
                    }
                } else if let Some(text) = output.as_str() {
                    return Ok(Some(create_chat_chunk(text)));
                }
            }

            // Check for text/content field
            if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                return Ok(Some(create_chat_chunk(text)));
            }
            if let Some(text) = json.get("content").and_then(|v| v.as_str()) {
                return Ok(Some(create_chat_chunk(text)));
            }

            // Check for error
            if let Some(error) = json.get("error").and_then(|v| v.as_str()) {
                return Err(ProviderError::replicate_prediction_failed(error));
            }
        }

        // If not JSON, treat as raw text output
        Ok(Some(create_chat_chunk(data)))
    }
}

/// Helper function to create a chat chunk from text
fn create_chat_chunk(text: &str) -> ChatChunk {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);

    ChatChunk {
        id: format!("chatcmpl-replicate-{}", timestamp),
        object: "chat.completion.chunk".to_string(),
        created: timestamp,
        model: "replicate".to_string(),
        system_fingerprint: None,
        choices: vec![ChatStreamChoice {
            index: 0,
            delta: ChatDelta {
                role: Some(crate::core::types::message::MessageRole::Assistant),
                content: Some(text.to_string()),
                thinking: None,
                tool_calls: None,
                function_call: None,
                audio: None,
                annotations: None,
            },
            logprobs: None,
            finish_reason: None,
        }],
        usage: None,
    }
}

/// Replicate stream type alias
pub type ReplicateStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    ReplicateTransformer,
>;

/// Create a Replicate stream from a bytes stream
pub fn create_replicate_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> ReplicateStream {
    let transformer = ReplicateTransformer::new();
    UnifiedSSEStream::new(Box::pin(stream), transformer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replicate_transformer_creation() {
        let transformer = ReplicateTransformer::new();
        assert_eq!(transformer.provider_name(), "replicate");
    }

    #[test]
    fn test_transform_raw_text() {
        let transformer = ReplicateTransformer::new();
        let result = transformer.transform_chunk("Hello, world!");
        assert!(result.is_ok());

        let chunk = result.unwrap().unwrap();
        assert_eq!(
            chunk.choices[0].delta.content,
            Some("Hello, world!".to_string())
        );
    }

    #[test]
    fn test_transform_empty_line() {
        let transformer = ReplicateTransformer::new();
        let result = transformer.transform_chunk("");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_transform_done_marker() {
        let transformer = ReplicateTransformer::new();

        let result1 = transformer.transform_chunk("[DONE]");
        assert!(result1.is_ok());
        assert!(result1.unwrap().is_none());

        let result2 = transformer.transform_chunk("done");
        assert!(result2.is_ok());
        assert!(result2.unwrap().is_none());
    }

    #[test]
    fn test_transform_json_output_string() {
        let transformer = ReplicateTransformer::new();
        let json = r#"{"output": "Hello, world!"}"#;
        let result = transformer.transform_chunk(json);
        assert!(result.is_ok());

        let chunk = result.unwrap().unwrap();
        assert_eq!(
            chunk.choices[0].delta.content,
            Some("Hello, world!".to_string())
        );
    }

    #[test]
    fn test_transform_json_output_array() {
        let transformer = ReplicateTransformer::new();
        let json = r#"{"output": ["Hello", ", ", "world", "!"]}"#;
        let result = transformer.transform_chunk(json);
        assert!(result.is_ok());

        let chunk = result.unwrap().unwrap();
        assert_eq!(
            chunk.choices[0].delta.content,
            Some("Hello, world!".to_string())
        );
    }

    #[test]
    fn test_transform_json_text_field() {
        let transformer = ReplicateTransformer::new();
        let json = r#"{"text": "Sample text"}"#;
        let result = transformer.transform_chunk(json);
        assert!(result.is_ok());

        let chunk = result.unwrap().unwrap();
        assert_eq!(
            chunk.choices[0].delta.content,
            Some("Sample text".to_string())
        );
    }

    #[test]
    fn test_transform_json_error() {
        let transformer = ReplicateTransformer::new();
        let json = r#"{"error": "Something went wrong"}"#;
        let result = transformer.transform_chunk(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_chat_chunk() {
        let chunk = create_chat_chunk("Test content");
        assert!(chunk.id.starts_with("chatcmpl-replicate-"));
        assert_eq!(chunk.object, "chat.completion.chunk");
        assert_eq!(chunk.model, "replicate");
        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(
            chunk.choices[0].delta.role,
            Some(crate::core::types::message::MessageRole::Assistant)
        );
        assert_eq!(
            chunk.choices[0].delta.content,
            Some("Test content".to_string())
        );
    }

    #[test]
    fn test_transformer_default() {
        let transformer = ReplicateTransformer::default();
        assert_eq!(transformer.provider_name(), "replicate");
    }

    #[test]
    fn test_transform_whitespace_only() {
        let transformer = ReplicateTransformer::new();
        let result = transformer.transform_chunk("   ");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_is_end_marker() {
        let transformer = ReplicateTransformer::new();
        assert!(transformer.is_end_marker("[DONE]"));
        assert!(transformer.is_end_marker("done"));
        assert!(transformer.is_end_marker("  [DONE]  "));
        assert!(!transformer.is_end_marker("data"));
    }
}
