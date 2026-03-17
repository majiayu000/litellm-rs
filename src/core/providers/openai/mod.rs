//! OpenAI Provider - New Architecture Implementation
//!
//! Complete OpenAI API integration following the unified provider architecture.
//! Supports all OpenAI services: Chat, Images, Audio, Embeddings, Fine-tuning, etc.

mod api_methods;
pub mod client;
#[cfg(test)]
mod client_tests;
pub mod config;
pub mod error;
pub mod error_mapper;
pub mod models;
pub mod streaming;
pub mod transformer;

// Feature-specific modules
pub mod capabilities;

// New functionality modules
pub mod advanced_chat;
pub mod completions;
pub mod fine_tuning;
pub mod image_edit;
pub mod image_variations;
pub mod realtime;
pub mod vector_stores;

// No top-level capability glob re-exports: use explicit module paths.
pub use client::OpenAIProvider;
pub use config::OpenAIConfig;
pub use error::OpenAIError;
pub use error_mapper::OpenAIErrorMapper;
pub use models::{OpenAIModelRegistry, get_openai_registry};
pub use transformer::{OpenAIRequestTransformer, OpenAIResponseTransformer, OpenAITransformer};
