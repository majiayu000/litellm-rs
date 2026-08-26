## Contents

- Status: Deprecated and Unwired
- Module Surface
- Vector Storage Reality

## Status: Deprecated and Unwired

`core::semantic_cache` (src/core/semantic_cache/) is **not reachable at runtime**:

- `cache.semantic_cache: true` fails startup validation: "Semantic cache is not wired into runtime request handling" (src/config/validation/cache_validators.rs:16). The flag must stay `false`.
- `SemanticCache`, `SemanticCacheConfig`, `SemanticCacheEntry`, `CacheStats`, and `EmbeddingProvider` are all marked `#[deprecated(since = "0.6.0")]` with removal scheduled for 0.7.0 (src/core/semantic_cache/mod.rs).
- The deterministic cache's own hook is a stub comment: "NOTE: Semantic cache lookup not yet implemented." (src/core/cache/llm_cache.rs:299).

Do not build on this module. It survives only so its design can be resurrected later; this page documents what actually exists in the tree.

## Module Surface

```rust
pub struct SemanticCache {                    // src/core/semantic_cache/cache.rs:18
    config: SemanticCacheConfig,
    vector_store: Arc<dyn VectorStore>,       // src/storage/vector/types.rs:45
    embedding_provider: Arc<dyn EmbeddingProvider>,
    cache_data: Arc<RwLock<CacheData>>,       // entries HashMap + CacheStats under one lock
}
```

- `get_cached_response(&ChatCompletionRequest)` — embeds the prompt, searches the vector store, returns the first hit at or above `similarity_threshold` whose entry passes TTL validation; updates access stats.
- `cache_response(&request, &response)` — stores a `SemanticCacheEntry` in both the vector store and the in-memory map; evicts the oldest 10% by `last_accessed` when over `max_cache_size`.
- `get_stats()` / `clear_cache()`.

Its own config defaults (`SemanticCacheConfig::default`, types.rs:51): `similarity_threshold: 0.85`, `max_cache_size: 10000`, `default_ttl_seconds: 3600`, `embedding_model: "text-embedding-ada-002"`, `enable_streaming_cache: false`, `min_prompt_length: 10`. Note these differ from the top-level YAML `cache.similarity_threshold` (0.95), which feeds only the unwired `LLMCacheConfig.similarity_threshold`.

Request filtering lives in `should_cache_request` (validation.rs:7): skip streaming unless `enable_streaming_cache`, skip requests with `tools` or `tool_choice`, skip `temperature > 0.7`. Helpers: `extract_prompt_text` and `hash_prompt` (utils.rs) join text parts and SHA-256 them.

## Vector Storage Reality

- The `VectorStore` trait (src/storage/vector/types.rs:45 — `search(vector, limit)`, `insert(Vec<VectorData>)`, `delete(ids)`) currently has **no implementors** in the tree.
- `QdrantStore` (src/storage/vector/qdrant.rs:12) is a plain reqwest REST client with inherent methods `store` / `search` / `get` / `delete` / `batch_store` / `count` / `health_check`; on first use it creates the collection with vector size 1536 and cosine distance. It does not implement `VectorStore`.
- `VectorStoreBackend` (src/storage/vector/backend.rs:17) dispatches on `VectorDbConfig.db_type`; only `"qdrant"` works — `"weaviate"` and `"pinecone"` return a "declared but not implemented yet" error.
- `VectorDbConfig` (src/config/models/file_storage.rs:84) fields: `db_type`, `url`, `api_key`, `index_name`, `allow_degraded`.
