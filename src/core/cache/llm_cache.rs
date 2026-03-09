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

        // TODO: Implement semantic cache lookup
        // if self.config.semantic_cache_enabled {
        //     if let Some(response) = self.semantic_lookup(request).await? {
        //         return Ok(Some(response));
        //     }
        // }

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
        let key = generate_chat_key(request);
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
mod tests {
    use super::*;
    use crate::core::models::openai::messages::{ChatMessage, MessageContent, MessageRole};
    use crate::core::models::openai::{ChatChoice, Usage};
    use std::sync::Arc;

    fn create_user_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text(content.to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }
    }

    fn create_assistant_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text(content.to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }
    }

    fn create_test_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![create_user_message("Hello")],
            ..Default::default()
        }
    }

    fn create_test_response() -> ChatCompletionResponse {
        ChatCompletionResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: create_assistant_message("Hello! How can I help you?"),
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 8,
                total_tokens: 18,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }),
            system_fingerprint: None,
        }
    }

    // ==================== LLMCache Tests ====================

    #[tokio::test]
    async fn test_llm_cache_creation() {
        let cache = LLMCache::memory_only();
        assert!(!cache.is_redis_available().await);
    }

    #[tokio::test]
    async fn test_llm_cache_chat_miss() {
        let cache = LLMCache::memory_only();
        let request = create_test_request();

        let result = cache.get_chat_response(&request).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_llm_cache_chat_hit() {
        let cache = LLMCache::memory_only();
        let request = create_test_request();
        let response = create_test_response();

        cache
            .cache_chat_response(&request, response.clone())
            .await
            .unwrap();

        let result = cache.get_chat_response(&request).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().id.as_str(), response.id.as_str());
    }

    #[tokio::test]
    async fn test_llm_cache_chat_hit_reuses_shared_payload() {
        let cache = LLMCache::memory_only();
        let request = create_test_request();
        let response = create_test_response();

        cache.cache_chat_response(&request, response).await.unwrap();

        let first = cache
            .get_chat_response(&request)
            .await
            .unwrap()
            .expect("first cache hit");
        let second = cache
            .get_chat_response(&request)
            .await
            .unwrap()
            .expect("second cache hit");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn test_llm_cache_chat_invalidate() {
        let cache = LLMCache::memory_only();
        let request = create_test_request();
        let response = create_test_response();

        cache.cache_chat_response(&request, response).await.unwrap();

        let invalidated = cache.invalidate_chat(&request).await.unwrap();
        assert!(invalidated);

        let result = cache.get_chat_response(&request).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_llm_cache_streaming_not_cached() {
        let cache = LLMCache::memory_only();
        let mut request = create_test_request();
        request.stream = Some(true);
        let response = create_test_response();

        cache.cache_chat_response(&request, response).await.unwrap();

        // Streaming requests should not be cached
        let result = cache.get_chat_response(&request).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_llm_cache_user_specific() {
        let config = LLMCacheConfig::memory_only().with_user_specific();
        let cache = LLMCache::new(config, None);
        let request = create_test_request();
        let response = create_test_response();

        // Cache with user1
        cache
            .cache_chat_response_with_user(&request, response.clone(), Some("user1"))
            .await
            .unwrap();

        // user1 should get a hit
        let result = cache
            .get_chat_response_with_user(&request, Some("user1"))
            .await
            .unwrap();
        assert!(result.is_some());

        // user2 should get a miss
        let result = cache
            .get_chat_response_with_user(&request, Some("user2"))
            .await
            .unwrap();
        assert!(result.is_none());
    }

    // ==================== Embedding Cache Tests ====================

    #[tokio::test]
    async fn test_llm_cache_embedding() {
        let cache = LLMCache::memory_only();

        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: serde_json::json!("Test input"),
            user: None,
        };

        let response = EmbeddingResponse {
            object: "list".to_string(),
            data: vec![],
            model: "text-embedding-ada-002".to_string(),
            usage: crate::core::models::openai::EmbeddingUsage {
                prompt_tokens: 3,
                total_tokens: 3,
            },
        };

        cache
            .cache_embedding_response(&request, response.clone())
            .await
            .unwrap();

        let result = cache.get_embedding_response(&request).await.unwrap();
        assert!(result.is_some());
        assert_eq!(
            result.as_ref().unwrap().model.as_str(),
            response.model.as_str()
        );
    }

    #[tokio::test]
    async fn test_llm_cache_embedding_hit_reuses_shared_payload() {
        let cache = LLMCache::memory_only();

        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: serde_json::json!("Test input"),
            user: None,
        };

        let response = EmbeddingResponse {
            object: "list".to_string(),
            data: vec![],
            model: "text-embedding-ada-002".to_string(),
            usage: crate::core::models::openai::EmbeddingUsage {
                prompt_tokens: 3,
                total_tokens: 3,
            },
        };

        cache
            .cache_embedding_response(&request, response)
            .await
            .unwrap();

        let first = cache
            .get_embedding_response(&request)
            .await
            .unwrap()
            .expect("first cache hit");
        let second = cache
            .get_embedding_response(&request)
            .await
            .unwrap()
            .expect("second cache hit");

        assert!(Arc::ptr_eq(&first, &second));
    }

    // ==================== Statistics Tests ====================

    #[tokio::test]
    async fn test_llm_cache_stats() {
        let cache = LLMCache::memory_only();
        let request = create_test_request();
        let response = create_test_response();

        // Generate some activity
        let _ = cache.get_chat_response(&request).await; // miss
        cache.cache_chat_response(&request, response).await.unwrap(); // write
        let _ = cache.get_chat_response(&request).await; // hit

        let stats = cache.chat_stats();
        assert_eq!(stats.memory_hits, 1);
        assert_eq!(stats.memory_misses, 1);
    }

    #[tokio::test]
    async fn test_llm_cache_combined_stats() {
        let cache = LLMCache::memory_only();

        let combined = cache.combined_stats();
        assert_eq!(combined.total_hits(), 0);
        assert_eq!(combined.hit_rate(), 0.0);
    }

    // ==================== Clear Tests ====================

    #[tokio::test]
    async fn test_llm_cache_clear() {
        let cache = LLMCache::memory_only();
        let request = create_test_request();
        let response = create_test_response();

        cache.cache_chat_response(&request, response).await.unwrap();

        cache.clear().await.unwrap();

        let result = cache.get_chat_response(&request).await.unwrap();
        assert!(result.is_none());
    }

    // ==================== CachedChatResponse Tests ====================

    #[test]
    fn test_cached_chat_response() {
        let response = create_test_response();
        let cached = CachedChatResponse::new(response.clone(), "gpt-4".to_string());

        assert!(cached.cached);
        assert_eq!(cached.model, "gpt-4");
        assert!(cached.cached_at > 0);

        let shared = cached.response_arc();
        assert!(Arc::ptr_eq(&shared, &cached.response));
        assert_eq!(shared.id.as_str(), response.id.as_str());
    }

    // ==================== LLMCacheConfig Tests ====================

    #[test]
    fn test_llm_cache_config_default() {
        let config = LLMCacheConfig::default();
        assert_eq!(config.chat_ttl, Duration::from_secs(3600));
        assert_eq!(config.embedding_ttl, Duration::from_secs(86400));
        assert!(!config.user_specific);
    }

    #[test]
    fn test_llm_cache_config_builder() {
        let config = LLMCacheConfig::default()
            .with_chat_ttl(Duration::from_secs(1800))
            .with_embedding_ttl(Duration::from_secs(7200))
            .with_user_specific();

        assert_eq!(config.chat_ttl, Duration::from_secs(1800));
        assert_eq!(config.embedding_ttl, Duration::from_secs(7200));
        assert!(config.user_specific);
    }
}
