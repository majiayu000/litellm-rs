---
name: caching-architecture
description: LiteLLM-RS Caching Architecture. Covers Redis caching, vector database semantic caching, multi-tier cache strategy, TTL management, and cache invalidation patterns. Use when adding or tuning gateway response caching — cache keys, the L1 in-memory/L2 Redis/L3 semantic tiers, TTLs and eviction, cache metrics, or cache invalidation.
---

# Caching Architecture Guide

## Overview

LiteLLM-RS implements a multi-tier caching system with Redis for exact-match caching and vector databases for semantic caching, optimizing both latency and cost.

### Cache Tiers

```
┌─────────────────────────────────────────────────────────────────┐
│                       Request                                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    L1: In-Memory Cache                          │
│  - LRU eviction                                                 │
│  - Microsecond latency                                          │
│  - Limited size (~10K entries)                                  │
└─────────────────────────────────────────────────────────────────┘
                              │ Miss
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    L2: Redis Cache                              │
│  - Exact match on request hash                                  │
│  - Millisecond latency                                          │
│  - TTL-based expiration                                         │
└─────────────────────────────────────────────────────────────────┘
                              │ Miss
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                 L3: Semantic Cache (Vector DB)                  │
│  - Similarity search on embeddings                              │
│  - Configurable similarity threshold                            │
│  - Qdrant/Weaviate/Pinecone backends                           │
└─────────────────────────────────────────────────────────────────┘
                              │ Miss
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    LLM Provider                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Configuration

```yaml
cache:
  enabled: true

  l1:
    enabled: true
    max_size: 10000
    ttl_seconds: 300

  l2:
    enabled: true
    redis_url: ${REDIS_URL}
    prefix: "litellm"
    ttl_seconds: 3600

  l3:
    enabled: true
    type: "qdrant"  # or "weaviate", "pinecone"
    url: ${QDRANT_URL}
    collection_name: "semantic_cache"
    vector_size: 1536
    similarity_threshold: 0.95
    ttl_seconds: 86400

  # Models to exclude from caching
  exclude_models:
    - "gpt-4-turbo-preview"  # Rapidly changing model

  # Skip caching for streaming requests
  skip_streaming: true
```

---

## Best Practices

### 1. Cache Key Determinism

Always ensure cache keys are deterministic for the same logical request:

```rust
// Good - deterministic key
fn generate_key(request: &ChatRequest) -> String {
    let normalized_messages: Vec<_> = request.messages
        .iter()
        .map(|m| (m.role.to_string(), m.content.clone()))
        .collect();
    // ...
}

// Bad - includes non-deterministic elements
fn generate_key(request: &ChatRequest) -> String {
    format!("{}-{}", request.model, uuid::Uuid::new_v4())
}
```

### 2. TTL Strategy

Use appropriate TTLs based on content volatility:

```rust
fn get_ttl_for_model(model: &str) -> Duration {
    match model {
        // Stable models - longer TTL
        "gpt-3.5-turbo" => Duration::from_secs(86400),  // 24 hours
        // Preview/beta models - shorter TTL
        _ if model.contains("preview") => Duration::from_secs(3600),  // 1 hour
        // Default
        _ => Duration::from_secs(43200),  // 12 hours
    }
}
```

### 3. Skip Caching When Appropriate

```rust
fn should_cache(request: &ChatRequest) -> bool {
    // Don't cache streaming requests
    if request.stream {
        return false;
    }

    // Don't cache if temperature > 0 (non-deterministic)
    if request.temperature.unwrap_or(1.0) > 0.0 {
        return false;
    }

    // Don't cache tool calls (may have side effects)
    if request.tools.is_some() {
        return false;
    }

    true
}
```
## References

- [reference/cache-key-generation.md](reference/cache-key-generation.md) — Deterministic SHA-256 request hashing and semantic cache key construction.
- [reference/in-memory-cache.md](reference/in-memory-cache.md) — L1 LRU in-memory cache with TTL expiration and eviction counters.
- [reference/redis-cache.md](reference/redis-cache.md) — L2 Redis cache manager: get/set/JSON helpers, batch get, pattern-based deletion.
- [reference/semantic-cache.md](reference/semantic-cache.md) — L3 vector-cache trait plus Qdrant collection setup, search, insert, and delete.
- [reference/unified-cache-manager.md](reference/unified-cache-manager.md) — CacheManager coordinating L1/L2/L3 lookups, tier population, and invalidation.
- [reference/cache-metrics.md](reference/cache-metrics.md) — CacheStats hit/miss/eviction counters and Prometheus metric reporting.
