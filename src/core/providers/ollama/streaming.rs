//! Streaming Module for Ollama
//!
//! Handles Ollama's streaming response format (NDJSON - newline-delimited JSON).
//! Ollama uses a different format than OpenAI's SSE, so we need a custom parser.

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::shared::MessageTransformer;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::message::MessageRole;
#[cfg(test)]
use crate::core::types::responses::ChatResponse;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice, FinishReason, Usage};
use bytes::Bytes;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Ollama streaming response chunk
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OllamaStreamChunk {
    /// Model name
    #[serde(default)]
    pub model: Option<String>,

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

    /// HTTP status associated with an error record
    #[serde(default)]
    pub status: Option<serde_json::Value>,
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
    #[serde(default)]
    pub index: Option<u32>,

    pub name: String,

    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Ollama stream wrapper that handles NDJSON parsing
pub struct OllamaStream<S> {
    inner: S,
    buffer: Vec<u8>,
    chunk_id: String,
    saw_tool_calls: bool,
    next_tool_index: u32,
    tool_calls: HashMap<u32, (String, String)>,
    tool_call_indices_by_id: HashMap<String, u32>,
    tool_call_arguments: HashMap<u32, serde_json::Value>,
    implicit_tool_indices: Vec<u32>,
    pending_error: Option<ProviderError>,
    finished: bool,
}

impl<S> OllamaStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    pub fn new(stream: S) -> Self {
        Self {
            inner: stream,
            buffer: Vec::new(),
            chunk_id: format!("ollama-{}", uuid::Uuid::new_v4()),
            saw_tool_calls: false,
            next_tool_index: 0,
            tool_calls: HashMap::new(),
            tool_call_indices_by_id: HashMap::new(),
            tool_call_arguments: HashMap::new(),
            implicit_tool_indices: Vec::new(),
            pending_error: None,
            finished: false,
        }
    }

    /// Parse a single line as an Ollama chunk
    fn parse_line(&mut self, line: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        let chunk: OllamaStreamChunk = serde_json::from_str(line).map_err(|e| {
            ProviderError::streaming_error("ollama", "chat", None, None, e.to_string())
        })?;

        // Check for error
        if let Some(error) = chunk.error.as_deref() {
            let status = chunk
                .status
                .as_ref()
                .and_then(serde_json::Value::as_u64)
                .and_then(|status| u16::try_from(status).ok())
                .filter(|status| (100..=599).contains(status))
                .unwrap_or(500);
            return Err(HttpErrorMapper::map_status_code("ollama", status, error).redacted());
        }

        // Convert to ChatChunk
        let chat_chunk = self.convert_chunk(chunk)?;
        Ok(Some(chat_chunk))
    }

    fn parse_bytes(&mut self, line: &[u8]) -> Result<Option<ChatChunk>, ProviderError> {
        let line = std::str::from_utf8(line).map_err(|error| {
            ProviderError::streaming_error("ollama", "chat", None, None, error.to_string())
        })?;
        self.parse_line(line)
    }

    /// Convert Ollama chunk to standard ChatChunk
    fn convert_chunk(&mut self, chunk: OllamaStreamChunk) -> Result<ChatChunk, ProviderError> {
        let model = chunk
            .model
            .as_deref()
            .filter(|model| !model.trim().is_empty())
            .ok_or_else(|| {
                ProviderError::response_parsing(
                    "ollama",
                    "Ollama streaming success record is missing a model",
                )
            })?;
        let mut delta = ChatDelta::default();

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
                        signature: None,
                        redacted_data: None,
                        is_start: None,
                        is_complete: None,
                    });

            // Convert tool calls if present
            if let Some(tool_calls) = &message.tool_calls {
                let mut converted = Vec::with_capacity(tool_calls.len());
                for (position, tc) in tool_calls.iter().enumerate() {
                    let index = match tc.function.index {
                        Some(index) => index,
                        None => {
                            tc.id
                                .as_ref()
                                .and_then(|id| self.tool_call_indices_by_id.get(id).copied())
                                .or_else(|| {
                                    self.implicit_tool_indices.get(position).copied().filter(
                                        |index| {
                                            self.tool_calls
                                                .get(index)
                                                .is_some_and(|(name, _)| name == &tc.function.name)
                                        },
                                    )
                                })
                                .unwrap_or(self.next_tool_index)
                        }
                    };
                    if tc.id.as_ref().is_some_and(|id| {
                        self.tool_call_indices_by_id
                            .get(id)
                            .is_some_and(|mapped_index| *mapped_index != index)
                    }) {
                        return Err(ProviderError::response_parsing(
                            "ollama",
                            "Ollama tool call ID changed index within the stream",
                        ));
                    }
                    if index < self.next_tool_index && !self.tool_calls.contains_key(&index) {
                        return Err(ProviderError::response_parsing(
                            "ollama",
                            format!(
                                "Ollama tool call index {index} precedes the next expected index {}",
                                self.next_tool_index
                            ),
                        ));
                    }
                    let id = match self.tool_calls.get(&index) {
                        Some((name, id)) => {
                            if name != &tc.function.name
                                || tc.id.as_ref().is_some_and(|upstream_id| upstream_id != id)
                            {
                                return Err(ProviderError::response_parsing(
                                    "ollama",
                                    format!(
                                        "Ollama tool call index {index} changed identity within the stream"
                                    ),
                                ));
                            }
                            id.clone()
                        }
                        None => {
                            let next_index = index.checked_add(1).ok_or_else(|| {
                                ProviderError::response_parsing(
                                    "ollama",
                                    "Ollama tool call index exceeds the supported range",
                                )
                            })?;
                            self.next_tool_index = self.next_tool_index.max(next_index);
                            let id = tc
                                .id
                                .clone()
                                .unwrap_or_else(|| format!("call_{}_{index}", self.chunk_id));
                            self.tool_calls
                                .insert(index, (tc.function.name.clone(), id.clone()));
                            if let Some(upstream_id) = &tc.id {
                                self.tool_call_indices_by_id
                                    .insert(upstream_id.clone(), index);
                            }
                            id
                        }
                    };
                    if position == self.implicit_tool_indices.len() {
                        self.implicit_tool_indices.push(index);
                    } else if let Some(mapped_index) = self.implicit_tool_indices.get_mut(position)
                    {
                        *mapped_index = index;
                    }
                    let arguments = match self.tool_call_arguments.get(&index) {
                        None if tc.function.arguments.is_null() => None,
                        None => {
                            self.tool_call_arguments
                                .insert(index, tc.function.arguments.clone());
                            Some(tc.function.arguments.to_string())
                        }
                        Some(previous)
                            if !tc.function.arguments.is_string()
                                && previous == &tc.function.arguments =>
                        {
                            None
                        }
                        Some(previous)
                            if !tc.function.arguments.is_string()
                                && !tc.function.arguments.is_null()
                                && !previous.is_string() =>
                        {
                            return Err(ProviderError::response_parsing(
                                "ollama",
                                format!(
                                    "Ollama tool call index {index} changed complete arguments within the stream"
                                ),
                            ));
                        }
                        Some(_) if tc.function.arguments.is_null() => None,
                        Some(_) => {
                            self.tool_call_arguments
                                .insert(index, tc.function.arguments.clone());
                            Some(tc.function.arguments.to_string())
                        }
                    };
                    converted.push(crate::core::types::responses::ToolCallDelta {
                        index,
                        id: Some(id),
                        tool_type: Some("function".to_string()),
                        function: Some(crate::core::types::responses::FunctionCallDelta {
                            name: Some(tc.function.name.clone()),
                            arguments,
                        }),
                    });
                }

                if !converted.is_empty() {
                    delta.tool_calls = Some(converted);
                }
            }
        }

        if delta
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty())
        {
            self.saw_tool_calls = true;
        }

        // Determine finish reason
        let finish_reason = if chunk.done {
            let upstream_reason = chunk
                .done_reason
                .as_deref()
                .and_then(MessageTransformer::parse_finish_reason);
            Some(match upstream_reason {
                None | Some(FinishReason::Stop) if self.saw_tool_calls => FinishReason::ToolCalls,
                Some(reason) => reason,
                None => FinishReason::Stop,
            })
        } else {
            None
        };

        // Build usage info (only on final chunk)
        let usage = if chunk.done {
            let prompt_tokens = chunk.prompt_eval_count.unwrap_or(0);
            let completion_tokens = chunk.eval_count.unwrap_or(0);
            let total_tokens = prompt_tokens
                .checked_add(completion_tokens)
                .ok_or_else(|| {
                    ProviderError::response_parsing(
                        "ollama",
                        "Ollama streaming token usage overflow",
                    )
                })?;
            Some(Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
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
            model: format!("ollama/{model}"),
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
        if let Some(error) = self.pending_error.take() {
            self.finished = true;
            return Poll::Ready(Some(Err(error)));
        }

        loop {
            // Check if we have a complete line in the buffer
            if let Some(newline_pos) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let mut line = self.buffer.drain(..=newline_pos).collect::<Vec<_>>();
                line.pop();

                match self.parse_bytes(&line) {
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
                    Err(e) => {
                        self.finished = true;
                        self.buffer.clear();
                        return Poll::Ready(Some(Err(e)));
                    }
                }
            }

            // Need more data from the underlying stream
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    self.buffer.extend_from_slice(&bytes);
                    // Continue loop to check for complete lines
                }
                Poll::Ready(Some(Err(e))) => {
                    self.finished = true;
                    self.buffer.clear();
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
                        match self.parse_bytes(&line) {
                            Ok(Some(chunk)) => {
                                if chunk
                                    .choices
                                    .first()
                                    .is_some_and(|choice| choice.finish_reason.is_some())
                                {
                                    self.finished = true;
                                    return Poll::Ready(Some(Ok(chunk)));
                                }
                                self.pending_error = Some(ProviderError::streaming_error(
                                    "ollama",
                                    "chat",
                                    None,
                                    None,
                                    "Ollama stream ended before a done record",
                                ));
                                return Poll::Ready(Some(Ok(chunk)));
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.finished = true;
                                return Poll::Ready(Some(Err(e)));
                            }
                        }
                    }
                    self.finished = true;
                    return Poll::Ready(Some(Err(ProviderError::streaming_error(
                        "ollama",
                        "chat",
                        None,
                        None,
                        "Ollama stream ended before a done record",
                    ))));
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
                ..Default::default()
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
                            content: Some(chunk_text),
                            ..Default::default()
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
                delta: ChatDelta::default(),
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
    use futures::StreamExt as _;

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
        assert_eq!(chunk.model.as_deref(), Some("llama3:8b"));
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

    #[tokio::test]
    async fn stream_preserves_utf8_split_across_transport_chunks() {
        let json = concat!(
            "{\"model\":\"llama3:8b\",\"message\":{\"role\":\"assistant\",",
            "\"content\":\"你好\"},\"done\":true,\"done_reason\":\"stop\"}\n"
        );
        let split = json.find('你').expect("fixture contains multibyte content") + 1;
        let chunks = vec![
            Ok::<Bytes, reqwest::Error>(Bytes::copy_from_slice(&json.as_bytes()[..split])),
            Ok(Bytes::copy_from_slice(&json.as_bytes()[split..])),
        ];
        let mut stream = OllamaStream::new(futures::stream::iter(chunks));

        let chunk = stream
            .next()
            .await
            .expect("stream should emit a chunk")
            .expect("split UTF-8 should remain valid");
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("你好"));
    }

    #[tokio::test]
    async fn streamed_tool_calls_before_terminal_chunk_override_stop_reason() {
        let first = concat!(
            "{\"model\":\"llama3:8b\",\"message\":{\"role\":\"assistant\",",
            "\"tool_calls\":[{\"function\":{\"name\":\"weather\",\"arguments\":{}}}]},",
            "\"done\":false}\n"
        );
        let last = concat!(
            "{\"model\":\"llama3:8b\",\"message\":{\"role\":\"assistant\",",
            "\"content\":\"\"},",
            "\"done\":true,\"done_reason\":\"stop\"}\n"
        );
        let chunks = vec![
            Ok::<Bytes, reqwest::Error>(Bytes::from_static(first.as_bytes())),
            Ok(Bytes::from_static(last.as_bytes())),
        ];
        let mut stream = OllamaStream::new(futures::stream::iter(chunks));

        let first = stream.next().await.unwrap().unwrap();
        let last = stream.next().await.unwrap().unwrap();
        let first_id = first.choices[0].delta.tool_calls.as_ref().unwrap()[0]
            .id
            .as_deref();
        assert!(first_id.is_some_and(|id| id.starts_with("call_ollama-")));
        assert!(last.choices[0].delta.tool_calls.is_none());
        assert_eq!(last.choices[0].finish_reason, Some(FinishReason::ToolCalls));
    }

    #[tokio::test]
    async fn error_only_stream_chunk_preserves_upstream_message() {
        let chunks = vec![Ok::<Bytes, reqwest::Error>(Bytes::from_static(
            b"{\"error\":\"runner crashed\"}\n",
        ))];
        let mut stream = OllamaStream::new(futures::stream::iter(chunks));

        let error = stream
            .next()
            .await
            .expect("error record should emit one result")
            .expect_err("error record should fail the stream");

        assert!(error.to_string().contains("runner crashed"));
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
                    audio: None,
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
