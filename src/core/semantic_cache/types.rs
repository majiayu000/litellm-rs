//! Type definitions for semantic caching

use crate::core::models::openai::ChatCompletionResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Semantic cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCacheEntry {
    /// Unique cache entry ID
    pub id: String,
    /// Original prompt/messages hash
    pub prompt_hash: String,
    /// Prompt embedding vector
    pub embedding: Vec<f32>,
    /// Cached response
    pub response: ChatCompletionResponse,
    /// Model used for the response
    pub model: String,
    /// Cache creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last access timestamp
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    /// Access count
    pub access_count: u64,
    /// TTL in seconds
    pub ttl_seconds: Option<u64>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Semantic cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCacheConfig {
    /// Similarity threshold (0.0 to 1.0)
    pub similarity_threshold: f64,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Default TTL in seconds
    pub default_ttl_seconds: u64,
    /// Embedding model to use
    pub embedding_model: String,
    /// Enable cache for streaming responses
    pub enable_streaming_cache: bool,
    /// Minimum prompt length to cache
    pub min_prompt_length: usize,
    /// Cache hit boost factor
    pub cache_hit_boost: f64,
}

impl Default for SemanticCacheConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.85,
            max_cache_size: 10000,
            default_ttl_seconds: 3600, // 1 hour
            embedding_model: "text-embedding-ada-002".to_string(),
            enable_streaming_cache: false,
            min_prompt_length: 10,
            cache_hit_boost: 1.1,
        }
    }
}

/// Cache statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Total cache entries
    pub total_entries: u64,
    /// Average similarity score for hits
    pub avg_hit_similarity: f64,
    /// Cache size in bytes (approximate)
    pub cache_size_bytes: u64,
}

/// Consolidated cache data - single lock for cache entries and statistics
#[derive(Debug, Default)]
pub(super) struct CacheData {
    /// In-memory cache for recent entries
    pub entries: HashMap<String, SemanticCacheEntry>,
    /// Cache statistics
    pub stats: CacheStats,
}

/// Trait for embedding providers
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for text
    async fn generate_embedding(
        &self,
        text: &str,
    ) -> crate::utils::error::gateway_error::Result<Vec<f32>>;

    /// Get embedding dimension
    fn embedding_dimension(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::openai::{
        ChatChoice, ChatCompletionResponse, ChatMessage, MessageContent, MessageRole, Usage,
    };
    use chrono::Utc;

    // ==================== Helper Functions ====================

    fn create_test_response() -> ChatCompletionResponse {
        ChatCompletionResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text("Test response".to_string())),
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
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }),
            system_fingerprint: None,
        }
    }

    fn create_test_cache_entry() -> SemanticCacheEntry {
        SemanticCacheEntry {
            id: "entry-123".to_string(),
            prompt_hash: "abc123def456".to_string(),
            embedding: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            response: create_test_response(),
            model: "gpt-4".to_string(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 1,
            ttl_seconds: Some(3600),
            metadata: HashMap::new(),
        }
    }

    // ==================== SemanticCacheEntry Tests ====================

    #[test]
    fn test_semantic_cache_entry_creation() {
        let entry = create_test_cache_entry();

        assert_eq!(entry.id, "entry-123");
        assert_eq!(entry.prompt_hash, "abc123def456");
        assert_eq!(entry.embedding.len(), 5);
        assert_eq!(entry.model, "gpt-4");
        assert_eq!(entry.access_count, 1);
        assert_eq!(entry.ttl_seconds, Some(3600));
    }

    #[test]
    fn test_semantic_cache_entry_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("user_id".to_string(), "user-456".to_string());
        metadata.insert("session_id".to_string(), "session-789".to_string());

        let entry = SemanticCacheEntry {
            id: "entry-with-meta".to_string(),
            prompt_hash: "hash123".to_string(),
            embedding: vec![0.5; 1536],
            response: create_test_response(),
            model: "gpt-4".to_string(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 5,
            ttl_seconds: Some(7200),
            metadata,
        };

        assert_eq!(entry.metadata.len(), 2);
        assert_eq!(entry.metadata.get("user_id"), Some(&"user-456".to_string()));
        assert_eq!(entry.access_count, 5);
    }

    #[test]
    fn test_semantic_cache_entry_clone() {
        let entry = create_test_cache_entry();
        let cloned = entry.clone();

        assert_eq!(cloned.id, entry.id);
        assert_eq!(cloned.prompt_hash, entry.prompt_hash);
        assert_eq!(cloned.embedding, entry.embedding);
        assert_eq!(cloned.model, entry.model);
        assert_eq!(cloned.access_count, entry.access_count);
    }

    #[test]
    fn test_semantic_cache_entry_debug() {
        let entry = create_test_cache_entry();
        let debug_str = format!("{:?}", entry);

        assert!(debug_str.contains("SemanticCacheEntry"));
        assert!(debug_str.contains("entry-123"));
    }

    #[test]
    fn test_semantic_cache_entry_serialization() {
        let entry = create_test_cache_entry();
        let json = serde_json::to_string(&entry).unwrap();

        assert!(json.contains("entry-123"));
        assert!(json.contains("abc123def456"));
        assert!(json.contains("gpt-4"));

        let parsed: SemanticCacheEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, entry.id);
        assert_eq!(parsed.prompt_hash, entry.prompt_hash);
    }

    #[test]
    fn test_semantic_cache_entry_without_ttl() {
        let entry = SemanticCacheEntry {
            id: "no-ttl-entry".to_string(),
            prompt_hash: "hash".to_string(),
            embedding: vec![0.1, 0.2],
            response: create_test_response(),
            model: "gpt-3.5-turbo".to_string(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 0,
            ttl_seconds: None,
            metadata: HashMap::new(),
        };

        assert!(entry.ttl_seconds.is_none());
    }

    #[test]
    fn test_semantic_cache_entry_large_embedding() {
        let entry = SemanticCacheEntry {
            id: "large-embedding".to_string(),
            prompt_hash: "hash".to_string(),
            embedding: vec![0.1; 1536], // OpenAI ada-002 dimension
            response: create_test_response(),
            model: "gpt-4".to_string(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 100,
            ttl_seconds: Some(86400),
            metadata: HashMap::new(),
        };

        assert_eq!(entry.embedding.len(), 1536);
    }

    // ==================== SemanticCacheConfig Tests ====================

    #[test]
    fn test_semantic_cache_config_default() {
        let config = SemanticCacheConfig::default();

        assert!((config.similarity_threshold - 0.85).abs() < f64::EPSILON);
        assert_eq!(config.max_cache_size, 10000);
        assert_eq!(config.default_ttl_seconds, 3600);
        assert_eq!(config.embedding_model, "text-embedding-ada-002");
        assert!(!config.enable_streaming_cache);
        assert_eq!(config.min_prompt_length, 10);
        assert!((config.cache_hit_boost - 1.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_semantic_cache_config_custom() {
        let config = SemanticCacheConfig {
            similarity_threshold: 0.95,
            max_cache_size: 50000,
            default_ttl_seconds: 7200,
            embedding_model: "text-embedding-3-large".to_string(),
            enable_streaming_cache: true,
            min_prompt_length: 50,
            cache_hit_boost: 1.5,
        };

        assert!((config.similarity_threshold - 0.95).abs() < f64::EPSILON);
        assert_eq!(config.max_cache_size, 50000);
        assert_eq!(config.default_ttl_seconds, 7200);
        assert!(config.enable_streaming_cache);
    }

    #[test]
    fn test_semantic_cache_config_clone() {
        let config = SemanticCacheConfig::default();
        let cloned = config.clone();

        assert_eq!(cloned.similarity_threshold, config.similarity_threshold);
        assert_eq!(cloned.max_cache_size, config.max_cache_size);
        assert_eq!(cloned.embedding_model, config.embedding_model);
    }

    #[test]
    fn test_semantic_cache_config_debug() {
        let config = SemanticCacheConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("SemanticCacheConfig"));
        assert!(debug_str.contains("0.85"));
        assert!(debug_str.contains("text-embedding-ada-002"));
    }

    #[test]
    fn test_semantic_cache_config_serialization() {
        let config = SemanticCacheConfig::default();
        let json = serde_json::to_string(&config).unwrap();

        assert!(json.contains("similarity_threshold"));
        assert!(json.contains("0.85"));
        assert!(json.contains("text-embedding-ada-002"));

        let parsed: SemanticCacheConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_cache_size, config.max_cache_size);
    }

    #[test]
    fn test_semantic_cache_config_edge_thresholds() {
        // Test minimum threshold
        let min_config = SemanticCacheConfig {
            similarity_threshold: 0.0,
            ..SemanticCacheConfig::default()
        };
        assert!((min_config.similarity_threshold - 0.0).abs() < f64::EPSILON);

        // Test maximum threshold
        let max_config = SemanticCacheConfig {
            similarity_threshold: 1.0,
            ..SemanticCacheConfig::default()
        };
        assert!((max_config.similarity_threshold - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_semantic_cache_config_different_models() {
        let models = vec![
            "text-embedding-ada-002",
            "text-embedding-3-small",
            "text-embedding-3-large",
            "voyage-large-2",
            "cohere-embed-english-v3.0",
        ];

        for model in models {
            let config = SemanticCacheConfig {
                embedding_model: model.to_string(),
                ..SemanticCacheConfig::default()
            };
            assert_eq!(config.embedding_model, model);
        }
    }

    // ==================== CacheStats Tests ====================

    #[test]
    fn test_cache_stats_default() {
        let stats = CacheStats::default();

        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.total_entries, 0);
        assert!((stats.avg_hit_similarity - 0.0).abs() < f64::EPSILON);
        assert_eq!(stats.cache_size_bytes, 0);
    }

    #[test]
    fn test_cache_stats_creation() {
        let stats = CacheStats {
            hits: 1000,
            misses: 500,
            total_entries: 2500,
            avg_hit_similarity: 0.92,
            cache_size_bytes: 1024 * 1024, // 1 MB
        };

        assert_eq!(stats.hits, 1000);
        assert_eq!(stats.misses, 500);
        assert_eq!(stats.total_entries, 2500);
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats {
            hits: 800,
            misses: 200,
            total_entries: 1000,
            avg_hit_similarity: 0.9,
            cache_size_bytes: 512 * 1024,
        };

        let total = stats.hits + stats.misses;
        let hit_rate = stats.hits as f64 / total as f64;
        assert!((hit_rate - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_stats_clone() {
        let stats = CacheStats {
            hits: 100,
            misses: 50,
            total_entries: 75,
            avg_hit_similarity: 0.88,
            cache_size_bytes: 10000,
        };

        let cloned = stats.clone();
        assert_eq!(cloned.hits, stats.hits);
        assert_eq!(cloned.misses, stats.misses);
        assert_eq!(cloned.avg_hit_similarity, stats.avg_hit_similarity);
    }

    #[test]
    fn test_cache_stats_debug() {
        let stats = CacheStats {
            hits: 500,
            misses: 100,
            total_entries: 300,
            avg_hit_similarity: 0.91,
            cache_size_bytes: 50000,
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("CacheStats"));
        assert!(debug_str.contains("500"));
    }

    #[test]
    fn test_cache_stats_serialization() {
        let stats = CacheStats {
            hits: 250,
            misses: 75,
            total_entries: 200,
            avg_hit_similarity: 0.89,
            cache_size_bytes: 25000,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("250"));
        assert!(json.contains("0.89"));

        let parsed: CacheStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hits, 250);
        assert_eq!(parsed.total_entries, 200);
    }

    #[test]
    fn test_cache_stats_zero_requests() {
        let stats = CacheStats::default();

        // Avoid division by zero
        let total = stats.hits + stats.misses;
        if total > 0 {
            let _hit_rate = stats.hits as f64 / total as f64;
        }
        // No panic should occur
        assert_eq!(total, 0);
    }

    #[test]
    fn test_cache_stats_large_values() {
        let stats = CacheStats {
            hits: u64::MAX / 2,
            misses: u64::MAX / 4,
            total_entries: u64::MAX / 8,
            avg_hit_similarity: 0.999,
            cache_size_bytes: u64::MAX / 16,
        };

        assert!(stats.hits > 0);
        assert!(stats.misses > 0);
    }

    // ==================== CacheData Tests ====================

    #[test]
    fn test_cache_data_default() {
        let data = CacheData::default();

        assert!(data.entries.is_empty());
        assert_eq!(data.stats.hits, 0);
        assert_eq!(data.stats.misses, 0);
    }

    #[test]
    fn test_cache_data_with_entries() {
        let mut data = CacheData::default();

        let entry1 = create_test_cache_entry();
        let entry2 = SemanticCacheEntry {
            id: "entry-456".to_string(),
            ..create_test_cache_entry()
        };

        data.entries.insert(entry1.id.clone(), entry1);
        data.entries.insert(entry2.id.clone(), entry2);

        assert_eq!(data.entries.len(), 2);
        assert!(data.entries.contains_key("entry-123"));
        assert!(data.entries.contains_key("entry-456"));
    }

    #[test]
    fn test_cache_data_update_stats() {
        let mut data = CacheData::default();

        data.stats.hits += 1;
        data.stats.total_entries += 1;

        assert_eq!(data.stats.hits, 1);
        assert_eq!(data.stats.total_entries, 1);
    }

    #[test]
    fn test_cache_data_entry_lookup() {
        let mut data = CacheData::default();

        let entry = create_test_cache_entry();
        let entry_id = entry.id.clone();
        data.entries.insert(entry_id.clone(), entry);

        let retrieved = data.entries.get(&entry_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().prompt_hash, "abc123def456");
    }

    #[test]
    fn test_cache_data_entry_removal() {
        let mut data = CacheData::default();

        let entry = create_test_cache_entry();
        let entry_id = entry.id.clone();
        data.entries.insert(entry_id.clone(), entry);

        assert!(data.entries.contains_key(&entry_id));

        data.entries.remove(&entry_id);
        assert!(!data.entries.contains_key(&entry_id));
    }

    #[test]
    fn test_cache_data_debug() {
        let data = CacheData::default();
        let debug_str = format!("{:?}", data);

        assert!(debug_str.contains("CacheData"));
        assert!(debug_str.contains("entries"));
        assert!(debug_str.contains("stats"));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_cache_workflow_simulation() {
        let mut data = CacheData::default();
        let config = SemanticCacheConfig::default();

        // Simulate cache miss and entry creation
        data.stats.misses += 1;

        let entry = SemanticCacheEntry {
            id: "new-entry".to_string(),
            prompt_hash: "query-hash".to_string(),
            embedding: vec![0.1; 1536],
            response: create_test_response(),
            model: "gpt-4".to_string(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 1,
            ttl_seconds: Some(config.default_ttl_seconds),
            metadata: HashMap::new(),
        };

        data.entries.insert(entry.id.clone(), entry);
        data.stats.total_entries += 1;

        assert_eq!(data.stats.misses, 1);
        assert_eq!(data.stats.total_entries, 1);
        assert_eq!(data.entries.len(), 1);
    }

    #[test]
    fn test_cache_hit_simulation() {
        let mut data = CacheData::default();

        // Add initial entry
        let entry = create_test_cache_entry();
        let entry_id = entry.id.clone();
        data.entries.insert(entry_id.clone(), entry.clone());
        data.stats.total_entries = 1;

        // Simulate cache hit
        if let Some(cached_entry) = data.entries.get_mut(&entry_id) {
            cached_entry.access_count += 1;
            cached_entry.last_accessed = Utc::now();
            data.stats.hits += 1;
        }

        assert_eq!(data.stats.hits, 1);
        assert_eq!(data.entries.get(&entry_id).unwrap().access_count, 2);
    }

    #[test]
    fn test_similarity_threshold_check() {
        let config = SemanticCacheConfig::default();

        // Simulated similarity scores
        let scores = vec![0.7, 0.8, 0.85, 0.9, 0.95];

        for score in scores {
            let is_hit = score >= config.similarity_threshold;
            if score >= 0.85 {
                assert!(is_hit, "Score {} should be a hit", score);
            } else {
                assert!(!is_hit, "Score {} should be a miss", score);
            }
        }
    }

    #[test]
    fn test_cache_eviction_logic() {
        let config = SemanticCacheConfig {
            max_cache_size: 3,
            ..SemanticCacheConfig::default()
        };

        let mut data = CacheData::default();

        // Fill cache to max
        for i in 0..config.max_cache_size {
            let entry = SemanticCacheEntry {
                id: format!("entry-{}", i),
                prompt_hash: format!("hash-{}", i),
                embedding: vec![0.1; 10],
                response: create_test_response(),
                model: "gpt-4".to_string(),
                created_at: Utc::now(),
                last_accessed: Utc::now(),
                access_count: i as u64,
                ttl_seconds: None,
                metadata: HashMap::new(),
            };
            data.entries.insert(entry.id.clone(), entry);
        }

        assert_eq!(data.entries.len(), config.max_cache_size);

        // Simulate eviction when adding new entry
        if data.entries.len() >= config.max_cache_size {
            // Find entry with lowest access count
            let to_evict = data
                .entries
                .iter()
                .min_by_key(|(_, e)| e.access_count)
                .map(|(k, _)| k.clone());

            if let Some(key) = to_evict {
                data.entries.remove(&key);
            }
        }

        assert_eq!(data.entries.len(), config.max_cache_size - 1);
    }

    #[test]
    fn test_min_prompt_length_check() {
        let config = SemanticCacheConfig::default();

        let prompts = vec![
            ("short", false),     // 5 chars < 10
            ("medium len", true), // 10 chars = 10
            ("this is a longer prompt that should be cached", true),
        ];

        for (prompt, should_cache) in prompts {
            let can_cache = prompt.len() >= config.min_prompt_length;
            assert_eq!(
                can_cache, should_cache,
                "Prompt '{}' caching mismatch",
                prompt
            );
        }
    }

    #[test]
    fn test_cache_hit_boost_calculation() {
        let config = SemanticCacheConfig::default();

        let base_score = 0.9;
        let boosted_score = base_score * config.cache_hit_boost;

        assert!((boosted_score - 0.99).abs() < 0.001);
    }

    #[test]
    fn test_streaming_cache_flag() {
        let disabled = SemanticCacheConfig::default();
        assert!(!disabled.enable_streaming_cache);

        let enabled = SemanticCacheConfig {
            enable_streaming_cache: true,
            ..SemanticCacheConfig::default()
        };
        assert!(enabled.enable_streaming_cache);
    }

    #[test]
    fn test_multiple_entries_same_model() {
        let mut data = CacheData::default();

        for i in 0..5 {
            let entry = SemanticCacheEntry {
                id: format!("entry-{}", i),
                prompt_hash: format!("hash-{}", i),
                embedding: vec![0.1 * i as f32; 10],
                response: create_test_response(),
                model: "gpt-4".to_string(),
                created_at: Utc::now(),
                last_accessed: Utc::now(),
                access_count: 0,
                ttl_seconds: Some(3600),
                metadata: HashMap::new(),
            };
            data.entries.insert(entry.id.clone(), entry);
        }

        let gpt4_entries: Vec<_> = data
            .entries
            .values()
            .filter(|e| e.model == "gpt-4")
            .collect();

        assert_eq!(gpt4_entries.len(), 5);
    }

    #[test]
    fn test_entries_by_access_count() {
        let mut data = CacheData::default();

        for i in 0..3 {
            let entry = SemanticCacheEntry {
                id: format!("entry-{}", i),
                prompt_hash: format!("hash-{}", i),
                embedding: vec![0.1; 5],
                response: create_test_response(),
                model: "gpt-4".to_string(),
                created_at: Utc::now(),
                last_accessed: Utc::now(),
                access_count: (i + 1) * 10,
                ttl_seconds: None,
                metadata: HashMap::new(),
            };
            data.entries.insert(entry.id.clone(), entry);
        }

        let most_accessed = data
            .entries
            .values()
            .max_by_key(|e| e.access_count)
            .unwrap();

        assert_eq!(most_accessed.access_count, 30);
    }
}
