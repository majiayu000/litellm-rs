//! Shared utilities for all providers.

use serde_json::Value;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{FinishReason, PromptTokensDetails, Usage};
use crate::core::types::thinking::ThinkingUsage;
use crate::core::types::{message::MessageContent, message::MessageRole};

mod gemini;
pub use gemini::*;

// Message Transformation Utilities

pub struct MessageTransformer;

impl MessageTransformer {
    pub fn role_to_string(role: &MessageRole) -> &'static str {
        match role {
            MessageRole::System => "system",
            MessageRole::Developer => "developer",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
            MessageRole::Function => "function",
        }
    }

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

    pub fn content_to_value(content: &Option<MessageContent>) -> Value {
        match content {
            Some(MessageContent::Text(text)) => Value::String(text.clone()),
            Some(MessageContent::Parts(parts)) => {
                serde_json::to_value(parts).unwrap_or(Value::Null)
            }
            None => Value::Null,
        }
    }

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

// Rate Limiting

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

// Response Validation

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

// Retry-After Parsing

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

// Vector Math Utilities

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

/// Read a declared token field without narrowing it before consistency checks.
pub(crate) fn strict_token_count(value: Option<&Value>) -> Option<u64> {
    value?.as_u64()
}

fn raw_token_sum(parts: &[u64]) -> Option<u128> {
    parts
        .iter()
        .try_fold(0_u128, |sum, part| sum.checked_add(u128::from(*part)))
}

fn saturating_token_count(tokens: u128) -> u32 {
    u32::try_from(tokens).unwrap_or(u32::MAX)
}

/// Normalize trusted raw components, optionally validating an endpoint-specific total.
///
/// `reported_total_parts` are deliberately independent from the billable parts:
/// direct Gemini excludes tool-use prompt tokens from its reported total, while
/// Vertex includes them. Cached tokens are validated against the raw base prompt.
pub(crate) fn strict_usage(
    prompt_parts: &[u64],
    completion_parts: &[u64],
    reported_total: Option<(u64, &[u64])>,
    cached_tokens: Option<(u64, u64)>,
) -> Option<Usage> {
    let raw_prompt = raw_token_sum(prompt_parts)?;
    let raw_completion = raw_token_sum(completion_parts)?;
    if raw_prompt == 0 && raw_completion == 0 {
        return None;
    }
    if let Some((reported, reported_parts)) = reported_total
        && u128::from(reported) != raw_token_sum(reported_parts)?
    {
        return None;
    }
    if let Some((cached, base_prompt)) = cached_tokens
        && cached > base_prompt
    {
        return None;
    }

    let cached = cached_tokens.map(|(cached, _)| saturating_token_count(u128::from(cached)));
    Some(Usage {
        prompt_tokens: saturating_token_count(raw_prompt),
        completion_tokens: saturating_token_count(raw_completion),
        total_tokens: saturating_token_count(raw_prompt.checked_add(raw_completion)?),
        prompt_tokens_details: cached.map(|cached| PromptTokensDetails {
            cached_tokens: Some(cached),
            cache_creation_tokens: None,
            cache_read_tokens: Some(cached),
            audio_tokens: None,
        }),
        completion_tokens_details: None,
        thinking_usage: None,
    })
}

pub(crate) fn strict_openai_chat_usage(usage: &Value) -> Option<Usage> {
    let prompt = strict_token_count(usage.get("prompt_tokens"))?;
    let completion = strict_token_count(usage.get("completion_tokens"))?;
    let total = strict_token_count(usage.get("total_tokens"))?;
    strict_usage(
        &[prompt],
        &[completion],
        Some((total, &[prompt, completion])),
        None,
    )
}

pub(crate) fn strict_openai_embedding_usage(usage: &Value) -> Option<Usage> {
    let prompt = strict_token_count(usage.get("prompt_tokens"))?;
    let completion = match usage.get("completion_tokens") {
        Some(value) => strict_token_count(Some(value))?,
        None => 0,
    };
    if completion != 0 {
        return None;
    }
    let total = strict_token_count(usage.get("total_tokens"))?;
    strict_usage(&[prompt], &[0], Some((total, &[prompt])), None)
}

fn strict_google_usage_metadata(metadata: &Value, vertex_total: bool) -> Option<Usage> {
    let prompt = strict_token_count(metadata.get("promptTokenCount"))?;
    let candidates = strict_token_count(metadata.get("candidatesTokenCount"))?;
    let tool_prompt = match metadata.get("toolUsePromptTokenCount") {
        Some(value) => strict_token_count(Some(value))?,
        None => 0,
    };
    let thoughts = match metadata.get("thoughtsTokenCount") {
        Some(value) => strict_token_count(Some(value))?,
        None => 0,
    };
    let cached = match metadata.get("cachedContentTokenCount") {
        Some(value) => Some((strict_token_count(Some(value))?, prompt)),
        None => None,
    };
    let total = strict_token_count(metadata.get("totalTokenCount"))?;
    let vertex_parts = [prompt, candidates, tool_prompt, thoughts];
    let direct_parts = [prompt, candidates, thoughts];
    let total_parts = if vertex_total {
        vertex_parts.as_slice()
    } else {
        direct_parts.as_slice()
    };
    strict_usage(
        &[prompt, tool_prompt],
        &[candidates, thoughts],
        Some((total, total_parts)),
        cached,
    )
}

#[cfg(any(feature = "providers-extra", feature = "providers-extended", test))]
pub(crate) fn strict_vertex_usage_metadata(metadata: &Value) -> Option<Usage> {
    strict_google_usage_metadata(metadata, true)
}

pub(crate) fn strict_direct_gemini_usage_metadata(metadata: &Value) -> Option<Usage> {
    let thoughts = match metadata.get("thoughtsTokenCount") {
        Some(value) => Some(strict_token_count(Some(value))?),
        None => None,
    };
    let mut usage = strict_google_usage_metadata(metadata, false)?;
    usage.thinking_usage = thoughts.map(|tokens| {
        ThinkingUsage::new(saturating_token_count(u128::from(tokens))).with_provider("gemini")
    });
    Some(usage)
}

// Testing Utilities

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

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::content::ContentPart;

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

    #[test]
    fn strict_usage_preserves_raw_domain_until_validation() {
        let values = serde_json::json!({"ok": 1, "bad": [null, "1", 1.5, -1, {}, []]});
        assert_eq!(strict_token_count(values.get("ok")), Some(1));
        for value in values["bad"].as_array().unwrap() {
            assert!(strict_token_count(Some(value)).is_none());
        }
        assert!(strict_usage(&[0], &[0], Some((0, &[0, 0])), None).is_none());
        let max_u32 = u64::from(u32::MAX);
        for (prompt, completion) in [(max_u32, 0), (max_u32 + 1, 0), (max_u32, 1)] {
            let usage = strict_usage(&[prompt], &[completion], None, None).unwrap();
            assert_eq!(usage.prompt_tokens, u32::MAX);
            assert_eq!(usage.total_tokens, u32::MAX);
        }
        assert!(strict_usage(&[u64::MAX], &[1], Some((u64::MAX, &[u64::MAX, 1])), None).is_none());
        let usage = strict_usage(&[u64::MAX], &[1], None, None).unwrap();
        assert_eq!(
            (usage.prompt_tokens, usage.total_tokens),
            (u32::MAX, u32::MAX)
        );
    }

    #[test]
    fn strict_google_usage_applies_endpoint_total_and_cache_policies() {
        let mut metadata = serde_json::json!({
            "promptTokenCount": 10, "toolUsePromptTokenCount": 2,
            "candidatesTokenCount": 3, "thoughtsTokenCount": 4,
            "cachedContentTokenCount": 5, "totalTokenCount": 19
        });
        let vertex = strict_vertex_usage_metadata(&metadata).unwrap();
        assert_eq!(vertex.prompt_tokens, 12);
        assert_eq!(vertex.completion_tokens, 7);
        assert_eq!(vertex.total_tokens, 19);
        assert!(strict_direct_gemini_usage_metadata(&metadata).is_none());
        metadata["totalTokenCount"] = serde_json::json!(17);
        let direct = strict_direct_gemini_usage_metadata(&metadata).unwrap();
        assert_eq!(direct.prompt_tokens, 12);
        assert_eq!(direct.completion_tokens, 7);
        assert_eq!(direct.total_tokens, 19);
        assert!(direct.completion_tokens_details.is_none());
        assert_eq!(direct.thinking_tokens(), Some(4));
        metadata["cachedContentTokenCount"] = serde_json::json!(10);
        assert!(strict_direct_gemini_usage_metadata(&metadata).is_some());
        metadata["cachedContentTokenCount"] = serde_json::json!(11);
        assert!(strict_direct_gemini_usage_metadata(&metadata).is_none());

        let tool_only = serde_json::json!({
            "promptTokenCount": 0, "toolUsePromptTokenCount": 2,
            "candidatesTokenCount": 0, "totalTokenCount": 0
        });
        let usage = strict_direct_gemini_usage_metadata(&tool_only).unwrap();
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!((usage.prompt_tokens, usage.total_tokens), (2, 2));
        let thought_only = serde_json::json!({
            "promptTokenCount": 0, "candidatesTokenCount": 0,
            "thoughtsTokenCount": 3, "totalTokenCount": 3
        });
        let usage = strict_direct_gemini_usage_metadata(&thought_only).unwrap();
        assert_eq!((usage.completion_tokens, usage.total_tokens), (3, 3));
        assert_eq!(usage.thinking_tokens(), Some(3));

        for (field, malformed) in [
            ("toolUsePromptTokenCount", serde_json::json!("2")),
            ("thoughtsTokenCount", Value::Null),
            ("cachedContentTokenCount", serde_json::json!([])),
        ] {
            let mut bad = serde_json::json!({
                "promptTokenCount": 1, "candidatesTokenCount": 1, "totalTokenCount": 2
            });
            bad[field] = malformed;
            assert!(strict_direct_gemini_usage_metadata(&bad).is_none());
        }
    }

    #[test]
    fn strict_embedding_zero_is_semantic() {
        let valid =
            serde_json::json!({"prompt_tokens": 8, "completion_tokens": 0, "total_tokens": 8});
        assert!(strict_openai_embedding_usage(&valid).is_some());
        for bad in [
            serde_json::json!({"prompt_tokens": 8, "total_tokens": 9}),
            serde_json::json!({"prompt_tokens": "8", "total_tokens": 8}),
            serde_json::json!({"prompt_tokens": 8, "completion_tokens": 1, "total_tokens": 8}),
        ] {
            assert!(strict_openai_embedding_usage(&bad).is_none());
        }
    }
}
