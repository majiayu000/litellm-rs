//! OpenAI provider error type alias
//!
//! The canonical `OpenAIError` with provider-specific constructors lives in
//! `src/core/providers/openai/error.rs` as a re-export of `ProviderError`.
//! This module re-exports the same type for use through the `types::errors` path.

pub use crate::core::providers::unified_provider::ProviderError as OpenAIError;

/// Result type alias
pub type OpenAIResult<T> = Result<T, OpenAIError>;
