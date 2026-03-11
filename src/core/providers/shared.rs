//! Shared utilities for all providers
//!
//! This module contains common functionality that can be reused across all providers,
//! following the DRY principle and Rust's composition over inheritance pattern.

use serde_json::Value;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::FinishReason;
use crate::core::types::{message::MessageContent, message::MessageRole};

// ============================================================================
// Message Transformation Utilities
// ============================================================================

pub struct MessageTransformer;

impl MessageTransformer {
    /// Convert role to OpenAI-compatible string
    pub fn role_to_string(role: &MessageRole) -> &'static str {
        match role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
            MessageRole::Function => "function",
        }
    }

    /// Parse string to MessageRole
    pub fn string_to_role(role: &str) -> MessageRole {
        match role {
            "system" => MessageRole::System,
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            "function" => MessageRole::Function,
            _ => MessageRole::User,
        }
    }

    /// Convert MessageContent to JSON Value
    pub fn content_to_value(content: &Option<MessageContent>) -> Value {
        match content {
            Some(MessageContent::Text(text)) => Value::String(text.clone()),
            Some(MessageContent::Parts(parts)) => {
                serde_json::to_value(parts).unwrap_or(Value::Null)
            }
            None => Value::Null,
        }
    }

    /// Parse finish reason string
    pub fn parse_finish_reason(reason: &str) -> Option<FinishReason> {
        match reason {
            "stop" => Some(FinishReason::Stop),
            "length" | "max_tokens" => Some(FinishReason::Length),
            "tool_calls" | "function_call" => Some(FinishReason::ToolCalls),
            "content_filter" => Some(FinishReason::ContentFilter),
            _ => None,
        }
    }
}

// ============================================================================
// Rate Limiting
// ============================================================================

use std::sync::Arc;
use tokio::sync::Semaphore;

/// Rate limiter for providers
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
}

impl RateLimiter {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(requests_per_second as usize)),
        }
    }

    pub async fn acquire(&self) -> Result<tokio::sync::SemaphorePermit<'_>, ProviderError> {
        self.semaphore
            .acquire()
            .await
            .map_err(|_| ProviderError::Other {
                provider: "rate_limiter",
                message: "Failed to acquire rate limit permit".to_string(),
            })
    }

    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

// ============================================================================
// Response Validation
// ============================================================================

pub struct ResponseValidator;

impl ResponseValidator {
    /// Validate that a response has required fields
    pub fn validate_chat_response(
        response: &Value,
        provider: &'static str,
    ) -> Result<(), ProviderError> {
        if !response.is_object() {
            return Err(ProviderError::ResponseParsing {
                provider,
                message: "Response is not an object".to_string(),
            });
        }

        // Check for required fields
        let required_fields = ["id", "choices", "created", "model"];
        for field in &required_fields {
            if response.get(field).is_none() {
                return Err(ProviderError::ResponseParsing {
                    provider,
                    message: format!("Missing required field: {}", field),
                });
            }
        }

        // Validate choices array
        if let Some(choices) = response.get("choices")
            && choices.as_array().is_none_or(|a| a.is_empty())
        {
            return Err(ProviderError::ResponseParsing {
                provider,
                message: "Choices must be a non-empty array".to_string(),
            });
        }

        Ok(())
    }
}

// ============================================================================
// Retry-After Parsing
// ============================================================================

/// Parse retry-after duration from a JSON response body.
///
/// Checks `retry_after` and `error.retry_after` fields, then falls back to
/// keyword detection ("rate limit" / "rate_limit" / "too many requests").
/// Returns a default of 60 seconds when only keywords match.
pub fn parse_retry_after_from_body(response_body: &str) -> Option<u64> {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(response_body) {
        if let Some(v) = json.get("retry_after").and_then(|v| v.as_u64()) {
            return Some(v);
        }
        if let Some(v) = json
            .get("error")
            .and_then(|e| e.get("retry_after"))
            .and_then(|v| v.as_u64())
        {
            return Some(v);
        }
    }
    let lower = response_body.to_lowercase();
    if lower.contains("rate limit")
        || lower.contains("rate_limit")
        || lower.contains("too many requests")
    {
        Some(60)
    } else {
        None
    }
}

// ============================================================================
// Vector Math Utilities
// ============================================================================

/// Calculate cosine similarity between two vectors.
pub fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> f32 {
    if vec1.len() != vec2.len() {
        return 0.0;
    }

    let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f32 = vec1.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm2: f32 = vec2.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm1 == 0.0 || norm2 == 0.0 {
        0.0
    } else {
        dot_product / (norm1 * norm2)
    }
}

/// Calculate L2 (Euclidean) distance between two vectors.
pub fn l2_distance(vec1: &[f32], vec2: &[f32]) -> f32 {
    if vec1.len() != vec2.len() {
        return f32::INFINITY;
    }

    vec1.iter()
        .zip(vec2.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Normalize a vector to unit length in-place.
pub fn normalize_vector(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

// ============================================================================
// Testing Utilities
// ============================================================================

#[cfg(test)]
pub mod test_utils {
    use super::*;
    use crate::core::types::chat::ChatMessage;
    use crate::core::types::responses::Usage;

    /// Create a mock ChatMessage for testing
    pub fn mock_message(role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: Some(MessageContent::Text(content.to_string())),
            ..Default::default()
        }
    }

    /// Create mock usage for testing
    pub fn mock_usage(prompt: u32, completion: u32) -> Usage {
        Usage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
            completion_tokens_details: None,
            prompt_tokens_details: None,
            thinking_usage: None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::content::ContentPart;

    // ==================== MessageTransformer Tests ====================

    #[test]
    fn test_message_transformer_role_to_string_system() {
        assert_eq!(
            MessageTransformer::role_to_string(&MessageRole::System),
            "system"
        );
    }

    #[test]
    fn test_message_transformer_role_to_string_user() {
        assert_eq!(
            MessageTransformer::role_to_string(&MessageRole::User),
            "user"
        );
    }

    #[test]
    fn test_message_transformer_role_to_string_assistant() {
        assert_eq!(
            MessageTransformer::role_to_string(&MessageRole::Assistant),
            "assistant"
        );
    }

    #[test]
    fn test_message_transformer_role_to_string_tool() {
        assert_eq!(
            MessageTransformer::role_to_string(&MessageRole::Tool),
            "tool"
        );
    }

    #[test]
    fn test_message_transformer_role_to_string_function() {
        assert_eq!(
            MessageTransformer::role_to_string(&MessageRole::Function),
            "function"
        );
    }

    #[test]
    fn test_message_transformer_string_to_role_system() {
        assert_eq!(
            MessageTransformer::string_to_role("system"),
            MessageRole::System
        );
    }

    #[test]
    fn test_message_transformer_string_to_role_user() {
        assert_eq!(
            MessageTransformer::string_to_role("user"),
            MessageRole::User
        );
    }

    #[test]
    fn test_message_transformer_string_to_role_assistant() {
        assert_eq!(
            MessageTransformer::string_to_role("assistant"),
            MessageRole::Assistant
        );
    }

    #[test]
    fn test_message_transformer_string_to_role_tool() {
        assert_eq!(
            MessageTransformer::string_to_role("tool"),
            MessageRole::Tool
        );
    }

    #[test]
    fn test_message_transformer_string_to_role_function() {
        assert_eq!(
            MessageTransformer::string_to_role("function"),
            MessageRole::Function
        );
    }

    #[test]
    fn test_message_transformer_string_to_role_unknown() {
        assert_eq!(
            MessageTransformer::string_to_role("unknown"),
            MessageRole::User
        );
        assert_eq!(MessageTransformer::string_to_role(""), MessageRole::User);
    }

    #[test]
    fn test_message_transformer_content_to_value_text() {
        let content = Some(MessageContent::Text("Hello, world!".to_string()));
        let value = MessageTransformer::content_to_value(&content);
        assert_eq!(value, Value::String("Hello, world!".to_string()));
    }

    #[test]
    fn test_message_transformer_content_to_value_parts() {
        let content = Some(MessageContent::Parts(vec![
            ContentPart::Text {
                text: "Part 1".to_string(),
            },
            ContentPart::Text {
                text: "Part 2".to_string(),
            },
        ]));
        let value = MessageTransformer::content_to_value(&content);
        assert!(value.is_array());
    }

    #[test]
    fn test_message_transformer_content_to_value_none() {
        let content: Option<MessageContent> = None;
        let value = MessageTransformer::content_to_value(&content);
        assert!(value.is_null());
    }

    #[test]
    fn test_message_transformer_parse_finish_reason_stop() {
        assert_eq!(
            MessageTransformer::parse_finish_reason("stop"),
            Some(FinishReason::Stop)
        );
    }

    #[test]
    fn test_message_transformer_parse_finish_reason_length() {
        assert_eq!(
            MessageTransformer::parse_finish_reason("length"),
            Some(FinishReason::Length)
        );
        assert_eq!(
            MessageTransformer::parse_finish_reason("max_tokens"),
            Some(FinishReason::Length)
        );
    }

    #[test]
    fn test_message_transformer_parse_finish_reason_tool_calls() {
        assert_eq!(
            MessageTransformer::parse_finish_reason("tool_calls"),
            Some(FinishReason::ToolCalls)
        );
        assert_eq!(
            MessageTransformer::parse_finish_reason("function_call"),
            Some(FinishReason::ToolCalls)
        );
    }

    #[test]
    fn test_message_transformer_parse_finish_reason_content_filter() {
        assert_eq!(
            MessageTransformer::parse_finish_reason("content_filter"),
            Some(FinishReason::ContentFilter)
        );
    }

    #[test]
    fn test_message_transformer_parse_finish_reason_unknown() {
        assert_eq!(MessageTransformer::parse_finish_reason("unknown"), None);
        assert_eq!(MessageTransformer::parse_finish_reason(""), None);
    }

    // ==================== RateLimiter Tests ====================

    #[test]
    fn test_rate_limiter_new() {
        let limiter = RateLimiter::new(10);
        assert_eq!(limiter.available_permits(), 10);
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire() {
        let limiter = RateLimiter::new(10);
        assert_eq!(limiter.available_permits(), 10);

        let _permit = limiter.acquire().await.unwrap();
        assert_eq!(limiter.available_permits(), 9);
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire_multiple() {
        let limiter = RateLimiter::new(5);

        let _permit1 = limiter.acquire().await.unwrap();
        let _permit2 = limiter.acquire().await.unwrap();
        let _permit3 = limiter.acquire().await.unwrap();

        assert_eq!(limiter.available_permits(), 2);
    }

    #[tokio::test]
    async fn test_rate_limiter_release() {
        let limiter = RateLimiter::new(10);

        {
            let _permit = limiter.acquire().await.unwrap();
            assert_eq!(limiter.available_permits(), 9);
        }
        // Permit is dropped, should be released
        assert_eq!(limiter.available_permits(), 10);
    }

    // ==================== ResponseValidator Tests ====================

    #[test]
    fn test_response_validator_valid_response() {
        let response = serde_json::json!({
            "id": "test-id",
            "choices": [{"message": {"content": "Hello"}}],
            "created": 1234567890,
            "model": "gpt-4"
        });

        let result = ResponseValidator::validate_chat_response(&response, "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_response_validator_missing_id() {
        let response = serde_json::json!({
            "choices": [{"message": {"content": "Hello"}}],
            "created": 1234567890,
            "model": "gpt-4"
        });

        let result = ResponseValidator::validate_chat_response(&response, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_response_validator_missing_choices() {
        let response = serde_json::json!({
            "id": "test-id",
            "created": 1234567890,
            "model": "gpt-4"
        });

        let result = ResponseValidator::validate_chat_response(&response, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_response_validator_empty_choices() {
        let response = serde_json::json!({
            "id": "test-id",
            "choices": [],
            "created": 1234567890,
            "model": "gpt-4"
        });

        let result = ResponseValidator::validate_chat_response(&response, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_response_validator_not_object() {
        let response = serde_json::json!([1, 2, 3]);

        let result = ResponseValidator::validate_chat_response(&response, "test");
        assert!(result.is_err());
    }

    // ==================== Test Utilities Tests ====================

    #[test]
    fn test_mock_message() {
        let message = test_utils::mock_message(MessageRole::User, "Hello");

        assert_eq!(message.role, MessageRole::User);
        match &message.content {
            Some(MessageContent::Text(text)) => assert_eq!(text, "Hello"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_mock_usage() {
        let usage = test_utils::mock_usage(100, 50);

        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }
}
