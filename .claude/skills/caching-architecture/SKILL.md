---
name: caching-architecture
description: LiteLLM-RS response caching architecture. Covers the two-tier deterministic cache (L1 in-memory + optional L2 Redis) behind LLMCache and DualCache, SHA-256 cache key generation with schema versioning, TTL and eviction policy, request-path wiring for chat completions and embeddings, cache statistics, and admin endpoints. Use when adding or tuning gateway response caching — cache keys, tiers, TTLs or eviction, cache metrics, or invalidation.
---

# Caching Architecture Guide

## Overview

LiteLLM-RS ships exactly one wired caching subsystem: an **exact-match response cache** for non-streaming chat completions and embeddings. It is a two-tier read-through cache, not a three-tier stack — semantic (vector) caching exists as a deprecated, unwired module (see below).

```
Request (non-streaming /v1/chat/completions, /v1/embeddings)
     │ lookup_chat / lookup_embedding (src/server/routes/ai/response_cache.rs)
     ▼
┌────────────────────────────────────────────────────────────┐
│ LLMCache  (src/core/cache/llm_cache.rs)                    │
│   chat_cache:      DualCache<CachedChatResponse>           │
│   embedding_cache: DualCache<CachedEmbeddingResponse>      │
└────────────────────────────────────────────────────────────┘
     │ per-key get / set
     ▼
┌────────────────────────────────────────────────────────────┐
│ DualCache<T>  (src/core/cache/dual.rs)                     │
│   L1  InMemoryCache<T> — DashMap, TTL, sampled eviction    │
│   L2  RedisCache<T>    — optional, backed by RedisPool     │
│   Read: L1 miss → L2 hit → repopulate L1                   │
│   Write: both tiers; L2 failure logs a warning, not fatal  │
└────────────────────────────────────────────────────────────┘
     │ miss
     ▼
LLM Provider → response stored back into both tiers
```

### What Is Wired vs Not

| Capability | Status |
|---|---|
| Exact-match response cache (chat + embeddings) | Wired: `AppState.response_cache`, built by `build_response_cache` (src/server/state.rs:143) |
| Semantic similarity cache | Not wired: `cache.semantic_cache: true` fails startup validation (src/config/validation/cache_validators.rs:16); `core::semantic_cache` is deprecated since 0.6.0, removal planned in 0.7.0 (src/core/semantic_cache/mod.rs) |
| Vector DB backends | Storage-only: `QdrantStore` implemented; weaviate/pinecone declared but return "not implemented yet" (src/storage/vector/backend.rs:29). Nothing connects them to caching at runtime |
| Cloud object-storage caches | `core::cache::cloud` (`CloudCache` trait; S3/GCS/Azure under feature `s3`) — not part of the request path |

---

## Configuration

```yaml
cache:
  enabled: true               # default false; requires ttl > 0
  ttl: 3600                   # seconds; applied to chat AND embedding entries
  max_size: 1000              # max entries per in-memory layer
  semantic_cache: false       # must stay false — true fails startup validation
  similarity_threshold: 0.95  # parsed but unused while semantic cache is unwired
```

These are the only five fields (`src/config/models/cache.rs:9`, `deny_unknown_fields`). There is no `l1`/`l2`/`l3` block, `redis_url`, `prefix`, `exclude_models`, or `skip_streaming` key.

- Redis is not configured here: the cache reuses the gateway's Redis pool. Without a pool it runs memory-only (`CacheMode::MemoryOnly`).
- `enabled: true` with `ttl: 0` logs an error and leaves the cache off (src/server/state.rs:148).
- Validation rejects `enabled: true, ttl: 0` outright (src/config/validation/cache_validators.rs:12).

---

## Request Flow

1. `POST /v1/chat/completions` calls `lookup_chat` before routing (src/server/routes/ai/chat.rs:112). A hit passes `ensure_chat_cache_pricing_gate` and returns immediately.
2. On a miss, the provider executes and `store_chat` writes the response (chat.rs:261). Embeddings do the same via `lookup_embedding` / `store_embedding` (src/server/routes/ai/embeddings.rs:98,283).
3. Chat lookups and stores are skipped when the request carries a per-key budget, sets `store: true`, or was marked bypassed by an upstream handler (`should_bypass_chat_cache`, src/server/routes/ai/response_cache.rs:25). Embeddings have no such bypass conditions.
4. Chat entries are scoped per caller: identity is `api_key:{id}` or `user:{id}`, optionally suffixed `:max_tokens_per_request:{limit}` (`cache_identity`, response_cache.rs:46). The key does not hash the separate client-supplied `ChatCompletionRequest.user`, so two requests from the same caller that differ only in that provider-facing field collide. Embedding entries are currently shared across callers: the route copies the identity into `EmbeddingRequest.user`, but `LLMCache` calls `generate_embedding_key` with no `user_id`, and that key does not hash `request.user`. Identical model/input embeddings therefore reuse one entry. Streaming chat requests are never cached (`LLMCache::get_chat_response_with_user`, src/core/cache/llm_cache.rs:280).
5. Cache lookup errors are logged and treated as misses. Gateway startup constructs only
   `Dual` or `MemoryOnly` caches, and `Dual` suppresses Redis L2 write failures. A
   programmatically installed `RedisOnly` `LLMCache` differs: Redis store errors propagate
   through `store_chat` / `store_embedding`, so the route returns an error after the
   provider call succeeded.

---

## Best Practices

### 1. Cache Key Determinism

Keys come from free functions, not a generator struct. They hash a canonical-JSON payload (sorted keys, transport fields stripped) with SHA-256 under schema version `v4`:

```rust
use litellm_rs::core::cache::{generate_chat_key, generate_chat_key_with_user};

let key = generate_chat_key(&request);                        // chat:gpt-4:v4:<64-hex>
let key = generate_chat_key_with_user(&request, Some(user));  // user-scoped variant
```

Do not add non-deterministic fields (`request_id`, `stream`, timestamps) — `canonical_json_string` already strips the known ones at the top level and inside `extra_body`. Details: [reference/cache-key-generation.md](reference/cache-key-generation.md).

### 2. TTL Strategy

```rust
// LLMCacheConfig::default() (src/core/cache/llm_cache.rs:55)
chat_ttl: Duration::from_secs(3600),       // 1 hour
embedding_ttl: Duration::from_secs(86400), // 24 hours — embeddings are deterministic
```

At startup `build_response_cache` overrides both from `cache.ttl`, so per-tier TTL tuning requires code changes, not YAML.

### 3. Skip Caching — the Real Rules

The deterministic path already enforces its own skips; do not re-implement them:

```rust
// src/core/cache/llm_cache.rs:280 — streaming requests are never cached
if request.stream.unwrap_or(false) {
    return Ok(None);
}

// src/server/routes/ai/response_cache.rs:25 — chat-only bypasses
fn should_bypass_chat_cache(request: &ChatCompletionRequest, context: &RequestContext) -> bool {
    context.metadata.get(BYPASS_CHAT_RESPONSE_CACHE_KEY).and_then(|v| v.as_bool()).unwrap_or(false)
        || context.api_key_budget_id().is_some()
        || request.store == Some(true)
}
```

The temperature/tools-based filtering you may find in `src/core/semantic_cache/validation.rs` (`should_cache_request`) belongs to the deprecated semantic cache and has no runtime effect.

---

## References

- [reference/response-cache.md](reference/response-cache.md) — LLMCache + DualCache composition, startup wiring, read/write paths, admin endpoints.
- [reference/cache-key-generation.md](reference/cache-key-generation.md) — Key functions, `v4` key format, canonicalization policy, `CacheKeyBuilder`.
- [reference/in-memory-cache.md](reference/in-memory-cache.md) — L1 `InMemoryCache<T>`: DashMap storage, TTL, sampled eviction, cleanup task.
- [reference/redis-cache.md](reference/redis-cache.md) — L2 `RedisCache<T>`: RedisPool usage, key prefix, serializable entry envelope.
- [reference/semantic-cache.md](reference/semantic-cache.md) — Deprecated semantic cache: status, module surface, vector storage reality.
- [reference/cache-metrics.md](reference/cache-metrics.md) — `AtomicCacheStats` / `CacheStatsSnapshot` / `CombinedCacheStats`, admin status endpoint, collector hooks.
