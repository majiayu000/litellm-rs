## Contents

- Key Functions
- Key Format and Schema Version
- Canonicalization Policy
- CacheKeyBuilder

## Key Functions

Key generation uses free functions in src/core/cache/key_generator.rs — there is no `CacheKeyGenerator` struct. All return `CacheKey` (src/core/cache/types.rs:14, a string plus a pre-computed u64 hash):

```rust
pub fn generate_chat_key(request: &ChatCompletionRequest) -> CacheKey;
pub fn generate_chat_key_with_user(request: &ChatCompletionRequest, user_id: Option<&str>) -> CacheKey;
pub fn generate_embedding_key(request: &EmbeddingRequest) -> CacheKey;
pub fn generate_embedding_key_with_user(request: &EmbeddingRequest, user_id: Option<&str>) -> CacheKey;

// Generic helpers for other payloads
pub fn generate_key_from_json<T: Serialize>(prefix: &str, request: &T) -> CacheKey;
pub fn generate_key_from_content(prefix: &str, content: &str) -> CacheKey;
pub fn generate_key_from_parts(prefix: &str, parts: &[&str]) -> CacheKey;
```

The chat key hashes a JSON payload containing `model`, `messages`, `temperature`,
`max_tokens`, `max_completion_tokens`, `top_p`, `n`, `stop`, penalties, `logit_bias`,
`functions`/`function_call`, `tools`/`tool_choice`, `parallel_tool_calls`,
`response_format`, `seed`, logprobs, `modalities`, `audio`, `reasoning_effort`,
`service_tier`, `prediction`, `safety_settings`, `cache_control`, `extra_body`, and the
separate authenticated-caller `user_id` argument (`key_generator.rs:33`). It does **not**
hash `ChatCompletionRequest.user`, even though that field is forwarded to providers (for
example, as Anthropic `metadata.user_id`). Requests from the same authenticated caller
that differ only in the client-supplied `user` therefore share a cache key. The embedding
key likewise hashes `model` + `input` + its separate optional `user_id` argument, not
`EmbeddingRequest.user`; the wired `LLMCache` embedding methods call
`generate_embedding_key`, so that argument is currently `None`.

Prefix constants (key_generator.rs:14): `CHAT_KEY_PREFIX = "chat"`, `EMBEDDING_KEY_PREFIX = "embed"`, `COMPLETION_KEY_PREFIX = "completion"`.

## Key Format and Schema Version

`versioned_key(prefix, namespace, digest)` in src/core/cache/key_policy.rs:58 produces:

```
{prefix}:{namespace}:{CACHE_KEY_SCHEMA_VERSION}:{sha256-hex}
chat:gpt-4:v4:9f2c...        # namespace = model when present
embed:text-embedding-ada-002:v4:ab13...  # embedding namespace = model
```

`CACHE_KEY_SCHEMA_VERSION` is `"v4"` (key_policy.rs:11). Bumping it cold-starts all existing entries so responses cached under an older key policy are never reused.

## Canonicalization Policy

`stable_digest_value(value)` (key_policy.rs:20) digests `canonical_json_string(value)` with SHA-256. Canonicalization (`canonicalize_json_value`, key_policy.rs:73):

- Sorts object keys recursively so field order never changes the digest.
- Drops non-deterministic fields listed by `is_non_deterministic_field`: `timestamp`, `request_id`, `trace_id`, `span_id`, `created_at`, `updated_at`, `id`, `stream`, `stream_options`.
- The drop applies only at the top level or directly inside `extra_body` (`should_exclude_field`) — an `id` nested inside a tool's JSON-schema properties is preserved, keeping distinct schemas distinct.

## CacheKeyBuilder

For custom keys without hand-rolled string concatenation (key_generator.rs:154):

```rust
let key = CacheKeyBuilder::new("test")   // prefix
    .with_part("part1")                  // required string part
    .add_optional(Some("part2"))         // included only if Some
    .add_num(123)                        // any Display value
    .build();                            // hashed like the free functions

CacheKeyBuilder::new("p").with_part("a").build_explicit() // "p:a", no hashing
```
