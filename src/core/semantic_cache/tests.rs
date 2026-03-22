//! Tests for semantic caching

#[cfg(test)]
use super::types::SemanticCacheConfig;
use super::utils::extract_prompt_text;
use super::validation::should_cache_request;
use crate::core::models::openai::ChatCompletionRequest;
use crate::core::models::openai::{ChatMessage, MessageContent, MessageRole};

#[test]
fn test_semantic_cache_config_default() {
    let config = SemanticCacheConfig::default();
    assert_eq!(config.similarity_threshold, 0.85);
    assert_eq!(config.max_cache_size, 10000);
    assert_eq!(config.default_ttl_seconds, 3600);
}

#[tokio::test]
async fn test_extract_prompt_text() {
    let messages = vec![
        ChatMessage {
            role: MessageRole::System,
            content: Some(MessageContent::Text(
                "You are a helpful assistant".to_string(),
            )),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        },
        ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello world".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        },
    ];

    let prompt_text = extract_prompt_text(&messages);
    assert!(prompt_text.contains("You are a helpful assistant"));
    assert!(prompt_text.contains("Hello world"));
}

#[tokio::test]
async fn test_should_cache_request() {
    let config = SemanticCacheConfig::default();

    let mut request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        max_tokens: None,
        max_completion_tokens: None,
        temperature: Some(0.1),
        top_p: None,
        n: None,
        stream: Some(false),
        stream_options: None,
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        functions: None,
        function_call: None,
        tools: None,
        tool_choice: None,
        response_format: None,
        seed: None,
        logprobs: None,
        top_logprobs: None,
        modalities: None,
        audio: None,
        reasoning_effort: None,
        store: None,
        metadata: None,
        service_tier: None,
    };

    // Should cache low temperature request
    assert!(should_cache_request(&config, &request));

    // Should not cache high temperature request
    request.temperature = Some(0.9);
    assert!(!should_cache_request(&config, &request));

    // Should not cache streaming request (by default)
    request.temperature = Some(0.1);
    request.stream = Some(true);
    assert!(!should_cache_request(&config, &request));
}
