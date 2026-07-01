//! Redis cache implementation
//!
//! This module provides a Redis-based cache layer for distributed caching
//! with serialization support using serde.

use super::types::{
    AtomicCacheStats, CacheEntry, CacheKey, DualCacheConfig, SerializableCacheEntry,
};
use crate::storage::redis::RedisPool;
use crate::utils::error::gateway_error::{GatewayError, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use tracing::{trace, warn};

/// Redis cache layer for distributed caching
pub struct RedisCache<T> {
    /// Redis connection pool
    pool: Arc<RedisPool>,
    /// Configuration
    config: DualCacheConfig,
    /// Statistics
    stats: Arc<AtomicCacheStats>,
    /// Phantom data for type parameter
    _marker: PhantomData<T>,
}

impl<T> RedisCache<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// Create a new Redis cache with the given pool and configuration
    pub fn new(pool: Arc<RedisPool>, config: DualCacheConfig) -> Self {
        Self {
            pool,
            config,
            stats: Arc::new(AtomicCacheStats::new()),
            _marker: PhantomData,
        }
    }

    /// Create with shared statistics
    pub fn with_stats(
        pool: Arc<RedisPool>,
        config: DualCacheConfig,
        stats: Arc<AtomicCacheStats>,
    ) -> Self {
        Self {
            pool,
            config,
            stats,
            _marker: PhantomData,
        }
    }

    /// Get a value from the cache
    pub async fn get(&self, key: &CacheKey) -> Result<Option<T>> {
        if self.pool.is_noop() {
            self.stats.record_redis_miss();
            return Ok(None);
        }

        let redis_key = self.make_redis_key(key);
        match self.pool.get(&redis_key).await? {
            Some(data) => {
                match self.deserialize::<SerializableCacheEntry<T>>(&data) {
                    Ok(entry) => {
                        if entry.is_expired() {
                            // Clean up expired entry
                            let _ = self.pool.delete(&redis_key).await;
                            self.stats.record_redis_miss();
                            trace!(key = %key, "Redis cache entry expired");
                            Ok(None)
                        } else {
                            self.stats.record_redis_hit();
                            trace!(key = %key, "Redis cache hit");
                            Ok(Some(entry.value))
                        }
                    }
                    Err(e) => {
                        warn!(key = %key, error = %e, "Failed to deserialize cache entry");
                        // Clean up corrupted entry
                        let _ = self.pool.delete(&redis_key).await;
                        self.stats.record_redis_miss();
                        Ok(None)
                    }
                }
            }
            None => {
                self.stats.record_redis_miss();
                trace!(key = %key, "Redis cache miss");
                Ok(None)
            }
        }
    }

    /// Get an entry with metadata from the cache
    pub async fn get_entry(&self, key: &CacheKey) -> Result<Option<CacheEntry<T>>> {
        if self.pool.is_noop() {
            self.stats.record_redis_miss();
            return Ok(None);
        }

        let redis_key = self.make_redis_key(key);
        match self.pool.get(&redis_key).await? {
            Some(data) => match self.deserialize::<SerializableCacheEntry<T>>(&data) {
                Ok(entry) => {
                    if entry.is_expired() {
                        let _ = self.pool.delete(&redis_key).await;
                        self.stats.record_redis_miss();
                        Ok(None)
                    } else {
                        self.stats.record_redis_hit();
                        Ok(Some(entry.into_cache_entry()))
                    }
                }
                Err(e) => {
                    warn!(key = %key, error = %e, "Failed to deserialize cache entry");
                    let _ = self.pool.delete(&redis_key).await;
                    self.stats.record_redis_miss();
                    Ok(None)
                }
            },
            None => {
                self.stats.record_redis_miss();
                Ok(None)
            }
        }
    }

    /// Set a value in the cache with the default TTL
    pub async fn set(&self, key: CacheKey, value: T) -> Result<()> {
        self.set_with_ttl(key, value, self.config.default_ttl).await
    }

    /// Set a value in the cache with a specific TTL
    pub async fn set_with_ttl(&self, key: CacheKey, value: T, ttl: Duration) -> Result<()> {
        if self.pool.is_noop() {
            return Ok(());
        }

        let entry = CacheEntry::new(value, ttl);
        let serializable: SerializableCacheEntry<T> = (&entry).into();
        let data = self.serialize(&serializable)?;
        let redis_key = self.make_redis_key(&key);

        self.pool
            .set(&redis_key, &data, Some(ttl.as_secs()))
            .await?;
        self.stats.record_write();
        trace!(key = %key, ttl_secs = ttl.as_secs(), "Redis cache set");

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
        if self.pool.is_noop() {
            return Ok(());
        }

        let entry = CacheEntry::with_size(value, ttl, size_bytes);
        let serializable: SerializableCacheEntry<T> = (&entry).into();
        let data = self.serialize(&serializable)?;
        let redis_key = self.make_redis_key(&key);

        self.pool
            .set(&redis_key, &data, Some(ttl.as_secs()))
            .await?;
        self.stats.record_write();

        Ok(())
    }

    /// Set a cache entry directly
    pub async fn set_entry(&self, key: CacheKey, entry: CacheEntry<T>) -> Result<()> {
        if self.pool.is_noop() {
            return Ok(());
        }

        let ttl = entry.remaining_ttl().unwrap_or(Duration::from_secs(1));
        let serializable: SerializableCacheEntry<T> = (&entry).into();
        let data = self.serialize(&serializable)?;
        let redis_key = self.make_redis_key(&key);

        self.pool
            .set(&redis_key, &data, Some(ttl.as_secs()))
            .await?;
        self.stats.record_write();

        Ok(())
    }

    /// Delete a value from the cache
    pub async fn delete(&self, key: &CacheKey) -> Result<bool> {
        if self.pool.is_noop() {
            return Ok(false);
        }

        let redis_key = self.make_redis_key(key);
        let existed = self.pool.exists(&redis_key).await?;

        if existed {
            self.pool.delete(&redis_key).await?;
            self.stats.record_deletion();
            trace!(key = %key, "Redis cache delete");
        }

        Ok(existed)
    }

    /// Clear all Redis entries owned by this cache prefix.
    pub async fn clear(&self) -> Result<usize> {
        if self.pool.is_noop() {
            return Ok(0);
        }

        let prefix = format!("{}:", self.config.key_prefix);
        self.pool.delete_by_prefix(&prefix).await
    }

    /// Check if a key exists in the cache
    pub async fn exists(&self, key: &CacheKey) -> Result<bool> {
        if self.pool.is_noop() {
            return Ok(false);
        }

        let redis_key = self.make_redis_key(key);
        self.pool.exists(&redis_key).await
    }

    /// Get the remaining TTL for a key
    pub async fn ttl(&self, key: &CacheKey) -> Result<Option<Duration>> {
        if self.pool.is_noop() {
            return Ok(None);
        }

        let redis_key = self.make_redis_key(key);
        let ttl_secs = self.pool.ttl(&redis_key).await?;

        if ttl_secs < 0 {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs(ttl_secs as u64)))
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> Arc<AtomicCacheStats> {
        Arc::clone(&self.stats)
    }

    /// Check if Redis is available
    pub async fn is_available(&self) -> bool {
        if self.pool.is_noop() {
            return false;
        }
        self.pool.health_check().await.is_ok()
    }

    /// Perform a health check
    pub async fn health_check(&self) -> Result<()> {
        self.pool.health_check().await
    }

    // ==================== Private Methods ====================

    /// Generate Redis key with prefix
    fn make_redis_key(&self, key: &CacheKey) -> String {
        format!("{}:{}", self.config.key_prefix, key.as_str())
    }

    /// Serialize a value to JSON string
    fn serialize<S: Serialize>(&self, value: &S) -> Result<String> {
        serde_json::to_string(value)
            .map_err(|e| GatewayError::Config(format!("Failed to serialize cache value: {}", e)))
    }

    /// Deserialize a value from JSON string
    fn deserialize<D: DeserializeOwned>(&self, data: &str) -> Result<D> {
        serde_json::from_str(data)
            .map_err(|e| GatewayError::Config(format!("Failed to deserialize cache value: {}", e)))
    }
}

/// Batch operations for Redis cache
impl<T> RedisCache<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// Get multiple values from the cache using Redis MGET
    pub async fn get_many(&self, keys: &[CacheKey]) -> Result<Vec<Option<T>>> {
        if self.pool.is_noop() || keys.is_empty() {
            return Ok(vec![None; keys.len()]);
        }

        let redis_keys: Vec<String> = keys.iter().map(|k| self.make_redis_key(k)).collect();
        let raw_values = self.pool.mget(&redis_keys).await?;

        let mut results = Vec::with_capacity(keys.len());
        for (i, raw) in raw_values.into_iter().enumerate() {
            match raw {
                Some(data) => match self.deserialize::<SerializableCacheEntry<T>>(&data) {
                    Ok(entry) if !entry.is_expired() => {
                        self.stats.record_redis_hit();
                        results.push(Some(entry.value));
                    }
                    Ok(_) => {
                        let _ = self.pool.delete(&redis_keys[i]).await;
                        self.stats.record_redis_miss();
                        results.push(None);
                    }
                    Err(e) => {
                        warn!(key = %keys[i], error = %e, "Failed to deserialize cache entry");
                        let _ = self.pool.delete(&redis_keys[i]).await;
                        self.stats.record_redis_miss();
                        results.push(None);
                    }
                },
                None => {
                    self.stats.record_redis_miss();
                    results.push(None);
                }
            }
        }

        Ok(results)
    }

    /// Set multiple values in the cache
    pub async fn set_many(&self, entries: &[(CacheKey, T, Duration)]) -> Result<()> {
        if self.pool.is_noop() || entries.is_empty() {
            return Ok(());
        }

        for (key, value, ttl) in entries {
            self.set_with_ttl(key.clone(), value.clone(), *ttl).await?;
        }

        Ok(())
    }

    /// Delete multiple keys from the cache
    pub async fn delete_many(&self, keys: &[CacheKey]) -> Result<usize> {
        if self.pool.is_noop() || keys.is_empty() {
            return Ok(0);
        }

        let mut deleted = 0;
        for key in keys {
            if self.delete(key).await? {
                deleted += 1;
            }
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a noop cache for testing
    fn create_noop_cache() -> RedisCache<String> {
        let pool = Arc::new(RedisPool::create_noop());
        let config = DualCacheConfig::redis_only();
        RedisCache::new(pool, config)
    }

    // ==================== Basic Noop Tests ====================
    // These tests verify behavior when Redis is not available

    #[tokio::test]
    async fn test_redis_cache_noop_get() {
        let cache = create_noop_cache();
        let key = CacheKey::new("test-key");

        let result = cache.get(&key).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_redis_cache_noop_set() {
        let cache = create_noop_cache();
        let key = CacheKey::new("test-key");

        // Should not error even in noop mode
        let result = cache.set(key, "value".to_string()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_redis_cache_noop_delete() {
        let cache = create_noop_cache();
        let key = CacheKey::new("test-key");

        let result = cache.delete(&key).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_redis_cache_noop_clear() {
        let cache = create_noop_cache();

        assert_eq!(cache.clear().await.ok(), Some(0));
    }

    #[tokio::test]
    async fn test_redis_cache_noop_exists() {
        let cache = create_noop_cache();
        let key = CacheKey::new("test-key");

        let result = cache.exists(&key).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_redis_cache_noop_ttl() {
        let cache = create_noop_cache();
        let key = CacheKey::new("test-key");

        let result = cache.ttl(&key).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_redis_cache_noop_availability() {
        let cache = create_noop_cache();
        assert!(!cache.is_available().await);
    }

    // ==================== Key Generation Tests ====================

    #[test]
    fn test_redis_key_generation() {
        let pool = Arc::new(RedisPool::create_noop());
        let config = DualCacheConfig::default();
        let cache: RedisCache<String> = RedisCache::new(pool, config);

        let key = CacheKey::new("my-key");
        let redis_key = cache.make_redis_key(&key);

        assert!(redis_key.starts_with("litellm:cache:"));
        assert!(redis_key.ends_with("my-key"));
    }

    #[test]
    fn test_redis_key_with_custom_prefix() {
        let pool = Arc::new(RedisPool::create_noop());
        let config = DualCacheConfig {
            key_prefix: "custom:prefix".to_string(),
            ..Default::default()
        };
        let cache: RedisCache<String> = RedisCache::new(pool, config);

        let key = CacheKey::new("my-key");
        let redis_key = cache.make_redis_key(&key);

        assert_eq!(redis_key, "custom:prefix:my-key");
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_serialization_roundtrip() {
        let pool = Arc::new(RedisPool::create_noop());
        let config = DualCacheConfig::default();
        let cache: RedisCache<String> = RedisCache::new(pool, config);

        let original = "test-value".to_string();
        let serialized = cache.serialize(&original).unwrap();
        let deserialized: String = cache.deserialize(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_serialization_complex_type() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct ComplexType {
            id: u64,
            name: String,
            tags: Vec<String>,
        }

        let pool = Arc::new(RedisPool::create_noop());
        let config = DualCacheConfig::default();
        let cache: RedisCache<ComplexType> = RedisCache::new(pool, config);

        let original = ComplexType {
            id: 123,
            name: "test".to_string(),
            tags: vec!["a".to_string(), "b".to_string()],
        };

        let serialized = cache.serialize(&original).unwrap();
        let deserialized: ComplexType = cache.deserialize(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }

    // ==================== Statistics Tests ====================

    #[tokio::test]
    async fn test_redis_cache_stats() {
        let cache = create_noop_cache();

        // Generate some cache activity
        let _ = cache.get(&CacheKey::new("miss1")).await;
        let _ = cache.get(&CacheKey::new("miss2")).await;

        let stats = cache.stats().snapshot();
        assert_eq!(stats.redis_misses, 2);
    }

    // ==================== Batch Operations Tests ====================

    #[tokio::test]
    async fn test_redis_cache_get_many_noop() {
        let cache = create_noop_cache();

        let keys = vec![
            CacheKey::new("key1"),
            CacheKey::new("key2"),
            CacheKey::new("key3"),
        ];

        let results = cache.get_many(&keys).await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_none()));
    }

    #[tokio::test]
    async fn test_redis_cache_set_many_noop() {
        let cache = create_noop_cache();

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

        let result = cache.set_many(&entries).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_redis_cache_delete_many_noop() {
        let cache = create_noop_cache();

        let keys = vec![CacheKey::new("key1"), CacheKey::new("key2")];

        let deleted = cache.delete_many(&keys).await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_redis_cache_empty_batch() {
        let cache = create_noop_cache();

        assert!(cache.get_many(&[]).await.unwrap().is_empty());
        assert!(cache.set_many(&[]).await.is_ok());
        assert_eq!(cache.delete_many(&[]).await.unwrap(), 0);
    }
}
