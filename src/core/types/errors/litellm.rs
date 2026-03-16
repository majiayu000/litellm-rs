//! LiteLLM error type alias
//!
//! The canonical error type is `GatewayError` in `crate::utils::error::gateway_error`.
//! This module re-exports it under the LiteLLM name for API compatibility.

/// Top-level error type alias for the LiteLLM gateway
pub type LiteLLMError = crate::utils::error::gateway_error::GatewayError;

/// Result type alias
pub type LiteLLMResult<T> = Result<T, LiteLLMError>;
