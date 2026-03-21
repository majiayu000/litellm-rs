//! In-memory cache implementation
//!
//! This module provides a high-performance in-memory cache using DashMap
//! for lock-free concurrent access with LRU eviction support.
//!
//! The LRU order tracker uses `tokio::sync::Mutex` to avoid blocking the
//! async executor thread under contention.

use super::types::{AtomicCacheStats, CacheEntry, CacheKey, DualCacheConfig, EvictionPolicy};
use dashmap::DashMap;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tracing::{debug, trace};

/// In-memory cache with LRU eviction and TTL expiration
pub struct InMemoryCache<T> {
    /// Main cache storage using DashMap for lock-free access
    cache: Arc<DashMap<CacheKey, CacheEntry<T>>>,
    /// LRU tracking using O(1) LruCache (used as an ordered set, value is ())
    ///
    /// Uses `tokio::sync::Mutex` so that waiting for the lock yields the async
    /// executor thread instead of blocking it, preventing executor starvation
    /// under high contention.
    lru_order: Arc<Mutex<LruCache<CacheKey, ()>>>,
    /// Configuration
    config: DualCacheConfig,
    /// Statistics
    stats: Arc<AtomicCacheStats>,
    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
    /// Notify for shutdown
    shutdown_notify: Arc<Notify>,
}

impl<T: Clone + Send + Sync + 'static> InMemoryCache<T> {
    /// Create a new in-memory cache with the given configuration
    pub fn new(config: DualCacheConfig) -> Self {
        Self::with_stats(config, Arc::new(AtomicCacheStats::new()))
    }

    /// Create a new in-memory cache with shared statistics
    pub fn with_stats(config: DualCacheConfig, stats: Arc<AtomicCacheStats>) -> Self {
        let cache = Arc::new(DashMap::with_capacity(config.max_size));
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_notify = Arc::new(Notify::new());
        let lru_cap = NonZeroUsize::new(config.max_size).unwrap_or(NonZeroUsize::MIN);

        Self {
            cache,
            lru_order: Arc::new(Mutex::new(LruCache::new(lru_cap))),
            config,
            stats,
            shutdown,
            shutdown_notify,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(DualCacheConfig::memory_only())
    }

    /// Start the background cleanup task
    pub fn start_cleanup_task(self: &Arc<Self>) {
        let cache = Arc::clone(self);
        let interval = self.config.cleanup_interval;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        cache.cleanup_expired().await;
                    }
                    _ = cache.shutdown_notify.notified() => {
                        debug!("In-memory cache cleanup task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Get a value from the cache
    pub async fn get(&self, key: &CacheKey) -> Option<T> {
        // Atomically remove expired entries to avoid TOCTOU race
        if let Some((_, removed)) = self.cache.remove_if(key, |_k, v| v.is_expired()) {
            self.remove_from_lru(key).await;
            self.stats.sub_total_size(removed.size_bytes);
            self.stats.set_entry_count(self.cache.len());
            self.stats.record_memory_miss();
            trace!(key = %key, "Cache entry expired");
            return None;
        }

        if let Some(mut entry) = self.cache.get_mut(key) {
            entry.touch();
            self.update_lru(key).await;
            self.stats.record_memory_hit();
            trace!(key = %key, "Cache hit");
            Some(entry.value.clone())
        } else {
            self.stats.record_memory_miss();
            trace!(key = %key, "Cache miss");
            None
        }
    }

    /// Get an entry with metadata from the cache
    pub async fn get_entry(&self, key: &CacheKey) -> Option<CacheEntry<T>> {
        // Atomically remove expired entries to avoid TOCTOU race
        if let Some((_, removed)) = self.cache.remove_if(key, |_k, v| v.is_expired()) {
            self.remove_from_lru(key).await;
            self.stats.sub_total_size(removed.size_bytes);
            self.stats.set_entry_count(self.cache.len());
            self.stats.record_memory_miss();
            return None;
        }

        if let Some(mut entry) = self.cache.get_mut(key) {
            entry.touch();
            self.update_lru(key).await;
            self.stats.record_memory_hit();
            Some(entry.clone())
        } else {
            self.stats.record_memory_miss();
            None
        }
    }

    /// Set a value in the cache with the default TTL
    pub async fn set(&self, key: CacheKey, value: T) {
        self.set_with_ttl(key, value, self.config.default_ttl).await;
    }

    /// Set a value in the cache with a specific TTL
    pub async fn set_with_ttl(&self, key: CacheKey, value: T, ttl: Duration) {
        // Check if we need to evict entries
        if self.cache.len() >= self.config.max_size {
            self.evict_one().await;
        }

        let entry = CacheEntry::new(value, ttl);
        let new_size = entry.size_bytes;
        // Atomic insert returns the old entry if key existed (no TOCTOU gap)
        let old = self.cache.insert(key.clone(), entry);
        self.stats.record_write();

        if let Some(old_entry) = old {
            self.stats.sub_total_size(old_entry.size_bytes);
            self.update_lru(&key).await;
        } else {
            self.add_to_lru(&key).await;
        }

        self.stats.add_total_size(new_size);
        self.stats.set_entry_count(self.cache.len());
        trace!(key = %key, ttl_secs = ttl.as_secs(), "Cache set");
    }

    /// Set a value with size tracking
    pub async fn set_with_size(&self, key: CacheKey, value: T, ttl: Duration, size_bytes: usize) {
        if self.cache.len() >= self.config.max_size {
            self.evict_one().await;
        }

        let entry = CacheEntry::with_size(value, ttl, size_bytes);
        let new_size = entry.size_bytes;
        // Atomic insert returns the old entry if key existed (no TOCTOU gap)
        let old = self.cache.insert(key.clone(), entry);
        self.stats.record_write();

        if let Some(old_entry) = old {
            self.stats.sub_total_size(old_entry.size_bytes);
            self.update_lru(&key).await;
        } else {
            self.add_to_lru(&key).await;
        }

        self.stats.add_total_size(new_size);
        self.stats.set_entry_count(self.cache.len());
    }

    /// Delete a value from the cache
    pub async fn delete(&self, key: &CacheKey) -> bool {
        if let Some((_, removed)) = self.cache.remove(key) {
            self.remove_from_lru(key).await;
            self.stats.record_deletion();
            self.stats.sub_total_size(removed.size_bytes);
            self.stats.set_entry_count(self.cache.len());
            trace!(key = %key, "Cache delete");
            true
        } else {
            false
        }
    }

    /// Check if a key exists in the cache
    pub async fn exists(&self, key: &CacheKey) -> bool {
        // Atomically remove expired entries to avoid TOCTOU race
        if self.cache.remove_if(key, |_k, v| v.is_expired()).is_some() {
            self.remove_from_lru(key).await;
            self.stats.set_entry_count(self.cache.len());
            return false;
        }
        self.cache.contains_key(key)
    }

    /// Get the remaining TTL for a key
    pub fn ttl(&self, key: &CacheKey) -> Option<Duration> {
        if let Some(entry) = self.cache.get(key) {
            entry.remaining_ttl()
        } else {
            None
        }
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) {
        self.cache.clear();
        self.lru_order.lock().await.clear();
        self.stats.reset();
        debug!("Cache cleared");
    }

    /// Get the number of entries in the cache
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get cache statistics
    pub fn stats(&self) -> Arc<AtomicCacheStats> {
        Arc::clone(&self.stats)
    }

    /// Get all keys in the cache
    pub fn keys(&self) -> Vec<CacheKey> {
        self.cache.iter().map(|r| r.key().clone()).collect()
    }

    /// Shutdown the cache and cleanup task
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.shutdown_notify.notify_waiters();
    }

    // ==================== Private Methods ====================

    /// Update LRU order for a key (O(1) via lru::LruCache)
    async fn update_lru(&self, key: &CacheKey) {
        let mut lru = self.lru_order.lock().await;
        // promote re-inserts the key to most-recent in O(1)
        if lru.promote(key) {
            return;
        }
        // Key was not in LRU (shouldn't normally happen), add it
        lru.push(key.clone(), ());
    }

    /// Add a key to the LRU order (O(1))
    async fn add_to_lru(&self, key: &CacheKey) {
        self.lru_order.lock().await.push(key.clone(), ());
    }

    /// Remove a key from the LRU order (O(1))
    async fn remove_from_lru(&self, key: &CacheKey) {
        self.lru_order.lock().await.pop_entry(key);
    }

    /// Evict one entry based on the eviction policy
    async fn evict_one(&self) {
        match self.config.eviction_policy {
            EvictionPolicy::LRU => self.evict_lru().await,
            EvictionPolicy::LFU => self.evict_lfu().await,
            EvictionPolicy::TTL => self.evict_ttl().await,
            EvictionPolicy::FIFO => self.evict_fifo().await,
        }
    }

    /// Evict the least recently used entry
    async fn evict_lru(&self) {
        let key = {
            let mut lru = self.lru_order.lock().await;
            lru.pop_lru().map(|(k, _)| k)
        };

        if let Some(key) = key {
            if let Some((_, removed)) = self.cache.remove(&key) {
                self.stats.sub_total_size(removed.size_bytes);
            }
            self.stats.record_eviction();
            self.stats.set_entry_count(self.cache.len());
            trace!(key = %key, "LRU eviction");
        }
    }

    /// Evict the least frequently used entry
    async fn evict_lfu(&self) {
        // Find entry with lowest access count
        let key_to_evict = self
            .cache
            .iter()
            .min_by_key(|entry| entry.value().access_count)
            .map(|entry| entry.key().clone());

        if let Some(key) = key_to_evict {
            if let Some((_, removed)) = self.cache.remove(&key) {
                self.stats.sub_total_size(removed.size_bytes);
            }
            self.remove_from_lru(&key).await;
            self.stats.record_eviction();
            self.stats.set_entry_count(self.cache.len());
            trace!(key = %key, "LFU eviction");
        }
    }

    /// Evict entry with shortest remaining TTL
    async fn evict_ttl(&self) {
        // First try to evict any expired entries
        let expired_key = self
            .cache
            .iter()
            .find(|entry| entry.value().is_expired())
            .map(|entry| entry.key().clone());

        if let Some(key) = expired_key {
            if let Some((_, removed)) = self.cache.remove(&key) {
                self.stats.sub_total_size(removed.size_bytes);
            }
            self.remove_from_lru(&key).await;
            self.stats.record_eviction();
            self.stats.set_entry_count(self.cache.len());
            return;
        }

        // Otherwise evict entry closest to expiration
        let key_to_evict = self
            .cache
            .iter()
            .min_by_key(|entry| entry.value().remaining_ttl().unwrap_or(Duration::ZERO))
            .map(|entry| entry.key().clone());

        if let Some(key) = key_to_evict {
            if let Some((_, removed)) = self.cache.remove(&key) {
                self.stats.sub_total_size(removed.size_bytes);
            }
            self.remove_from_lru(&key).await;
            self.stats.record_eviction();
            self.stats.set_entry_count(self.cache.len());
            trace!(key = %key, "TTL eviction");
        }
    }

    /// Evict the oldest entry (FIFO)
    async fn evict_fifo(&self) {
        let key_to_evict = self
            .cache
            .iter()
            .min_by_key(|entry| entry.value().created_at)
            .map(|entry| entry.key().clone());

        if let Some(key) = key_to_evict {
            if let Some((_, removed)) = self.cache.remove(&key) {
                self.stats.sub_total_size(removed.size_bytes);
            }
            self.remove_from_lru(&key).await;
            self.stats.record_eviction();
            self.stats.set_entry_count(self.cache.len());
            trace!(key = %key, "FIFO eviction");
        }
    }

    /// Clean up expired entries
    async fn cleanup_expired(&self) {
        let mut expired_keys = Vec::new();

        for entry in self.cache.iter() {
            if entry.value().is_expired() {
                expired_keys.push(entry.key().clone());
            }
        }

        let count = expired_keys.len();
        for key in expired_keys {
            if let Some((_, removed)) = self.cache.remove(&key) {
                self.stats.sub_total_size(removed.size_bytes);
            }
            self.remove_from_lru(&key).await;
            self.stats.record_eviction();
        }

        if count > 0 {
            debug!(count = count, "Cleaned up expired entries");
            self.stats.set_entry_count(self.cache.len());
        }
    }
}

impl<T> Drop for InMemoryCache<T> {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.shutdown_notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Basic Operations Tests ====================

    #[test]
    fn test_cache_new() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[tokio::test]
    async fn test_cache_set_get() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("test-key");

        cache.set(key.clone(), "test-value".to_string()).await;

        let result = cache.get(&key).await;
        assert_eq!(result, Some("test-value".to_string()));
    }

    #[tokio::test]
    async fn test_cache_get_nonexistent() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("nonexistent");

        let result = cache.get(&key).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_delete() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("to-delete");

        cache.set(key.clone(), "value".to_string()).await;
        assert!(cache.exists(&key).await);

        let deleted = cache.delete(&key).await;
        assert!(deleted);
        assert!(!cache.exists(&key).await);
    }

    #[tokio::test]
    async fn test_cache_delete_nonexistent() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("nonexistent");

        let deleted = cache.delete(&key).await;
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_cache_exists() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("exists-key");

        assert!(!cache.exists(&key).await);
        cache.set(key.clone(), "value".to_string()).await;
        assert!(cache.exists(&key).await);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();

        cache.set(CacheKey::new("key1"), "value1".to_string()).await;
        cache.set(CacheKey::new("key2"), "value2".to_string()).await;
        cache.set(CacheKey::new("key3"), "value3".to_string()).await;

        assert_eq!(cache.len(), 3);
        cache.clear().await;
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    // ==================== TTL Tests ====================

    #[tokio::test]
    async fn test_cache_ttl_expiration() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("expiring-key");

        cache
            .set_with_ttl(key.clone(), "value".to_string(), Duration::from_millis(10))
            .await;
        assert!(cache.exists(&key).await);

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!cache.exists(&key).await);
    }

    #[tokio::test]
    async fn test_cache_get_expired() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("expiring-key");

        cache
            .set_with_ttl(key.clone(), "value".to_string(), Duration::from_millis(10))
            .await;
        tokio::time::sleep(Duration::from_millis(20)).await;

        let result = cache.get(&key).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_ttl_remaining() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("ttl-key");

        cache
            .set_with_ttl(key.clone(), "value".to_string(), Duration::from_secs(60))
            .await;

        let ttl = cache.ttl(&key);
        assert!(ttl.is_some());
        assert!(ttl.unwrap() <= Duration::from_secs(60));
    }

    // ==================== Eviction Tests ====================

    #[tokio::test]
    async fn test_cache_lru_eviction() {
        let config = DualCacheConfig::default()
            .with_max_size(3)
            .with_eviction_policy(EvictionPolicy::LRU);
        let cache: InMemoryCache<String> = InMemoryCache::new(config);

        cache.set(CacheKey::new("key1"), "value1".to_string()).await;
        cache.set(CacheKey::new("key2"), "value2".to_string()).await;
        cache.set(CacheKey::new("key3"), "value3".to_string()).await;

        // Access key1 and key2 to make them more recent
        cache.get(&CacheKey::new("key1")).await;
        cache.get(&CacheKey::new("key2")).await;

        // Add key4, should evict key3 (least recently used)
        cache.set(CacheKey::new("key4"), "value4".to_string()).await;

        assert!(cache.exists(&CacheKey::new("key1")).await);
        assert!(cache.exists(&CacheKey::new("key2")).await);
        assert!(!cache.exists(&CacheKey::new("key3")).await);
        assert!(cache.exists(&CacheKey::new("key4")).await);
    }

    #[tokio::test]
    async fn test_cache_lfu_eviction() {
        let config = DualCacheConfig::default()
            .with_max_size(3)
            .with_eviction_policy(EvictionPolicy::LFU);
        let cache: InMemoryCache<String> = InMemoryCache::new(config);

        cache.set(CacheKey::new("key1"), "value1".to_string()).await;
        cache.set(CacheKey::new("key2"), "value2".to_string()).await;
        cache.set(CacheKey::new("key3"), "value3".to_string()).await;

        // Access key1 multiple times
        for _ in 0..5 {
            cache.get(&CacheKey::new("key1")).await;
        }
        // Access key2 a few times
        for _ in 0..2 {
            cache.get(&CacheKey::new("key2")).await;
        }
        // key3 has lowest access count

        // Add key4, should evict key3 (least frequently used)
        cache.set(CacheKey::new("key4"), "value4".to_string()).await;

        assert!(cache.exists(&CacheKey::new("key1")).await);
        assert!(cache.exists(&CacheKey::new("key2")).await);
        // key3 should be evicted
        assert!(cache.exists(&CacheKey::new("key4")).await);
    }

    // ==================== Statistics Tests ====================

    #[tokio::test]
    async fn test_cache_stats_hits_misses() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("stats-key");

        cache.set(key.clone(), "value".to_string()).await;

        // Generate hits
        cache.get(&key).await;
        cache.get(&key).await;

        // Generate misses
        cache.get(&CacheKey::new("nonexistent1")).await;
        cache.get(&CacheKey::new("nonexistent2")).await;

        let stats = cache.stats().snapshot();
        assert_eq!(stats.memory_hits, 2);
        assert_eq!(stats.memory_misses, 2);
    }

    #[tokio::test]
    async fn test_cache_stats_writes() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();

        cache.set(CacheKey::new("key1"), "value1".to_string()).await;
        cache.set(CacheKey::new("key2"), "value2".to_string()).await;

        let stats = cache.stats().snapshot();
        assert_eq!(stats.writes, 2);
    }

    #[tokio::test]
    async fn test_cache_stats_deletions() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("to-delete");

        cache.set(key.clone(), "value".to_string()).await;
        cache.delete(&key).await;

        let stats = cache.stats().snapshot();
        assert_eq!(stats.deletions, 1);
    }

    // ==================== Concurrent Access Tests ====================

    #[tokio::test]
    async fn test_cache_concurrent_read_write() {
        use std::sync::Arc;

        let cache = Arc::new(InMemoryCache::<i32>::with_defaults());
        let mut handles = vec![];

        // Writers
        for i in 0..4 {
            let cache_clone = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                for j in 0..25 {
                    let key = CacheKey::new(format!("key-{}-{}", i, j));
                    cache_clone.set(key, i * 25 + j).await;
                }
            });
            handles.push(handle);
        }

        // Readers
        for _ in 0..4 {
            let cache_clone = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                for i in 0..4 {
                    for j in 0..25 {
                        let key = CacheKey::new(format!("key-{}-{}", i, j));
                        let _ = cache_clone.get(&key).await;
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // Just verify no panics occurred
        assert!(cache.len() <= 100);
    }

    // ==================== Entry Metadata Tests ====================

    #[tokio::test]
    async fn test_cache_get_entry() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("entry-key");

        cache
            .set_with_size(
                key.clone(),
                "value".to_string(),
                Duration::from_secs(60),
                100,
            )
            .await;

        let entry = cache.get_entry(&key).await;
        assert!(entry.is_some());

        let entry = entry.unwrap();
        assert_eq!(entry.value, "value");
        assert_eq!(entry.size_bytes, 100);
        assert_eq!(entry.access_count, 1); // One access from get_entry
    }

    #[tokio::test]
    async fn test_cache_keys() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();

        cache.set(CacheKey::new("key1"), "value1".to_string()).await;
        cache.set(CacheKey::new("key2"), "value2".to_string()).await;
        cache.set(CacheKey::new("key3"), "value3".to_string()).await;

        let keys = cache.keys();
        assert_eq!(keys.len(), 3);
    }

    // ==================== Update Tests ====================

    #[tokio::test]
    async fn test_cache_update_existing() {
        let cache: InMemoryCache<String> = InMemoryCache::with_defaults();
        let key = CacheKey::new("update-key");

        cache.set(key.clone(), "initial".to_string()).await;
        cache.set(key.clone(), "updated".to_string()).await;

        let result = cache.get(&key).await;
        assert_eq!(result, Some("updated".to_string()));
        assert_eq!(cache.len(), 1);
    }
}
