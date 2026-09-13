//! In-memory cache implementation
//!
//! This module provides a high-performance in-memory cache using DashMap
//! for lock-free concurrent access with LRU eviction support.
//!
//! Hot-path eviction metadata is maintained with per-entry atomics in sharded
//! indexes so cache hits and writes do not serialize on a global LRU lock.

use super::types::{AtomicCacheStats, CacheEntry, CacheKey, DualCacheConfig, EvictionPolicy};
use dashmap::DashMap;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::sync::Notify;
use tracing::{debug, trace};

const EVICTION_SAMPLE_SIZE: usize = 64;
const MIN_ACCESS_SHARDS: usize = 4;
const MAX_ACCESS_SHARDS: usize = 64;

#[derive(Debug)]
struct CacheAccessMeta {
    last_access_tick: AtomicU64,
    access_count: AtomicU64,
}

impl CacheAccessMeta {
    fn new(insert_tick: u64) -> Self {
        Self {
            last_access_tick: AtomicU64::new(insert_tick),
            access_count: AtomicU64::new(0),
        }
    }

    fn record_access(&self, tick: u64) -> u64 {
        self.last_access_tick.store(tick, Ordering::Relaxed);
        self.access_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn reset_for_insert(&self, tick: u64) {
        self.last_access_tick.store(tick, Ordering::Relaxed);
        self.access_count.store(0, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u64, u64) {
        (
            self.last_access_tick.load(Ordering::Relaxed),
            self.access_count.load(Ordering::Relaxed),
        )
    }
}

#[derive(Debug)]
struct EvictionCandidate {
    key: CacheKey,
    last_access_tick: u64,
    access_count: u64,
    created_at: Instant,
    remaining_ttl: Option<Duration>,
}

/// In-memory cache with LRU eviction and TTL expiration
pub struct InMemoryCache<T> {
    /// Main cache storage using DashMap for lock-free access
    cache: Arc<DashMap<CacheKey, CacheEntry<T>>>,
    /// Sharded eviction metadata. Shard count defaults to available CPU
    /// parallelism rounded to a bounded power of two.
    access_meta: Arc<Vec<DashMap<CacheKey, CacheAccessMeta>>>,
    /// Per-shard bounded candidate queues for sampled eviction. Duplicates are
    /// allowed; candidates are validated against `access_meta` and `cache`.
    access_queue: Arc<Vec<Mutex<VecDeque<CacheKey>>>>,
    /// Monotonic logical clock for access ordering.
    access_clock: AtomicU64,
    /// Rotates the first shard inspected by sampled eviction.
    eviction_cursor: AtomicUsize,
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
        let access_shards = default_access_shard_count();
        let access_meta: Arc<Vec<DashMap<CacheKey, CacheAccessMeta>>> =
            Arc::new((0..access_shards).map(|_| DashMap::new()).collect());
        let access_queue: Arc<Vec<Mutex<VecDeque<CacheKey>>>> = Arc::new(
            (0..access_shards)
                .map(|_| Mutex::new(VecDeque::new()))
                .collect(),
        );

        Self {
            cache,
            access_meta,
            access_queue,
            access_clock: AtomicU64::new(0),
            eviction_cursor: AtomicUsize::new(0),
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
            self.remove_access_meta(key);
            self.stats.sub_total_size(removed.size_bytes);
            self.stats.set_entry_count(self.cache.len());
            self.stats.record_memory_miss();
            trace!(key = %key, "Cache entry expired");
            return None;
        }

        if let Some(entry) = self.cache.get(key) {
            let value = entry.value.clone();
            drop(entry);
            self.record_access(key);
            self.stats.record_memory_hit();
            trace!(key = %key, "Cache hit");
            Some(value)
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
            self.remove_access_meta(key);
            self.stats.sub_total_size(removed.size_bytes);
            self.stats.set_entry_count(self.cache.len());
            self.stats.record_memory_miss();
            return None;
        }

        if let Some(entry) = self.cache.get(key) {
            let mut snapshot = entry.clone();
            drop(entry);
            let access_count = self.record_access(key);
            snapshot.access_count = access_count;
            snapshot.last_accessed = Instant::now();
            self.stats.record_memory_hit();
            Some(snapshot)
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
        self.reset_access_meta_for_insert(&key);
        self.stats.record_write();

        if let Some(old_entry) = old {
            self.stats.sub_total_size(old_entry.size_bytes);
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
        self.reset_access_meta_for_insert(&key);
        self.stats.record_write();

        if let Some(old_entry) = old {
            self.stats.sub_total_size(old_entry.size_bytes);
        }

        self.stats.add_total_size(new_size);
        self.stats.set_entry_count(self.cache.len());
    }

    /// Delete a value from the cache
    pub async fn delete(&self, key: &CacheKey) -> bool {
        if let Some((_, removed)) = self.cache.remove(key) {
            self.remove_access_meta(key);
            self.stats.record_deletion();
            self.stats.sub_total_size(removed.size_bytes);
            self.stats.set_entry_count(self.cache.len());
            trace!(key = %key, "Cache delete");
            true
        } else {
            false
        }
    }

    /// Atomically delete `key` only while the live (non-expired) value matches `predicate`.
    ///
    /// Uses DashMap `remove_if` so a concurrent replacement under the same key is
    /// not removed after a stale match decision.
    pub async fn delete_if<F>(&self, key: &CacheKey, predicate: F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        let removed = self.cache.remove_if(key, |_k, entry| {
            !entry.is_expired() && predicate(&entry.value)
        });
        if let Some((_, removed)) = removed {
            self.remove_access_meta(key);
            self.stats.record_deletion();
            self.stats.sub_total_size(removed.size_bytes);
            self.stats.set_entry_count(self.cache.len());
            trace!(key = %key, "Cache conditional delete");
            true
        } else {
            false
        }
    }

    /// Check if a key exists in the cache
    pub async fn exists(&self, key: &CacheKey) -> bool {
        // Atomically remove expired entries to avoid TOCTOU race
        if self.cache.remove_if(key, |_k, v| v.is_expired()).is_some() {
            self.remove_access_meta(key);
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
        for shard in self.access_meta.iter() {
            shard.clear();
        }
        for queue in self.access_queue.iter() {
            lock_queue(queue).clear();
        }
        self.access_clock.store(0, Ordering::Relaxed);
        self.eviction_cursor.store(0, Ordering::Relaxed);
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

    fn next_access_tick(&self) -> u64 {
        self.access_clock.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn access_shard_index(&self, key: &CacheKey) -> usize {
        key.hash_value() as usize % self.access_meta.len()
    }

    fn access_shard(&self, key: &CacheKey) -> &DashMap<CacheKey, CacheAccessMeta> {
        &self.access_meta[self.access_shard_index(key)]
    }

    fn enqueue_eviction_key(&self, key: &CacheKey) {
        let index = self.access_shard_index(key);
        lock_queue(&self.access_queue[index]).push_back(key.clone());
    }

    fn reset_access_meta_for_insert(&self, key: &CacheKey) {
        let tick = self.next_access_tick();
        let shard = self.access_shard(key);

        if let Some(meta) = shard.get(key) {
            meta.reset_for_insert(tick);
        } else {
            shard.insert(key.clone(), CacheAccessMeta::new(tick));
        }
        self.enqueue_eviction_key(key);
    }

    fn record_access(&self, key: &CacheKey) -> u64 {
        let tick = self.next_access_tick();
        let shard = self.access_shard(key);

        let count = if let Some(meta) = shard.get(key) {
            meta.record_access(tick)
        } else {
            let meta = CacheAccessMeta::new(tick);
            let count = meta.record_access(tick);
            shard.insert(key.clone(), meta);
            count
        };
        self.enqueue_eviction_key(key);
        count
    }

    fn remove_access_meta(&self, key: &CacheKey) {
        self.access_shard(key).remove(key);
    }

    fn remove_access_meta_if_unchanged(
        &self,
        key: &CacheKey,
        last_access_tick: u64,
        access_count: u64,
    ) {
        self.access_shard(key).remove_if(key, |_key, meta| {
            meta.snapshot() == (last_access_tick, access_count)
        });
    }

    fn eviction_candidates(&self) -> Vec<EvictionCandidate> {
        let target = self.cache.len().min(EVICTION_SAMPLE_SIZE);
        let mut candidates = Vec::with_capacity(target);
        if self.cache.is_empty() {
            return candidates;
        }

        let shard_count = self.access_meta.len();
        let start = self.eviction_cursor.fetch_add(1, Ordering::Relaxed) % shard_count;
        let mut seen = HashSet::with_capacity(target);

        for offset in 0..shard_count {
            if candidates.len() >= target {
                break;
            }

            let shard_index = (start + offset) % shard_count;
            let sampled = self.sample_eviction_keys(shard_index, target - candidates.len());
            if sampled.is_empty() {
                continue;
            }
            let shard = &self.access_meta[(start + offset) % shard_count];

            let mut requeue = Vec::new();
            for key in sampled {
                if !seen.insert(key.clone()) {
                    requeue.push(key);
                    continue;
                }
                if let Some(meta) = shard.get(&key) {
                    let (last_access_tick, access_count) = meta.snapshot();
                    if let Some(entry) = self.cache.get(&key) {
                        if candidates.len() < target {
                            candidates.push(EvictionCandidate {
                                key: key.clone(),
                                last_access_tick,
                                access_count,
                                created_at: entry.created_at,
                                remaining_ttl: entry.remaining_ttl(),
                            });
                        }
                        requeue.push(key);
                    } else {
                        shard.remove_if(&key, |_key, meta| {
                            meta.snapshot() == (last_access_tick, access_count)
                        });
                    }
                }
            }

            self.requeue_eviction_keys(shard_index, requeue);
        }

        candidates
    }

    fn expired_eviction_candidate(&self) -> Option<EvictionCandidate> {
        let shard_count = self.access_meta.len();
        let start = self.eviction_cursor.fetch_add(1, Ordering::Relaxed) % shard_count;
        let mut inspected = 0;

        for offset in 0..shard_count {
            if inspected >= EVICTION_SAMPLE_SIZE {
                break;
            }

            let shard_index = (start + offset) % shard_count;
            let sampled = self.sample_eviction_keys(shard_index, EVICTION_SAMPLE_SIZE - inspected);
            inspected += sampled.len();
            if sampled.is_empty() {
                continue;
            }

            let shard = &self.access_meta[shard_index];
            let mut requeue = Vec::new();
            let mut expired = None;

            for key in sampled {
                if let Some(meta) = shard.get(&key) {
                    let (last_access_tick, access_count) = meta.snapshot();
                    if let Some(entry) = self.cache.get(&key) {
                        if entry.is_expired() && expired.is_none() {
                            expired = Some(EvictionCandidate {
                                key: key.clone(),
                                last_access_tick,
                                access_count,
                                created_at: entry.created_at,
                                remaining_ttl: entry.remaining_ttl(),
                            });
                        }
                        requeue.push(key);
                    } else {
                        shard.remove_if(&key, |_key, meta| {
                            meta.snapshot() == (last_access_tick, access_count)
                        });
                    }
                }
            }

            self.requeue_eviction_keys(shard_index, requeue);
            if expired.is_some() {
                return expired;
            }
        }

        None
    }

    fn sample_eviction_keys(&self, shard_index: usize, budget: usize) -> Vec<CacheKey> {
        if budget == 0 {
            return Vec::new();
        }

        let attempts = budget.saturating_mul(2).min(EVICTION_SAMPLE_SIZE);
        let mut sampled = Vec::with_capacity(attempts);
        let mut queue = lock_queue(&self.access_queue[shard_index]);
        for _ in 0..attempts {
            let Some(key) = queue.pop_front() else {
                break;
            };
            sampled.push(key);
        }
        sampled
    }

    fn requeue_eviction_keys(&self, shard_index: usize, keys: Vec<CacheKey>) {
        if keys.is_empty() {
            return;
        }

        let mut queue = lock_queue(&self.access_queue[shard_index]);
        for key in keys {
            queue.push_back(key);
        }
    }

    /// Evict one entry based on the eviction policy
    async fn evict_one(&self) -> bool {
        for _ in 0..EVICTION_SAMPLE_SIZE {
            let removed = match self.config.eviction_policy {
                EvictionPolicy::LRU => self.evict_lru().await,
                EvictionPolicy::LFU => self.evict_lfu().await,
                EvictionPolicy::TTL => self.evict_ttl().await,
                EvictionPolicy::FIFO => self.evict_fifo().await,
            };
            if removed || self.cache.len() < self.config.max_size {
                return removed;
            }
        }

        false
    }

    /// Evict the least recently used entry
    async fn evict_lru(&self) -> bool {
        let candidate = self
            .eviction_candidates()
            .into_iter()
            .min_by_key(|candidate| candidate.last_access_tick);

        if let Some(candidate) = candidate {
            return self.evict_candidate(candidate, "LRU");
        }

        false
    }

    /// Evict the least frequently used entry
    async fn evict_lfu(&self) -> bool {
        let candidate = self
            .eviction_candidates()
            .into_iter()
            .min_by_key(|candidate| (candidate.access_count, candidate.last_access_tick));

        if let Some(candidate) = candidate {
            return self.evict_candidate(candidate, "LFU");
        }

        false
    }

    /// Evict entry with shortest remaining TTL
    async fn evict_ttl(&self) -> bool {
        if let Some(candidate) = self.expired_eviction_candidate() {
            return self.evict_candidate(candidate, "TTL");
        }

        let candidate = self
            .eviction_candidates()
            .into_iter()
            .min_by_key(|candidate| candidate.remaining_ttl.unwrap_or(Duration::ZERO));

        if let Some(candidate) = candidate {
            return self.evict_candidate(candidate, "TTL");
        }

        false
    }

    /// Evict the oldest entry (FIFO)
    async fn evict_fifo(&self) -> bool {
        let candidate = self
            .eviction_candidates()
            .into_iter()
            .min_by_key(|candidate| candidate.created_at);

        if let Some(candidate) = candidate {
            return self.evict_candidate(candidate, "FIFO");
        }

        false
    }

    fn evict_candidate(&self, candidate: EvictionCandidate, policy: &'static str) -> bool {
        if let Some((_, removed)) = self.cache.remove_if(&candidate.key, |_key, entry| {
            entry.created_at == candidate.created_at
        }) {
            self.stats.sub_total_size(removed.size_bytes);
            self.stats.record_eviction();
            self.stats.set_entry_count(self.cache.len());
            trace!(key = %candidate.key, policy = policy, "Cache eviction");
            self.remove_access_meta_if_unchanged(
                &candidate.key,
                candidate.last_access_tick,
                candidate.access_count,
            );
            return true;
        }

        self.remove_access_meta_if_unchanged(
            &candidate.key,
            candidate.last_access_tick,
            candidate.access_count,
        );
        false
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
            self.remove_access_meta(&key);
            self.stats.record_eviction();
        }

        if count > 0 {
            debug!(count = count, "Cleaned up expired entries");
            self.stats.set_entry_count(self.cache.len());
        }
    }
}

fn default_access_shard_count() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(MIN_ACCESS_SHARDS)
        .next_power_of_two()
        .clamp(MIN_ACCESS_SHARDS, MAX_ACCESS_SHARDS)
}

fn lock_queue(queue: &Mutex<VecDeque<CacheKey>>) -> MutexGuard<'_, VecDeque<CacheKey>> {
    queue
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl<T> Drop for InMemoryCache<T> {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.shutdown_notify.notify_waiters();
    }
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
