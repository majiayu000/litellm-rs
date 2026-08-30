//! OpenAI Provider - New Architecture Implementation
//!
//! Complete OpenAI API integration following the unified provider architecture.
//! Covers chat, image generation, and embeddings. Auxiliary surfaces
//! (audio transcription, completions, fine-tuning, image edit/variations,
//! vector stores, realtime, advanced chat helpers) were declared but never
//! reached from any live code path and were removed.

mod api_methods;
pub(crate) use api_methods::execute_image_edit;
pub mod client;
mod client_convenience;
#[cfg(test)]
mod client_tests;
pub mod config;
pub mod error;
pub mod error_mapper;
pub mod models;
pub mod streaming;
#[cfg(test)]
mod streaming_request_tests;
pub mod transformer;

pub use client::OpenAIProvider;
pub use config::OpenAIConfig;
pub use error::OpenAIError;
pub use error_mapper::OpenAIErrorMapper;
pub use models::{OpenAIModelRegistry, get_openai_registry};
pub use transformer::{OpenAIRequestTransformer, OpenAIResponseTransformer, OpenAITransformer};
