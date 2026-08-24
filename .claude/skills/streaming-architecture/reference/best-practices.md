## Best Practices

### 1. Expect arbitrary SSE line/event boundaries, with a UTF-8 caveat

The parser retains the incomplete tail after the last `\n` across
`process_bytes` calls. Never assume reads are event-aligned, and never
pre-split SSE frames yourself outside the parser. However, each read is
decoded independently with `String::from_utf8_lossy`; a network boundary that
splits one multibyte UTF-8 code point is replaced rather than reconstructed.
The current implementation therefore handles arbitrary ASCII/line/event
boundaries, but does not faithfully handle every possible raw byte boundary.

```rust
// src/core/providers/base/sse.rs, test_sse_parser_multiline
let results1 = parser.process_bytes(chunk1)?; // partial JSON line: buffered
let results3 = parser.process_bytes(chunk3)?; // complete event emitted here
```

### 2. Treat transformers as per-stream state

`AnthropicTransformer` tracks the message id (`Mutex<Option<String>>`) and
tool-name map; `GeminiTransformer` tracks deferred usage and seen tool-call
candidates. `Clone` deliberately resets that state (Anthropic's clone sets
`message_id` to `None`; Gemini rebuilds via `with_usage_policy`), so give each
stream its own transformer instance instead of sharing one across concurrent
streams.

### 3. Map failures to the right ProviderError constructor

```rust
// malformed payload -> src/core/providers/unified_provider_methods.rs
ProviderError::response_parsing(provider, msg)
// in-band error object (gemini.rs)
ProviderError::api_error(provider, status, msg)
// in-band error event (anthropic.rs)
ProviderError::streaming_error(provider, "chat", None, None, msg)
```

Transport errors need no wrapping in transformers — `UnifiedSSEStream`
converts reqwest errors via `ProviderError::network` itself.

### 4. Never drop a chunk over auxiliary-field failures

Follow `openai.rs`: malformed `logprobs` or `usage` are logged with
`tracing::error!` and the field dropped while the chunk still flows. Usage
feeds billing; failing the whole chunk would lose content too.

### 5. Keep cross-chunk state in the transformer

Wire-format parsing belongs in `transform_chunk`; anything spanning chunks —
message ids (anthropic), generated response ids like
`format!("chatcmpl-{}", uuid)` (cohere), deferred usage finalization
(gemini) — lives as transformer state so the parser stays provider-generic.

### 6. Respect existing bounds

The chunk queue caps at `MAX_CHUNK_BUFFER_SIZE` (10_000) and server routes use
a bounded channel; do not introduce unbounded buffering in front of either.
