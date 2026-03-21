//! Vector store backend enum with dispatch methods

use crate::config::models::file_storage::VectorDbConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use tracing::info;

use super::pinecone::PineconeStore;
use super::qdrant::QdrantStore;
use super::types::{SearchResult, VectorPoint};
use super::weaviate::WeaviateStore;

/// Vector store backend enum
#[derive(Debug, Clone)]
pub enum VectorStoreBackend {
    /// Qdrant vector database
    Qdrant(QdrantStore),
    /// Weaviate vector database
    Weaviate(WeaviateStore),
    /// Pinecone vector database
    Pinecone(PineconeStore),
}

impl VectorStoreBackend {
    /// Create a new vector store instance
    pub async fn new(config: &VectorDbConfig) -> Result<Self> {
        info!("Initializing vector database: {}", config.db_type);

        match config.db_type.as_str() {
            "qdrant" => Ok(VectorStoreBackend::Qdrant(QdrantStore::new(config).await?)),
            "weaviate" => Ok(VectorStoreBackend::Weaviate(
                WeaviateStore::new(config).await?,
            )),
            "pinecone" => Ok(VectorStoreBackend::Pinecone(
                PineconeStore::new(config).await?,
            )),
            _ => Err(GatewayError::Config(format!(
                "Unsupported vector DB type: {}",
                config.db_type
            ))),
        }
    }

    /// Store a vector with metadata
    pub async fn store(
        &self,
        id: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        match self {
            VectorStoreBackend::Qdrant(store) => store.store(id, vector, metadata).await,
            VectorStoreBackend::Weaviate(store) => store.store(id, vector, metadata).await,
            VectorStoreBackend::Pinecone(store) => store.store(id, vector, metadata).await,
        }
    }

    /// Search for similar vectors
    pub async fn search(
        &self,
        query_vector: &[f32],
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SearchResult>> {
        match self {
            VectorStoreBackend::Qdrant(store) => store.search(query_vector, limit, threshold).await,
            VectorStoreBackend::Weaviate(store) => {
                store.search(query_vector, limit, threshold).await
            }
            VectorStoreBackend::Pinecone(store) => {
                store.search(query_vector, limit, threshold).await
            }
        }
    }

    /// Delete a vector by ID
    pub async fn delete(&self, id: &str) -> Result<()> {
        match self {
            VectorStoreBackend::Qdrant(store) => store.delete(id).await,
            VectorStoreBackend::Weaviate(store) => store.delete(id).await,
            VectorStoreBackend::Pinecone(store) => store.delete(id).await,
        }
    }

    /// Get a vector by ID
    pub async fn get(&self, id: &str) -> Result<Option<VectorPoint>> {
        match self {
            VectorStoreBackend::Qdrant(store) => store.get(id).await,
            VectorStoreBackend::Weaviate(store) => store.get(id).await,
            VectorStoreBackend::Pinecone(store) => store.get(id).await,
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        match self {
            VectorStoreBackend::Qdrant(store) => store.health_check().await,
            VectorStoreBackend::Weaviate(store) => store.health_check().await,
            VectorStoreBackend::Pinecone(store) => store.health_check().await,
        }
    }

    /// Close connections
    pub async fn close(&self) -> Result<()> {
        match self {
            VectorStoreBackend::Qdrant(_store) => Ok(()), // No explicit close needed for HTTP clients
            VectorStoreBackend::Weaviate(_store) => Ok(()),
            VectorStoreBackend::Pinecone(_store) => Ok(()),
        }
    }

    /// Batch store vectors
    pub async fn batch_store(&self, points: &[VectorPoint]) -> Result<()> {
        match self {
            VectorStoreBackend::Qdrant(store) => store.batch_store(points).await,
            VectorStoreBackend::Weaviate(store) => store.batch_store(points).await,
            VectorStoreBackend::Pinecone(store) => store.batch_store(points).await,
        }
    }

    /// Count vectors in collection
    pub async fn count(&self) -> Result<u64> {
        match self {
            VectorStoreBackend::Qdrant(store) => store.count().await,
            VectorStoreBackend::Weaviate(store) => store.count().await,
            VectorStoreBackend::Pinecone(store) => store.count().await,
        }
    }
}
