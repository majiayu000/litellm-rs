//! LLM-specific caching layer
//!
//! This module provides high-level caching functionality specifically designed
//! for LLM requests and responses, including chat completions and embeddings.

use super::dual::DualCache;
use super::key_generator::{
    generate_chat_key, generate_chat_key_with_user, generate_embedding_key,
};
use super::types::{CacheKey, CacheStatsSnapshot, DualCacheConfig};
use crate::core::models::openai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
};
use crate::storage::redis::RedisPool;
use crate::utils::error::gateway_error::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, trace};

/// LLM-specific cache wrapping DualCache
///
/// Provides convenient methods for caching LLM requests and responses
/// with automatic key generation and serialization.
pub struct LLMCache {
    /// Chat completion cache
    chat_cache: DualCache<CachedChatResponse>,
    /// Embedding cache
    embedding_cache: DualCache<CachedEmbeddingResponse>,
    /// Configuration
    config: LLMCacheConfig,
}

/// Configuration for LLM cache
#[derive(Debug, Clone)]
pub struct LLMCacheConfig {
    /// Base cache configuration
    pub cache_config: DualCacheConfig,
    /// TTL for chat completions
    pub chat_ttl: Duration,
    /// TTL for embeddings
    pub embedding_ttl: Duration,
    /// Enable user-specific caching
    pub user_specific: bool,
    /// Enable semantic similarity caching (future feature)
    pub semantic_cache_enabled: bool,
    /// Similarity threshold for semantic cache
    pub similarity_threshold: f64,
}

impl Default for LLMCacheConfig {
    fn default() -> Self {
        Self {
            cache_config: DualCacheConfig::default(),
            chat_ttl: Duration::from_secs(3600),       // 1 hour
            embedding_ttl: Duration::from_secs(86400), // 24 hours (embeddings are deterministic)
            user_specific: false,
            semantic_cache_enabled: false,
            similarity_threshold: 0.95,
        }
    }
}

impl LLMCacheConfig {
    /// Create a memory-only configuration
    pub fn memory_only() -> Self {
        Self {
            cache_config: DualCacheConfig::memory_only(),
            ..Default::default()
        }
    }

    /// Set the chat TTL
    pub fn with_chat_ttl(mut self, ttl: Duration) -> Self {
        self.chat_ttl = ttl;
        self
    }

    /// Set the embedding TTL
    pub fn with_embedding_ttl(mut self, ttl: Duration) -> Self {
        self.embedding_ttl = ttl;
        self
    }

    /// Enable user-specific caching
    pub fn with_user_specific(mut self) -> Self {
        self.user_specific = true;
        self
    }
}

fn serialize_chat_response_arc<S>(
    response: &Arc<ChatCompletionResponse>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    response.as_ref().serialize(serializer)
}

fn deserialize_chat_response_arc<'de, D>(
    deserializer: D,
) -> std::result::Result<Arc<ChatCompletionResponse>, D::Error>
where
    D: Deserializer<'de>,
{
    ChatCompletionResponse::deserialize(deserializer).map(Arc::new)
}

fn serialize_embedding_response_arc<S>(
    response: &Arc<EmbeddingResponse>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    response.as_ref().serialize(serializer)
}

fn deserialize_embedding_response_arc<'de, D>(
    deserializer: D,
) -> std::result::Result<Arc<EmbeddingResponse>, D::Error>
where
    D: Deserializer<'de>,
{
    EmbeddingResponse::deserialize(deserializer).map(Arc::new)
}

/// Cached chat response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedChatResponse {
    /// The original response
    #[serde(
        serialize_with = "serialize_chat_response_arc",
        deserialize_with = "deserialize_chat_response_arc"
    )]
    pub response: Arc<ChatCompletionResponse>,
    /// Model used for the request
    pub model: String,
    /// Whether this was a cached response
    pub cached: bool,
    /// Cache timestamp
    pub cached_at: u64,
}

impl CachedChatResponse {
    /// Create a new cached response
    pub fn new(response: ChatCompletionResponse, model: String) -> Self {
        Self::from_arc_response(Arc::new(response), model)
    }

    /// Create a new cached response from a shared payload
    pub fn from_arc_response(response: Arc<ChatCompletionResponse>, model: String) -> Self {
        Self {
            response,
            model,
            cached: true,
            cached_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Clone the shared response payload
    pub fn response_arc(&self) -> Arc<ChatCompletionResponse> {
        Arc::clone(&self.response)
    }

    /// Get the shared response payload
    pub fn into_response_arc(self) -> Arc<ChatCompletionResponse> {
        self.response
    }

    /// Get the underlying response
    pub fn into_response(self) -> ChatCompletionResponse {
        Arc::try_unwrap(self.response).unwrap_or_else(|response| (*response).clone())
    }
}

/// Cached embedding response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEmbeddingResponse {
    /// The original response
    #[serde(
        serialize_with = "serialize_embedding_response_arc",
        deserialize_with = "deserialize_embedding_response_arc"
    )]
    pub response: Arc<EmbeddingResponse>,
    /// Model used for the request
    pub model: String,
    /// Whether this was a cached response
    pub cached: bool,
    /// Cache timestamp
    pub cached_at: u64,
}

impl CachedEmbeddingResponse {
    /// Create a new cached response
    pub fn new(response: EmbeddingResponse, model: String) -> Self {
        Self::from_arc_response(Arc::new(response), model)
    }

    /// Create a new cached response from a shared payload
    pub fn from_arc_response(response: Arc<EmbeddingResponse>, model: String) -> Self {
        Self {
            response,
            model,
            cached: true,
            cached_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Clone the shared response payload
    pub fn response_arc(&self) -> Arc<EmbeddingResponse> {
        Arc::clone(&self.response)
    }

    /// Get the shared response payload
    pub fn into_response_arc(self) -> Arc<EmbeddingResponse> {
        self.response
    }

    /// Get the underlying response
    pub fn into_response(self) -> EmbeddingResponse {
        Arc::try_unwrap(self.response).unwrap_or_else(|response| (*response).clone())
    }
}

impl LLMCache {
    /// Create a new LLM cache with the given configuration
    pub fn new(config: LLMCacheConfig, redis_pool: Option<Arc<RedisPool>>) -> Self {
        let chat_cache = DualCache::new(config.cache_config.clone(), redis_pool.clone());
        let embedding_cache = DualCache::new(config.cache_config.clone(), redis_pool);

        Self {
            chat_cache,
            embedding_cache,
            config,
        }
    }

    /// Create a memory-only LLM cache
    pub fn memory_only() -> Self {
        Self::new(LLMCacheConfig::memory_only(), None)
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(LLMCacheConfig::default(), None)
    }

    /// Start background cleanup tasks
    pub fn start_cleanup_tasks(&self) {
        self.chat_cache.start_cleanup_task();
        self.embedding_cache.start_cleanup_task();
    }

    // ==================== Chat Completion Methods ====================

    /// Get a cached chat completion response
    pub async fn get_chat_response(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<Option<Arc<ChatCompletionResponse>>> {
        self.get_chat_response_with_user(request, None).await
    }

    /// Get a cached chat completion response with user ID
    pub async fn get_chat_response_with_user(
        &self,
        request: &ChatCompletionRequest,
        user_id: Option<&str>,
    ) -> Result<Option<Arc<ChatCompletionResponse>>> {
        // Don't cache streaming requests
        if request.stream.unwrap_or(false) {
            return Ok(None);
        }

        let key = if self.config.user_specific {
            generate_chat_key_with_user(request, user_id)
        } else {
            generate_chat_key(request)
        };

        if let Some(cached) = self.chat_cache.get(&key).await? {
            trace!(
                model = %cached.model,
                key = %key,
                "Chat cache hit"
            );
            return Ok(Some(cached.response_arc()));
        }

        // NOTE: Semantic cache lookup not yet implemented.

        Ok(None)
    }

    /// Cache a chat completion response
    pub async fn cache_chat_response(
        &self,
        request: &ChatCompletionRequest,
        response: ChatCompletionResponse,
    ) -> Result<()> {
        self.cache_chat_response_with_user(request, response, None)
            .await
    }

    /// Cache a chat completion response with user ID
    pub async fn cache_chat_response_with_user(
        &self,
        request: &ChatCompletionRequest,
        response: ChatCompletionResponse,
        user_id: Option<&str>,
    ) -> Result<()> {
        // Don't cache streaming requests
        if request.stream.unwrap_or(false) {
            return Ok(());
        }

        let key = if self.config.user_specific {
            generate_chat_key_with_user(request, user_id)
        } else {
            generate_chat_key(request)
        };

        let cached = CachedChatResponse::new(response, request.model.clone());
        self.chat_cache
            .set_with_ttl(key.clone(), cached, self.config.chat_ttl)
            .await?;

        trace!(
            model = %request.model,
            key = %key,
            ttl_secs = self.config.chat_ttl.as_secs(),
            "Chat response cached"
        );

        Ok(())
    }

    /// Invalidate a cached chat response
    pub async fn invalidate_chat(&self, request: &ChatCompletionRequest) -> Result<bool> {
        self.invalidate_chat_with_user(request, None).await
    }

    /// Invalidate a cached chat response, honoring `config.user_specific`
    ///
    /// When user-specific caching is enabled, the cache is keyed by user id.
    /// Invalidation must use the same key, or the per-user entry stays live
    /// until its TTL expires — leaking stale responses across users.
    pub async fn invalidate_chat_with_user(
        &self,
        request: &ChatCompletionRequest,
        user_id: Option<&str>,
    ) -> Result<bool> {
        let key = if self.config.user_specific {
            generate_chat_key_with_user(request, user_id)
        } else {
            generate_chat_key(request)
        };
        self.chat_cache.delete(&key).await
    }

    /// Invalidate a cached chat response only while it still matches `expected`.
    ///
    /// Key-only deletes can remove a concurrent replacement that another request
    /// already stored after invalidating the same poisoned entry. Matching on
    /// completion identity (`id` + `created` + `model`) skips the delete when the
    /// live value is no longer the rejected payload.
    pub async fn invalidate_chat_with_user_matching(
        &self,
        request: &ChatCompletionRequest,
        user_id: Option<&str>,
        expected: &ChatCompletionResponse,
    ) -> Result<bool> {
        let key = if self.config.user_specific {
            generate_chat_key_with_user(request, user_id)
        } else {
            generate_chat_key(request)
        };

        let Some(current) = self.chat_cache.get(&key).await? else {
            return Ok(false);
        };
        if !chat_response_matches_cached(current.response.as_ref(), expected) {
            return Ok(false);
        }
        self.chat_cache.delete(&key).await
    }

    // ==================== Embedding Methods ====================

    /// Get a cached embedding response
    pub async fn get_embedding_response(
        &self,
        request: &EmbeddingRequest,
    ) -> Result<Option<Arc<EmbeddingResponse>>> {
        let key = generate_embedding_key(request);

        if let Some(cached) = self.embedding_cache.get(&key).await? {
            trace!(
                model = %cached.model,
                key = %key,
                "Embedding cache hit"
            );
            return Ok(Some(cached.response_arc()));
        }

        Ok(None)
    }

    /// Cache an embedding response
    pub async fn cache_embedding_response(
        &self,
        request: &EmbeddingRequest,
        response: EmbeddingResponse,
    ) -> Result<()> {
        let key = generate_embedding_key(request);
        let cached = CachedEmbeddingResponse::new(response, request.model.clone());

        self.embedding_cache
            .set_with_ttl(key.clone(), cached, self.config.embedding_ttl)
            .await?;

        trace!(
            model = %request.model,
            key = %key,
            ttl_secs = self.config.embedding_ttl.as_secs(),
            "Embedding response cached"
        );

        Ok(())
    }

    /// Invalidate a cached embedding response
    pub async fn invalidate_embedding(&self, request: &EmbeddingRequest) -> Result<bool> {
        let key = generate_embedding_key(request);
        self.embedding_cache.delete(&key).await
    }

    // ==================== Generic Methods ====================

    /// Get a value by key directly
    pub async fn get<T>(&self, _key: &CacheKey) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
    {
        // Use a generic cache for arbitrary types
        // For now, this is a placeholder - in production you'd want a separate cache
        Ok(None)
    }

    /// Set a value by key directly
    pub async fn set<T>(&self, _key: CacheKey, _value: T, _ttl: Duration) -> Result<()>
    where
        T: serde::Serialize + Clone + Send + Sync + 'static,
    {
        // Placeholder for generic cache operations
        Ok(())
    }

    // ==================== Statistics and Management ====================

    /// Get chat cache statistics
    pub fn chat_stats(&self) -> CacheStatsSnapshot {
        self.chat_cache.stats()
    }

    /// Get embedding cache statistics
    pub fn embedding_stats(&self) -> CacheStatsSnapshot {
        self.embedding_cache.stats()
    }

    /// Get combined statistics
    pub fn combined_stats(&self) -> CombinedCacheStats {
        CombinedCacheStats {
            chat: self.chat_cache.stats(),
            embedding: self.embedding_cache.stats(),
        }
    }

    /// Clear all caches
    pub async fn clear(&self) -> Result<()> {
        self.chat_cache.clear().await?;
        self.embedding_cache.clear().await?;
        info!("LLM caches cleared");
        Ok(())
    }

    /// Clear chat cache only
    pub async fn clear_chat(&self) -> Result<()> {
        self.chat_cache.clear().await
    }

    /// Clear embedding cache only
    pub async fn clear_embedding(&self) -> Result<()> {
        self.embedding_cache.clear().await
    }

    /// Check if Redis is available
    pub async fn is_redis_available(&self) -> bool {
        self.chat_cache.is_redis_available().await
    }

    /// Get the configuration
    pub fn config(&self) -> &LLMCacheConfig {
        &self.config
    }

    /// Shutdown the cache
    pub fn shutdown(&self) {
        self.chat_cache.shutdown();
        self.embedding_cache.shutdown();
    }
}

/// True when `current` is still the same completion that `expected` rejected.
///
/// Completion `id` is unique per provider response; `created` and `model` guard
/// against accidental id reuse across stores.
fn chat_response_matches_cached(
    current: &ChatCompletionResponse,
    expected: &ChatCompletionResponse,
) -> bool {
    current.id == expected.id
        && current.created == expected.created
        && current.model == expected.model
}

/// Combined cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedCacheStats {
    /// Chat cache statistics
    pub chat: CacheStatsSnapshot,
    /// Embedding cache statistics
    pub embedding: CacheStatsSnapshot,
}

impl CombinedCacheStats {
    /// Get total hits across all caches
    pub fn total_hits(&self) -> u64 {
        self.chat.total_hits() + self.embedding.total_hits()
    }

    /// Get total misses across all caches
    pub fn total_misses(&self) -> u64 {
        self.chat.total_misses() + self.embedding.total_misses()
    }

    /// Get combined hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_hits() + self.total_misses();
        if total == 0 {
            0.0
        } else {
            self.total_hits() as f64 / total as f64
        }
    }
}

#[cfg(test)]
#[path = "llm_cache_tests.rs"]
mod tests;
