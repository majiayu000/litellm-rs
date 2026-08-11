//! Semantic caching for AI responses
//!
//! This module provides intelligent caching based on semantic similarity of prompts.
//!
//! Boundary:
//! - `crate::core::cache` handles deterministic key-based cache.
//! - This module handles vector-similarity semantic cache.

mod cache;
mod types;
mod utils;
mod validation;

#[cfg(test)]
mod tests;

// Re-export main types and structs for backward compatibility
#[deprecated(
    since = "0.6.0",
    note = "semantic cache is unwired and scheduled for removal in 0.7.0; cache.semantic_cache remains rejected"
)]
pub use cache::SemanticCache;
#[deprecated(
    since = "0.6.0",
    note = "semantic cache is unwired and scheduled for removal in 0.7.0; cache.semantic_cache remains rejected"
)]
pub use types::{CacheStats, EmbeddingProvider, SemanticCacheConfig, SemanticCacheEntry};
