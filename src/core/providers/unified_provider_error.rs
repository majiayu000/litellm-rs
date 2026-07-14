/// Unified provider error type - single error for all providers
/// This eliminates the need for error type conversion and simplifies the architecture
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication failed for {provider}: {message}")]
    Authentication {
        provider: &'static str,
        message: String,
    },

    #[error("Rate limit exceeded for {provider}: {message}")]
    RateLimit {
        provider: &'static str,
        message: String,
        retry_after: Option<u64>,
        /// Requests per minute limit
        rpm_limit: Option<u32>,
        /// Tokens per minute limit
        tpm_limit: Option<u32>,
        /// Current usage level
        current_usage: Option<f64>,
    },

    #[error("Quota exceeded for {provider}: {message}")]
    QuotaExceeded {
        provider: &'static str,
        message: String,
    },

    #[error("Model '{model}' not found for {provider}")]
    ModelNotFound {
        provider: &'static str,
        model: String,
    },

    #[error("Invalid request for {provider}: {message}")]
    InvalidRequest {
        provider: &'static str,
        message: String,
    },

    #[error("Network error for {provider}: {message}")]
    Network {
        provider: &'static str,
        message: String,
    },

    #[error("Provider {provider} is unavailable: {message}")]
    ProviderUnavailable {
        provider: &'static str,
        message: String,
    },

    #[error("Feature '{feature}' not supported by {provider}")]
    NotSupported {
        provider: &'static str,
        feature: String,
    },

    #[error("Feature '{feature}' not implemented for {provider}")]
    NotImplemented {
        provider: &'static str,
        feature: String,
    },

    #[error("Configuration error for {provider}: {message}")]
    Configuration {
        provider: &'static str,
        message: String,
    },

    #[error("Serialization error for {provider}: {message}")]
    Serialization {
        provider: &'static str,
        message: String,
    },

    #[error("Timeout for {provider}: {message}")]
    Timeout {
        provider: &'static str,
        message: String,
    },

    // Enhanced error variants based on ultrathink analysis
    /// Context length exceeded with structured limits (VertexAI pattern)
    #[error("Context length exceeded for {provider}: max {max} tokens, got {actual} tokens")]
    ContextLengthExceeded {
        provider: &'static str,
        max: usize,
        actual: usize,
    },

    /// Content filtered by safety systems (VertexAI/OpenAI pattern)
    #[error("Content filtered by {provider} safety systems: {reason}")]
    ContentFiltered {
        provider: &'static str,
        reason: String,
        /// Policy categories that were violated
        policy_violations: Option<Vec<String>>,
        /// Whether this might succeed with prompt modification
        potentially_retryable: Option<bool>,
    },

    /// API error with status code (Universal pattern)
    #[error("API error for {provider} (status {status}): {message}")]
    ApiError {
        provider: &'static str,
        status: u16,
        message: String,
        /// Provider mapper asserted that retry is safe for this modeled error.
        retryable: bool,
    },

    /// Token limit exceeded (separate from context length)
    #[error("Token limit exceeded for {provider}: {message}")]
    TokenLimitExceeded {
        provider: &'static str,
        message: String,
    },

    /// Feature disabled by provider (VertexAI pattern)
    #[error("Feature disabled for {provider}: {feature}")]
    FeatureDisabled {
        provider: &'static str,
        feature: String,
    },

    /// Azure deployment specific error
    #[error("Azure deployment error for {deployment}: {message}")]
    DeploymentError {
        provider: &'static str,
        deployment: String,
        message: String,
    },

    /// Response parsing error (universal pattern)
    #[error("Failed to parse {provider} response: {message}")]
    ResponseParsing {
        provider: &'static str,
        message: String,
    },

    /// Multi-provider routing error (OpenRouter pattern)
    #[error("Routing error from {provider}: tried {attempted_providers:?}, final error: {message}")]
    RoutingError {
        provider: &'static str,
        attempted_providers: Vec<String>,
        message: String,
    },

    /// Transformation error between provider formats (OpenRouter pattern)
    #[error("Transformation error for {provider}: from {from_format} to {to_format}: {message}")]
    TransformationError {
        provider: &'static str,
        from_format: String,
        to_format: String,
        message: String,
    },

    /// Async operation cancelled (Rust async pattern)
    #[error("Operation cancelled for {provider}: {operation_type}")]
    Cancelled {
        provider: &'static str,
        operation_type: String,
        /// Reason for cancellation
        cancellation_reason: Option<String>,
    },

    /// Streaming operation error (SSE/WebSocket pattern)
    #[error("Streaming error for {provider}: {stream_type} at position {position:?}")]
    Streaming {
        provider: &'static str,
        /// Type of stream (chat, completion, etc.)
        stream_type: String,
        /// Position in stream where error occurred
        position: Option<u64>,
        /// Last valid chunk received
        last_chunk: Option<String>,
        /// Error message
        message: String,
    },

    #[error("{provider} error: {message}")]
    Other {
        provider: &'static str,
        message: String,
    },
}
