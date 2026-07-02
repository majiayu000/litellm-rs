use super::*;

// ==================== CacheKey Tests ====================

#[test]
fn test_cache_key_new() {
    let key = CacheKey::new("test-key");
    assert_eq!(key.as_str(), "test-key");
    assert!(key.hash_value() > 0);
}

#[test]
fn test_cache_key_from_parts() {
    let key = CacheKey::from_parts("chat", &["gpt-4", "user-123"]);
    assert_eq!(key.as_str(), "chat:gpt-4:user-123");
}

#[test]
fn test_cache_key_to_redis_key() {
    let key = CacheKey::new("my-key");
    assert_eq!(key.to_redis_key(), "litellm:cache:my-key");
}

#[test]
fn test_cache_key_equality() {
    let key1 = CacheKey::new("same-key");
    let key2 = CacheKey::new("same-key");
    assert_eq!(key1, key2);
    assert_eq!(key1.hash_value(), key2.hash_value());
}

#[test]
fn test_cache_key_inequality() {
    let key1 = CacheKey::new("key-1");
    let key2 = CacheKey::new("key-2");
    assert_ne!(key1, key2);
}

#[test]
fn test_cache_key_display() {
    let key = CacheKey::new("display-key");
    assert_eq!(format!("{}", key), "display-key");
}

#[test]
fn test_cache_key_from_string() {
    let key: CacheKey = "from-string".into();
    assert_eq!(key.as_str(), "from-string");
}

// ==================== CacheEntry Tests ====================

#[test]
fn test_cache_entry_new() {
    let entry = CacheEntry::new("value", Duration::from_secs(60));
    assert_eq!(entry.value, "value");
    assert_eq!(entry.ttl, Duration::from_secs(60));
    assert_eq!(entry.access_count, 0);
}

#[test]
fn test_cache_entry_with_size() {
    let entry = CacheEntry::with_size("value", Duration::from_secs(60), 100);
    assert_eq!(entry.size_bytes, 100);
}

#[test]
fn test_cache_entry_not_expired() {
    let entry = CacheEntry::new("value", Duration::from_secs(3600));
    assert!(!entry.is_expired());
}

#[test]
fn test_cache_entry_expired() {
    let entry = CacheEntry::new("value", Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(10));
    assert!(entry.is_expired());
}

#[test]
fn test_cache_entry_remaining_ttl() {
    let entry = CacheEntry::new("value", Duration::from_secs(60));
    let remaining = entry.remaining_ttl();
    assert!(remaining.is_some());
    assert!(remaining.unwrap() <= Duration::from_secs(60));
}

#[test]
fn test_cache_entry_touch() {
    let mut entry = CacheEntry::new("value", Duration::from_secs(60));
    assert_eq!(entry.access_count, 0);
    entry.touch();
    assert_eq!(entry.access_count, 1);
    entry.touch();
    entry.touch();
    assert_eq!(entry.access_count, 3);
}

#[test]
fn test_cache_entry_age() {
    let entry = CacheEntry::new("value", Duration::from_secs(60));
    std::thread::sleep(Duration::from_millis(10));
    let age = entry.age();
    assert!(age >= Duration::from_millis(10));
}

// ==================== SerializableCacheEntry Tests ====================

#[test]
fn test_serializable_entry_conversion() {
    let entry = CacheEntry::new("test-value".to_string(), Duration::from_secs(300));
    let serializable: SerializableCacheEntry<String> = (&entry).into();
    assert_eq!(serializable.value, "test-value");
    assert_eq!(serializable.ttl_secs, 300);
}

#[test]
fn test_serializable_entry_roundtrip() {
    let original = CacheEntry::with_size("roundtrip".to_string(), Duration::from_secs(120), 50);
    let serializable: SerializableCacheEntry<String> = (&original).into();
    let restored = serializable.into_cache_entry();
    assert_eq!(restored.value, "roundtrip");
    assert_eq!(restored.ttl.as_secs(), 120);
    assert_eq!(restored.size_bytes, 50);
}

// ==================== EvictionPolicy Tests ====================

#[test]
fn test_eviction_policy_default() {
    let policy = EvictionPolicy::default();
    assert_eq!(policy, EvictionPolicy::LRU);
}

#[test]
fn test_eviction_policy_display() {
    assert_eq!(format!("{}", EvictionPolicy::LRU), "lru");
    assert_eq!(format!("{}", EvictionPolicy::LFU), "lfu");
    assert_eq!(format!("{}", EvictionPolicy::TTL), "ttl");
    assert_eq!(format!("{}", EvictionPolicy::FIFO), "fifo");
}

#[test]
fn test_eviction_policy_serialize() {
    let policy = EvictionPolicy::LRU;
    let json = serde_json::to_string(&policy).unwrap();
    assert_eq!(json, "\"lru\"");
}

#[test]
fn test_eviction_policy_deserialize() {
    let policy: EvictionPolicy = serde_json::from_str("\"lfu\"").unwrap();
    assert_eq!(policy, EvictionPolicy::LFU);
}

// ==================== CacheMode Tests ====================

#[test]
fn test_cache_mode_default() {
    let mode = CacheMode::default();
    assert_eq!(mode, CacheMode::Dual);
}

#[test]
fn test_cache_mode_serialize() {
    let mode = CacheMode::MemoryOnly;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"memory_only\"");
}

// ==================== DualCacheConfig Tests ====================

#[test]
fn test_dual_cache_config_default() {
    let config = DualCacheConfig::default();
    assert_eq!(config.max_size, 10000);
    assert_eq!(config.default_ttl, Duration::from_secs(3600));
    assert_eq!(config.eviction_policy, EvictionPolicy::LRU);
    assert_eq!(config.mode, CacheMode::Dual);
    assert!(config.enable_stats);
}

#[test]
fn test_dual_cache_config_memory_only() {
    let config = DualCacheConfig::memory_only();
    assert_eq!(config.mode, CacheMode::MemoryOnly);
}

#[test]
fn test_dual_cache_config_redis_only() {
    let config = DualCacheConfig::redis_only();
    assert_eq!(config.mode, CacheMode::RedisOnly);
}

#[test]
fn test_dual_cache_config_builder() {
    let config = DualCacheConfig::default()
        .with_max_size(5000)
        .with_ttl(Duration::from_secs(1800))
        .with_eviction_policy(EvictionPolicy::LFU);

    assert_eq!(config.max_size, 5000);
    assert_eq!(config.default_ttl, Duration::from_secs(1800));
    assert_eq!(config.eviction_policy, EvictionPolicy::LFU);
}

// ==================== AtomicCacheStats Tests ====================

#[test]
fn test_atomic_cache_stats_default() {
    let stats = AtomicCacheStats::default();
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.memory_hits, 0);
    assert_eq!(snapshot.redis_hits, 0);
}

#[test]
fn test_atomic_cache_stats_record() {
    let stats = AtomicCacheStats::new();
    stats.record_memory_hit();
    stats.record_memory_hit();
    stats.record_memory_miss();
    stats.record_redis_hit();
    stats.record_write();
    stats.record_eviction();

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.memory_hits, 2);
    assert_eq!(snapshot.memory_misses, 1);
    assert_eq!(snapshot.redis_hits, 1);
    assert_eq!(snapshot.writes, 1);
    assert_eq!(snapshot.evictions, 1);
}

#[test]
fn test_atomic_cache_stats_reset() {
    let stats = AtomicCacheStats::new();
    stats.record_memory_hit();
    stats.record_redis_hit();
    stats.reset();

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.memory_hits, 0);
    assert_eq!(snapshot.redis_hits, 0);
}

#[test]
fn test_atomic_cache_stats_concurrent() {
    use std::sync::Arc;
    use std::thread;

    let stats = Arc::new(AtomicCacheStats::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let stats_clone = Arc::clone(&stats);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                stats_clone.record_memory_hit();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(stats.snapshot().memory_hits, 1000);
}

// ==================== CacheStatsSnapshot Tests ====================

#[test]
fn test_cache_stats_snapshot_hit_rate() {
    let snapshot = CacheStatsSnapshot {
        memory_hits: 80,
        memory_misses: 20,
        redis_hits: 0,
        redis_misses: 0,
        ..Default::default()
    };

    assert!((snapshot.hit_rate() - 0.8).abs() < 0.001);
    assert!((snapshot.memory_hit_rate() - 0.8).abs() < 0.001);
}

#[test]
fn test_cache_stats_snapshot_zero_requests() {
    let snapshot = CacheStatsSnapshot::default();
    assert_eq!(snapshot.hit_rate(), 0.0);
    assert_eq!(snapshot.total_requests(), 0);
}

#[test]
fn test_cache_stats_snapshot_combined() {
    let snapshot = CacheStatsSnapshot {
        memory_hits: 50,
        memory_misses: 20,
        redis_hits: 30,
        redis_misses: 10,
        ..Default::default()
    };

    assert_eq!(snapshot.total_hits(), 80);
    assert_eq!(snapshot.total_misses(), 30);
    assert_eq!(snapshot.total_requests(), 110);
}
