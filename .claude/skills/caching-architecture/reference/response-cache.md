## Contents

- Composition
- Startup Wiring
- Read Path
- Write Path
- Failure and Fallback Behavior
- Batch Operations and Cache Warming
- Invalidation and Administration

## Composition

There is no unified `CacheManager`. The runtime cache is `LLMCache` wrapping two type-specialized `DualCache<T>` instances (src/core/cache/llm_cache.rs:25):

```rust
pub struct LLMCache {
    chat_cache: DualCache<CachedChatResponse>,
    embedding_cache: DualCache<CachedEmbeddingResponse>,
    config: LLMCacheConfig,
}

pub struct DualCache<T> {                 // src/core/cache/dual.rs:30
    memory: Arc<InMemoryCache<T>>,        // L1, always present
    redis: Option<RedisCache<T>>,         // L2, optional (needs a RedisPool)
    config: DualCacheConfig,
    stats: Arc<AtomicCacheStats>,         // shared by both layers
}
```

`LLMCache` owns key generation and response wrappers (`CachedChatResponse`, `CachedEmbeddingResponse` — an `Arc<ChatCompletionResponse>`/`Arc<EmbeddingResponse>` plus model, `cached` flag, and `cached_at`). Chat methods select `generate_chat_key_with_user` when `config.user_specific` is set. Wired embedding methods always call `generate_embedding_key`; they do not use the `_with_user` variant. Main methods:

- `get_chat_response_with_user(request, user_id)` — returns `Option<Arc<ChatCompletionResponse>>`; skips streaming requests.
- `cache_chat_response_with_user(request, response, user_id)` — stores under the chat TTL.
- `get_embedding_response` / `cache_embedding_response` / `invalidate_embedding`.
- `invalidate_chat_with_user` — deletes by the same user-scoped key used for lookup; using plain `invalidate_chat` while `user_specific` is enabled would leak stale per-user entries (llm_cache.rs:352).
- `chat_stats()`, `embedding_stats()`, `combined_stats()`.

The generic `LLMCache::get::<T>` / `set::<T>` methods are placeholders that do nothing (return `Ok(None)` / `Ok(())`) — do not use them.

## Startup Wiring

`build_response_cache(config, redis)` in src/server/state.rs:143 constructs `AppState.response_cache: Option<Arc<LLMCache>>`:

- Requires `gateway.cache.enabled == true` and `ttl > 0`; otherwise no cache is built (ttl 0 logs an error first).
- With a live Redis pool: `DualCacheConfig::default()` → mode `Dual`. Without one: `DualCacheConfig::memory_only()`.
- `max_size` and TTL come from `cache.max_size` / `cache.ttl`; both chat and embedding TTLs are set to the same value.
- `user_specific` is hard-coded `true` for chat key selection;
  `semantic_cache_enabled` is hard-coded `false`. Embedding lookup/store still pass no
  separate user ID, and `generate_embedding_key` ignores `EmbeddingRequest.user`, so
  identical model/input requests share an embedding entry across callers.
- `cache.start_cleanup_tasks()` spawns background expiry sweeps for both layers.

## Read Path

`DualCache::get` dispatches on `CacheMode` (src/core/cache/types.rs:250): `MemoryOnly`, `RedisOnly`, or `Dual` (default).

In `Dual` mode (`get_dual`, dual.rs:116):

1. Check L1 memory — sub-millisecond DashMap hit.
2. On L1 miss, check L2 Redis. A Redis hit repopulates L1 before returning (read-through).
3. Both miss → `Ok(None)`.

`get_entry(key)` follows the same tier order and returns the full `CacheEntry<T>` (value,
TTL, size, and access metadata). On a Redis hit it reconstructs the entry's original TTL
and aged creation time, but L1 promotion passes that full TTL to `set_with_size`, which
creates a fresh in-memory entry. The original TTL is therefore restarted in L1 rather
than reduced to Redis's remaining TTL, so the promoted entry can outlive L2.

## Write Path

- `set(key, value)` uses `config.default_ttl`; `set_with_ttl(key, value, ttl)` overrides it.
- In `Dual` mode, writes go to memory first, then Redis. A Redis write failure logs `warn!("Failed to write to Redis cache")` and the operation still succeeds (dual.rs:209).
- `set_with_size(key, value, ttl, size_bytes)` tracks byte size for stats across both layers.

## Failure and Fallback Behavior

- `RedisOnly` requested without a pool logs a warning, but it does **not** fall
  back to memory: reads miss and writes/deletes are no-ops. `Dual` without a
  pool uses its memory tier only (`dual.rs:60`, plus the mode dispatch in
  `get`/`set_with_ttl`/`delete`).
- Every `RedisPool` method is guarded by `pool.is_noop()` (no Redis configured) — reads miss, writes no-op.
- Corrupted Redis entries are deleted on read and counted as misses (redis_cache.rs:81).
- `is_redis_available()` probes the actual connection; exposed via the admin status endpoint.

## Batch Operations and Cache Warming

On `DualCache<T>` (src/core/cache/dual.rs:400-486):

- `get_many(keys)` / `set_many(&[(key, value, ttl)])` / `delete_many(keys)` — sequential loops over the single-key operations.
- `warm_from_redis(keys)` — loads Redis entries into L1 for keys not already resident (skipped entirely in `MemoryOnly` mode).
- `warm_with_entries(&[(key, value, ttl)])` — pre-seeds L1, skipping existing keys.

## Invalidation and Administration

- Per-entry invalidation is mode-dependent. `RedisOnly` propagates Redis delete errors;
  `Dual` removes L1 first but treats Redis deletion as best-effort (`unwrap_or(false)`). A
  failed L2 deletion still returns `Ok`, and that stale Redis value can repopulate L1 on
  the next lookup. `clear()` empties memory and deletes every Redis key under the
  `litellm:cache:` prefix, propagating a Redis clear error.
- HTTP admin API (`src/server/routes/admin.rs:53-120`): when JWT or API-key auth is
  enabled, an admin-role user is required and other callers receive 403. When both auth
  methods are disabled (valid only with `allow_anonymous: true`), `require_cache_admin`
  returns early, so these endpoints are unauthenticated:
  - `GET /admin/cache` and `GET /admin/cache/status` — returns `CacheAdminResponse`: enabled flags, `CombinedCacheStats`, Redis availability; HTTP 501 when the cache is unwired.
  - `POST /admin/cache/clear` — calls `LLMCache::clear()` (both chat and embedding caches).
