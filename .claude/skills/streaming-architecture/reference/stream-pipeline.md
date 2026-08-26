## Contents

- Parser Modes and Event Dispatch
- Provider Wiring
- HTTP Response Streaming

## Parser Modes and Event Dispatch

`UnifiedSSEParser<T>` has two entry paths that share one line-processing core:

```rust
// src/core/providers/base/sse.rs
pub fn process_bytes(&mut self, bytes: &[u8]) -> Result<Vec<ChatChunk>, ProviderError>;
fn process_stream_bytes(&mut self, bytes: &[u8]) -> Result<Vec<ChatChunk>, ProviderError>; // private
```

Per line: `SSEEvent::from_line` parses fields; multiple `data` lines of an
event join with `\n`; a blank line dispatches the accumulated event through
`process_event`, which:

1. Returns nothing for empty data.
2. On an end marker (`transformer.is_end_marker`): stream mode calls
   `transformer.finish_stream()`; non-stream mode returns nothing.
3. Otherwise calls `transform_stream_chunk` (stream mode) or
   `transform_chunk` (non-stream mode).

## Provider Wiring

Providers wrap a reqwest response body in `UnifiedSSEStream`. Real example
from `src/core/providers/openai/streaming.rs`:

```rust
use crate::core::providers::base::sse::{OpenAICompatibleTransformer, UnifiedSSEStream};

pub type OpenAIStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    OpenAICompatibleTransformer,
>;

pub fn create_openai_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> OpenAIStream {
    let transformer = OpenAICompatibleTransformer::new("openai");
    UnifiedSSEStream::new(Box::pin(stream), transformer)
}
```

The same pattern appears across providers (`anthropic/streaming.rs`,
`gemini/streaming.rs`, `cohere/streaming.rs`, ...):
`UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer)`.
For plain OpenAI-compatible responses,
`create_provider_sse_stream(response, provider_name)` in
`src/core/providers/base/sse.rs` builds the boxed stream with an
`OpenAICompatibleTransformer` in one call.

### poll_next semantics

```rust
type Item = Result<ChatChunk, ProviderError>;
```

- Buffered chunks drain first (`chunk_buffer.pop_front()`), then any
  `pending_error`.
- A network read whose chunks plus buffered chunks would exceed
  `MAX_CHUNK_BUFFER_SIZE` fails with `ProviderError::network`.
- An upstream `Err` is wrapped as
  `ProviderError::network(provider, format!("Stream error: {error}"))`; any
  chunks from `parser.finish_stream()` flush before the error surfaces.
- Upstream `None` drains `parser.finish_stream()` (leftover partial line,
  pending event, transformer tail chunk) before ending the stream.

## HTTP Response Streaming

Server routes convert provider chunks back into SSE frames for clients. The
canonical path is `src/server/routes/ai/chat_streaming.rs`
(`completions_streaming.rs` and `responses_stream.rs` follow the same shape):

```rust
let (tx, rx) = mpsc::channel::<Bytes>(8);
let idle_timeout_secs = state.config.load().gateway.server.stream_idle_timeout;

tokio::spawn(async move {
    // loop over stream.next(), guarded by tokio::time::timeout when
    // idle_timeout_secs > 0; select on tx.closed() to detect disconnects
});

let sse_stream =
    tokio_stream::wrappers::ReceiverStream::new(rx).map(Ok::<_, actix_web::error::Error>);

Ok(HttpResponse::Ok()
    .insert_header((CONTENT_TYPE, "text/event-stream"))
    .insert_header((CACHE_CONTROL, "no-cache"))
    .insert_header(("Connection", "keep-alive"))
    .insert_header(("X-Request-ID", context.request_id.as_str()))
    .streaming(sse_stream))
```

Inside the spawned task, per upstream item:

- `Ok(chunk)`: usage is captured as it passes; empty chunks (no choices, no
  usage) are skipped; `convert_core_chunk_to_streaming`
  (`src/server/routes/ai/chat.rs`) converts the chunk and frames go to `tx`.
- `Err(e)`: classified by `sse_error_classification` and sent as an SSE error
  frame built by `format_sse_error`
  (both in `src/server/routes/ai/chat_sse.rs`).
- Idle timeout expiry sends a timeout error frame, records
  `ProviderError::timeout`, and closes the stream.
- After the loop, the terminal marker is emitted:
  `Event::default().data("[DONE]").to_bytes()`
  (`Event` from `crate::core::streaming::types`).

There is no generic `StreamProcessor` pipeline type — `UnifiedSSEStream` is
the only stream adapter between the provider HTTP body and the route task.
