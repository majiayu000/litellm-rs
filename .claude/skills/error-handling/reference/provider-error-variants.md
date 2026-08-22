## ProviderError (24 Variants)

```rust
// src/core/providers/unified_provider.rs

#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    // Authentication & Authorization
    #[error("[{provider}] Authentication failed: {message}")]
    Authentication {
        provider: &'static str,
        message: String,
    },

    // Rate Limiting & Quotas
    #[error("[{provider}] Rate limit exceeded: {message}")]
    RateLimit {
        provider: &'static str,
        message: String,
        retry_after: Option<u64>,
        rpm_limit: Option<u32>,
        tpm_limit: Option<u32>,
        current_usage: Option<u32>,
    },

    #[error("[{provider}] Quota exceeded: {message}")]
    QuotaExceeded {
        provider: &'static str,
        message: String,
    },

    // Model & Request Errors
    #[error("[{provider}] Model not found: {model}")]
    ModelNotFound {
        provider: &'static str,
        model: String,
    },

    #[error("[{provider}] Invalid request: {message}")]
    InvalidRequest {
        provider: &'static str,
        message: String,
    },

    // Network & Availability
    #[error("[{provider}] Network error: {message}")]
    Network {
        provider: &'static str,
        message: String,
    },

    #[error("[{provider}] Request timeout: {message}")]
    Timeout {
        provider: &'static str,
        message: String,
    },

    #[error("[{provider}] Provider unavailable: {message}")]
    ProviderUnavailable {
        provider: &'static str,
        message: String,
    },

    // Feature Support
    #[error("[{provider}] Feature not supported: {feature}")]
    NotSupported {
        provider: &'static str,
        feature: String,
    },

    #[error("[{provider}] Not implemented: {feature}")]
    NotImplemented {
        provider: &'static str,
        feature: String,
    },

    #[error("[{provider}] Feature disabled: {feature}")]
    FeatureDisabled {
        provider: &'static str,
        feature: String,
    },

    // Content & Token Limits
    #[error("[{provider}] Context length exceeded: max={max}, actual={actual}")]
    ContextLengthExceeded {
        provider: &'static str,
        max: u32,
        actual: u32,
    },

    #[error("[{provider}] Token limit exceeded: {message}")]
    TokenLimitExceeded {
        provider: &'static str,
        message: String,
    },

    #[error("[{provider}] Content filtered: {reason}")]
    ContentFiltered {
        provider: &'static str,
        reason: String,
        policy_violations: Option<Vec<String>>,
        potentially_retryable: Option<bool>,
    },

    // Configuration & Serialization
    #[error("[{provider}] Configuration error: {message}")]
    Configuration {
        provider: &'static str,
        message: String,
    },

    #[error("[{provider}] Serialization error: {message}")]
    Serialization {
        provider: &'static str,
        message: String,
    },

    // Advanced Errors
    #[error("[{provider}] API error (status={status}): {message}")]
    ApiError {
        provider: &'static str,
        status: u16,
        message: String,
    },

    #[error("[{provider}] Deployment error ({deployment}): {message}")]
    DeploymentError {
        provider: &'static str,
        deployment: String,
        message: String,
    },

    #[error("[{provider}] Response parsing error: {message}")]
    ResponseParsing {
        provider: &'static str,
        message: String,
    },

    #[error("[{provider}] Routing error: {message}")]
    RoutingError {
        provider: &'static str,
        attempted_providers: Vec<String>,
        message: String,
    },

    #[error("[{provider}] Transformation error ({from_format} -> {to_format}): {message}")]
    TransformationError {
        provider: &'static str,
        from_format: String,
        to_format: String,
        message: String,
    },

    #[error("[{provider}] Streaming error ({stream_type}): {message}")]
    Streaming {
        provider: &'static str,
        stream_type: String,
        position: Option<u64>,
        last_chunk: Option<String>,
        message: String,
    },

    #[error("[{provider}] Operation cancelled ({operation_type}): {cancellation_reason}")]
    Cancelled {
        provider: &'static str,
        operation_type: String,
        cancellation_reason: String,
    },

    #[error("[{provider}] Error: {message}")]
    Other {
        provider: &'static str,
        message: String,
    },
}
```
