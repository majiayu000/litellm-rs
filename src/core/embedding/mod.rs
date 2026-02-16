//! Python LiteLLM compatible embedding API
//!
//! This module provides a Python LiteLLM-style API for generating embeddings.
//! It serves as the main entry point for embedding functionality, providing a unified
//! interface to call embedding APIs from multiple providers.
//!
//! # Example
//!
//! ```rust,no_run
//! use litellm_rs::core::embedding::{embedding, embed_text, embed_texts, cosine_similarity, EmbeddingOptions};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Simple embedding with default options
//! let response = embedding(
//!     "openai/text-embedding-ada-002",
//!     "Hello, world!",
//!     None,
//! ).await?;
//!
//! // Get embeddings for a single text
//! let vector = embed_text("text-embedding-3-small", "Hello").await?;
//!
//! // Get embeddings for multiple texts
//! let vectors = embed_texts("text-embedding-3-small", &["Hello", "World"]).await?;
//!
//! // Calculate similarity
//! let similarity = cosine_similarity(&vectors[0], &vectors[1]);
//!
//! // With custom options
//! let options = EmbeddingOptions::new()
//!     .with_dimensions(256)
//!     .with_api_key("sk-...");
//!
//! let response = embedding(
//!     "text-embedding-3-small",
//!     vec!["text1", "text2"],
//!     Some(options),
//! ).await?;
//! # Ok(())
//! # }
//! ```

mod helpers;
mod options;
mod router;
mod types;

#[cfg(test)]
mod tests;

// Re-export main types
pub use helpers::{cosine_similarity, dot_product, euclidean_distance, normalize};
pub use options::EmbeddingOptions;
pub use router::{EmbeddingRouter, get_global_embedding_router};
pub use types::EmbeddingInput;

// Re-export response types from core types
pub use crate::core::types::responses::{EmbeddingData, EmbeddingResponse, EmbeddingUsage};

/// LiteLLM Error type alias
pub type LiteLLMError = crate::utils::error::gateway_error::GatewayError;

/// Create embeddings using any supported provider
///
/// This is the main entry point for generating embeddings. It supports multiple
/// providers through model prefixes (e.g., "openai/text-embedding-ada-002").
///
/// # Arguments
///
/// * `model` - Model identifier, optionally prefixed with provider (e.g., "openai/text-embedding-3-small")
/// * `input` - Input text(s) to embed (can be a single string, &str, or Vec of strings)
/// * `options` - Optional configuration for the embedding request
///
/// # Returns
///
/// Returns an `EmbeddingResponse` containing the embedding vectors and usage information.
///
/// # Errors
///
/// Returns an error if:
/// - No provider is configured for the specified model
/// - The API request fails
/// - The response cannot be parsed
///
/// # Example
///
/// ```rust,no_run
/// use litellm_rs::core::embedding::{embedding, EmbeddingOptions};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Simple usage
/// let response = embedding("text-embedding-3-small", "Hello, world!", None).await?;
///
/// // With options
/// let options = EmbeddingOptions::new().with_dimensions(256);
/// let response = embedding("text-embedding-3-small", "Hello", Some(options)).await?;
///
/// // Multiple texts
/// let response = embedding("text-embedding-3-small", vec!["Hello", "World"], None).await?;
/// # Ok(())
/// # }
/// ```
pub async fn embedding(
    model: &str,
    input: impl Into<EmbeddingInput>,
    options: Option<EmbeddingOptions>,
) -> crate::utils::error::gateway_error::Result<EmbeddingResponse> {
    let router = get_global_embedding_router().await?;
    router
        .embed(model, input.into(), options.unwrap_or_default())
        .await
}

/// Async version of embedding (for compatibility, all Rust async is the same)
pub async fn aembedding(
    model: &str,
    input: impl Into<EmbeddingInput>,
    options: Option<EmbeddingOptions>,
) -> crate::utils::error::gateway_error::Result<EmbeddingResponse> {
    embedding(model, input, options).await
}

/// Embed a single text and return the embedding vector
///
/// This is a convenience function for embedding a single text and extracting
/// the embedding vector directly.
///
/// # Arguments
///
/// * `model` - Model identifier
/// * `text` - Text to embed
///
/// # Returns
///
/// Returns the embedding vector as `Vec<f32>`.
///
/// # Example
///
/// ```rust,no_run
/// use litellm_rs::core::embedding::embed_text;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let vector = embed_text("text-embedding-3-small", "Hello, world!").await?;
/// println!("Embedding dimension: {}", vector.len());
/// # Ok(())
/// # }
/// ```
pub async fn embed_text(
    model: &str,
    text: &str,
) -> crate::utils::error::gateway_error::Result<Vec<f32>> {
    let response = embedding(model, text, None).await?;

    response
        .data
        .into_iter()
        .next()
        .map(|d| d.embedding)
        .ok_or_else(|| {
            crate::utils::error::gateway_error::GatewayError::internal(
                "No embedding data in response",
            )
        })
}

/// Embed multiple texts and return their embedding vectors
///
/// This is a convenience function for embedding multiple texts at once
/// and extracting the embedding vectors directly.
///
/// # Arguments
///
/// * `model` - Model identifier
/// * `texts` - Slice of texts to embed
///
/// # Returns
///
/// Returns a vector of embedding vectors, one for each input text.
///
/// # Example
///
/// ```rust,no_run
/// use litellm_rs::core::embedding::{embed_texts, cosine_similarity};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let vectors = embed_texts("text-embedding-3-small", &["Hello", "World"]).await?;
/// let similarity = cosine_similarity(&vectors[0], &vectors[1]);
/// println!("Similarity: {}", similarity);
/// # Ok(())
/// # }
/// ```
pub async fn embed_texts(
    model: &str,
    texts: &[&str],
) -> crate::utils::error::gateway_error::Result<Vec<Vec<f32>>> {
    let input: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
    let response = embedding(model, input, None).await?;

    // Sort by index to maintain order
    let mut embeddings: Vec<(u32, Vec<f32>)> = response
        .data
        .into_iter()
        .map(|d| (d.index, d.embedding))
        .collect();
    embeddings.sort_by_key(|(idx, _)| *idx);

    Ok(embeddings.into_iter().map(|(_, emb)| emb).collect())
}

/// Embed texts with custom options and return embedding vectors
///
/// Like `embed_texts` but allows passing custom options.
///
/// # Arguments
///
/// * `model` - Model identifier
/// * `texts` - Slice of texts to embed
/// * `options` - Embedding options (dimensions, encoding format, etc.)
///
/// # Returns
///
/// Returns a vector of embedding vectors, one for each input text.
pub async fn embed_texts_with_options(
    model: &str,
    texts: &[&str],
    options: EmbeddingOptions,
) -> crate::utils::error::gateway_error::Result<Vec<Vec<f32>>> {
    let input: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
    let response = embedding(model, input, Some(options)).await?;

    // Sort by index to maintain order
    let mut embeddings: Vec<(u32, Vec<f32>)> = response
        .data
        .into_iter()
        .map(|d| (d.index, d.embedding))
        .collect();
    embeddings.sort_by_key(|(idx, _)| *idx);

    Ok(embeddings.into_iter().map(|(_, emb)| emb).collect())
}
