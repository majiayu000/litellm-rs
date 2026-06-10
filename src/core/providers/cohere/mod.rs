//! Cohere Provider
//!
//! Complete Cohere AI integration supporting:
//! - Chat completions (Command models, v1 and v2 APIs)
//! - Embeddings (embed-english-v3.0, embed-multilingual-v3.0, etc.)
//! - Reranking (rerank-english-v3.0, rerank-multilingual-v3.0, etc.)
//! - RAG support with citations and documents

mod chat;
mod config;
mod embed;
mod error;
mod provider;
mod rerank;
mod streaming;

#[cfg(test)]
mod tests;

// Re-export main types
pub use config::{CohereApiVersion, CohereConfig};
pub use error::CohereError;
pub use provider::CohereProvider;
pub use rerank::{RerankRequest, RerankResponse, RerankResult};
