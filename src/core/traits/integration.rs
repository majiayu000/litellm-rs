//! Integration trait definitions
//!
//! Provides unified interface for all external integrations (observability, logging, etc.)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Result type for integration operations
pub type IntegrationResult<T> = Result<T, IntegrationError>;

/// Integration error types
#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    #[error("Integration not enabled: {name}")]
    NotEnabled { name: String },

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Integration error: {0}")]
    Other(String),
}

impl IntegrationError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }

    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    pub fn auth(msg: impl Into<String>) -> Self {
        Self::Authentication(msg.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// LLM request start event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStartEvent {
    /// Unique request ID
    pub request_id: String,
    /// Model name
    pub model: String,
    /// Provider name
    pub provider: Option<String>,
    /// Input messages/prompt
    pub input: serde_json::Value,
    /// Model parameters (temperature, max_tokens, etc.)
    pub parameters: HashMap<String, serde_json::Value>,
    /// User ID
    pub user_id: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Request timestamp (Unix milliseconds)
    pub timestamp_ms: i64,
}

impl LlmStartEvent {
    pub fn new(request_id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            model: model.into(),
            provider: None,
            input: serde_json::Value::Null,
            parameters: HashMap::new(),
            user_id: None,
            session_id: None,
            metadata: HashMap::new(),
            tags: Vec::new(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn input(mut self, input: serde_json::Value) -> Self {
        self.input = input;
        self
    }

    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.parameters.insert(key.into(), value);
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// LLM request end event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEndEvent {
    /// Request ID (matches start event)
    pub request_id: String,
    /// Model name
    pub model: String,
    /// Provider name
    pub provider: Option<String>,
    /// Output content
    pub output: serde_json::Value,
    /// Input token count
    pub input_tokens: Option<u32>,
    /// Output token count
    pub output_tokens: Option<u32>,
    /// Total cost in USD
    pub cost_usd: Option<f64>,
    /// Latency in milliseconds
    pub latency_ms: u64,
    /// Time to first token in milliseconds
    pub ttft_ms: Option<u64>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Request timestamp (Unix milliseconds)
    pub timestamp_ms: i64,
}

impl LlmEndEvent {
    pub fn new(request_id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            model: model.into(),
            provider: None,
            output: serde_json::Value::Null,
            input_tokens: None,
            output_tokens: None,
            cost_usd: None,
            latency_ms: 0,
            ttft_ms: None,
            metadata: HashMap::new(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn output(mut self, output: serde_json::Value) -> Self {
        self.output = output;
        self
    }

    pub fn tokens(mut self, input: u32, output: u32) -> Self {
        self.input_tokens = Some(input);
        self.output_tokens = Some(output);
        self
    }

    pub fn cost(mut self, cost_usd: f64) -> Self {
        self.cost_usd = Some(cost_usd);
        self
    }

    pub fn latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub fn ttft(mut self, ttft_ms: u64) -> Self {
        self.ttft_ms = Some(ttft_ms);
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// LLM error event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmErrorEvent {
    /// Request ID (matches start event)
    pub request_id: String,
    /// Model name
    pub model: String,
    /// Provider name
    pub provider: Option<String>,
    /// Error message
    pub error_message: String,
    /// Error type/code
    pub error_type: Option<String>,
    /// HTTP status code if applicable
    pub status_code: Option<u16>,
    /// Whether the error is retryable
    pub retryable: bool,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Error timestamp (Unix milliseconds)
    pub timestamp_ms: i64,
}

impl LlmErrorEvent {
    pub fn new(
        request_id: impl Into<String>,
        model: impl Into<String>,
        error_message: impl Into<String>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            model: model.into(),
            provider: None,
            error_message: error_message.into(),
            error_type: None,
            status_code: None,
            retryable: false,
            metadata: HashMap::new(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn error_type(mut self, error_type: impl Into<String>) -> Self {
        self.error_type = Some(error_type.into());
        self
    }

    pub fn status_code(mut self, code: u16) -> Self {
        self.status_code = Some(code);
        self
    }

    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// LLM streaming event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStreamEvent {
    /// Request ID
    pub request_id: String,
    /// Chunk index (0-based)
    pub chunk_index: u32,
    /// Chunk content
    pub content: String,
    /// Is this the final chunk?
    pub is_final: bool,
    /// Cumulative tokens so far
    pub tokens_so_far: Option<u32>,
    /// Timestamp (Unix milliseconds)
    pub timestamp_ms: i64,
}

impl LlmStreamEvent {
    pub fn new(
        request_id: impl Into<String>,
        chunk_index: u32,
        content: impl Into<String>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            chunk_index,
            content: content.into(),
            is_final: false,
            tokens_so_far: None,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn final_chunk(mut self) -> Self {
        self.is_final = true;
        self
    }

    pub fn tokens_so_far(mut self, tokens: u32) -> Self {
        self.tokens_so_far = Some(tokens);
        self
    }
}

/// Embedding start event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingStartEvent {
    /// Request ID
    pub request_id: String,
    /// Model name
    pub model: String,
    /// Provider name
    pub provider: Option<String>,
    /// Number of inputs
    pub input_count: usize,
    /// User ID
    pub user_id: Option<String>,
    /// Timestamp (Unix milliseconds)
    pub timestamp_ms: i64,
}

/// Embedding end event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingEndEvent {
    /// Request ID
    pub request_id: String,
    /// Model name
    pub model: String,
    /// Provider name
    pub provider: Option<String>,
    /// Total tokens used
    pub total_tokens: Option<u32>,
    /// Cost in USD
    pub cost_usd: Option<f64>,
    /// Latency in milliseconds
    pub latency_ms: u64,
    /// Timestamp (Unix milliseconds)
    pub timestamp_ms: i64,
}

/// Guardrail decision surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailDecisionSurface {
    /// Request/input inspection.
    Input,
    /// Response/output inspection.
    Output,
}

/// Sanitized action taken after a guardrail check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailDecisionAction {
    /// Content was allowed without mutation.
    Allow,
    /// Content was allowed and recorded.
    Log,
    /// Content was blocked.
    Block,
    /// Content was masked.
    Mask,
}

/// Sanitized guardrail decision. Never includes prompt, match, or response text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailDecisionEvent {
    /// Request ID when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Whether this decision applied to input or output.
    pub surface: GuardrailDecisionSurface,
    /// Action taken.
    pub action: GuardrailDecisionAction,
    /// Policy families that contributed identities (for example `pii`).
    pub policy_ids: Vec<String>,
    /// Specific rule identities (Display/debug names only).
    pub rule_ids: Vec<String>,
    /// Requested or served model when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Selected provider when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Selected deployment when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment: Option<String>,
    /// Event timestamp (Unix milliseconds).
    pub timestamp_ms: i64,
}

impl GuardrailDecisionEvent {
    /// Create a metadata-only decision event.
    pub fn new(surface: GuardrailDecisionSurface, action: GuardrailDecisionAction) -> Self {
        Self {
            request_id: None,
            surface,
            action,
            policy_ids: Vec::new(),
            rule_ids: Vec::new(),
            model: None,
            provider: None,
            deployment: None,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Embedding error event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingErrorEvent {
    /// Request ID (matches start event)
    pub request_id: String,
    /// Model name
    pub model: String,
    /// Provider name
    pub provider: Option<String>,
    /// Sanitized error message
    pub error_message: String,
    /// Error type/code
    pub error_type: Option<String>,
    /// Request latency in milliseconds
    pub latency_ms: u64,
    /// Error timestamp (Unix milliseconds)
    pub timestamp_ms: i64,
}

/// Cache hit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHitEvent {
    /// Request ID
    pub request_id: String,
    /// Cache key
    pub cache_key: String,
    /// Cache backend (memory, redis, s3, etc.)
    pub cache_backend: String,
    /// Time saved in milliseconds (estimated)
    pub time_saved_ms: Option<u64>,
    /// Cost saved in USD (estimated)
    pub cost_saved_usd: Option<f64>,
    /// Timestamp (Unix milliseconds)
    pub timestamp_ms: i64,
}

/// Core integration trait - all integrations must implement this
#[async_trait]
pub trait Integration: Send + Sync {
    /// Get the integration name
    fn name(&self) -> &'static str;

    /// Check if the integration is enabled
    fn is_enabled(&self) -> bool;

    /// Called when an LLM request starts
    async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()>;

    /// Called when an LLM request completes successfully
    async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()>;

    /// Called when an LLM request fails
    async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()>;

    /// Called for each streaming chunk (optional)
    async fn on_llm_stream(&self, event: &LlmStreamEvent) -> IntegrationResult<()> {
        let _ = event;
        Ok(())
    }

    /// Called when an embedding request starts (optional)
    async fn on_embedding_start(&self, event: &EmbeddingStartEvent) -> IntegrationResult<()> {
        let _ = event;
        Ok(())
    }

    /// Called when an embedding request completes (optional)
    async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
        let _ = event;
        Ok(())
    }

    /// Called when an embedding request fails (optional)
    async fn on_embedding_error(&self, event: &EmbeddingErrorEvent) -> IntegrationResult<()> {
        let _ = event;
        Ok(())
    }

    /// Called on cache hit (optional)
    async fn on_cache_hit(&self, event: &CacheHitEvent) -> IntegrationResult<()> {
        let _ = event;
        Ok(())
    }

    /// Called after a guardrail CheckResult is determined (optional).
    async fn on_guardrail_decision(&self, event: &GuardrailDecisionEvent) -> IntegrationResult<()> {
        let _ = event;
        Ok(())
    }

    /// Flush any pending data
    async fn flush(&self) -> IntegrationResult<()>;

    /// Graceful shutdown
    async fn shutdown(&self) -> IntegrationResult<()>;
}

/// Type alias for boxed integration
pub type BoxedIntegration = Arc<dyn Integration>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_start_event_builder() {
        let event = LlmStartEvent::new("req-123", "gpt-4")
            .provider("openai")
            .input(serde_json::json!({"messages": []}))
            .user_id("user-456")
            .session_id("session-789")
            .param("temperature", serde_json::json!(0.7))
            .metadata("custom", serde_json::json!("value"))
            .tag("production");

        assert_eq!(event.request_id, "req-123");
        assert_eq!(event.model, "gpt-4");
        assert_eq!(event.provider, Some("openai".to_string()));
        assert_eq!(event.user_id, Some("user-456".to_string()));
        assert!(event.parameters.contains_key("temperature"));
        assert!(event.tags.contains(&"production".to_string()));
    }

    #[test]
    fn test_llm_end_event_builder() {
        let event = LlmEndEvent::new("req-123", "gpt-4")
            .provider("openai")
            .output(serde_json::json!({"content": "Hello!"}))
            .tokens(100, 50)
            .cost(0.05)
            .latency(150)
            .ttft(50);

        assert_eq!(event.request_id, "req-123");
        assert_eq!(event.input_tokens, Some(100));
        assert_eq!(event.output_tokens, Some(50));
        assert_eq!(event.cost_usd, Some(0.05));
        assert_eq!(event.latency_ms, 150);
        assert_eq!(event.ttft_ms, Some(50));
    }

    #[test]
    fn test_llm_error_event_builder() {
        let event = LlmErrorEvent::new("req-123", "gpt-4", "Rate limited")
            .provider("openai")
            .error_type("RateLimitError")
            .status_code(429)
            .retryable(true);

        assert_eq!(event.request_id, "req-123");
        assert_eq!(event.error_message, "Rate limited");
        assert_eq!(event.error_type, Some("RateLimitError".to_string()));
        assert_eq!(event.status_code, Some(429));
        assert!(event.retryable);
    }

    #[test]
    fn test_llm_stream_event_builder() {
        let event = LlmStreamEvent::new("req-123", 0, "Hello")
            .tokens_so_far(5)
            .final_chunk();

        assert_eq!(event.request_id, "req-123");
        assert_eq!(event.chunk_index, 0);
        assert_eq!(event.content, "Hello");
        assert!(event.is_final);
        assert_eq!(event.tokens_so_far, Some(5));
    }

    #[test]
    fn test_integration_error_constructors() {
        let err = IntegrationError::config("Invalid API key");
        assert!(matches!(err, IntegrationError::Configuration(_)));

        let err = IntegrationError::connection("Connection refused");
        assert!(matches!(err, IntegrationError::Connection(_)));

        let err = IntegrationError::auth("Invalid token");
        assert!(matches!(err, IntegrationError::Authentication(_)));

        let err = IntegrationError::other("Unknown error");
        assert!(matches!(err, IntegrationError::Other(_)));
    }
}
