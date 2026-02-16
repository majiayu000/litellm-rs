//! Unified Provider Error Handling
//!
//! Single error type for all providers - optimized design for simplicity and performance
//!
//! This module provides a unified error handling system for all AI providers.
//!
//! ## Core Components
//!
//! ### `ProviderError` Enum
//! A comprehensive error type that covers all possible failure scenarios across different AI providers.
//!
//! | Variant | Purpose | HTTP Status | Retryable |
//! |------|------|------------|--------|
//! | Authentication | Authentication failed | 401 | No |
//! | RateLimit | Rate limit exceeded | 429 | Yes (after delay) |
//! | ModelNotFound | Model not found | 404 | No |
//! | InvalidRequest | Invalid request | 400 | No |
//! | Network | Network error | 500 | Yes |
//! | Timeout | Timeout | 408 | Yes |
//! | Internal | Internal error | 500 | Yes |
//! | ServiceUnavailable | Service unavailable | 503 | Yes |
//! | QuotaExceeded | Quota exceeded | 402 | No |
//! | NotSupported | Feature not supported | 501 | No |
//! | Other | Other error | 500 | No |
//!
//! ## Usage
//!
//! ```rust
//! use litellm_rs::ProviderError;
//!
//! // 1. Direct construction
//! let err = ProviderError::Authentication {
//!     provider: "openai",
//!     message: "Invalid API key".to_string()
//! };
//!
//! // 2. Use factory methods (preferred)
//! let err = ProviderError::authentication("openai", "Invalid API key");
//! let err = ProviderError::rate_limit("anthropic", Some(60));
//!
//! // 3. Check error properties
//! if err.is_retryable() {
//!     if let Some(delay) = err.retry_delay() {
//!         println!("Retry after {} seconds", delay);
//!     }
//! }
//! ```
//!
//! ## Migration Guide
//!
//! For migrating from provider-specific error types:
//!
//! ```rust
//! // Old code
//! // pub enum MyProviderError { ... }
//!
//! // New code - use unified error type
//! use litellm_rs::ProviderError;
//! pub type MyProviderError = ProviderError;
//! ```
//!
//! ## Design Advantages
//!
//! - **Unified Interface**: Single error type for all providers eliminates conversion overhead
//! - **Rich Context**: Structured error information with provider-specific details
//! - **Retry Logic**: Built-in retry determination and delay calculation
//! - **HTTP Mapping**: Automatic HTTP status code mapping for web APIs
//! - **Performance**: Zero-cost abstractions with compile-time optimization

// Re-export ContextualError from the dedicated module
pub use super::contextual_error::ContextualError;

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

impl ProviderError {
    /// Create authentication error
    pub fn authentication(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Authentication {
            provider,
            message: message.into(),
        }
    }

    /// Create rate limit error
    pub fn rate_limit(provider: &'static str, retry_after: Option<u64>) -> Self {
        Self::RateLimit {
            provider,
            message: match retry_after {
                Some(seconds) => format!("Rate limit exceeded. Retry after {} seconds", seconds),
                None => "Rate limit exceeded".to_string(),
            },
            retry_after,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        }
    }

    /// Create enhanced rate limit error with usage details
    pub fn rate_limit_with_limits(
        provider: &'static str,
        retry_after: Option<u64>,
        rpm_limit: Option<u32>,
        tpm_limit: Option<u32>,
        current_usage: Option<f64>,
    ) -> Self {
        let message = match (rpm_limit, tpm_limit) {
            (Some(rpm), Some(tpm)) => {
                format!("Rate limit exceeded: {}RPM, {}TPM limits reached", rpm, tpm)
            }
            (Some(rpm), None) => format!("Rate limit exceeded: {}RPM limit reached", rpm),
            (None, Some(tpm)) => format!("Rate limit exceeded: {}TPM limit reached", tpm),
            (None, None) => "Rate limit exceeded".to_string(),
        };

        Self::RateLimit {
            provider,
            message,
            retry_after,
            rpm_limit,
            tpm_limit,
            current_usage,
        }
    }

    /// Create quota exceeded error
    pub fn quota_exceeded(provider: &'static str, message: impl Into<String>) -> Self {
        Self::QuotaExceeded {
            provider,
            message: message.into(),
        }
    }

    /// Create simple rate limit error (convenience method)
    pub fn rate_limit_simple(provider: &'static str, message: impl Into<String>) -> Self {
        Self::RateLimit {
            provider,
            message: message.into(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        }
    }

    /// Create rate limit error with retry_after only
    pub fn rate_limit_with_retry(
        provider: &'static str,
        message: impl Into<String>,
        retry_after: Option<u64>,
    ) -> Self {
        Self::RateLimit {
            provider,
            message: message.into(),
            retry_after,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        }
    }

    /// Create model not found error
    pub fn model_not_found(provider: &'static str, model: impl Into<String>) -> Self {
        Self::ModelNotFound {
            provider,
            model: model.into(),
        }
    }

    /// Create invalid request error
    pub fn invalid_request(provider: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            provider,
            message: message.into(),
        }
    }

    /// Create network error
    pub fn network(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Network {
            provider,
            message: message.into(),
        }
    }

    /// Create provider unavailable error
    pub fn provider_unavailable(provider: &'static str, message: impl Into<String>) -> Self {
        Self::ProviderUnavailable {
            provider,
            message: message.into(),
        }
    }

    /// Create not supported error
    pub fn not_supported(provider: &'static str, feature: impl Into<String>) -> Self {
        Self::NotSupported {
            provider,
            feature: feature.into(),
        }
    }

    /// Create not implemented error
    pub fn not_implemented(provider: &'static str, feature: impl Into<String>) -> Self {
        Self::NotImplemented {
            provider,
            feature: feature.into(),
        }
    }

    /// Create configuration error
    pub fn configuration(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Configuration {
            provider,
            message: message.into(),
        }
    }

    /// Create serialization error
    pub fn serialization(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Serialization {
            provider,
            message: message.into(),
        }
    }

    /// Create timeout error
    pub fn timeout(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Timeout {
            provider,
            message: message.into(),
        }
    }

    /// Create initialization error (provider failed to start)
    pub fn initialization(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Network {
            provider,
            message: format!("Initialization failed: {}", message.into()),
        }
    }

    // Enhanced factory methods for new error variants

    /// Create context length exceeded error with structured data
    pub fn context_length_exceeded(provider: &'static str, max: usize, actual: usize) -> Self {
        Self::ContextLengthExceeded {
            provider,
            max,
            actual,
        }
    }

    /// Create API error with status code
    pub fn api_error(provider: &'static str, status: u16, message: impl Into<String>) -> Self {
        Self::ApiError {
            provider,
            status,
            message: message.into(),
        }
    }

    /// Create token limit exceeded error
    pub fn token_limit_exceeded(provider: &'static str, message: impl Into<String>) -> Self {
        Self::TokenLimitExceeded {
            provider,
            message: message.into(),
        }
    }

    /// Create feature disabled error
    pub fn feature_disabled(provider: &'static str, feature: impl Into<String>) -> Self {
        Self::FeatureDisabled {
            provider,
            feature: feature.into(),
        }
    }

    /// Create Azure deployment error
    pub fn deployment_error(deployment: impl Into<String>, message: impl Into<String>) -> Self {
        Self::DeploymentError {
            provider: "azure",
            deployment: deployment.into(),
            message: message.into(),
        }
    }

    /// Create response parsing error
    pub fn response_parsing(provider: &'static str, message: impl Into<String>) -> Self {
        Self::ResponseParsing {
            provider,
            message: message.into(),
        }
    }

    /// Create routing error
    pub fn routing_error(
        provider: &'static str,
        attempted_providers: Vec<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::RoutingError {
            provider,
            attempted_providers,
            message: message.into(),
        }
    }

    /// Create transformation error
    pub fn transformation_error(
        provider: &'static str,
        from_format: impl Into<String>,
        to_format: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::TransformationError {
            provider,
            from_format: from_format.into(),
            to_format: to_format.into(),
            message: message.into(),
        }
    }

    /// Create content filtered error
    pub fn content_filtered(
        provider: &'static str,
        reason: impl Into<String>,
        policy_violations: Option<Vec<String>>,
        potentially_retryable: Option<bool>,
    ) -> Self {
        Self::ContentFiltered {
            provider,
            reason: reason.into(),
            policy_violations,
            potentially_retryable,
        }
    }

    /// Create cancellation error
    pub fn cancelled(
        provider: &'static str,
        operation_type: impl Into<String>,
        cancellation_reason: Option<String>,
    ) -> Self {
        Self::Cancelled {
            provider,
            operation_type: operation_type.into(),
            cancellation_reason,
        }
    }

    /// Create streaming error
    pub fn streaming_error(
        provider: &'static str,
        stream_type: impl Into<String>,
        position: Option<u64>,
        last_chunk: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Streaming {
            provider,
            stream_type: stream_type.into(),
            position,
            last_chunk,
            message: message.into(),
        }
    }

    /// Create other/generic error
    pub fn other(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Other {
            provider,
            message: message.into(),
        }
    }

    /// Get the provider name that caused this error
    pub fn provider(&self) -> &'static str {
        match self {
            Self::Authentication { provider, .. }
            | Self::RateLimit { provider, .. }
            | Self::QuotaExceeded { provider, .. }
            | Self::ModelNotFound { provider, .. }
            | Self::InvalidRequest { provider, .. }
            | Self::Network { provider, .. }
            | Self::ProviderUnavailable { provider, .. }
            | Self::NotSupported { provider, .. }
            | Self::NotImplemented { provider, .. }
            | Self::Configuration { provider, .. }
            | Self::Serialization { provider, .. }
            | Self::Timeout { provider, .. }
            | Self::ContextLengthExceeded { provider, .. }
            | Self::ContentFiltered { provider, .. }
            | Self::ApiError { provider, .. }
            | Self::TokenLimitExceeded { provider, .. }
            | Self::FeatureDisabled { provider, .. }
            | Self::DeploymentError { provider, .. }
            | Self::ResponseParsing { provider, .. }
            | Self::RoutingError { provider, .. }
            | Self::TransformationError { provider, .. }
            | Self::Cancelled { provider, .. }
            | Self::Streaming { provider, .. }
            | Self::Other { provider, .. } => provider,
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network { .. }
            | Self::Timeout { .. }
            | Self::RateLimit { .. }
            | Self::ProviderUnavailable { .. } => true,

            // API errors depend on status code
            Self::ApiError { status, .. } => matches!(*status, 429 | 500..=599),

            // Deployment errors might be retryable depending on the issue
            Self::DeploymentError { .. } => true,

            // Streaming errors are typically retryable
            Self::Streaming { .. } => true,

            // Content filtered might be retryable with prompt changes
            Self::ContentFiltered { potentially_retryable, .. } => {
                potentially_retryable.unwrap_or(false)
            },

            // All other errors are not retryable
            Self::Authentication { .. }
            | Self::QuotaExceeded { .. }
            | Self::ModelNotFound { .. }
            | Self::InvalidRequest { .. }
            | Self::NotSupported { .. }
            | Self::NotImplemented { .. }
            | Self::Configuration { .. }
            | Self::Serialization { .. }
            | Self::ContextLengthExceeded { .. }
            | Self::TokenLimitExceeded { .. }
            | Self::FeatureDisabled { .. }
            | Self::ResponseParsing { .. }
            | Self::RoutingError { .. }
            | Self::TransformationError { .. }
            | Self::Cancelled { .. } // User cancelled, don't retry
            | Self::Other { .. } => false,
        }
    }

    /// Get retry delay in seconds
    pub fn retry_delay(&self) -> Option<u64> {
        match self {
            Self::RateLimit { retry_after, .. } => *retry_after,
            Self::Network { .. } | Self::Timeout { .. } => Some(1),
            Self::ProviderUnavailable { .. } => Some(5),

            // API errors with 429 (rate limit) or 5xx get retry delays
            Self::ApiError { status, .. } => match *status {
                429 => Some(60),      // Rate limit, wait longer
                500..=599 => Some(3), // Server errors, shorter delay
                _ => None,
            },

            // Deployment errors get a retry delay
            Self::DeploymentError { .. } => Some(5),

            // Streaming errors get a shorter retry delay
            Self::Streaming { .. } => Some(2),

            // Content filtered - conditional retry
            Self::ContentFiltered {
                potentially_retryable,
                ..
            } => {
                if potentially_retryable.unwrap_or(false) {
                    Some(10) // Allow time for prompt modification
                } else {
                    None
                }
            }

            // All other errors have no retry delay
            Self::Authentication { .. }
            | Self::QuotaExceeded { .. }
            | Self::ModelNotFound { .. }
            | Self::InvalidRequest { .. }
            | Self::NotSupported { .. }
            | Self::NotImplemented { .. }
            | Self::Configuration { .. }
            | Self::Serialization { .. }
            | Self::ContextLengthExceeded { .. }
            | Self::TokenLimitExceeded { .. }
            | Self::FeatureDisabled { .. }
            | Self::ResponseParsing { .. }
            | Self::RoutingError { .. }
            | Self::TransformationError { .. }
            | Self::Cancelled { .. }
            | Self::Other { .. } => None,
        }
    }

    /// Create an error with request context for better debugging.
    ///
    /// Returns a `ContextualError` that wraps this error with additional request information.
    ///
    /// # Example
    /// ```rust
    /// # use litellm_rs::ProviderError;
    /// let err = ProviderError::network("openai", "Connection refused")
    ///     .with_context("req-123", Some("gpt-4"));
    /// ```
    pub fn with_context(
        self,
        request_id: impl Into<String>,
        model: Option<&str>,
    ) -> ContextualError {
        ContextualError::new(self, request_id, model)
    }

    /// Get HTTP status code for this error
    pub fn http_status(&self) -> u16 {
        match self {
            Self::Authentication { .. } => 401,
            Self::RateLimit { .. } => 429,
            Self::QuotaExceeded { .. } => 402, // Payment Required
            Self::ModelNotFound { .. } => 404,
            Self::InvalidRequest { .. } => 400,
            Self::Configuration { .. } => 400,
            Self::NotSupported { .. } => 405,
            Self::NotImplemented { .. } => 501,
            Self::Network { .. } | Self::Timeout { .. } | Self::ProviderUnavailable { .. } => 503,
            Self::Serialization { .. } => 500,

            // Enhanced error variants with appropriate HTTP status codes
            Self::ContextLengthExceeded { .. } => 413, // Payload Too Large
            Self::ContentFiltered { .. } => 400,       // Bad Request (content policy violation)
            Self::ApiError { status, .. } => *status,  // Use the actual API status
            Self::TokenLimitExceeded { .. } => 413,    // Payload Too Large
            Self::FeatureDisabled { .. } => 403,       // Forbidden (feature not available)
            Self::DeploymentError { .. } => 404,       // Not Found (deployment not found)
            Self::ResponseParsing { .. } => 502,       // Bad Gateway (upstream response invalid)
            Self::RoutingError { .. } => 503, // Service Unavailable (no providers available)
            Self::TransformationError { .. } => 500, // Internal Server Error (conversion failed)
            Self::Cancelled { .. } => 499,    // Client Closed Request
            Self::Streaming { .. } => 500,    // Internal Server Error (streaming failed)

            Self::Other { .. } => 500,
        }
    }
}

// Error conversions are in provider_error_conversions.rs module

/// Type alias for backward compatibility
pub type UnifiedProviderError = ProviderError;

/// Generate standard provider error helper functions.
///
/// Creates 9 free functions for a provider: `{prefix}_config_error`, `{prefix}_auth_error`,
/// `{prefix}_api_error`, `{prefix}_network_error`, `{prefix}_parse_error`,
/// `{prefix}_stream_error`, `{prefix}_rate_limit_error`, `{prefix}_model_error`,
/// `{prefix}_validation_error`.
///
/// # Usage
/// ```ignore
/// define_provider_error_helpers!("gemini", gemini);
/// // Generates: gemini_config_error, gemini_auth_error, etc.
/// ```
///
/// Provider-specific helpers (e.g. `gemini_safety_error`) should be defined manually.
#[macro_export]
macro_rules! define_provider_error_helpers {
    ($provider:expr, $prefix:ident) => {
        ::paste::paste! {
            /// Create configuration error
            pub fn [<$prefix _config_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::configuration($provider, msg.into())
            }

            /// Create authentication error
            pub fn [<$prefix _auth_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::authentication($provider, msg.into())
            }

            /// Create API error with status code
            pub fn [<$prefix _api_error>](status: u16, msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::api_error($provider, status, msg.into())
            }

            /// Create network error
            pub fn [<$prefix _network_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::network($provider, msg.into())
            }

            /// Create parsing/serialization error
            pub fn [<$prefix _parse_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::serialization($provider, msg.into())
            }

            /// Create streaming error
            pub fn [<$prefix _stream_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::streaming_error($provider, "chat", None, None, msg.into())
            }

            /// Create rate limit error
            pub fn [<$prefix _rate_limit_error>](retry_after: Option<u64>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::rate_limit($provider, retry_after)
            }

            /// Create model not found error
            pub fn [<$prefix _model_error>](model: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::model_not_found($provider, model.into())
            }

            /// Create validation/invalid request error
            pub fn [<$prefix _validation_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::invalid_request($provider, msg.into())
            }
        }
    };
}

// Legacy methods and ProviderErrorTrait implementation are in provider_error_conversions.rs

/// Generate standard provider error methods on `ProviderError`.
///
/// Creates methods like `{prefix}_authentication`, `{prefix}_rate_limit`, etc.
/// as `impl ProviderError` associated functions.
///
/// # Usage
/// ```ignore
/// impl_provider_error_helpers!("huggingface", huggingface);
/// // Generates: ProviderError::huggingface_authentication(...), etc.
/// ```
///
/// Provider-specific methods should be defined in a separate `impl` block.
#[macro_export]
macro_rules! impl_provider_error_helpers {
    ($provider:expr, $prefix:ident) => {
        ::paste::paste! {
            impl $crate::core::providers::unified_provider::ProviderError {
                /// Create authentication error
                pub fn [<$prefix _authentication>](message: impl Into<String>) -> Self {
                    Self::authentication($provider, message)
                }

                /// Create rate limit error
                pub fn [<$prefix _rate_limit>](retry_after: Option<u64>) -> Self {
                    Self::rate_limit($provider, retry_after)
                }

                /// Create model not found error
                pub fn [<$prefix _model_not_found>](model: impl Into<String>) -> Self {
                    Self::model_not_found($provider, model)
                }

                /// Create invalid request error
                pub fn [<$prefix _invalid_request>](message: impl Into<String>) -> Self {
                    Self::invalid_request($provider, message)
                }

                /// Create network error
                pub fn [<$prefix _network_error>](message: impl Into<String>) -> Self {
                    Self::network($provider, message)
                }

                /// Create timeout error
                pub fn [<$prefix _timeout>](message: impl Into<String>) -> Self {
                    Self::Timeout {
                        provider: $provider,
                        message: message.into(),
                    }
                }

                /// Create response parsing error
                pub fn [<$prefix _response_parsing>](message: impl Into<String>) -> Self {
                    Self::response_parsing($provider, message)
                }

                /// Create configuration error
                pub fn [<$prefix _configuration>](message: impl Into<String>) -> Self {
                    Self::configuration($provider, message)
                }

                /// Create API error with status code
                pub fn [<$prefix _api_error>](status: u16, message: impl Into<String>) -> Self {
                    Self::ApiError {
                        provider: $provider,
                        status,
                        message: message.into(),
                    }
                }

                /// Check if this is a provider-specific error
                pub fn [<is_ $prefix _error>](&self) -> bool {
                    self.provider() == $provider
                }
            }
        }
    };
}

/// Default HTTP status-code → `ProviderError` mapping shared by most providers.
///
/// Providers with custom handling (e.g. Gemini, LangGraph, Databricks) should
/// implement `map_http_error` manually and call this for the status codes they
/// don't need to override.
pub fn default_http_error_mapper(
    provider: &'static str,
    status_code: u16,
    response_body: &str,
) -> ProviderError {
    match status_code {
        400 => {
            let message = parse_error_message_from_body(response_body)
                .unwrap_or_else(|| response_body.to_string());
            ProviderError::invalid_request(provider, message)
        }
        401 => ProviderError::authentication(provider, "Invalid API key"),
        403 => ProviderError::authentication(provider, "Permission denied"),
        404 => ProviderError::model_not_found(provider, "Model not found"),
        429 => {
            let retry_after = parse_retry_after_from_body(response_body);
            ProviderError::rate_limit(provider, retry_after)
        }
        500..=599 => ProviderError::api_error(provider, status_code, response_body),
        _ => ProviderError::api_error(provider, status_code, response_body),
    }
}

/// Try to extract an error message from a JSON response body.
///
/// Checks `error.message` and top-level `message` fields.
pub fn parse_error_message_from_body(response_body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(response_body).ok()?;
    json.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .or_else(|| json.get("message").and_then(|m| m.as_str()))
        .map(|s| s.to_string())
}

/// Try to extract a retry-after duration from a JSON response body.
///
/// Checks `retry_after` and `error.retry_after` fields, then falls back to
/// keyword detection ("rate limit" / "rate_limit" / "too many requests").
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

/// Generate a standard `ErrorMapper` implementation for a provider.
///
/// Creates:
/// - `{Name}ErrorMapper` struct
/// - `ErrorMapper<ProviderError>` impl delegating to `default_http_error_mapper`
///
/// # Usage
/// ```ignore
/// define_standard_error_mapper!("deepseek", DeepSeek);
/// // Generates: pub struct DeepSeekErrorMapper; + impl ErrorMapper<ProviderError>
/// ```
#[macro_export]
macro_rules! define_standard_error_mapper {
    ($provider:expr, $name:ident) => {
        ::paste::paste! {
            /// Error mapper for the provider, using the standard HTTP status code mapping.
            #[derive(Debug)]
            pub struct [<$name ErrorMapper>];

            impl $crate::core::traits::error_mapper::trait_def::ErrorMapper<
                $crate::core::providers::unified_provider::ProviderError,
            > for [<$name ErrorMapper>]
            {
                fn map_http_error(
                    &self,
                    status_code: u16,
                    response_body: &str,
                ) -> $crate::core::providers::unified_provider::ProviderError {
                    $crate::core::providers::unified_provider::default_http_error_mapper(
                        $provider,
                        status_code,
                        response_body,
                    )
                }
            }
        }
    };
}

/// Extended HTTP status-code → `ProviderError` mapping with additional codes.
///
/// Handles 402 (quota), 413 (context length), 408/504 (timeout), 502/503
/// (provider unavailable) in addition to the standard codes.
pub fn extended_http_error_mapper(
    provider: &'static str,
    status_code: u16,
    response_body: &str,
) -> ProviderError {
    match status_code {
        400 => ProviderError::invalid_request(provider, response_body),
        401 | 403 => ProviderError::authentication(provider, response_body),
        402 => ProviderError::quota_exceeded(provider, response_body),
        404 => ProviderError::model_not_found(provider, response_body),
        408 | 504 => ProviderError::timeout(provider, response_body),
        413 => ProviderError::context_length_exceeded(provider, 0, 0),
        429 => ProviderError::rate_limit(provider, None),
        500 => ProviderError::api_error(provider, status_code, response_body),
        502 | 503 => ProviderError::provider_unavailable(provider, response_body),
        _ => ProviderError::api_error(provider, status_code, response_body),
    }
}

/// Generate an extended `ErrorMapper` implementation for a provider.
///
/// Like `define_standard_error_mapper!` but includes 402, 413, 408/504, 502/503.
#[macro_export]
macro_rules! define_extended_error_mapper {
    ($provider:expr, $name:ident) => {
        ::paste::paste! {
            /// Error mapper for the provider, using the extended HTTP status code mapping.
            #[derive(Debug)]
            pub struct [<$name ErrorMapper>];

            impl $crate::core::traits::error_mapper::trait_def::ErrorMapper<
                $crate::core::providers::unified_provider::ProviderError,
            > for [<$name ErrorMapper>]
            {
                fn map_http_error(
                    &self,
                    status_code: u16,
                    response_body: &str,
                ) -> $crate::core::providers::unified_provider::ProviderError {
                    $crate::core::providers::unified_provider::extended_http_error_mapper(
                        $provider,
                        status_code,
                        response_body,
                    )
                }
            }
        }
    };
}
