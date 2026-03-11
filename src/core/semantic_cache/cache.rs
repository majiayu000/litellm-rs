//! Core semantic cache implementation

use super::types::{
    CacheData, CacheStats, EmbeddingProvider, SemanticCacheConfig, SemanticCacheEntry,
};
use super::utils::{extract_prompt_text, hash_prompt};
use super::validation::{is_entry_valid, should_cache_request};
use crate::core::models::openai::{ChatCompletionRequest, ChatCompletionResponse};
use crate::storage::vector::VectorStore;
use crate::utils::error::gateway_error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Semantic cache implementation
pub struct SemanticCache {
    /// Cache configuration
    config: SemanticCacheConfig,
    /// Vector store for embeddings
    vector_store: Arc<dyn VectorStore>,
    /// Embedding provider for generating embeddings
    embedding_provider: Arc<dyn EmbeddingProvider>,
    /// Consolidated cache data - single lock for cache entries and statistics
    cache_data: Arc<RwLock<CacheData>>,
}

impl SemanticCache {
    /// Create a new semantic cache
    pub async fn new(
        config: SemanticCacheConfig,
        vector_store: Arc<dyn VectorStore>,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Result<Self> {
        info!(
            "Initializing semantic cache with threshold: {}",
            config.similarity_threshold
        );

        Ok(Self {
            config,
            vector_store,
            embedding_provider,
            cache_data: Arc::new(RwLock::new(CacheData::default())),
        })
    }

    /// Try to get a cached response for the given request
    pub async fn get_cached_response(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<Option<ChatCompletionResponse>> {
        // Check if caching is appropriate for this request
        if !should_cache_request(&self.config, request) {
            return Ok(None);
        }

        // Generate prompt text for embedding
        let prompt_text = extract_prompt_text(&request.messages);

        if prompt_text.len() < self.config.min_prompt_length {
            debug!("Prompt too short for caching: {} chars", prompt_text.len());
            return Ok(None);
        }

        // Generate embedding for the prompt
        let embedding = match self
            .embedding_provider
            .generate_embedding(&prompt_text)
            .await
        {
            Ok(emb) => emb,
            Err(e) => {
                warn!("Failed to generate embedding for cache lookup: {}", e);
                return Ok(None);
            }
        };

        // Search for similar entries in vector store
        let search_results = self.vector_store.search(embedding, 10).await?;

        // Find the best match
        for result in search_results {
            if result.score >= self.config.similarity_threshold as f32
                && let Some(entry) = self.get_cache_entry(&result.id).await?
            {
                // Check if entry is still valid
                if is_entry_valid(&entry) {
                    // Update access and hit statistics with single lock
                    {
                        let mut data = self.cache_data.write().await;
                        if let Some(cache_entry) = data.entries.get_mut(&result.id) {
                            cache_entry.last_accessed = chrono::Utc::now();
                            cache_entry.access_count += 1;
                        }
                        data.stats.hits += 1;
                        data.stats.avg_hit_similarity = (data.stats.avg_hit_similarity
                            * (data.stats.hits - 1) as f64
                            + result.score as f64)
                            / data.stats.hits as f64;
                    }

                    info!(
                        "Cache hit! Similarity: {:.3}, Entry: {}",
                        result.score, result.id
                    );
                    return Ok(Some(entry.response));
                } else {
                    // Remove expired entry
                    self.remove_cache_entry(&result.id).await?;
                }
            }
        }

        // No cache hit
        {
            let mut data = self.cache_data.write().await;
            data.stats.misses += 1;
        }

        debug!(
            "Cache miss for prompt: {}",
            prompt_text.chars().take(100).collect::<String>()
        );
        Ok(None)
    }

    /// Cache a response for the given request
    pub async fn cache_response(
        &self,
        request: &ChatCompletionRequest,
        response: &ChatCompletionResponse,
    ) -> Result<()> {
        // Check if caching is appropriate
        if !should_cache_request(&self.config, request) {
            return Ok(());
        }

        let prompt_text = extract_prompt_text(&request.messages);

        if prompt_text.len() < self.config.min_prompt_length {
            return Ok(());
        }

        // Generate embedding for the prompt
        let embedding = self
            .embedding_provider
            .generate_embedding(&prompt_text)
            .await?;

        // Create cache entry
        let entry = SemanticCacheEntry {
            id: Uuid::new_v4().to_string(),
            prompt_hash: hash_prompt(&prompt_text),
            embedding: embedding.clone(),
            response: response.clone(),
            model: request.model.clone(),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
            ttl_seconds: Some(self.config.default_ttl_seconds),
            metadata: HashMap::new(),
        };

        // Store in vector store
        let vector_data = crate::storage::vector::VectorData {
            id: entry.id.clone(),
            vector: embedding,
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert(
                    "prompt_hash".to_string(),
                    serde_json::to_value(&entry.prompt_hash)?,
                );
                metadata.insert(
                    "created_at".to_string(),
                    serde_json::to_value(entry.created_at)?,
                );
                metadata
            },
        };
        self.vector_store.insert(vec![vector_data]).await?;

        // Store in memory cache and update statistics with single lock
        let should_evict = {
            let mut data = self.cache_data.write().await;
            data.entries.insert(entry.id.clone(), entry);
            data.stats.total_entries += 1;
            data.entries.len() > self.config.max_cache_size
        };

        // Check cache size limits (eviction outside lock)
        if should_evict {
            self.evict_old_entries().await?;
        }

        info!("Cached response for model: {}", request.model);
        Ok(())
    }

    /// Get cache entry by ID
    async fn get_cache_entry(&self, entry_id: &str) -> Result<Option<SemanticCacheEntry>> {
        let data = self.cache_data.read().await;
        Ok(data.entries.get(entry_id).cloned())
    }

    /// Remove cache entry
    async fn remove_cache_entry(&self, entry_id: &str) -> Result<()> {
        // Remove from memory cache
        {
            let mut data = self.cache_data.write().await;
            data.entries.remove(entry_id);
        }

        // Remove from vector store
        self.vector_store.delete(vec![entry_id.to_string()]).await?;

        Ok(())
    }

    /// Evict old entries when cache is full
    async fn evict_old_entries(&self) -> Result<()> {
        let entries_to_remove: Vec<String> = {
            let data = self.cache_data.read().await;

            // Sort entries by last access time and remove oldest 10%
            let mut entries: Vec<_> = data
                .entries
                .iter()
                .map(|(k, v)| (k.clone(), v.last_accessed))
                .collect();
            entries.sort_by_key(|(_, last_accessed)| *last_accessed);

            let evict_count = (entries.len() as f64 * 0.1).ceil() as usize;
            entries
                .iter()
                .take(evict_count)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let evict_count = entries_to_remove.len();

        // Remove from cache
        {
            let mut data = self.cache_data.write().await;
            for entry_id in &entries_to_remove {
                data.entries.remove(entry_id);
            }
        }

        // Also remove from vector store (async)
        for entry_id in entries_to_remove {
            let vector_store = self.vector_store.clone();
            tokio::spawn(async move {
                if let Err(e) = vector_store.delete(vec![entry_id]).await {
                    warn!("Failed to delete entry from vector store: {}", e);
                }
            });
        }

        info!("Evicted {} old cache entries", evict_count);
        Ok(())
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.cache_data.read().await.stats.clone()
    }

    /// Clear all cache entries
    pub async fn clear_cache(&self) -> Result<()> {
        // Clear cache and reset statistics with single lock
        {
            let mut data = self.cache_data.write().await;
            data.entries.clear();
            data.stats = CacheStats::default();
        }

        // Note: Vector store doesn't have clear_all method in current implementation
        // In a full implementation, you would delete all vectors or recreate the collection

        info!("Cleared all cache entries");
        Ok(())
    }
}
