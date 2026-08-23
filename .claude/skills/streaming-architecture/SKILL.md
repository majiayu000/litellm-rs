---
name: streaming-architecture
description: LiteLLM-RS Streaming Architecture. Covers UnifiedSSEParser line buffering, the SSETransformer trait, UnifiedSSEStream backpressure and overflow guarding, provider-specific transformers, and server-side SSE emission. Use when debugging SSE parsing, writing or modifying a provider stream transformer, wiring the stream processing pipeline, or tuning the stream idle timeout.
---

# Streaming Architecture Guide

## Overview

Provider streaming lives in `src/core/providers/base/sse.rs` plus per-provider
transformers under `src/core/providers/base/sse/` (`openai.rs`, `anthropic.rs`,
`gemini.rs`, `cohere.rs`, `databricks.rs`). The layer consumes a provider's raw
SSE byte stream and yields `Result<ChatChunk, ProviderError>` items in an
OpenAI-compatible shape, so the server routes never see provider-specific
formats.

### Streaming Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                  Provider SSE byte stream                       │
│  reqwest::Response::bytes_stream()                              │
│  (OpenAI, Anthropic, Google, ...)                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  UnifiedSSEStream<S, T>                         │
│  - polls upstream bytes, feeds UnifiedSSEParser                 │
│  - chunk_buffer: VecDeque<ChatChunk>, capped at 10_000          │
│  - Item = Result<ChatChunk, ProviderError>                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  UnifiedSSEParser<T>                            │
│  - String line buffer (incomplete tail retained across reads)   │
│  - SSEEvent field parsing, multi-line data joining              │
│  - end-marker / finish_stream dispatch                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  SSETransformer (per provider)                  │
│  - transform_chunk / transform_stream_chunk                     │
│  - normalizes wire format to ChatChunk                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Server route re-serialization                  │
│  ChatChunk -> SSE frames ("data: {...}\n\n") + final [DONE]     │
└─────────────────────────────────────────────────────────────────┘
```

The parser owns its transformer: `UnifiedSSEParser<T: SSETransformer>` calls
back into `T` while parsing, so there is no separate processing stage between
parser and transformer.

---

## Core Components

### SSEEvent

```rust
// src/core/providers/base/sse.rs
#[derive(Debug, Clone)]
pub struct SSEEvent {
    pub event_type: Option<String>,
    pub data: String,
    pub id: Option<String>,
    pub retry: Option<u64>,
}
```

`SSEEvent::from_line(&str) -> Option<SSEEvent>` parses one SSE field line:

- Empty lines and `:` comment lines return `None`.
- `data`, `event`, `id`, and `retry` set the matching field; whitespace after
  the colon is trimmed.
- `retry` must parse as `u64`, otherwise `None`; unknown fields return `None`.

The parser accumulates multiple `data` lines of one event, joining them with
`\n`, and dispatches on the blank line that terminates the event.

### SSETransformer Trait

```rust
// src/core/providers/base/sse.rs
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

    fn parse_finish_reason(&self, reason: &str) -> Option<FinishReason> { ... }
}
```

- Errors are `ProviderError`
  (`crate::core::providers::unified_provider::ProviderError`). There is no
  dedicated `StreamError` enum.
- The default `parse_finish_reason` maps case-insensitively:
  `stop|end_turn` -> Stop, `length|max_tokens` -> Length,
  `tool_calls|function_call|tool_use` -> ToolCalls,
  `content_filter|safety|recitation` -> ContentFilter,
  `stop_sequence` -> StopSequence, `refusal` -> Refusal,
  `pause_turn` -> PauseTurn; unknown strings yield `None`.
- Built-in implementations: `OpenAICompatibleTransformer`,
  `AnthropicTransformer`, `GeminiTransformer`, `CohereTransformer`,
  `DatabricksTransformer` (see
  [reference/provider-transformers.md](reference/provider-transformers.md)).

### UnifiedSSEParser\<T\>

```rust
// src/core/providers/base/sse.rs
pub struct UnifiedSSEParser<T: SSETransformer> {
    transformer: T,
    buffer: String,
    current_event: Option<SSEEvent>,
}

impl<T: SSETransformer> UnifiedSSEParser<T> {
    pub fn new(transformer: T) -> Self;
    pub fn process_bytes(&mut self, bytes: &[u8]) -> Result<Vec<ChatChunk>, ProviderError>;
}
```

- The buffer is a `String`, not a byte deque. Bytes are lossily decoded and
  appended; only text up to the last `\n` is processed and the incomplete tail
  stays buffered for the next call.
- `process_bytes` runs non-stream mode: an end marker produces nothing and
  events go through `transform_chunk`.
- `UnifiedSSEStream` drives the private `process_stream_bytes` path (stream
  mode): an end marker triggers `transformer.finish_stream()` instead, and data
  goes through `transform_stream_chunk`.
- No size cap applies to this buffer.
- The private `finish_stream` flushes any leftover partial line and pending
  event, then appends `transformer.finish_stream()` output.

### UnifiedSSEStream\<S, T\>

```rust
// src/core/providers/base/sse.rs
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
```

`poll_next` order: pop `chunk_buffer`, then take `pending_error`, then return
`None` once `finished`, otherwise poll `inner` and feed bytes through
`process_stream_bytes`.

- A read that yields zero complete chunks stores nothing; the stream returns
  `Pending` after `cx.waker().wake_by_ref()`.
- If buffered plus new chunks would exceed `MAX_CHUNK_BUFFER_SIZE` (10_000),
  it yields `Err(ProviderError::network(...))` instead of growing unboundedly.
- Transport errors are wrapped as
  `ProviderError::network(provider, format!("Stream error: {error}"))`; chunks
  drained from `parser.finish_stream()` are emitted before the error item.
- Upstream end-of-stream sets `finished` and drains `parser.finish_stream()`
  before returning `None`.

Helper `create_provider_sse_stream(response, provider_name)` boxes
`response.bytes_stream()` behind an `OpenAICompatibleTransformer`.

---

## References

- [reference/provider-transformers.md](reference/provider-transformers.md) — behavior of the OpenAI-compatible, Anthropic, Gemini, Cohere, and Databricks transformers
- [reference/stream-pipeline.md](reference/stream-pipeline.md) — UnifiedSSEStream pipeline internals, provider wiring pattern, and actix HTTP response streaming
- [reference/buffer-management.md](reference/buffer-management.md) — parser line buffer retention and chunk-buffer overflow guard
- [reference/configuration.md](reference/configuration.md) — `server.stream_idle_timeout` and fixed buffering constants
- [reference/best-practices.md](reference/best-practices.md) — incremental parsing, per-stream state, error mapping, usage handling
