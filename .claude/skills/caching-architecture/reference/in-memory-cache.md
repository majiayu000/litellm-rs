## In-Memory Cache (L1)

### LRU Cache Implementation

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use linked_hash_map::LinkedHashMap;

pub struct InMemoryCache {
    cache: RwLock<LinkedHashMap<String, CacheEntry>>,
    max_size: usize,
    stats: CacheStats,
}

#[derive(Clone)]
struct CacheEntry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
    created_at: Instant,
}

impl InMemoryCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: RwLock::new(LinkedHashMap::new()),
            max_size,
            stats: CacheStats::default(),
        }
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut cache = self.cache.write().unwrap();

        if let Some(entry) = cache.get_refresh(key) {
            // Check expiration
            if let Some(expires_at) = entry.expires_at {
                if Instant::now() > expires_at {
                    cache.remove(key);
                    self.stats.misses.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            }

            self.stats.hits.fetch_add(1, Ordering::Relaxed);
            return Some(entry.value.clone());
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    pub fn set(&self, key: String, value: Vec<u8>, ttl: Option<Duration>) {
        let mut cache = self.cache.write().unwrap();

        // Evict if at capacity
        while cache.len() >= self.max_size {
            cache.pop_front();
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }

        let entry = CacheEntry {
            value,
            expires_at: ttl.map(|t| Instant::now() + t),
            created_at: Instant::now(),
        };

        cache.insert(key, entry);
    }

    pub fn invalidate(&self, key: &str) {
        let mut cache = self.cache.write().unwrap();
        cache.remove(key);
    }

    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }
}
```
