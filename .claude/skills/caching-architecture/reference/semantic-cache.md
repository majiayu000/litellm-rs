# Semantic cache removal

The unused `core::semantic_cache` module and its public types were removed from
unreleased source after their 0.7 removal deadline. This does not change the
already published v0.7.0 package.

Remove `cache.semantic_cache` and `cache.similarity_threshold` from JSON/YAML,
even if set to false/default values. The existing unknown-field validation
rejects both. `LLMCacheConfig.semantic_cache_enabled` and its similarity threshold
also no longer exist. The admin cache response no longer exposes
`semantic_cache_enabled`.

Use the deterministic `core::cache::LLMCache` response cache with `enabled`,
`ttl`, and `max_size`; see [response-cache.md](response-cache.md). Vector storage
is separate and does not provide an automatic semantic response-cache layer.
Do not generate removed configuration fields or imports.
