//! Error types and traits for unified error handling across providers
//!
//! This module provides a hierarchical error system with provider-agnostic interfaces.
//!
//! ## Architecture Overview
//!
//! The error system is organized into multiple layers:
//!
//! ## 1. Trait Layer
//! - `ProviderErrorTrait`: Common interface for all provider errors
//! - Provides unified methods:
//!   - retry_delay(): Recommended retry delay duration
//!   - http_status(): HTTP status code mapping
//!   - Factory methods: not_supported(), authentication_failed() etc
//!
//! ## 2. Unified Implementation Layer
//! - `ProviderError`: Single concrete error type for all providers
//! - Common variants:
//!   - Authentication: Authentication failed
//!   - RateLimit: Rate limit exceeded with retry information
//!   - ModelNotFound: Requested model does not exist
//!   - InvalidRequest: Malformed or invalid request
//!   - Network: Network connectivity issues
//!   - Timeout: Request timeout exceeded
//!   - ApiError: Provider-specific API errors
//!   - ServiceUnavailable: Service temporarily unavailable
//!   - QuotaExceeded: Usage quota exceeded
//!   - NotSupported: Feature not supported by provider
//!   - ContentFiltered: Content blocked by safety filters
//!
//! ## Usage
//! ```rust
//! // All providers use unified ProviderError
//! use litellm_rs::ProviderError;
//!
//! // Create errors using factory methods
//! let err = ProviderError::authentication("openai", "Invalid API key");
//! let err = ProviderError::rate_limit("anthropic", Some(60));
//!
//! // Inspect error facts; routing decisions use RetryPolicy::decide
//! if let Some(delay) = err.retry_delay() {
//!     println!("Provider suggested retry delay: {} seconds", delay);
//! }
//! ```
//!
//! ## Design Principles
//! 1. **Unified Interface**: Single error type eliminates conversion overhead
//! 2. **Extensible**: Define interfaces through traits for future expansion
//! 3. **Zero-cost abstraction**: Use static dispatch, no runtime overhead
//! 4. **Rich Context**: Structured error information with provider-specific details

mod config;
mod litellm;
mod macros;
mod openai;
mod traits;

// Re-export all types for backward compatibility
pub use config::{ConfigError, ConfigResult};
pub use litellm::{LiteLLMError, LiteLLMResult};
pub use openai::{OpenAIError, OpenAIResult};
pub use traits::ProviderErrorTrait;
