//! Rerank result caching

use super::types::{RerankRequest, RerankResponse};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Simple in-memory cache for rerank results
pub struct RerankCache {
    /// Cache entries
    entries: tokio::sync::RwLock<HashMap<String, CacheEntry>>,
    /// Maximum cache size
    max_size: usize,
    /// Default TTL
    default_ttl: Duration,
}

struct CacheEntry {
    response: RerankResponse,
    created_at: Instant,
    ttl: Duration,
}

impl RerankCache {
    /// Create a new cache
    pub fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            entries: tokio::sync::RwLock::new(HashMap::new()),
            max_size,
            default_ttl,
        }
    }

    /// Generate cache key from request
    fn cache_key(request: &RerankRequest) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        request.model.hash(&mut hasher);
        request.query.hash(&mut hasher);
        for doc in &request.documents {
            doc.get_text().hash(&mut hasher);
        }
        request.top_n.hash(&mut hasher);
        format!("rerank:{:x}", hasher.finish())
    }

    /// Get cached response
    pub async fn get(&self, request: &RerankRequest) -> Option<RerankResponse> {
        let key = Self::cache_key(request);
        let entries = self.entries.read().await;

        if let Some(entry) = entries.get(&key)
            && entry.created_at.elapsed() < entry.ttl
        {
            return Some(entry.response.clone());
        }
        None
    }

    /// Set cached response
    pub async fn set(&self, request: &RerankRequest, response: &RerankResponse) {
        let key = Self::cache_key(request);
        let mut entries = self.entries.write().await;

        // Evict if at capacity
        if entries.len() >= self.max_size {
            // Remove oldest entries (expired ones first)
            entries.retain(|_, entry| entry.created_at.elapsed() < entry.ttl);

            // If still at capacity, remove random entry
            if entries.len() >= self.max_size
                && let Some(key_to_remove) = entries.keys().next().cloned()
            {
                entries.remove(&key_to_remove);
            }
        }

        entries.insert(
            key,
            CacheEntry {
                response: response.clone(),
                created_at: Instant::now(),
                ttl: self.default_ttl,
            },
        );
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }

    /// Get cache statistics
    pub async fn stats(&self) -> RerankCacheStats {
        let entries = self.entries.read().await;
        let valid_entries = entries
            .values()
            .filter(|e| e.created_at.elapsed() < e.ttl)
            .count();

        RerankCacheStats {
            total_entries: entries.len(),
            valid_entries,
            max_size: self.max_size,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct RerankCacheStats {
    /// Total entries in cache
    pub total_entries: usize,
    /// Valid (non-expired) entries
    pub valid_entries: usize,
    /// Maximum cache size
    pub max_size: usize,
}
