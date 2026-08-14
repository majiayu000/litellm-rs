//! Ollama Provider
//!
//! Ollama provides local AI model inference through a simple HTTP API.
//! This implementation supports chat completions (streaming and non-streaming),
//! embeddings, and local model management.
//!
//! ## Features
//! - Chat completions with streaming support
//! - Embeddings generation
//! - Local model management (list, pull, show)
//! - Ollama-specific parameters (mirostat, num_ctx, etc.)
//! - Vision/multimodal support for compatible models
//! - Tool/function calling for supported models
//!
//! ## Configuration
//! The default API base is `http://localhost:11434`.
//! Set `OLLAMA_API_BASE` environment variable to customize.

// Core modules
mod config;
mod embedding;
mod error;
mod model_info;
mod provider;
mod request_options;
#[cfg(test)]
mod runtime_tests;
mod streaming;

// Re-export main types for external use
pub use config::OllamaConfig;
pub use error::OllamaError;
pub use model_info::{OllamaModelInfo, get_model_info};
pub use provider::OllamaProvider;
