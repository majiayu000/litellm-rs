//! Streaming Module for Ollama
//!
//! Handles Ollama's streaming response format (NDJSON - newline-delimited JSON).
//! Ollama uses a different format than OpenAI's SSE, so we need a custom parser.

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::message::MessageRole;
#[cfg(test)]
use crate::core::types::responses::ChatResponse;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice, Usage};
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Ollama streaming response chunk
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OllamaStreamChunk {
    /// Model name
    pub model: String,

    /// Message content
    #[serde(default)]
    pub message: Option<OllamaMessage>,

    /// Whether this is the final chunk
    #[serde(default)]
    pub done: bool,

    /// Done reason (only present when done=true)
    #[serde(default)]
    pub done_reason: Option<String>,

    /// Prompt evaluation count (only present when done=true)
    #[serde(default)]
    pub prompt_eval_count: Option<u32>,

    /// Evaluation count (only present when done=true)
    #[serde(default)]
    pub eval_count: Option<u32>,

    /// Error message (if any)
    #[serde(default)]
    pub error: Option<String>,
}

/// Ollama message in streaming response
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OllamaMessage {
    /// Role of the message sender
    pub role: String,

    /// Message content
    #[serde(default)]
    pub content: Option<String>,

    /// Thinking/reasoning content (for reasoning models)
    #[serde(default)]
    pub thinking: Option<String>,

    /// Tool calls (if any)
    #[serde(default)]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}

/// Ollama tool call format
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OllamaToolCall {
    #[serde(default)]
    pub id: Option<String>,

    pub function: OllamaToolFunction,
}

/// Ollama tool function format
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OllamaToolFunction {
    pub name: String,

    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Ollama stream wrapper that handles NDJSON parsing
pub struct OllamaStream<S> {
    inner: S,
    buffer: String,
    chunk_id: String,
    finished: bool,
}

impl<S> OllamaStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    pub fn new(stream: S) -> Self {
        Self {
            inner: stream,
            buffer: String::new(),
            chunk_id: format!("ollama-{}", uuid::Uuid::new_v4()),
            finished: false,
        }
    }

    /// Parse a single line as an Ollama chunk
    fn parse_line(&self, line: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        let chunk: OllamaStreamChunk = serde_json::from_str(line).map_err(|e| {
            ProviderError::streaming_error("ollama", "chat", None, None, e.to_string())
        })?;

        // Check for error
        if let Some(error) = chunk.error {
            return Err(ProviderError::api_error("ollama", 500, error));
        }

        // Convert to ChatChunk
        let chat_chunk = self.convert_chunk(chunk)?;
        Ok(Some(chat_chunk))
    }

    /// Convert Ollama chunk to standard ChatChunk
    fn convert_chunk(&self, chunk: OllamaStreamChunk) -> Result<ChatChunk, ProviderError> {
        let mut delta = ChatDelta {
            role: None,
            content: None,
            thinking: None,
            tool_calls: None,
            function_call: None,
        };

        // Extract message content
        if let Some(message) = &chunk.message {
            // Set role if present
            if message.role == "assistant" {
                delta.role = Some(MessageRole::Assistant);
            }

            // Set content
            delta.content = message.content.clone();

            // Set thinking content (for reasoning models)
            delta.thinking =
                message
                    .thinking
                    .as_ref()
                    .map(|t| crate::core::types::thinking::ThinkingDelta {
                        content: Some(t.clone()),
                        is_start: None,
                        is_complete: None,
                    });

            // Convert tool calls if present
            if let Some(tool_calls) = &message.tool_calls {
                let converted: Vec<_> = tool_calls
                    .iter()
                    .enumerate()
                    .map(|(i, tc)| crate::core::types::responses::ToolCallDelta {
                        index: i as u32,
                        id: tc.id.clone(),
                        tool_type: Some("function".to_string()),
                        function: Some(crate::core::types::responses::FunctionCallDelta {
                            name: Some(tc.function.name.clone()),
                            arguments: Some(tc.function.arguments.to_string()),
                        }),
                    })
                    .collect();

                if !converted.is_empty() {
                    delta.tool_calls = Some(converted);
                }
            }
        }

        // Determine finish reason
        let finish_reason = if chunk.done {
            let reason_str = chunk.done_reason.as_deref().unwrap_or("stop");
            Some(match reason_str {
                "stop" => crate::core::types::responses::FinishReason::Stop,
                "length" => crate::core::types::responses::FinishReason::Length,
                "tool_calls" => crate::core::types::responses::FinishReason::ToolCalls,
                "content_filter" => crate::core::types::responses::FinishReason::ContentFilter,
                "function_call" => crate::core::types::responses::FinishReason::FunctionCall,
                _ => crate::core::types::responses::FinishReason::Stop,
            })
        } else {
            None
        };

        // Build usage info (only on final chunk)
        let usage = if chunk.done {
            Some(Usage {
                prompt_tokens: chunk.prompt_eval_count.unwrap_or(0),
                completion_tokens: chunk.eval_count.unwrap_or(0),
                total_tokens: chunk.prompt_eval_count.unwrap_or(0) + chunk.eval_count.unwrap_or(0),
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            })
        } else {
            None
        };

        Ok(ChatChunk {
            id: self.chunk_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: format!("ollama/{}", chunk.model),
            system_fingerprint: None,
            choices: vec![ChatStreamChoice {
                index: 0,
                delta,
                finish_reason,
                logprobs: None,
            }],
            usage,
        })
    }
}

impl<S> Stream for OllamaStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }

        loop {
            // Check if we have a complete line in the buffer
            if let Some(newline_pos) = self.buffer.find('\n') {
                let line = self.buffer[..newline_pos].to_string();
                self.buffer = self.buffer[newline_pos + 1..].to_string();

                match self.parse_line(&line) {
                    Ok(Some(chunk)) => {
                        // Check if this is the final chunk
                        if chunk
                            .choices
                            .first()
                            .is_some_and(|c| c.finish_reason.is_some())
                        {
                            self.finished = true;
                        }
                        return Poll::Ready(Some(Ok(chunk)));
                    }
                    Ok(None) => continue, // Empty line, try next
                    Err(e) => return Poll::Ready(Some(Err(e))),
                }
            }

            // Need more data from the underlying stream
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    let text = String::from_utf8_lossy(&bytes);
                    self.buffer.push_str(&text);
                    // Continue loop to check for complete lines
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(ProviderError::streaming_error(
                        "ollama",
                        "chat",
                        None,
                        None,
                        e.to_string(),
                    ))));
                }
                Poll::Ready(None) => {
                    // Stream ended, process any remaining data
                    if !self.buffer.is_empty() {
                        let line = std::mem::take(&mut self.buffer);
                        match self.parse_line(&line) {
                            Ok(Some(chunk)) => {
                                self.finished = true;
                                return Poll::Ready(Some(Ok(chunk)));
                            }
                            Ok(None) => {
                                self.finished = true;
                                return Poll::Ready(None);
                            }
                            Err(e) => return Poll::Ready(Some(Err(e))),
                        }
                    }
                    self.finished = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// Convert a complete ChatResponse to stream chunks
#[cfg(test)]
fn response_to_chunks(response: ChatResponse) -> Vec<ChatChunk> {
    let mut chunks = Vec::new();

    // Create initial chunk with role
    chunks.push(ChatChunk {
        id: response.id.clone(),
        object: "chat.completion.chunk".to_string(),
        created: response.created,
        model: response.model.clone(),
        system_fingerprint: response.system_fingerprint.clone(),
        choices: vec![ChatStreamChoice {
            index: 0,
            delta: ChatDelta {
                role: Some(MessageRole::Assistant),
                content: None,
                thinking: None,
                tool_calls: None,
                function_call: None,
            },
            finish_reason: None,
            logprobs: None,
        }],
        usage: None,
    });

    // Create content chunks
    if let Some(choice) = response.choices.first() {
        if let Some(content) = &choice.message.content {
            use crate::core::types::message::MessageContent;
            let text = match content {
                MessageContent::Text(text) => text.clone(),
                MessageContent::Parts(_) => content.to_string(),
            };

            // Split content into smaller chunks for more natural streaming
            let words: Vec<&str> = text.split_whitespace().collect();
            let chunk_size = 5;

            for word_chunk in words.chunks(chunk_size) {
                let chunk_text = word_chunk.join(" ") + " ";
                chunks.push(ChatChunk {
                    id: response.id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created: response.created,
                    model: response.model.clone(),
                    system_fingerprint: response.system_fingerprint.clone(),
                    choices: vec![ChatStreamChoice {
                        index: 0,
                        delta: ChatDelta {
                            role: None,
                            content: Some(chunk_text),
                            thinking: None,
                            tool_calls: None,
                            function_call: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    usage: None,
                });
            }
        }

        // Add final chunk with finish_reason
        chunks.push(ChatChunk {
            id: response.id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: response.created,
            model: response.model.clone(),
            system_fingerprint: response.system_fingerprint.clone(),
            choices: vec![ChatStreamChoice {
                index: 0,
                delta: ChatDelta {
                    role: None,
                    content: None,
                    thinking: None,
                    tool_calls: None,
                    function_call: None,
                },
                finish_reason: choice.finish_reason.clone(),
                logprobs: None,
            }],
            usage: response.usage.clone(),
        });
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_stream_chunk_deserialization() {
        let json = r#"{
            "model": "llama3:8b",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": "Hello"
            },
            "done": false
        }"#;

        let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.model, "llama3:8b");
        assert!(!chunk.done);
        assert!(chunk.message.is_some());
        assert_eq!(chunk.message.unwrap().content, Some("Hello".to_string()));
    }

    #[test]
    fn test_ollama_stream_chunk_done() {
        let json = r#"{
            "model": "llama3:8b",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": ""
            },
            "done": true,
            "done_reason": "stop",
            "prompt_eval_count": 10,
            "eval_count": 50,
            "total_duration": 1000000000
        }"#;

        let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.done);
        assert_eq!(chunk.done_reason, Some("stop".to_string()));
        assert_eq!(chunk.prompt_eval_count, Some(10));
        assert_eq!(chunk.eval_count, Some(50));
    }

    #[test]
    fn test_ollama_stream_chunk_with_tool_calls() {
        let json = r#"{
            "model": "llama3:8b",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "function": {
                            "name": "get_weather",
                            "arguments": {"location": "NYC"}
                        }
                    }
                ]
            },
            "done": true,
            "done_reason": "tool_calls"
        }"#;

        let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
        let tool_calls = chunk.message.as_ref().unwrap().tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls.first().unwrap().function.name, "get_weather");
    }

    #[test]
    fn test_ollama_stream_chunk_with_thinking() {
        let json = r#"{
            "model": "deepseek-r1",
            "message": {
                "role": "assistant",
                "content": "",
                "thinking": "Let me think about this..."
            },
            "done": false
        }"#;

        let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
        let message = chunk.message.unwrap();
        assert_eq!(
            message.thinking,
            Some("Let me think about this...".to_string())
        );
    }

    #[test]
    fn test_ollama_stream_chunk_error() {
        let json = r#"{
            "model": "llama3:8b",
            "error": "model not found",
            "done": true
        }"#;

        let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.error, Some("model not found".to_string()));
    }

    #[test]
    fn test_response_to_chunks() {
        use crate::core::types::responses::ChatChoice;
        use crate::core::types::{chat::ChatMessage, message::MessageContent};

        let response = ChatResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "ollama/llama3:8b".to_string(),
            system_fingerprint: None,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text("Hello world".to_string())),
                    thinking: None,
                    tool_calls: None,
                    function_call: None,
                    name: None,
                    tool_call_id: None,
                },
                finish_reason: Some(crate::core::types::responses::FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 2,
                total_tokens: 12,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
        };

        let chunks = response_to_chunks(response);

        // Should have at least 3 chunks: role, content, finish
        assert!(chunks.len() >= 3);

        // First chunk should have role
        assert!(chunks[0].choices.first().unwrap().delta.role.is_some());

        // Last chunk should have finish_reason
        let last = chunks.last().unwrap();
        assert!(last.choices.first().unwrap().finish_reason.is_some());
        assert!(last.usage.is_some());
    }
}
