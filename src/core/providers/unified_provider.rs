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

#[path = "unified_provider_error.rs"]
mod error;
#[path = "unified_provider_http_mapping.rs"]
mod http_mapping;
#[path = "unified_provider_macros.rs"]
mod macros;
#[path = "unified_provider_methods.rs"]
mod methods;

// Re-export ContextualError from the dedicated module.
pub use super::contextual_error::ContextualError;
pub use error::ProviderError;
pub use http_mapping::{
    default_http_error_mapper, extended_http_error_mapper, parse_error_message_from_body,
};
