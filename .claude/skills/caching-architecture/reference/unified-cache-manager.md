## Contents

- Unified Cache Manager

## Unified Cache Manager

```rust
pub struct CacheManager {
    l1_cache: Option<Arc<InMemoryCache>>,
    l2_cache: Option<Arc<RedisCache>>,
    l3_cache: Option<Arc<dyn VectorCache>>,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    config: CacheConfig,
}

impl CacheManager {
    pub async fn new(config: CacheConfig) -> Result<Self, CacheError> {
        let l1_cache = if config.l1_enabled {
            Some(Arc::new(InMemoryCache::new(config.l1_max_size)))
        } else {
            None
        };

        let l2_cache = if config.l2_enabled {
            Some(Arc::new(RedisCache::new(&config.redis_url, &config.cache_prefix).await?))
        } else {
            None
        };

        let l3_cache: Option<Arc<dyn VectorCache>> = if config.l3_enabled {
            Some(Arc::new(QdrantCache::new(&config.qdrant_url, &config.collection_name, config.vector_size).await?))
        } else {
            None
        };

        Ok(Self {
            l1_cache,
            l2_cache,
            l3_cache,
            embedding_provider: None,
            config,
        })
    }

    pub async fn get(&self, request: &ChatRequest) -> Result<Option<ChatResponse>, CacheError> {
        let key = CacheKeyGenerator::generate_key(request);

        // L1: In-memory cache
        if let Some(l1) = &self.l1_cache {
            if let Some(bytes) = l1.get(&key) {
                let response: ChatResponse = serde_json::from_slice(&bytes)?;
                return Ok(Some(response));
            }
        }

        // L2: Redis cache
        if let Some(l2) = &self.l2_cache {
            if let Some(response) = l2.get_json::<ChatResponse>(&key).await? {
                // Populate L1
                if let Some(l1) = &self.l1_cache {
                    let bytes = serde_json::to_vec(&response)?;
                    l1.set(key.clone(), bytes, Some(self.config.l1_ttl));
                }
                return Ok(Some(response));
            }
        }

        // L3: Semantic cache
        if let (Some(l3), Some(embedding_provider)) = (&self.l3_cache, &self.embedding_provider) {
            let semantic_key = CacheKeyGenerator::generate_semantic_key(request);
            let embedding = embedding_provider.embed(&semantic_key).await?;

            let hits = l3.search(&embedding, 1, self.config.similarity_threshold).await?;

            if let Some(hit) = hits.first() {
                let response: ChatResponse = serde_json::from_slice(&hit.value)?;

                // Populate L1 and L2
                let bytes = serde_json::to_vec(&response)?;
                if let Some(l1) = &self.l1_cache {
                    l1.set(key.clone(), bytes.clone(), Some(self.config.l1_ttl));
                }
                if let Some(l2) = &self.l2_cache {
                    l2.set(&key, &bytes, Some(self.config.l2_ttl)).await?;
                }

                return Ok(Some(response));
            }
        }

        Ok(None)
    }

    pub async fn set(&self, request: &ChatRequest, response: &ChatResponse) -> Result<(), CacheError> {
        let key = CacheKeyGenerator::generate_key(request);
        let bytes = serde_json::to_vec(response)?;

        // L1
        if let Some(l1) = &self.l1_cache {
            l1.set(key.clone(), bytes.clone(), Some(self.config.l1_ttl));
        }

        // L2
        if let Some(l2) = &self.l2_cache {
            l2.set(&key, &bytes, Some(self.config.l2_ttl)).await?;
        }

        // L3 (semantic)
        if let (Some(l3), Some(embedding_provider)) = (&self.l3_cache, &self.embedding_provider) {
            let semantic_key = CacheKeyGenerator::generate_semantic_key(request);
            let embedding = embedding_provider.embed(&semantic_key).await?;

            let metadata = CacheMetadata {
                model: request.model.clone(),
                created_at: chrono::Utc::now().timestamp(),
                token_count: response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0),
            };

            l3.insert(&key, &embedding, &bytes, &metadata).await?;
        }

        Ok(())
    }

    pub async fn invalidate(&self, request: &ChatRequest) -> Result<(), CacheError> {
        let key = CacheKeyGenerator::generate_key(request);

        if let Some(l1) = &self.l1_cache {
            l1.invalidate(&key);
        }

        if let Some(l2) = &self.l2_cache {
            l2.delete(&key).await?;
        }

        if let Some(l3) = &self.l3_cache {
            l3.delete(&key).await?;
        }

        Ok(())
    }
}
```
