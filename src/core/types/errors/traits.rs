//! Error traits for unified error handling

/// Common trait for all provider error types
///
/// Provides a unified interface for error handling across different AI providers.
pub trait ProviderErrorTrait: std::error::Error + Send + Sync + 'static {
    /// Get the error type as a string identifier
    fn error_type(&self) -> &'static str;

    /// Get the recommended retry delay in seconds
    fn retry_delay(&self) -> Option<u64>;

    /// Get the appropriate HTTP status code for this error
    fn http_status(&self) -> u16;

    /// Create a "feature not supported" error
    fn not_supported(feature: &str) -> Self;

    /// Create an authentication failure error
    fn authentication_failed(reason: &str) -> Self;

    /// Create a rate limit error with optional retry delay
    fn rate_limited(retry_after: Option<u64>) -> Self;

    /// Create a network error
    fn network_error(details: &str) -> Self;

    /// Create a parsing/serialization error
    fn parsing_error(details: &str) -> Self;

    /// Create a "feature not implemented" error
    fn not_implemented(feature: &str) -> Self;
}
