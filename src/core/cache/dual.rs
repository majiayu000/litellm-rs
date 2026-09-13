//! Dual cache implementation
//!
//! This module provides a two-tier cache system combining in-memory and Redis caches
//! for optimal performance and distributed consistency.

use super::memory::InMemoryCache;
use super::redis_cache::RedisCache;
use super::types::{
    AtomicCacheStats, CacheEntry, CacheKey, CacheMode, CacheStatsSnapshot, CacheWriteIdentity,
    DualCacheConfig,
};
use crate::storage::redis::RedisPool;
use crate::utils::error::gateway_error::Result;
use dashmap::DashMap;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, trace, warn};

/// Shard count for locks that serialize L2→L1 read-through fills with deletes.
const L1_FILL_LOCK_SHARDS: usize = 64;

/// Short-lived barrier that rejects recreating a just-invalidated logical value.
#[derive(Clone, Debug)]
struct StaleWriteBarrier {
    identity: u64,
    expires_at: Instant,
}

/// Dual-layer cache combining in-memory and Redis caches
///
/// Read strategy:
/// 1. Check memory cache first (sub-millisecond)
/// 2. On memory miss, check Redis cache
/// 3. On Redis hit, populate memory cache for future reads
///
/// Write strategy:
/// - Write to both memory and Redis caches
/// - Dual-mode writes take the same per-key shard lock as invalidation and
///   refuse payloads matching a recent conditional-delete barrier so a stale
///   `store` cannot recreate a just-invalidated entry
///
/// Invalidation strategy:
/// - Delete from both L1 (memory) and L2 (Redis) when present
/// - Dual-mode Redis delete errors propagate (same as RedisOnly) so
///   poisoned-entry invalidation cannot report success while L2 still holds
///   the value. Dual writes still only warn on Redis failure.
/// - L1 fills from L2 take the same per-key shard lock as Dual deletes and
///   perform a single Redis read under that lock so invalidation cannot be
///   raced by read-through and hit/miss stats are not double-counted.
/// - Dual-mode `delete_if` records a write-identity barrier for each removed
///   value so concurrent writers that skip the lock window still cannot
///   recreate the same logical payload until the barrier expires.
pub struct DualCache<T> {
    /// In-memory cache layer (L1)
    memory: Arc<InMemoryCache<T>>,
    /// Redis cache layer (L2)
    redis: Option<RedisCache<T>>,
    /// Configuration
    config: DualCacheConfig,
    /// Shared statistics
    stats: Arc<AtomicCacheStats>,
    /// Serializes Dual-mode L2→L1 population with L1/L2 invalidation/writes.
    l1_fill_locks: Arc<Vec<Mutex<()>>>,
    /// Per-key barriers rejecting stale recreation of conditionally deleted values.
    stale_write_barriers: Arc<DashMap<CacheKey, Vec<StaleWriteBarrier>>>,
}

impl<T> DualCache<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + CacheWriteIdentity + 'static,
{
    /// Create a new dual cache with the given configuration
    pub fn new(config: DualCacheConfig, redis_pool: Option<Arc<RedisPool>>) -> Self {
        let stats = Arc::new(AtomicCacheStats::new());
        let memory = Arc::new(InMemoryCache::with_stats(
            config.clone(),
            Arc::clone(&stats),
        ));

        let redis = match (&config.mode, redis_pool) {
            (CacheMode::MemoryOnly, _) => None,
            (_, Some(pool)) => Some(RedisCache::with_stats(
                pool,
                config.clone(),
                Arc::clone(&stats),
            )),
            (CacheMode::RedisOnly, None) => {
                warn!(
                    "Redis-only mode requested but no Redis pool provided, falling back to memory-only"
                );
                None
            }
            (CacheMode::Dual, None) => {
                debug!("No Redis pool provided, using memory-only cache");
                None
            }
        };

        Self {
            memory,
            redis,
            config,
            stats,
            l1_fill_locks: Arc::new((0..L1_FILL_LOCK_SHARDS).map(|_| Mutex::new(())).collect()),
            stale_write_barriers: Arc::new(DashMap::new()),
        }
    }

    /// Create a memory-only cache
    pub fn memory_only(config: DualCacheConfig) -> Self {
        let mut config = config;
        config.mode = CacheMode::MemoryOnly;
        Self::new(config, None)
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(DualCacheConfig::default(), None)
    }

    /// Start the background cleanup task for the memory cache
    pub fn start_cleanup_task(&self) {
        self.memory.start_cleanup_task();
    }

    fn l1_fill_lock(&self, key: &CacheKey) -> &Mutex<()> {
        &self.l1_fill_locks[key.hash_value() as usize % self.l1_fill_locks.len()]
    }

    fn install_stale_write_barrier(&self, key: &CacheKey, value: &T) {
        let identity = value.cache_write_identity();
        let expires_at = Instant::now() + self.config.default_ttl;
        let now = Instant::now();
        self.stale_write_barriers
            .entry(key.clone())
            .and_modify(|barriers| {
                barriers.retain(|b| b.expires_at > now);
                if !barriers.iter().any(|b| b.identity == identity) {
                    barriers.push(StaleWriteBarrier {
                        identity,
                        expires_at,
                    });
                }
            })
            .or_insert_with(|| {
                vec![StaleWriteBarrier {
                    identity,
                    expires_at,
                }]
            });
    }

    fn is_stale_write_blocked(&self, key: &CacheKey, value: &T) -> bool {
        let identity = value.cache_write_identity();
        let now = Instant::now();
        let Some(mut barriers) = self.stale_write_barriers.get_mut(key) else {
            return false;
        };
        barriers.retain(|b| b.expires_at > now);
        barriers.iter().any(|b| b.identity == identity)
    }

    /// Get a value from the cache
    ///
    /// Checks memory cache first, then Redis. On Redis hit,
    /// populates memory cache for future reads.
    pub async fn get(&self, key: &CacheKey) -> Result<Option<T>> {
        match self.config.mode {
            CacheMode::MemoryOnly => Ok(self.memory.get(key).await),
            CacheMode::RedisOnly => {
                if let Some(ref redis) = self.redis {
                    redis.get(key).await
                } else {
                    Ok(None)
                }
            }
            CacheMode::Dual => self.get_dual(key).await,
        }
    }

    /// Get from dual cache with read-through pattern
    async fn get_dual(&self, key: &CacheKey) -> Result<Option<T>> {
        // L1: Check memory cache first (fastest path)
        if let Some(value) = self.memory.get(key).await {
            trace!(key = %key, "Dual cache L1 hit");
            return Ok(Some(value));
        }

        if self.redis.is_none() {
            trace!(key = %key, "Dual cache miss");
            return Ok(None);
        }

        self.fill_l1_from_l2(key).await
    }

    /// Fill L1 from L2 under the fill/invalidate lock with a single Redis read.
    ///
    /// Holding the same shard lock as Dual deletes while reading L2 prevents a
    /// concurrent invalidation from leaving a resurrected L1 entry. Doing the
    /// Redis read only under that lock (after an L1 recheck) avoids a second
    /// Redis round trip and double-counting hit/miss stats on every L1 miss.
    async fn fill_l1_from_l2(&self, key: &CacheKey) -> Result<Option<T>> {
        let redis = match self.redis.as_ref() {
            Some(redis) => redis,
            None => return Ok(None),
        };

        let _guard = self.l1_fill_lock(key).lock().await;

        if let Some(value) = self.memory.get(key).await {
            return Ok(Some(value));
        }

        match redis.get(key).await? {
            Some(fresh) => {
                if self.is_stale_write_blocked(key, &fresh) {
                    trace!(
                        key = %key,
                        "Dual cache skipped L1 fill for invalidated logical value"
                    );
                    return Ok(None);
                }
                self.memory.set(key.clone(), fresh.clone()).await;
                trace!(key = %key, "Dual cache L2 hit, populated L1");
                Ok(Some(fresh))
            }
            None => {
                trace!(key = %key, "Dual cache miss");
                Ok(None)
            }
        }
    }

    /// Get an entry with metadata from the cache
    pub async fn get_entry(&self, key: &CacheKey) -> Result<Option<CacheEntry<T>>> {
        match self.config.mode {
            CacheMode::MemoryOnly => Ok(self.memory.get_entry(key).await),
            CacheMode::RedisOnly => {
                if let Some(ref redis) = self.redis {
                    redis.get_entry(key).await
                } else {
                    Ok(None)
                }
            }
            CacheMode::Dual => self.get_entry_dual(key).await,
        }
    }

    async fn get_entry_dual(&self, key: &CacheKey) -> Result<Option<CacheEntry<T>>> {
        if let Some(entry) = self.memory.get_entry(key).await {
            return Ok(Some(entry));
        }

        let redis = match self.redis.as_ref() {
            Some(redis) => redis,
            None => return Ok(None),
        };

        let _guard = self.l1_fill_lock(key).lock().await;

        if let Some(existing) = self.memory.get_entry(key).await {
            return Ok(Some(existing));
        }

        match redis.get_entry(key).await? {
            Some(fresh) => {
                if self.is_stale_write_blocked(key, &fresh.value) {
                    trace!(
                        key = %key,
                        "Dual cache skipped L1 entry fill for invalidated logical value"
                    );
                    return Ok(None);
                }
                self.memory
                    .set_with_size(
                        key.clone(),
                        fresh.value.clone(),
                        fresh.ttl,
                        fresh.size_bytes,
                    )
                    .await;
                Ok(Some(fresh))
            }
            None => Ok(None),
        }
    }

    /// Set a value in the cache with the default TTL
    pub async fn set(&self, key: CacheKey, value: T) -> Result<()> {
        self.set_with_ttl(key, value, self.config.default_ttl).await
    }

    /// Set a value in the cache with a specific TTL
    pub async fn set_with_ttl(&self, key: CacheKey, value: T, ttl: Duration) -> Result<()> {
        match self.config.mode {
            CacheMode::MemoryOnly => {
                self.memory.set_with_ttl(key, value, ttl).await;
                Ok(())
            }
            CacheMode::RedisOnly => {
                if let Some(ref redis) = self.redis {
                    redis.set_with_ttl(key, value, ttl).await
                } else {
                    Ok(())
                }
            }
            CacheMode::Dual => self.set_dual(key, value, ttl).await,
        }
    }

    /// Set in both cache layers under the fill/invalidate lock.
    ///
    /// Holding the lock serializes writers with Dual `delete`/`delete_if`. A
    /// write-identity barrier installed by conditional invalidation rejects
    /// recreating the same logical payload after a successful CAS delete.
    async fn set_dual(&self, key: CacheKey, value: T, ttl: Duration) -> Result<()> {
        let _guard = self.l1_fill_lock(&key).lock().await;
        if self.is_stale_write_blocked(&key, &value) {
            trace!(
                key = %key,
                "Dual cache rejected stale write after conditional invalidation"
            );
            return Ok(());
        }

        // Write to memory cache
        self.memory
            .set_with_ttl(key.clone(), value.clone(), ttl)
            .await;

        // Write to Redis cache (asynchronous)
        if let Some(ref redis) = self.redis
            && let Err(e) = redis.set_with_ttl(key.clone(), value, ttl).await
        {
            warn!(key = %key, error = %e, "Failed to write to Redis cache");
            // Don't fail the operation if Redis write fails
        }

        trace!(key = %key, ttl_secs = ttl.as_secs(), "Dual cache set");
        Ok(())
    }

    /// Set a value with size tracking
    pub async fn set_with_size(
        &self,
        key: CacheKey,
        value: T,
        ttl: Duration,
        size_bytes: usize,
    ) -> Result<()> {
        match self.config.mode {
            CacheMode::MemoryOnly => {
                self.memory.set_with_size(key, value, ttl, size_bytes).await;
                Ok(())
            }
            CacheMode::RedisOnly => {
                if let Some(ref redis) = self.redis {
                    redis.set_with_size(key, value, ttl, size_bytes).await
                } else {
                    Ok(())
                }
            }
            CacheMode::Dual => {
                let _guard = self.l1_fill_lock(&key).lock().await;
                if self.is_stale_write_blocked(&key, &value) {
                    trace!(
                        key = %key,
                        "Dual cache rejected stale sized write after conditional invalidation"
                    );
                    return Ok(());
                }
                self.memory
                    .set_with_size(key.clone(), value.clone(), ttl, size_bytes)
                    .await;
                if let Some(ref redis) = self.redis
                    && let Err(e) = redis
                        .set_with_size(key.clone(), value, ttl, size_bytes)
                        .await
                {
                    warn!(key = %key, error = %e, "Redis write failed in dual cache mode");
                }
                Ok(())
            }
        }
    }

    /// Delete a value from both cache layers
    pub async fn delete(&self, key: &CacheKey) -> Result<bool> {
        let mut deleted = false;

        match self.config.mode {
            CacheMode::MemoryOnly => {
                deleted = self.memory.delete(key).await;
            }
            CacheMode::RedisOnly => {
                if let Some(ref redis) = self.redis {
                    deleted = redis.delete(key).await?;
                }
            }
            CacheMode::Dual => {
                // Hold the fill lock so an in-flight L2→L1 populate cannot
                // resurrect this key after we clear both layers.
                let _guard = self.l1_fill_lock(key).lock().await;
                if self.memory.delete(key).await {
                    deleted = true;
                }

                // Delete from Redis. Propagate L2 failures (unlike Dual writes,
                // which only warn): invalidation of poisoned entries must not
                // report success when Redis still holds the value.
                if let Some(ref redis) = self.redis
                    && redis.delete(key).await?
                {
                    deleted = true;
                }
            }
        }

        trace!(key = %key, deleted = deleted, "Dual cache delete");
        Ok(deleted)
    }

    /// Conditionally delete from each configured layer while the live value matches.
    ///
    /// Each layer evaluates the predicate against its own current value and deletes
    /// atomically (DashMap `remove_if` for L1, Redis Lua CAS for L2), so a
    /// concurrent replacement is not removed after a stale match. Dual mode also
    /// holds the L1-fill lock so read-through cannot repopulate L1 after a
    /// successful layer delete, and installs a write-identity barrier so a
    /// concurrent Dual `set` cannot recreate the same logical payload.
    pub async fn delete_if<F>(&self, key: &CacheKey, predicate: F) -> Result<bool>
    where
        F: Fn(&T) -> bool,
    {
        let mut deleted = false;

        match self.config.mode {
            CacheMode::MemoryOnly => {
                deleted = self.memory.delete_if(key, &predicate).await.is_some();
            }
            CacheMode::RedisOnly => {
                if let Some(ref redis) = self.redis {
                    deleted = redis.delete_if(key, &predicate).await?.is_some();
                }
            }
            CacheMode::Dual => {
                let _guard = self.l1_fill_lock(key).lock().await;
                if let Some(removed) = self.memory.delete_if(key, &predicate).await {
                    self.install_stale_write_barrier(key, &removed);
                    deleted = true;
                }
                // Propagate L2 failures the same way as unconditional Dual delete.
                if let Some(ref redis) = self.redis
                    && let Some(removed) = redis.delete_if(key, &predicate).await?
                {
                    self.install_stale_write_barrier(key, &removed);
                    deleted = true;
                }
            }
        }

        trace!(key = %key, deleted = deleted, "Dual cache conditional delete");
        Ok(deleted)
    }

    /// Check if a key exists in either cache layer
    pub async fn exists(&self, key: &CacheKey) -> Result<bool> {
        match self.config.mode {
            CacheMode::MemoryOnly => Ok(self.memory.exists(key).await),
            CacheMode::RedisOnly => {
                if let Some(ref redis) = self.redis {
                    redis.exists(key).await
                } else {
                    Ok(false)
                }
            }
            CacheMode::Dual => {
                // Check memory first
                if self.memory.exists(key).await {
                    return Ok(true);
                }

                // Check Redis
                if let Some(ref redis) = self.redis {
                    return redis.exists(key).await;
                }

                Ok(false)
            }
        }
    }

    /// Get the remaining TTL for a key
    pub async fn ttl(&self, key: &CacheKey) -> Result<Option<Duration>> {
        match self.config.mode {
            CacheMode::MemoryOnly => Ok(self.memory.ttl(key)),
            CacheMode::RedisOnly => {
                if let Some(ref redis) = self.redis {
                    redis.ttl(key).await
                } else {
                    Ok(None)
                }
            }
            CacheMode::Dual => {
                // Prefer memory TTL
                if let Some(ttl) = self.memory.ttl(key) {
                    return Ok(Some(ttl));
                }

                // Fall back to Redis
                if let Some(ref redis) = self.redis {
                    return redis.ttl(key).await;
                }

                Ok(None)
            }
        }
    }

    /// Clear all entries from both cache layers
    pub async fn clear(&self) -> Result<()> {
        // Clear memory cache
        self.memory.clear().await;

        let mut redis_deleted = 0_usize;
        if self.config.mode != CacheMode::MemoryOnly
            && let Some(ref redis) = self.redis
        {
            redis_deleted = redis.clear().await?;
        }

        debug!(redis_deleted, "Dual cache cleared");
        Ok(())
    }

    /// Get the number of entries in the memory cache
    pub fn memory_len(&self) -> usize {
        self.memory.len()
    }

    /// Check if the memory cache is empty
    pub fn is_memory_empty(&self) -> bool {
        self.memory.is_empty()
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStatsSnapshot {
        self.stats.snapshot()
    }

    /// Get the raw statistics for atomic updates
    pub fn atomic_stats(&self) -> Arc<AtomicCacheStats> {
        Arc::clone(&self.stats)
    }

    /// Check if Redis is available
    pub async fn is_redis_available(&self) -> bool {
        if let Some(ref redis) = self.redis {
            redis.is_available().await
        } else {
            false
        }
    }

    /// Get the current cache mode
    pub fn mode(&self) -> CacheMode {
        self.config.mode
    }

    /// Get the configuration
    pub fn config(&self) -> &DualCacheConfig {
        &self.config
    }

    /// Shutdown the cache
    pub fn shutdown(&self) {
        self.memory.shutdown();
    }
}

/// Batch operations for dual cache
impl<T> DualCache<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + CacheWriteIdentity + 'static,
{
    /// Get multiple values from the cache
    pub async fn get_many(&self, keys: &[CacheKey]) -> Result<Vec<Option<T>>> {
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            results.push(self.get(key).await?);
        }

        Ok(results)
    }

    /// Set multiple values in the cache
    pub async fn set_many(&self, entries: &[(CacheKey, T, Duration)]) -> Result<()> {
        for (key, value, ttl) in entries {
            self.set_with_ttl(key.clone(), value.clone(), *ttl).await?;
        }
        Ok(())
    }

    /// Delete multiple keys from the cache
    pub async fn delete_many(&self, keys: &[CacheKey]) -> Result<usize> {
        let mut deleted = 0;
        for key in keys {
            if self.delete(key).await? {
                deleted += 1;
            }
        }
        Ok(deleted)
    }
}

/// Cache warming operations
impl<T> DualCache<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + CacheWriteIdentity + 'static,
{
    /// Warm the memory cache from Redis
    ///
    /// Loads entries from Redis into memory for specified keys. Per-key Redis
    /// failures are skipped so a transient error on one key does not abort the
    /// rest of the batch.
    pub async fn warm_from_redis(&self, keys: &[CacheKey]) -> Result<usize> {
        let redis = match self.redis.as_ref() {
            Some(r) if self.config.mode != CacheMode::MemoryOnly => r,
            _ => return Ok(0),
        };
        let mut warmed = 0;

        for key in keys {
            // Skip if already in memory
            if self.memory.exists(key).await {
                continue;
            }

            let _guard = self.l1_fill_lock(key).lock().await;
            if self.memory.exists(key).await {
                continue;
            }
            // Single L2 read under the fill lock: avoid double Redis traffic and
            // keep warming from resurrecting a concurrently invalidated entry.
            let fresh = match redis.get_entry(key).await {
                Ok(Some(entry)) => entry,
                Ok(None) => continue,
                Err(e) => {
                    warn!(key = %key, error = %e, "Failed to warm key from Redis; skipping");
                    continue;
                }
            };
            if self.is_stale_write_blocked(key, &fresh.value) {
                continue;
            }
            self.memory
                .set_with_size(key.clone(), fresh.value, fresh.ttl, fresh.size_bytes)
                .await;
            warmed += 1;
        }

        debug!(count = warmed, "Warmed memory cache from Redis");
        Ok(warmed)
    }

    /// Warm the memory cache with provided entries
    pub async fn warm_with_entries(&self, entries: &[(CacheKey, T, Duration)]) -> usize {
        let mut warmed = 0;

        for (key, value, ttl) in entries {
            if !self.memory.exists(key).await {
                self.memory
                    .set_with_ttl(key.clone(), value.clone(), *ttl)
                    .await;
                warmed += 1;
            }
        }

        debug!(count = warmed, "Warmed memory cache with entries");
        warmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Configuration Tests ====================

    #[test]
    fn test_dual_cache_default_config() {
        let cache: DualCache<String> = DualCache::with_defaults();
        assert_eq!(cache.mode(), CacheMode::Dual);
        assert!(cache.is_memory_empty());
    }

    #[test]
    fn test_dual_cache_memory_only() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());
        assert_eq!(cache.mode(), CacheMode::MemoryOnly);
    }

    // ==================== Memory-Only Tests ====================

    #[tokio::test]
    async fn test_dual_cache_memory_set_get() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());
        let key = CacheKey::new("test-key");

        cache
            .set(key.clone(), "test-value".to_string())
            .await
            .unwrap();
        let result = cache.get(&key).await.unwrap();

        assert_eq!(result, Some("test-value".to_string()));
    }

    #[tokio::test]
    async fn test_dual_cache_memory_delete() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());
        let key = CacheKey::new("to-delete");

        cache.set(key.clone(), "value".to_string()).await.unwrap();
        assert!(cache.exists(&key).await.unwrap());

        let deleted = cache.delete(&key).await.unwrap();
        assert!(deleted);
        assert!(!cache.exists(&key).await.unwrap());
    }

    /// Dual mode must not swallow Redis layer results via unwrap_or: with a
    /// noop Redis pool, delete still clears L1 and returns Ok (L2 is a no-op
    /// Ok(false)). Real Redis errors propagate via `?` (see Dual delete arm).
    #[tokio::test]
    async fn dual_mode_delete_clears_memory_when_redis_is_noop() {
        let config = DualCacheConfig {
            mode: CacheMode::Dual,
            ..DualCacheConfig::default()
        };
        let cache: DualCache<String> =
            DualCache::new(config, Some(Arc::new(RedisPool::create_noop())));
        let key = CacheKey::new("poisoned-entry");

        cache
            .set(key.clone(), "blocked-response".to_string())
            .await
            .unwrap();
        assert!(cache.memory.exists(&key).await);

        let deleted = cache
            .delete(&key)
            .await
            .expect("noop Redis must not turn Dual delete into Err");
        assert!(deleted, "L1 delete must count as success");
        assert!(!cache.memory.exists(&key).await);
    }

    #[tokio::test]
    async fn test_dual_cache_memory_ttl() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());
        let key = CacheKey::new("ttl-key");

        cache
            .set_with_ttl(key.clone(), "value".to_string(), Duration::from_secs(60))
            .await
            .unwrap();

        let ttl = cache.ttl(&key).await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() <= Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_dual_cache_memory_clear() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());

        cache
            .set(CacheKey::new("key1"), "value1".to_string())
            .await
            .unwrap();
        cache
            .set(CacheKey::new("key2"), "value2".to_string())
            .await
            .unwrap();

        assert_eq!(cache.memory_len(), 2);

        cache.clear().await.unwrap();
        assert!(cache.is_memory_empty());
    }

    // ==================== Batch Operations Tests ====================

    #[tokio::test]
    async fn test_dual_cache_get_many() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());

        cache
            .set(CacheKey::new("key1"), "value1".to_string())
            .await
            .unwrap();
        cache
            .set(CacheKey::new("key2"), "value2".to_string())
            .await
            .unwrap();

        let keys = vec![
            CacheKey::new("key1"),
            CacheKey::new("key2"),
            CacheKey::new("key3"),
        ];

        let results = cache.get_many(&keys).await.unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Some("value1".to_string()));
        assert_eq!(results[1], Some("value2".to_string()));
        assert_eq!(results[2], None);
    }

    #[tokio::test]
    async fn test_dual_cache_set_many() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());

        let entries = vec![
            (
                CacheKey::new("key1"),
                "value1".to_string(),
                Duration::from_secs(60),
            ),
            (
                CacheKey::new("key2"),
                "value2".to_string(),
                Duration::from_secs(60),
            ),
            (
                CacheKey::new("key3"),
                "value3".to_string(),
                Duration::from_secs(60),
            ),
        ];

        cache.set_many(&entries).await.unwrap();

        assert_eq!(cache.memory_len(), 3);
        assert!(cache.exists(&CacheKey::new("key1")).await.unwrap());
        assert!(cache.exists(&CacheKey::new("key2")).await.unwrap());
        assert!(cache.exists(&CacheKey::new("key3")).await.unwrap());
    }

    #[tokio::test]
    async fn test_dual_cache_delete_many() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());

        cache
            .set(CacheKey::new("key1"), "value1".to_string())
            .await
            .unwrap();
        cache
            .set(CacheKey::new("key2"), "value2".to_string())
            .await
            .unwrap();
        cache
            .set(CacheKey::new("key3"), "value3".to_string())
            .await
            .unwrap();

        let keys = vec![CacheKey::new("key1"), CacheKey::new("key2")];
        let deleted = cache.delete_many(&keys).await.unwrap();

        assert_eq!(deleted, 2);
        assert_eq!(cache.memory_len(), 1);
        assert!(cache.exists(&CacheKey::new("key3")).await.unwrap());
    }

    // ==================== Cache Warming Tests ====================

    #[tokio::test]
    async fn test_dual_cache_warm_with_entries() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());

        let entries = vec![
            (
                CacheKey::new("key1"),
                "value1".to_string(),
                Duration::from_secs(60),
            ),
            (
                CacheKey::new("key2"),
                "value2".to_string(),
                Duration::from_secs(60),
            ),
        ];

        let warmed = cache.warm_with_entries(&entries).await;
        assert_eq!(warmed, 2);

        // Warming again should not add duplicates
        let warmed_again = cache.warm_with_entries(&entries).await;
        assert_eq!(warmed_again, 0);
    }

    // ==================== Statistics Tests ====================

    #[tokio::test]
    async fn test_dual_cache_stats() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());
        let key = CacheKey::new("stats-key");

        cache.set(key.clone(), "value".to_string()).await.unwrap();
        cache.get(&key).await.unwrap();
        cache.get(&key).await.unwrap();
        cache.get(&CacheKey::new("miss")).await.unwrap();

        let stats = cache.stats();
        assert_eq!(stats.memory_hits, 2);
        assert_eq!(stats.memory_misses, 1);
    }

    // ==================== Entry Metadata Tests ====================

    #[tokio::test]
    async fn test_dual_cache_get_entry() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());
        let key = CacheKey::new("entry-key");

        cache
            .set_with_size(
                key.clone(),
                "value".to_string(),
                Duration::from_secs(60),
                100,
            )
            .await
            .unwrap();

        let entry = cache.get_entry(&key).await.unwrap();
        assert!(entry.is_some());

        let entry = entry.unwrap();
        assert_eq!(entry.value, "value");
        assert_eq!(entry.size_bytes, 100);
    }

    // ==================== Expiration Tests ====================

    #[tokio::test]
    async fn test_dual_cache_expiration() {
        let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());
        let key = CacheKey::new("expiring-key");

        cache
            .set_with_ttl(key.clone(), "value".to_string(), Duration::from_millis(10))
            .await
            .unwrap();

        assert!(cache.exists(&key).await.unwrap());

        tokio::time::sleep(Duration::from_millis(20)).await;

        let result = cache.get(&key).await.unwrap();
        assert!(result.is_none());
    }

    // ==================== Complex Type Tests ====================

    #[tokio::test]
    async fn test_dual_cache_complex_type() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct ComplexValue {
            id: u64,
            name: String,
            scores: Vec<f64>,
        }

        impl CacheWriteIdentity for ComplexValue {
            fn cache_write_identity(&self) -> u64 {
                super::super::types::serialize_write_identity(self)
            }
        }

        let cache: DualCache<ComplexValue> = DualCache::memory_only(DualCacheConfig::default());
        let key = CacheKey::new("complex-key");

        let value = ComplexValue {
            id: 123,
            name: "test".to_string(),
            scores: vec![1.0, 2.5, 3.7],
        };

        cache.set(key.clone(), value.clone()).await.unwrap();
        let result = cache.get(&key).await.unwrap();

        assert_eq!(result, Some(value));
    }

    #[tokio::test]
    async fn dual_delete_if_blocks_stale_recreate_of_same_logical_value() {
        let config = DualCacheConfig {
            mode: CacheMode::Dual,
            ..DualCacheConfig::default()
        };
        let cache: DualCache<String> =
            DualCache::new(config, Some(Arc::new(RedisPool::create_noop())));
        let key = CacheKey::new("stale-write");

        cache.set(key.clone(), "poison".to_string()).await.unwrap();
        assert!(cache.delete_if(&key, |v| v == "poison").await.unwrap());

        // Stale writer recreates the same logical payload after invalidation.
        cache.set(key.clone(), "poison".to_string()).await.unwrap();
        assert_eq!(
            cache.get(&key).await.unwrap(),
            None,
            "stale recreate of invalidated payload must be rejected"
        );

        // A distinct replacement must still be accepted.
        cache.set(key.clone(), "safe".to_string()).await.unwrap();
        assert_eq!(cache.get(&key).await.unwrap(), Some("safe".to_string()));
    }
}
