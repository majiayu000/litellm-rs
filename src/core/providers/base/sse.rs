//! Shared parser and stream wrapper for provider Server-Sent Events.

use bytes::Bytes;
use futures::Stream;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

mod anthropic;
mod cohere;
mod databricks;
mod gemini;
mod openai;

pub use anthropic::AnthropicTransformer;
pub use cohere::CohereTransformer;
pub use databricks::DatabricksTransformer;
pub use gemini::GeminiTransformer;
pub use openai::OpenAICompatibleTransformer;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{ChatChunk, FinishReason};

#[derive(Debug, Clone, PartialEq)]
pub enum SSEEventType {
    Data,
    Event,
    Id,
    Retry,
    Comment,
}

#[derive(Debug, Clone)]
pub struct SSEEvent {
    pub event_type: Option<String>,
    pub data: String,
    pub id: Option<String>,
    pub retry: Option<u64>,
}

impl SSEEvent {
    pub fn from_line(line: &str) -> Option<Self> {
        if line.is_empty() || line.starts_with(':') {
            return None;
        }

        if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = line[colon_pos + 1..].trim_start();

            match field {
                "data" => Some(SSEEvent {
                    event_type: None,
                    data: value.to_string(),
                    id: None,
                    retry: None,
                }),
                "event" => Some(SSEEvent {
                    event_type: Some(value.to_string()),
                    data: String::new(),
                    id: None,
                    retry: None,
                }),
                "id" => Some(SSEEvent {
                    event_type: None,
                    data: String::new(),
                    id: Some(value.to_string()),
                    retry: None,
                }),
                "retry" => {
                    if let Ok(retry_ms) = value.parse::<u64>() {
                        Some(SSEEvent {
                            event_type: None,
                            data: String::new(),
                            id: None,
                            retry: Some(retry_ms),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

pub trait SSETransformer: Send + Sync {
    fn provider_name(&self) -> &'static str;

    fn is_end_marker(&self, data: &str) -> bool {
        data.trim() == "[DONE]"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError>;

    fn transform_stream_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        self.transform_chunk(data)
    }

    fn finish_stream(&self) -> Result<Option<ChatChunk>, ProviderError> {
        Ok(None)
    }

    fn parse_finish_reason(&self, reason: &str) -> Option<FinishReason> {
        match reason.to_ascii_lowercase().as_str() {
            "stop" | "end_turn" => Some(FinishReason::Stop),
            "length" | "max_tokens" => Some(FinishReason::Length),
            "tool_calls" | "function_call" | "tool_use" => Some(FinishReason::ToolCalls),
            "content_filter" | "safety" | "recitation" => Some(FinishReason::ContentFilter),
            "stop_sequence" => Some(FinishReason::StopSequence),
            "refusal" => Some(FinishReason::Refusal),
            "pause_turn" => Some(FinishReason::PauseTurn),
            _ => None,
        }
    }
}

pub struct UnifiedSSEParser<T: SSETransformer> {
    transformer: T,
    buffer: String,
    current_event: Option<SSEEvent>,
}

impl<T: SSETransformer> UnifiedSSEParser<T> {
    pub fn new(transformer: T) -> Self {
        Self {
            transformer,
            buffer: String::new(),
            current_event: None,
        }
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) -> Result<Vec<ChatChunk>, ProviderError> {
        self.process_bytes_with_mode(bytes, false)
    }

    fn process_stream_bytes(&mut self, bytes: &[u8]) -> Result<Vec<ChatChunk>, ProviderError> {
        self.process_bytes_with_mode(bytes, true)
    }

    fn process_bytes_with_mode(
        &mut self,
        bytes: &[u8],
        stream_mode: bool,
    ) -> Result<Vec<ChatChunk>, ProviderError> {
        let text = String::from_utf8_lossy(bytes);
        self.buffer.push_str(&text);

        let mut chunks = Vec::new();
        let last_newline = self.buffer.rfind('\n');

        if let Some(pos) = last_newline {
            let complete_part = self.buffer[..=pos].to_string();
            let incomplete_part = self.buffer[pos + 1..].to_string();
            self.buffer = incomplete_part;
            for line in complete_part.lines() {
                if let Some(chunk) = self.process_line(line, stream_mode)? {
                    chunks.push(chunk);
                }
            }
        }
        Ok(chunks)
    }

    fn process_line(
        &mut self,
        line: &str,
        stream_mode: bool,
    ) -> Result<Option<ChatChunk>, ProviderError> {
        if line.is_empty() {
            if let Some(event) = self.current_event.take() {
                return self.process_event(event, stream_mode);
            }
            return Ok(None);
        }

        if let Some(event) = SSEEvent::from_line(line) {
            if !event.data.is_empty() {
                if let Some(ref mut current) = self.current_event {
                    if !current.data.is_empty() {
                        current.data.push('\n');
                    }
                    current.data.push_str(&event.data);
                } else {
                    self.current_event = Some(event);
                }
            } else if event.event_type.is_some() || event.id.is_some() || event.retry.is_some() {
                if let Some(ref mut current) = self.current_event {
                    if event.event_type.is_some() {
                        current.event_type = event.event_type;
                    }
                    if event.id.is_some() {
                        current.id = event.id;
                    }
                    if event.retry.is_some() {
                        current.retry = event.retry;
                    }
                } else {
                    self.current_event = Some(event);
                }
            }
        }

        Ok(None)
    }

    fn process_event(
        &self,
        event: SSEEvent,
        stream_mode: bool,
    ) -> Result<Option<ChatChunk>, ProviderError> {
        if event.data.is_empty() {
            return Ok(None);
        }

        if self.transformer.is_end_marker(&event.data) {
            return if stream_mode {
                self.transformer.finish_stream()
            } else {
                Ok(None)
            };
        }

        if stream_mode {
            self.transformer.transform_stream_chunk(&event.data)
        } else {
            self.transformer.transform_chunk(&event.data)
        }
    }

    fn finish_stream(&mut self) -> Result<Vec<ChatChunk>, ProviderError> {
        let mut chunks = Vec::new();
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            chunks.extend(self.process_line(&line, true)?);
        }
        if let Some(event) = self.current_event.take() {
            chunks.extend(self.process_event(event, true)?);
        }
        if let Some(chunk) = self.transformer.finish_stream()? {
            chunks.push(chunk);
        }
        Ok(chunks)
    }
}

const MAX_CHUNK_BUFFER_SIZE: usize = 10_000;

pub struct UnifiedSSEStream<S, T>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin,
    T: SSETransformer + Clone,
{
    inner: S,
    parser: UnifiedSSEParser<T>,
    chunk_buffer: VecDeque<ChatChunk>,
    pending_error: Option<ProviderError>,
    finished: bool,
}

impl<S, T> UnifiedSSEStream<S, T>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin,
    T: SSETransformer + Clone,
{
    pub fn new(stream: S, transformer: T) -> Self {
        Self {
            inner: stream,
            parser: UnifiedSSEParser::new(transformer),
            chunk_buffer: VecDeque::new(),
            pending_error: None,
            finished: false,
        }
    }
}

impl<S, T> Stream for UnifiedSSEStream<S, T>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin,
    T: SSETransformer + Clone + Unpin,
{
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(chunk) = this.chunk_buffer.pop_front() {
            return Poll::Ready(Some(Ok(chunk)));
        }
        if let Some(error) = this.pending_error.take() {
            return Poll::Ready(Some(Err(error)));
        }
        if this.finished {
            return Poll::Ready(None);
        }

        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => match this.parser.process_stream_bytes(&bytes) {
                Ok(chunks) => {
                    if chunks.is_empty() {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    } else {
                        if this.chunk_buffer.len() + chunks.len() > MAX_CHUNK_BUFFER_SIZE {
                            return Poll::Ready(Some(Err(ProviderError::network(
                                this.parser.transformer.provider_name(),
                                format!(
                                    "SSE chunk buffer exceeded limit of {} chunks",
                                    MAX_CHUNK_BUFFER_SIZE
                                ),
                            ))));
                        }
                        this.chunk_buffer.extend(chunks);
                        if let Some(chunk) = this.chunk_buffer.pop_front() {
                            Poll::Ready(Some(Ok(chunk)))
                        } else {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                    }
                }
                Err(e) => Poll::Ready(Some(Err(e))),
            },
            Poll::Ready(Some(Err(error))) => {
                let error = ProviderError::network(
                    this.parser.transformer.provider_name(),
                    format!("Stream error: {error}"),
                );
                this.finished = true;
                match this.parser.finish_stream() {
                    Ok(chunks) if !chunks.is_empty() => {
                        this.chunk_buffer.extend(chunks);
                        this.pending_error = Some(error);
                        Poll::Ready(this.chunk_buffer.pop_front().map(Ok))
                    }
                    Ok(_) => Poll::Ready(Some(Err(error))),
                    Err(error) => Poll::Ready(Some(Err(error))),
                }
            }
            Poll::Ready(None) => {
                this.finished = true;
                match this.parser.finish_stream() {
                    Ok(chunks) => {
                        this.chunk_buffer.extend(chunks);
                        Poll::Ready(this.chunk_buffer.pop_front().map(Ok))
                    }
                    Err(error) => Poll::Ready(Some(Err(error))),
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub fn create_provider_sse_stream(
    response: reqwest::Response,
    provider_name: &'static str,
) -> Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>> {
    let transformer = OpenAICompatibleTransformer::new(provider_name);
    let stream = UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer);
    Box::pin(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_event_parsing() {
        let event = SSEEvent::from_line("data: test data").unwrap();
        assert_eq!(event.data, "test data");

        let event = SSEEvent::from_line("event: message").unwrap();
        assert_eq!(event.event_type, Some("message".to_string()));

        let event = SSEEvent::from_line("id: 123").unwrap();
        assert_eq!(event.id, Some("123".to_string()));

        let event = SSEEvent::from_line("retry: 5000").unwrap();
        assert_eq!(event.retry, Some(5000));

        // Comments should be ignored
        assert!(SSEEvent::from_line(": comment").is_none());
    }

    #[test]
    fn test_openai_transformer() {
        let transformer = OpenAICompatibleTransformer::new("test");

        // Test end marker
        assert!(transformer.is_end_marker("[DONE]"));
        assert!(!transformer.is_end_marker("data: {\"test\": 1}"));

        // Test JSON transformation
        let json_data = r#"{
            "id": "test-id",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello"},
                "finish_reason": null
            }]
        }"#;

        let result = transformer.transform_chunk(json_data).unwrap().unwrap();
        assert_eq!(result.id, "test-id");
        assert_eq!(result.model, "gpt-4");
        assert_eq!(result.choices[0].delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_openai_transformer_reasoning_content_to_thinking() {
        let transformer = OpenAICompatibleTransformer::new("test");

        let json_data = r#"{
            "id": "test-id-reasoning",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "Answer",
                    "reasoning_content": "chain-of-thought"
                },
                "finish_reason": null
            }]
        }"#;

        let result = transformer.transform_chunk(json_data).unwrap().unwrap();
        assert_eq!(
            result.choices[0]
                .delta
                .thinking
                .as_ref()
                .and_then(|t| t.content.as_ref())
                .map(String::as_str),
            Some("chain-of-thought")
        );
    }

    #[test]
    fn test_openai_transformer_reasoning_to_thinking() {
        let transformer = OpenAICompatibleTransformer::new("test");

        let json_data = r#"{
            "id": "test-id-reasoning",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "openai-reasoning",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "Answer",
                    "reasoning": "openai chain-of-thought"
                },
                "finish_reason": null
            }]
        }"#;

        let result = match transformer.transform_chunk(json_data) {
            Ok(Some(result)) => result,
            Ok(None) => panic!("SSE chunk should produce a result"),
            Err(error) => panic!("SSE chunk transformation should succeed: {error}"),
        };
        assert_eq!(
            result.choices[0]
                .delta
                .thinking
                .as_ref()
                .and_then(|t| t.content.as_ref())
                .map(String::as_str),
            Some("openai chain-of-thought")
        );
    }

    #[test]
    fn test_openai_transformer_empty_reasoning_content_falls_back_to_reasoning() {
        let transformer = OpenAICompatibleTransformer::new("test");

        let json_data = r#"{
            "id": "test-id-reasoning",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "openai-reasoning",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "Answer",
                    "reasoning_content": "",
                    "reasoning": "fallback chain-of-thought"
                },
                "finish_reason": null
            }]
        }"#;

        let result = match transformer.transform_chunk(json_data) {
            Ok(Some(result)) => result,
            Ok(None) => panic!("SSE chunk should produce a result"),
            Err(error) => panic!("SSE chunk transformation should succeed: {error}"),
        };
        assert_eq!(
            result.choices[0]
                .delta
                .thinking
                .as_ref()
                .and_then(|t| t.content.as_ref())
                .map(String::as_str),
            Some("fallback chain-of-thought")
        );
    }

    #[test]
    fn test_anthropic_stream_tool_use_deltas() {
        let transformer = AnthropicTransformer::new("claude-test");

        let start = transformer
            .transform_chunk(
                r#"{
                    "type": "content_block_start",
                    "index": 1,
                    "content_block": {
                        "type": "tool_use",
                        "id": "toolu_123",
                        "name": "get_weather",
                        "input": {}
                    }
                }"#,
            )
            .unwrap()
            .unwrap();
        let start_call = start.choices[0].delta.tool_calls.as_ref().unwrap()[0].clone();
        assert_eq!(start_call.index, 1);
        assert_eq!(start_call.id.as_deref(), Some("toolu_123"));
        assert_eq!(start_call.tool_type.as_deref(), Some("function"));
        assert_eq!(
            start_call.function.as_ref().and_then(|f| f.name.as_deref()),
            Some("get_weather")
        );
        assert_eq!(
            start_call
                .function
                .as_ref()
                .and_then(|f| f.arguments.as_deref()),
            None
        );

        let args = transformer
            .transform_chunk(
                r#"{
                    "type": "content_block_delta",
                    "index": 1,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": "{\"location\": \"San Fra"
                    }
                }"#,
            )
            .unwrap()
            .unwrap();
        let args_call = args.choices[0].delta.tool_calls.as_ref().unwrap()[0].clone();
        assert_eq!(args_call.index, 1);
        assert_eq!(
            args_call
                .function
                .as_ref()
                .and_then(|f| f.arguments.as_deref()),
            Some("{\"location\": \"San Fra")
        );

        let stop = transformer
            .transform_chunk(
                r#"{
                    "type": "message_delta",
                    "delta": {"stop_reason": "tool_use"},
                    "usage": {"input_tokens": 10, "output_tokens": 3}
                }"#,
            )
            .unwrap()
            .unwrap();
        assert_eq!(stop.choices[0].finish_reason, Some(FinishReason::ToolCalls));
    }

    #[test]
    fn test_anthropic_stream_thinking_deltas() {
        let transformer = AnthropicTransformer::new("claude-test");

        let start = transformer
            .transform_chunk(
                r#"{
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {"type": "thinking", "thinking": ""}
                }"#,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            start.choices[0]
                .delta
                .thinking
                .as_ref()
                .and_then(|thinking| thinking.is_start),
            Some(true)
        );

        let thinking = transformer
            .transform_chunk(
                r#"{
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {
                        "type": "thinking_delta",
                        "thinking": "Let me reason."
                    }
                }"#,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            thinking.choices[0]
                .delta
                .thinking
                .as_ref()
                .and_then(|thinking| thinking.content.as_deref()),
            Some("Let me reason.")
        );

        let signature = transformer
            .transform_chunk(
                r#"{
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {
                        "type": "signature_delta",
                        "signature": "sig_123"
                    }
                }"#,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            signature.choices[0]
                .delta
                .thinking
                .as_ref()
                .and_then(|thinking| thinking.signature.as_deref()),
            Some("sig_123")
        );
    }

    #[test]
    fn test_sse_parser_multiline() {
        let transformer = OpenAICompatibleTransformer::new("test");
        let mut parser = UnifiedSSEParser::new(transformer);

        // Simulate receiving data in chunks
        let chunk1 = b"data: {\"id\": \"test\", ";
        let chunk2 = b"\"choices\": [{\"delta\": {\"content\": \"Hi\"}, \"index\": 0}], ";
        let chunk3 = b"\"model\": \"gpt-4\", \"created\": 123}\n\n";

        let results1 = parser.process_bytes(chunk1).unwrap();
        assert!(results1.is_empty()); // Not complete yet

        let results2 = parser.process_bytes(chunk2).unwrap();
        assert!(results2.is_empty()); // Still not complete

        let results3 = parser.process_bytes(chunk3).unwrap();
        assert_eq!(results3.len(), 1); // Now we have a complete event
        assert_eq!(results3[0].choices[0].delta.content, Some("Hi".to_string()));
    }

    /// Verify the buffer size limit constant matches the documented value.
    #[test]
    fn test_max_chunk_buffer_size_constant() {
        assert_eq!(MAX_CHUNK_BUFFER_SIZE, 10_000);
    }

    /// Verify that delivering more than MAX_CHUNK_BUFFER_SIZE events in a single
    /// network read triggers the OOM guard and returns an error instead of
    /// growing the buffer without bound.
    #[tokio::test]
    async fn test_buffer_overflow_returns_error() {
        use futures::StreamExt;

        // Build one SSE payload that contains MAX_CHUNK_BUFFER_SIZE + 1 complete events.
        // Each line is: data: <valid JSON>\n\n
        let event_json = r#"{"id":"x","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"a"},"finish_reason":null}]}"#;
        let single_line = format!("data: {event_json}\n\n");
        let big_payload = single_line.repeat(MAX_CHUNK_BUFFER_SIZE + 1);

        // Wrap in a stream that yields one large Ok(Bytes) item.
        let mock_stream = futures::stream::iter(vec![Ok::<bytes::Bytes, reqwest::Error>(
            bytes::Bytes::from(big_payload),
        )]);

        let transformer = OpenAICompatibleTransformer::new("test");
        let mut sse_stream = UnifiedSSEStream::new(mock_stream, transformer);

        // Consume the stream until we hit the expected error.
        let mut got_overflow_error = false;
        while let Some(result) = sse_stream.next().await {
            if let Err(err) = result {
                let msg = format!("{err:?}");
                assert!(
                    msg.contains(&MAX_CHUNK_BUFFER_SIZE.to_string()),
                    "Expected buffer-limit message, got: {msg}"
                );
                got_overflow_error = true;
                break;
            }
        }

        assert!(
            got_overflow_error,
            "Stream should have returned a buffer-overflow error"
        );
    }

    #[test]
    fn test_default_parse_finish_reason_covers_anthropic_and_gemini() {
        let t = OpenAICompatibleTransformer::new("test");
        assert_eq!(t.parse_finish_reason("stop"), Some(FinishReason::Stop));
        assert_eq!(t.parse_finish_reason("STOP"), Some(FinishReason::Stop));
        assert_eq!(t.parse_finish_reason("end_turn"), Some(FinishReason::Stop));
        assert_eq!(t.parse_finish_reason("length"), Some(FinishReason::Length));
        assert_eq!(
            t.parse_finish_reason("max_tokens"),
            Some(FinishReason::Length)
        );
        assert_eq!(
            t.parse_finish_reason("MAX_TOKENS"),
            Some(FinishReason::Length)
        );
        assert_eq!(
            t.parse_finish_reason("tool_calls"),
            Some(FinishReason::ToolCalls)
        );
        assert_eq!(
            t.parse_finish_reason("tool_use"),
            Some(FinishReason::ToolCalls)
        );
        assert_eq!(
            t.parse_finish_reason("content_filter"),
            Some(FinishReason::ContentFilter)
        );
        assert_eq!(
            t.parse_finish_reason("safety"),
            Some(FinishReason::ContentFilter)
        );
        assert_eq!(
            t.parse_finish_reason("SAFETY"),
            Some(FinishReason::ContentFilter)
        );
        assert_eq!(
            t.parse_finish_reason("recitation"),
            Some(FinishReason::ContentFilter)
        );
        assert_eq!(
            t.parse_finish_reason("RECITATION"),
            Some(FinishReason::ContentFilter)
        );
        assert_eq!(
            t.parse_finish_reason("stop_sequence"),
            Some(FinishReason::StopSequence)
        );
        assert_eq!(
            t.parse_finish_reason("refusal"),
            Some(FinishReason::Refusal)
        );
        assert_eq!(
            t.parse_finish_reason("pause_turn"),
            Some(FinishReason::PauseTurn)
        );
        assert_eq!(t.parse_finish_reason("not_a_real_reason"), None);
    }
}
