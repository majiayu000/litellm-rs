//! Rerank API for document relevance scoring
//!
//! This module provides reranking functionality to score and reorder documents
//! based on their relevance to a query. It's commonly used in RAG (Retrieval
//! Augmented Generation) systems to improve retrieval quality.
//!
//! ## Supported Providers
//! - Cohere (rerank-english-v3.0, rerank-multilingual-v3.0)
//! - Jina AI (jina-reranker-v2-base-multilingual)
//! - Voyage AI (rerank-2, rerank-2-lite)
//! - OpenAI (via embeddings + similarity)
//!
//! ## Example
//! ```rust,no_run
//! # use litellm_rs::core::rerank::{RerankRequest, RerankDocument, RerankProvider};
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let request = RerankRequest {
//!     model: "cohere/rerank-english-v3.0".to_string(),
//!     query: "What is the capital of France?".to_string(),
//!     documents: vec![
//!         RerankDocument::text("Paris is the capital of France."),
//!         RerankDocument::text("London is the capital of England."),
//!         RerankDocument::text("Berlin is the capital of Germany."),
//!     ],
//!     top_n: Some(2),
//!     return_documents: Some(true),
//!     ..Default::default()
//! };
//!
//! // provider.rerank(request).await? would be called here
//! # Ok(())
//! # }
//! ```

mod cache;
pub mod providers;
mod service;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use cache::{RerankCache, RerankCacheStats};
pub use providers::{CohereRerankProvider, JinaRerankProvider, VoyageRerankProvider};
pub use service::{RerankProvider, RerankService};
pub use types::{RerankDocument, RerankRequest, RerankResponse, RerankResult, RerankUsage};
