use super::*;
use crate::core::models::openai::messages::{ChatMessage, MessageContent, MessageRole};
use crate::core::models::openai::{ChatChoice, Usage};
use std::sync::Arc;

fn create_user_message(content: &str) -> ChatMessage {
    ChatMessage {
        thinking: None,
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
        thinking: None,
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
            thinking_usage: None,
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

#[tokio::test]
async fn test_invalidate_chat_with_user_honors_user_specific_key() {
    let config = LLMCacheConfig::memory_only().with_user_specific();
    let cache = LLMCache::new(config, None);
    let request = create_test_request();
    let response = create_test_response();

    cache
        .cache_chat_response_with_user(&request, response, Some("user1"))
        .await
        .unwrap();
    assert!(
        cache
            .get_chat_response_with_user(&request, Some("user1"))
            .await
            .unwrap()
            .is_some()
    );

    // Pre-fix bug: invalidate_chat ignored user_specific and deleted the
    // wrong (no-user) key, leaving the per-user entry live.
    let invalidated = cache
        .invalidate_chat_with_user(&request, Some("user1"))
        .await
        .unwrap();
    assert!(invalidated);
    assert!(
        cache
            .get_chat_response_with_user(&request, Some("user1"))
            .await
            .unwrap()
            .is_none()
    );
}
