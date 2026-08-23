## Contents

- AtomicCacheStats (lock-free counters)
- CacheStatsSnapshot (point-in-time view)
- CombinedCacheStats and Admin Endpoint
- Collector Hooks

## Cache Metrics

There is no `CacheStats` struct with a `report_metrics` method in the deterministic cache path. Statistics come in three shapes.

## AtomicCacheStats (lock-free counters)

src/core/cache/types.rs:366 — shared by `InMemoryCache` and `RedisCache` inside one `DualCache<T>`:

```rust
pub struct AtomicCacheStats {
    pub memory_hits: AtomicU64,
    pub memory_misses: AtomicU64,
    pub redis_hits: AtomicU64,
    pub redis_misses: AtomicU64,
    pub writes: AtomicU64,
    pub deletions: AtomicU64,
    pub evictions: AtomicU64,
    pub entry_count: AtomicUsize,
    pub total_size_bytes: AtomicUsize,
}
```

Recording methods: `record_memory_hit`, `record_memory_miss`, `record_redis_hit`, `record_redis_miss`, `record_write`, `record_deletion`, `record_eviction`; size setters `set_entry_count` / `add_total_size` / `sub_total_size`; plus `snapshot()`, `reset()`.

## CacheStatsSnapshot (point-in-time view)

`AtomicCacheStats::snapshot()` yields `CacheStatsSnapshot` (types.rs:483) with the same fields as plain integers and derived helpers: `total_hits()`, `total_misses()`, `total_requests()`, `hit_rate()`, `memory_hit_rate()`, `redis_hit_rate()`. Read it via `DualCache::stats()` or `LLMCache::{chat_stats, embedding_stats}`.

## CombinedCacheStats and Admin Endpoint

`LLMCache::combined_stats()` returns `CombinedCacheStats { chat, embedding }` (src/core/cache/llm_cache.rs:497) with its own `total_hits` / `total_misses` / `hit_rate`. This is what `GET /admin/cache` serializes (`CacheAdminResponse.stats`) alongside Redis availability; `POST /admin/cache/clear` resets both layers (src/server/routes/admin.rs). The separate `CacheStats` in src/core/semantic_cache/types.rs (hits/misses/total_entries/avg_hit_similarity) belongs to the deprecated semantic cache only.

## Collector Hooks

The monitoring collector exposes `record_cache_hit()` / `record_cache_miss()` (src/monitoring/metrics/collector.rs:163), which bump `performance.cache_hits` / `cache_misses` counters surfaced in metrics snapshots. They are independent of `AtomicCacheStats` — call both if you need request-path and per-layer numbers.
