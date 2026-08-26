## Contents

- OpenAICompatibleTransformer
- AnthropicTransformer
- GeminiTransformer
- CohereTransformer
- DatabricksTransformer

## OpenAICompatibleTransformer

Source: `src/core/providers/base/sse/openai.rs`. The reusable workhorse: any
OpenAI-compatible upstream (Tier 1 catalog providers, Azure, Mistral, ...)
streams through it with a different `provider` label.

```rust
pub struct OpenAICompatibleTransformer {
    provider: &'static str,
}

impl OpenAICompatibleTransformer {
    pub fn new(provider: &'static str) -> Self;
}
```

`transform_chunk` behavior:

- JSON must parse and `choices` must be an array, otherwise
  `ProviderError::response_parsing`.
- Defaults for missing fields: `id` -> `"stream-chunk"`, `model` ->
  `"unknown"`, `created` -> current timestamp; `system_fingerprint` passes
  through when present.
- Each choice's `delta` deserializes into `ChatDelta`; a missing `delta` is a
  `ProviderError::response_parsing`.
- A non-empty `reasoning_content` (preferred) or `reasoning` string in the
  delta is mapped to `delta.thinking = Some(ThinkingDelta { content, ..Default::default() })`
  (DeepSeek/OpenAI reasoning models).
- Choice `index` prefers the upstream value and falls back to array position —
  with `n > 1`, each chunk carries one choice with its real index.
- Malformed `logprobs` or `usage` never fails the chunk: the field is logged
  via `tracing::error!` and dropped while the chunk still flows.

## AnthropicTransformer

Source: `src/core/providers/base/sse/anthropic.rs`. Handles Anthropic's
event-based protocol (the `type` field of each JSON payload).

```rust
pub struct AnthropicTransformer {
    model: String,
    tool_name_map: HashMap<String, String>,
    message_id: Mutex<Option<String>>,
}

impl AnthropicTransformer {
    pub fn new(model: impl Into<String>) -> Self;
    pub fn with_tool_name_map(mut self, tool_name_map: HashMap<String, String>) -> Self;
}
```

`Clone` resets `message_id` to `None`, so each cloned stream keeps independent
message ids.

Event handling by `type`:

| Event | Behavior |
|-------|----------|
| `message_start` | Stores `message.id`; emits a chunk with `role: Assistant` carrying that id |
| `content_block_start` | `tool_use`: emits `ToolCallDelta` with name restored through `tool_name_map` and non-empty `input` serialized as arguments. `thinking`/`redacted_thinking`: emits `ThinkingDelta::start()`. `text` and unknown blocks yield nothing |
| `content_block_delta` | `text_delta` -> `delta.content`; `input_json_delta` -> `partial_json` becomes tool-call arguments; `thinking_delta` -> thinking content; `signature_delta` -> thinking signature |
| `message_delta` | Maps `delta.stop_reason` (`end_turn`->Stop, `max_tokens`->Length, `tool_use`->ToolCalls, plus `stop_sequence`/`refusal`/`pause_turn`) and builds `Usage` from `input_tokens`/`output_tokens`, folding cache token counts into `PromptTokensDetails` |
| `message_stop` | Emits an empty-choices chunk carrying the stored message id |
| `error` | Returns `ProviderError::streaming_error("anthropic", "chat", None, None, message)` |
| `content_block_stop`, `ping` | Ignored |

Unknown event types log a warning and are skipped.

## GeminiTransformer

Source: `src/core/providers/base/sse/gemini.rs`. Handles the
candidates/parts streaming format plus strict usage accounting.

```rust
pub struct GeminiTransformer {
    provider: &'static str,
    model: String,
    chunk_id: String, // format!("gemini-stream-{}", nanos)
    usage_policy: Option<GeminiUsagePolicy>,
    stream_usage: Arc<Mutex<GeminiStreamUsage>>,
    tool_call_candidates: Arc<Mutex<HashSet<u32>>>,
}

impl GeminiTransformer {
    pub fn new(model: impl Into<String>) -> Self;
}
```

`new` targets provider `"gemini"` with direct-API usage policy;
`new_vertex` (feature-gated) targets `"vertex_ai"`. `Clone` rebuilds usage and
tool-call state instead of sharing it.

Behavior:

- `transform_chunk` parses `candidates[].content.parts[]`, joining all text
  parts per candidate; tool calls are extracted via the shared
  `google_tool_loop` helpers (`candidate_index`, `parse_function_call_parts`),
  preferring the upstream candidate index over array position.
- An in-band `error` object returns
  `ProviderError::api_error("gemini", code, message)`.
- Usage is governed by a small state machine (`GeminiStreamUsage`: Missing /
  Valid / Invalid / Finalized). In stream mode (`transform_stream_chunk`
  override) usage is suppressed on data chunks and recorded instead; chunks
  with no choices are skipped.
- `finish_stream` override emits exactly one final usage-only chunk after the
  stream ends (empty chunk when observed usage was invalid).

## CohereTransformer

Source: `src/core/providers/base/sse/cohere.rs`. Supports both API versions.

```rust
pub struct CohereTransformer {
    model: String,
    response_id: String, // format!("chatcmpl-{}", uuid)
    use_v2: bool,
}

impl CohereTransformer {
    pub fn new(model: impl Into<String>, use_v2: bool) -> Self;
}
```

The event name is read from the payload's `type` field, falling back to
`event`.

- v2 events: `content-delta` extracts
  `delta.message.content.text` (or a bare string content); `message-end`
  carries finish reason and optional `usage.tokens`. All other events
  (`message-start`, `content-start`, `content-end`, `tool-call-*`,
  `citation-*`) are skipped.
- v1 events: `text-generation` (payload `text`) and `stream-end`
  (top-level `finish_reason`); others skipped.

Finish reasons normalize through `parse_cohere_finish_reason`
(`stop|complete|end_turn` -> Stop, `length|max_tokens` -> Length,
`tool_calls|tool_use` -> ToolCalls, `content_filter` -> ContentFilter).

## DatabricksTransformer

Source: `src/core/providers/base/sse/databricks.rs`. A unit struct over the
OpenAI-compatible shape with one extension:

- `delta.content` may be a string or an array of `{ "text": ... }` blocks
  (Claude-style array content); array items are concatenated into one string.

Everything else follows the OpenAI-compatible shape with its own defaults:
missing `id` becomes `"chunk"`, missing `model` stays empty, missing `created`
becomes the current timestamp, choice `index` falls back to `0`, and finish
reasons go through the trait-default `parse_finish_reason`.
