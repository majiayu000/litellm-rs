//! Validation logic for cache entries and requests

use super::types::{SemanticCacheConfig, SemanticCacheEntry};
use crate::core::models::openai::ChatCompletionRequest;

/// Check if a request should be cached
pub fn should_cache_request(config: &SemanticCacheConfig, request: &ChatCompletionRequest) -> bool {
    // Don't cache streaming requests unless explicitly enabled
    if request.stream.unwrap_or(false) && !config.enable_streaming_cache {
        return false;
    }

    // Don't cache requests with function calls (they might have side effects)
    if request.tools.is_some() || request.tool_choice.is_some() {
        return false;
    }

    // Don't cache requests with high randomness
    if let Some(temperature) = request.temperature
        && temperature > 0.7
    {
        return false;
    }

    true
}

/// Check if cache entry is still valid
pub fn is_entry_valid(entry: &SemanticCacheEntry) -> bool {
    if let Some(ttl_seconds) = entry.ttl_seconds {
        let expiry_time = entry.created_at + chrono::Duration::seconds(ttl_seconds as i64);
        chrono::Utc::now() < expiry_time
    } else {
        true // No TTL means never expires
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::openai::{
        ChatCompletionResponse, ChatMessage, MessageContent, MessageRole, ToolChoice,
    };
    use std::collections::HashMap;

    fn create_default_config() -> SemanticCacheConfig {
        SemanticCacheConfig::default()
    }

    fn create_config_with_streaming() -> SemanticCacheConfig {
        SemanticCacheConfig {
            enable_streaming_cache: true,
            ..Default::default()
        }
    }

    fn create_basic_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
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
            stream_options: None,
            modalities: None,
            audio: None,
            reasoning_effort: None,
        }
    }

    fn create_cache_entry_with_ttl(
        ttl_seconds: Option<u64>,
        created_offset_secs: i64,
    ) -> SemanticCacheEntry {
        let created_at = chrono::Utc::now() + chrono::Duration::seconds(created_offset_secs);
        SemanticCacheEntry {
            id: "test-entry".to_string(),
            prompt_hash: "abc123".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            response: ChatCompletionResponse {
                id: "resp-1".to_string(),
                object: "chat.completion".to_string(),
                created: 1234567890,
                model: "gpt-4".to_string(),
                system_fingerprint: None,
                choices: vec![],
                usage: None,
            },
            model: "gpt-4".to_string(),
            created_at,
            last_accessed: created_at,
            access_count: 1,
            ttl_seconds,
            metadata: HashMap::new(),
        }
    }

    // ==================== should_cache_request Tests ====================

    #[test]
    fn test_should_cache_basic_request() {
        let config = create_default_config();
        let request = create_basic_request();
        assert!(should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_not_cache_streaming_by_default() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.stream = Some(true);
        assert!(!should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_cache_streaming_when_enabled() {
        let config = create_config_with_streaming();
        let mut request = create_basic_request();
        request.stream = Some(true);
        assert!(should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_cache_non_streaming_with_streaming_disabled() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.stream = Some(false);
        assert!(should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_not_cache_with_tools() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.tools = Some(vec![]);
        assert!(!should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_not_cache_with_tool_choice() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.tool_choice = Some(ToolChoice::Auto("auto".to_string()));
        assert!(!should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_not_cache_high_temperature() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.temperature = Some(0.8);
        assert!(!should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_cache_low_temperature() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.temperature = Some(0.3);
        assert!(should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_cache_at_threshold_temperature() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.temperature = Some(0.7);
        assert!(should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_not_cache_just_above_threshold() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.temperature = Some(0.71);
        assert!(!should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_cache_zero_temperature() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.temperature = Some(0.0);
        assert!(should_cache_request(&config, &request));
    }

    #[test]
    fn test_should_cache_no_temperature_set() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.temperature = None;
        assert!(should_cache_request(&config, &request));
    }

    // ==================== is_entry_valid Tests ====================

    #[test]
    fn test_entry_valid_no_ttl() {
        let entry = create_cache_entry_with_ttl(None, 0);
        assert!(is_entry_valid(&entry));
    }

    #[test]
    fn test_entry_valid_within_ttl() {
        // Created now with 3600 second TTL (should be valid)
        let entry = create_cache_entry_with_ttl(Some(3600), 0);
        assert!(is_entry_valid(&entry));
    }

    #[test]
    fn test_entry_invalid_expired() {
        // Created 2 hours ago with 1 hour TTL (should be expired)
        let entry = create_cache_entry_with_ttl(Some(3600), -7200);
        assert!(!is_entry_valid(&entry));
    }

    #[test]
    fn test_entry_valid_just_before_expiry() {
        // Created 59 minutes ago with 1 hour TTL (should still be valid)
        let entry = create_cache_entry_with_ttl(Some(3600), -3540);
        assert!(is_entry_valid(&entry));
    }

    #[test]
    fn test_entry_invalid_just_after_expiry() {
        // Created 61 minutes ago with 1 hour TTL (should be expired)
        let entry = create_cache_entry_with_ttl(Some(3600), -3660);
        assert!(!is_entry_valid(&entry));
    }

    #[test]
    fn test_entry_valid_short_ttl() {
        // Created now with 1 second TTL (should be valid immediately)
        let entry = create_cache_entry_with_ttl(Some(1), 0);
        assert!(is_entry_valid(&entry));
    }

    #[test]
    fn test_entry_valid_long_ttl() {
        // Created 1 day ago with 1 week TTL (should still be valid)
        let entry = create_cache_entry_with_ttl(Some(604800), -86400);
        assert!(is_entry_valid(&entry));
    }

    #[test]
    fn test_entry_invalid_zero_ttl() {
        // With TTL of 0, entry is immediately expired
        let entry = create_cache_entry_with_ttl(Some(0), -1);
        assert!(!is_entry_valid(&entry));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_cache_all_conditions_met() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.stream = Some(false);
        request.temperature = Some(0.5);
        request.tools = None;
        request.tool_choice = None;
        assert!(should_cache_request(&config, &request));
    }

    #[test]
    fn test_cache_multiple_disqualifying_conditions() {
        let config = create_default_config();
        let mut request = create_basic_request();
        request.stream = Some(true);
        request.temperature = Some(0.9);
        request.tools = Some(vec![]);
        // Even though multiple conditions fail, first one short-circuits
        assert!(!should_cache_request(&config, &request));
    }
}
