//! OpenAI Provider Error Handling
//!
//! OpenAI uses the unified ProviderError with specific constructor methods for OpenAI-specific contexts

pub use crate::core::providers::unified_provider::ProviderError as OpenAIError;

/// OpenAI-specific error constructors
impl OpenAIError {
    /// Create OpenAI authentication error
    pub fn openai_authentication(message: impl Into<String>) -> Self {
        Self::authentication("openai", message)
    }

    /// Create OpenAI rate limit error with detailed context  
    pub fn openai_rate_limit(
        retry_after: Option<u64>,
        rpm_limit: Option<u32>,
        tpm_limit: Option<u32>,
        current_usage: Option<f64>,
    ) -> Self {
        Self::rate_limit_with_limits("openai", retry_after, rpm_limit, tpm_limit, current_usage)
    }

    /// Create OpenAI content policy violation error
    pub fn openai_content_filtered(
        reason: impl Into<String>,
        policy_violations: Option<Vec<String>>,
        potentially_retryable: bool,
    ) -> Self {
        Self::ContentFiltered {
            provider: "openai",
            reason: reason.into(),
            policy_violations,
            potentially_retryable: Some(potentially_retryable),
        }
    }

    /// Create OpenAI context length exceeded error  
    pub fn openai_context_exceeded(max: usize, actual: usize) -> Self {
        Self::ContextLengthExceeded {
            provider: "openai",
            max,
            actual,
        }
    }

    /// Create OpenAI model not found error
    pub fn openai_model_not_found(model: impl Into<String>) -> Self {
        Self::model_not_found("openai", model)
    }

    /// Create OpenAI quota exceeded error
    pub fn openai_quota_exceeded(message: impl Into<String>) -> Self {
        Self::quota_exceeded("openai", message)
    }

    /// Create OpenAI bad request error
    pub fn openai_bad_request(message: impl Into<String>) -> Self {
        Self::invalid_request("openai", message)
    }

    /// Create OpenAI network error
    pub fn openai_network_error(message: impl Into<String>) -> Self {
        Self::network("openai", message)
    }

    /// Create OpenAI timeout error
    pub fn openai_timeout(message: impl Into<String>) -> Self {
        Self::Timeout {
            provider: "openai",
            message: message.into(),
        }
    }

    /// Create OpenAI streaming error
    pub fn openai_streaming_error(
        stream_type: impl Into<String>,
        position: Option<u64>,
        message: impl Into<String>,
    ) -> Self {
        Self::streaming_error("openai", stream_type, position, None, message)
    }

    /// Create OpenAI cancellation error  
    pub fn openai_cancelled(operation_type: impl Into<String>, reason: Option<String>) -> Self {
        Self::cancelled("openai", operation_type, reason)
    }

    /// Create OpenAI response parsing error
    pub fn openai_response_parsing(message: impl Into<String>) -> Self {
        Self::response_parsing("openai", message)
    }

    /// Create OpenAI serialization error
    pub fn openai_serialization(message: impl Into<String>) -> Self {
        Self::serialization("openai", message)
    }

    /// Create OpenAI configuration error
    pub fn openai_configuration(message: impl Into<String>) -> Self {
        Self::configuration("openai", message)
    }

    /// Create OpenAI API error with status code
    pub fn openai_api_error(status: u16, message: impl Into<String>) -> Self {
        Self::ApiError {
            provider: "openai",
            status,
            message: message.into(),
        }
    }

    /// Create generic OpenAI error
    pub fn openai_other(message: impl Into<String>) -> Self {
        Self::other("openai", message)
    }
}

/// Async-compatible error utilities for OpenAI  
impl OpenAIError {
    /// Get async-friendly retry delay
    pub async fn async_retry_delay(&self) -> Option<std::time::Duration> {
        self.retry_delay().map(std::time::Duration::from_secs)
    }

    /// Check if this is an OpenAI-specific error
    pub fn is_openai_error(&self) -> bool {
        self.provider() == "openai"
    }

    /// Get OpenAI error category for metrics
    pub fn openai_category(&self) -> &'static str {
        match self {
            Self::Authentication { .. } => "auth",
            Self::RateLimit { .. } => "rate_limit",
            Self::ContentFiltered { .. } => "content_policy",
            Self::ContextLengthExceeded { .. } => "context_limit",
            Self::QuotaExceeded { .. } => "quota",
            Self::ModelNotFound { .. } => "model",
            Self::Streaming { .. } => "streaming",
            Self::Cancelled { .. } => "cancelled",
            Self::Network { .. } | Self::Timeout { .. } => "network",
            Self::ResponseParsing { .. } | Self::Serialization { .. } => "parsing",
            _ => "other",
        }
    }
}

// Note: serde_json::Error conversion is handled by the base ProviderError

impl From<tokio::time::error::Elapsed> for OpenAIError {
    fn from(error: tokio::time::error::Elapsed) -> Self {
        Self::openai_timeout(format!("Operation timed out: {}", error))
    }
}

// Specific error types for OpenAI contexts

/// OpenAI Content Policy Types
#[derive(Debug, Clone)]
pub enum OpenAIContentPolicyType {
    Violence,
    Sexual,
    Hate,
    Harassment,
    SelfHarm,
    Illegal,
    Deception,
    Other(String),
}

impl std::fmt::Display for OpenAIContentPolicyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Violence => write!(f, "violence"),
            Self::Sexual => write!(f, "sexual"),
            Self::Hate => write!(f, "hate"),
            Self::Harassment => write!(f, "harassment"),
            Self::SelfHarm => write!(f, "self-harm"),
            Self::Illegal => write!(f, "illegal"),
            Self::Deception => write!(f, "deception"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

/// OpenAI Operation Types for cancellation tracking
#[derive(Debug, Clone)]
pub enum OpenAIOperationType {
    ChatCompletion,
    TextCompletion,
    ImageGeneration,
    AudioTranscription,
    Embedding,
    FineTuning,
    Other(String),
}

impl std::fmt::Display for OpenAIOperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChatCompletion => write!(f, "chat_completion"),
            Self::TextCompletion => write!(f, "text_completion"),
            Self::ImageGeneration => write!(f, "image_generation"),
            Self::AudioTranscription => write!(f, "audio_transcription"),
            Self::Embedding => write!(f, "embedding"),
            Self::FineTuning => write!(f, "fine_tuning"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

/// OpenAI Stream Types  
#[derive(Debug, Clone)]
pub enum OpenAIStreamType {
    ChatCompletion,
    TextCompletion,
    AudioTranscription,
}

impl std::fmt::Display for OpenAIStreamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChatCompletion => write!(f, "chat_completion"),
            Self::TextCompletion => write!(f, "text_completion"),
            Self::AudioTranscription => write!(f, "audio_transcription"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_authentication_error() {
        let err = OpenAIError::openai_authentication("Invalid API key");
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "auth");
    }

    #[test]
    fn test_openai_rate_limit_error() {
        let err = OpenAIError::openai_rate_limit(Some(60), Some(100), Some(10000), Some(0.5));
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "rate_limit");
    }

    #[test]
    fn test_openai_content_filtered_error() {
        let err = OpenAIError::openai_content_filtered(
            "Content violates policy",
            Some(vec!["violence".to_string()]),
            false,
        );
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "content_policy");
    }

    #[test]
    fn test_openai_context_exceeded_error() {
        let err = OpenAIError::openai_context_exceeded(4096, 5000);
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "context_limit");
    }

    #[test]
    fn test_openai_model_not_found_error() {
        let err = OpenAIError::openai_model_not_found("gpt-5");
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "model");
    }

    #[test]
    fn test_openai_network_error() {
        let err = OpenAIError::openai_network_error("Connection failed");
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "network");
    }

    #[test]
    fn test_openai_timeout_error() {
        let err = OpenAIError::openai_timeout("Request timed out after 30s");
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "network");
    }

    #[test]
    fn test_openai_streaming_error() {
        let err = OpenAIError::openai_streaming_error("chat", Some(100), "Stream interrupted");
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "streaming");
    }

    #[test]
    fn test_openai_api_error() {
        let err = OpenAIError::openai_api_error(500, "Internal server error");
        assert!(err.is_openai_error());
        assert_eq!(err.openai_category(), "other");
    }

    #[test]
    fn test_content_policy_type_display() {
        assert_eq!(OpenAIContentPolicyType::Violence.to_string(), "violence");
        assert_eq!(OpenAIContentPolicyType::Sexual.to_string(), "sexual");
        assert_eq!(OpenAIContentPolicyType::Hate.to_string(), "hate");
        assert_eq!(
            OpenAIContentPolicyType::Other("custom".to_string()).to_string(),
            "custom"
        );
    }

    #[test]
    fn test_operation_type_display() {
        assert_eq!(
            OpenAIOperationType::ChatCompletion.to_string(),
            "chat_completion"
        );
        assert_eq!(
            OpenAIOperationType::ImageGeneration.to_string(),
            "image_generation"
        );
        assert_eq!(
            OpenAIOperationType::Other("custom_op".to_string()).to_string(),
            "custom_op"
        );
    }

    #[test]
    fn test_stream_type_display() {
        assert_eq!(
            OpenAIStreamType::ChatCompletion.to_string(),
            "chat_completion"
        );
        assert_eq!(
            OpenAIStreamType::AudioTranscription.to_string(),
            "audio_transcription"
        );
    }
}
