//! OpenAI-Like Provider - Generic OpenAI-Compatible API Provider
//!
//! This provider allows connecting to any OpenAI-compatible API endpoint.
//! It supports custom API bases, API keys, and headers, making it ideal for:
//! - Self-hosted models (LMStudio, Ollama, vLLM)
//! - Third-party OpenAI-compatible services
//! - Custom proxy endpoints
//! - Testing and development

pub mod config;
pub mod error;
pub mod models;
pub mod provider;
mod request_headers;
pub mod streaming;

// Re-exports for easy access
pub use config::OpenAILikeConfig;
pub use error::OpenAILikeError;
pub use models::OpenAILikeModelRegistry;
pub use provider::OpenAILikeProvider;
