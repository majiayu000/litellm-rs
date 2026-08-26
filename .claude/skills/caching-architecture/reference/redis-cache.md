## Contents

- Construction and Key Layout
- Serializable Entry Envelope
- Operations
- Noop Pool Behavior

## Construction and Key Layout

`RedisCache<T>` (src/core/cache/redis_cache.rs:18) is generic over the cached value and wraps a shared connection pool — it does not own a `ConnectionManager` or build clients from URLs:

```rust
pub struct RedisCache<T> {
    pool: Arc<RedisPool>,          // src/storage/redis/pool.rs
    config: DualCacheConfig,
    stats: Arc<AtomicCacheStats>,
    _marker: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static> RedisCache<T> {
    pub fn new(pool: Arc<RedisPool>, config: DualCacheConfig) -> Self;
    pub fn with_stats(pool: Arc<RedisPool>, config: DualCacheConfig, stats: Arc<AtomicCacheStats>) -> Self;
}
```

Redis keys are formed from the `CacheKey` string: `CacheKey::to_redis_key` (src/core/cache/types.rs:57) produces `litellm:cache:{key}`, and `RedisCache::clear` deletes everything under `config.key_prefix` (`litellm:cache`) via `pool.delete_by_prefix`. Values are stored as JSON with `pool.set(key, data, Some(ttl_secs))`, so expiration is enforced server-side by Redis and again on read via the envelope's `expires_at_unix`.

## Serializable Entry Envelope

What gets stored is a `SerializableCacheEntry<T>` (src/core/cache/types.rs:155), not the bare value:

```rust
pub struct SerializableCacheEntry<T> {
    pub value: T,
    pub ttl_secs: u64,
    pub created_at_unix: u64,
    pub expires_at_unix: u64,
    pub access_count: u64,
    pub size_bytes: usize,
}
```

`From<&CacheEntry<T>>` converts for storage; `into_cache_entry()` reconstructs a live `CacheEntry<T>`. On read, an envelope whose `expires_at_unix` has passed is deleted and reported as a miss; a deserialization failure deletes the corrupted entry and also counts as a miss.

## Operations

- `get(&CacheKey) -> Result<Option<T>>`, `get_entry` (full metadata), `set`, `set_with_ttl`, `set_with_size`, `set_entry`.
- `delete(&CacheKey) -> Result<bool>` — checks `exists` first, deletes, records the deletion stat.
- `exists`, `ttl` (negative Redis TTL maps to `None`), `clear` (prefix-wide, returns deleted count).
- `stats()` — the shared `Arc<AtomicCacheStats>`; `is_available()` probes connectivity.

## Noop Pool Behavior

Every method short-circuits when `pool.is_noop()` (src/storage/redis/pool.rs:101 — Redis not configured): reads return `None`, writes and deletes succeed without side effects. This is how `DualCache` stays correct when the gateway runs without Redis.
