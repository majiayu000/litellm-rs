//! Contextual Error Wrapper
//!
//! Provides error context with request ID, model, and timestamp for better debugging and logging.

use super::unified_provider::ProviderError;

/// Error with request context for better debugging and logging.
///
/// Wraps a `ProviderError` with additional context like request ID and model,
/// making it easier to trace errors in production logs.
#[derive(Debug, Clone)]
pub struct ContextualError {
    /// The underlying provider error
    pub inner: ProviderError,
    /// Request ID for tracing
    pub request_id: String,
    /// Model that was being used (if applicable)
    pub model: Option<String>,
    /// When the error occurred
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl std::fmt::Display for ContextualError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[request_id={}] {}", self.request_id, self.inner)?;
        if let Some(model) = &self.model {
            write!(f, " (model: {})", model)?;
        }
        Ok(())
    }
}

impl std::error::Error for ContextualError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

impl ContextualError {
    /// Create a new contextual error
    pub fn new(inner: ProviderError, request_id: impl Into<String>, model: Option<&str>) -> Self {
        Self {
            inner,
            request_id: request_id.into(),
            model: model.map(|s| s.to_string()),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Get the underlying provider error
    pub fn inner(&self) -> &ProviderError {
        &self.inner
    }

    /// Get the request ID
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Get the model if available
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Get retry delay in seconds
    pub fn retry_delay(&self) -> Option<u64> {
        self.inner.retry_delay()
    }

    /// Get HTTP status code
    pub fn http_status(&self) -> u16 {
        self.inner.http_status()
    }

    /// Get the provider name
    pub fn provider(&self) -> &'static str {
        self.inner.provider()
    }

    /// Convert to a JSON-serializable error response
    pub fn to_error_response(&self) -> serde_json::Value {
        serde_json::json!({
            "error": {
                "message": self.inner.to_string(),
                "type": format!("{:?}", std::mem::discriminant(&self.inner)),
                "code": self.http_status(),
                "request_id": self.request_id,
                "model": self.model,
                "provider": self.provider(),
                "retryable": crate::core::providers::failure::ProviderFailureFacts::from_error(&self.inner).legacy_retryable,
                "retry_after": self.retry_delay(),
            }
        })
    }
}

impl From<ContextualError> for ProviderError {
    fn from(err: ContextualError) -> Self {
        err.inner
    }
}
