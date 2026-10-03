//! Cohere Provider
//!
//! Complete Cohere AI integration supporting:
//! - Chat completions (Command models, v1 and v2 APIs)
//! - Embeddings (embed-english-v3.0, embed-multilingual-v3.0, etc.)
//! - Reranking (rerank-english-v3.0, rerank-multilingual-v3.0, etc.)
//! - RAG support with citations and documents

#[cfg(feature = "providers-extended")]
mod chat;
#[cfg(feature = "providers-extended")]
mod config;
#[cfg(feature = "providers-extended")]
mod embed;
#[cfg(feature = "providers-extended")]
mod error;
mod models;
#[cfg(feature = "providers-extended")]
mod provider;
#[cfg(feature = "providers-extended")]
mod rerank;
#[cfg(feature = "providers-extended")]
mod streaming;

#[cfg(all(test, feature = "providers-extended"))]
mod tests;

// Re-export main types
#[cfg(feature = "providers-extended")]
pub use config::{CohereApiVersion, CohereConfig};
#[cfg(feature = "providers-extended")]
pub use error::CohereError;
#[cfg(feature = "providers-extended")]
pub use provider::CohereProvider;
#[cfg(feature = "providers-extended")]
pub use rerank::{RerankRequest, RerankResponse, RerankResult};

/// Callable chat IDs from the same metadata used by the native provider.
pub(crate) fn supported_chat_models() -> Vec<String> {
    models::create_model_registry()
        .into_iter()
        .filter(|model| {
            model
                .capabilities
                .contains(&crate::core::types::model::ProviderCapability::ChatCompletion)
        })
        .map(|model| model.id)
        .collect()
}
