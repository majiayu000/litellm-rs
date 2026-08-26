## Contents

- Construction and Storage
- Eviction
- Expiration and Cleanup

## Construction and Storage

`InMemoryCache<T>` (src/core/cache/memory.rs:64) is generic over the cached value — there is no non-generic `InMemoryCache` holding `Vec<u8>` blobs. Storage is a `DashMap<CacheKey, CacheEntry<T>>` for lock-free concurrent access, not a `LinkedHashMap`; eviction metadata is kept in separate sharded indexes so hits and writes do not serialize on one lock.

```rust
pub struct InMemoryCache<T> {
    cache: Arc<DashMap<CacheKey, CacheEntry<T>>>,
    // sharded access metadata + bounded per-shard candidate queues for sampled eviction
    config: DualCacheConfig,
    stats: Arc<AtomicCacheStats>,
    // shutdown flag/notify for the cleanup task
}

impl<T: Clone + Send + Sync + 'static> InMemoryCache<T> {
    pub fn new(config: DualCacheConfig) -> Self;
    pub fn with_stats(config: DualCacheConfig, stats: Arc<AtomicCacheStats>) -> Self;
}
```

Each value is a `CacheEntry<T>` (src/core/cache/types.rs:82): `value`, `ttl`, `created_at`, `expires_at_unix`, `access_count`, `last_accessed`, `size_bytes`. Async methods: `get`, `get_entry`, `set`, `set_with_ttl`, `set_with_size`, `delete`, `exists`, `clear`. Sync helpers: `ttl` (remaining TTL), `len`, `is_empty`, `stats`, `keys`, `shutdown`. Expired entries are removed atomically on read (`remove_if`) to avoid TOCTOU races.

Configuration comes from `DualCacheConfig` (src/core/cache/types.rs:262): `max_size` (default 10000), `default_ttl` (3600 s), `eviction_policy`, `mode`, `enable_stats`, `cleanup_interval` (60 s), `key_prefix` (`"litellm:cache"`), `enable_compression`, `compression_threshold`.

## Eviction

The policy is selected by `EvictionPolicy` (src/core/cache/types.rs:222) — `LRU` (default), `LFU`, `TTL`, `FIFO`:

- Access tracking uses per-entry atomics (`last_access_tick`, `access_count`) in sharded `DashMap`s; a monotonic logical clock orders accesses.
- When `max_size` is reached, eviction samples up to 64 candidates (`EVICTION_SAMPLE_SIZE`) across shards, validates them against live state, and evicts per policy. The first shard inspected rotates each round.
- Evictions increment the shared `AtomicCacheStats.evictions` counter.

## Expiration and Cleanup

- Every entry carries its own TTL; reads return `None` and drop the entry once `created_at.elapsed() > ttl`.
- `start_cleanup_task(&Arc<Self>)` (memory.rs:126) spawns a Tokio task that sweeps expired entries every `cleanup_interval` until `shutdown()` is signaled. `DualCache::start_cleanup_task` and `LLMCache::start_cleanup_tasks` start this for their layers.
