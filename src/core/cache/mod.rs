//! DualCache System for LiteLLM-RS Gateway
//!
//! This module provides a high-performance, two-tier caching system combining
//! in-memory and Redis caches for optimal latency and distributed consistency.
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+
//! |     LLMCache      |  <- High-level API for LLM requests
//! +-------------------+
//!          |
//! +-------------------+
//! |    DualCache      |  <- Two-tier cache orchestration
//! +-------------------+
//!      /         \
//! +---------+  +---------+
//! | Memory  |  |  Redis  |
//! | (L1)    |  |  (L2)   |
//! +---------+  +---------+
//! ```
//!
//! ## Features
//!
//! - **Sub-millisecond lookups**: In-memory cache with DashMap for lock-free access
//! - **Distributed caching**: Redis backend for multi-instance deployments
//! - **Automatic cache warming**: L1 populated from L2 on cache hits
//! - **Multiple eviction policies**: LRU, LFU, TTL, FIFO
//! - **LLM-specific caching**: Specialized key generation for chat and embedding requests
//! - **Cache statistics**: Comprehensive hit/miss tracking
//!
//! ## Boundary
//!
//! This module is the canonical deterministic cache subsystem.
//! For semantic similarity caching, use `crate::core::semantic_cache`.
//! Legacy `crate::core::cache_manager` is compatibility-only.
//!
//! ## Usage
//!
//! ### Basic DualCache Usage
//!
//! ```rust,ignore
//! use litellm_rs::core::cache::{DualCache, DualCacheConfig, CacheKey};
//!
//! // Create a memory-only cache
//! let cache: DualCache<String> = DualCache::memory_only(DualCacheConfig::default());
//!
//! // Set and get values
//! let key = CacheKey::new("my-key");
//! cache.set(key.clone(), "my-value".to_string()).await?;
//! let value = cache.get(&key).await?;
//! ```
//!
//! ### LLM-Specific Caching
//!
//! ```rust,ignore
//! use litellm_rs::core::cache::{LLMCache, LLMCacheConfig};
//!
//! // Create LLM cache
//! let cache = LLMCache::memory_only();
//!
//! // Cache chat completion
//! let response = cache.get_chat_response(&request).await?;
//! if response.is_none() {
//!     let result = call_llm(&request).await?;
//!     cache.cache_chat_response(&request, result.clone()).await?;
//! }
//! ```
//!
//! ### With Redis Backend
//!
//! ```rust,ignore
//! use litellm_rs::core::cache::{DualCache, DualCacheConfig};
//! use litellm_rs::storage::redis::RedisPool;
//!
//! let redis_pool = RedisPool::new(&redis_config).await?;
//! let config = DualCacheConfig::default();
//! let cache: DualCache<String> = DualCache::new(config, Some(Arc::new(redis_pool)));
//! ```
//!
//! ## Module Structure
//!
//! - [`types`] - Core type definitions (CacheKey, CacheEntry, CacheConfig)
//! - [`memory`] - In-memory cache implementation with DashMap
//! - [`redis_cache`] - Redis cache layer for distributed caching
//! - [`dual`] - DualCache combining both layers
//! - [`key_generator`] - Cache key generation utilities
//! - [`llm_cache`] - LLM-specific caching for chat and embeddings

pub mod cloud;
pub mod dual;
pub mod key_generator;
pub mod llm_cache;
pub mod memory;
pub mod redis_cache;
pub mod types;

// Re-export main types for convenient access
pub use dual::DualCache;
pub use key_generator::{
    CHAT_KEY_PREFIX, COMPLETION_KEY_PREFIX, CacheKeyBuilder, EMBEDDING_KEY_PREFIX,
    generate_chat_key, generate_chat_key_with_user, generate_embedding_key,
    generate_embedding_key_with_user, generate_key_from_content, generate_key_from_json,
    generate_key_from_parts,
};
pub use llm_cache::{
    CachedChatResponse, CachedEmbeddingResponse, CombinedCacheStats, LLMCache, LLMCacheConfig,
};
pub use memory::InMemoryCache;
pub use redis_cache::RedisCache;
pub use types::{
    AtomicCacheStats, CacheEntry, CacheKey, CacheMode, CacheStatsSnapshot, DualCacheConfig,
    EvictionPolicy, SerializableCacheEntry,
};

// Cloud cache re-exports
pub use cloud::{CacheMetadata, CloudCache, CloudCacheConfig};

#[cfg(feature = "s3")]
pub use cloud::{
    AzureBlobCache, AzureBlobCacheConfig, GcsCache, GcsCacheConfig, S3Cache, S3CacheConfig,
    S3StorageClass,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ==================== Integration Tests ====================

    #[tokio::test]
    async fn test_dual_cache_integration() {
        let config = DualCacheConfig::memory_only();
        let cache: DualCache<String> = DualCache::new(config, None);

        // Test basic operations
        let key = CacheKey::new("integration-test");
        cache
            .set_with_ttl(
                key.clone(),
                "test-value".to_string(),
                Duration::from_secs(60),
            )
            .await
            .unwrap();

        let result = cache.get(&key).await.unwrap();
        assert_eq!(result, Some("test-value".to_string()));

        // Test delete
        let deleted = cache.delete(&key).await.unwrap();
        assert!(deleted);

        let result = cache.get(&key).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_cache_standalone() {
        let config = DualCacheConfig::default().with_max_size(100);
        let cache: InMemoryCache<i32> = InMemoryCache::new(config);

        for i in 0..50 {
            let key = CacheKey::new(format!("key-{}", i));
            cache.set(key, i);
        }

        assert_eq!(cache.len(), 50);

        // Verify values
        for i in 0..50 {
            let key = CacheKey::new(format!("key-{}", i));
            assert_eq!(cache.get(&key), Some(i));
        }
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        use crate::core::models::openai::ChatCompletionRequest;
        use crate::core::models::openai::messages::{ChatMessage, MessageContent, MessageRole};

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            temperature: Some(0.7),
            ..Default::default()
        };

        let key1 = generate_chat_key(&request);
        let key2 = generate_chat_key(&request);

        // Same request should produce same key
        assert_eq!(key1, key2);

        // Key should be prefixed correctly
        assert!(key1.as_str().starts_with("chat:gpt-4:"));
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let config = DualCacheConfig::memory_only();
        let cache: DualCache<String> = DualCache::new(config, None);

        // Generate some activity
        let key = CacheKey::new("stats-test");

        // Miss
        let _ = cache.get(&key).await;

        // Write
        cache.set(key.clone(), "value".to_string()).await.unwrap();

        // Hit
        let _ = cache.get(&key).await;

        let stats = cache.stats();
        assert_eq!(stats.memory_hits, 1);
        assert_eq!(stats.memory_misses, 1);
        assert!(stats.hit_rate() > 0.0);
    }

    #[test]
    fn test_cache_key_builder() {
        let key = CacheKeyBuilder::new("test")
            .with_part("part1")
            .with_part("part2")
            .add_num(123)
            .build();

        assert!(key.as_str().starts_with("test:"));
    }

    #[test]
    fn test_eviction_policy_variants() {
        assert_eq!(format!("{}", EvictionPolicy::LRU), "lru");
        assert_eq!(format!("{}", EvictionPolicy::LFU), "lfu");
        assert_eq!(format!("{}", EvictionPolicy::TTL), "ttl");
        assert_eq!(format!("{}", EvictionPolicy::FIFO), "fifo");
    }

    #[test]
    fn test_cache_config_builder() {
        let config = DualCacheConfig::default()
            .with_max_size(5000)
            .with_ttl(Duration::from_secs(1800))
            .with_eviction_policy(EvictionPolicy::LFU);

        assert_eq!(config.max_size, 5000);
        assert_eq!(config.default_ttl, Duration::from_secs(1800));
        assert_eq!(config.eviction_policy, EvictionPolicy::LFU);
    }

    #[tokio::test]
    async fn test_llm_cache_integration() {
        use crate::core::models::openai::messages::{ChatMessage, MessageContent, MessageRole};
        use crate::core::models::openai::{
            ChatChoice, ChatCompletionRequest, ChatCompletionResponse, Usage,
        };

        let cache = LLMCache::memory_only();

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            ..Default::default()
        };

        let response = ChatCompletionResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text("Hello! How can I help?".to_string())),
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                    audio: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 5,
                completion_tokens: 10,
                total_tokens: 15,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }),
            system_fingerprint: None,
        };

        // Cache miss
        let result = cache.get_chat_response(&request).await.unwrap();
        assert!(result.is_none());

        // Cache response
        cache
            .cache_chat_response(&request, response.clone())
            .await
            .unwrap();

        // Cache hit
        let result = cache.get_chat_response(&request).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().id.as_str(), "chatcmpl-123");
    }
}
