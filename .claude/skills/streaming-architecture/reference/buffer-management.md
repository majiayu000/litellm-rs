## Buffer Management

Streaming uses two buffers with different jobs.

### Parser line buffer (`UnifiedSSEParser.buffer`)

```rust
// src/core/providers/base/sse.rs
pub struct UnifiedSSEParser<T: SSETransformer> {
    transformer: T,
    buffer: String,
    current_event: Option<SSEEvent>,
}
```

- `process_bytes_with_mode` lossily decodes each incoming read into the
  buffer, splits at `rfind('\n')`, processes the complete prefix line by line,
  and keeps the incomplete text tail for the next call. SSE line/event splits
  assemble correctly (see `test_sse_parser_multiline`), but a network boundary
  inside a multibyte UTF-8 code point does not: per-read
  `String::from_utf8_lossy` replaces the partial byte sequence before it can be
  joined with the next read.
- There is no size cap on this buffer; a provider emitting an unterminated
  line would grow it.
- No public reset API exists. The private `finish_stream` drains leftovers via
  `std::mem::take`.

### Chunk queue overflow guard (`UnifiedSSEStream.chunk_buffer`)

```rust
// src/core/providers/base/sse.rs
const MAX_CHUNK_BUFFER_SIZE: usize = 10_000;

chunk_buffer: VecDeque<ChatChunk>,
```

- Batches the chunks produced by a single upstream read and drains them FIFO
  via `pop_front` before polling the network again.
- If existing plus new chunks would exceed 10_000, the stream yields
  `Err(ProviderError::network(provider, "SSE chunk buffer exceeded limit of 10000 chunks"))`
  instead of growing without bound.
- Guarded by `test_max_chunk_buffer_size_constant` and
  `test_buffer_overflow_returns_error` in `sse.rs`.

### Backpressure

- When an upstream read yields zero complete chunks, `poll_next` re-arms
  itself with `cx.waker().wake_by_ref()` and returns `Pending`.
- Server routes hand frames to a bounded `mpsc::channel::<Bytes>(8)`; actix
  drains that channel through `HttpResponse::streaming`, so slow clients
  apply backpressure at the route task boundary.
