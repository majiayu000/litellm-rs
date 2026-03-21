//! Qdrant vector store implementation

use crate::config::models::file_storage::VectorDbConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use tracing::{debug, info};

use super::types::{SearchResult, VectorPoint};

/// Qdrant vector store
#[derive(Debug, Clone)]
pub struct QdrantStore {
    url: String,
    api_key: Option<String>,
    collection: String,
    client: reqwest::Client,
}

impl QdrantStore {
    /// Create a new Qdrant store
    pub async fn new(config: &VectorDbConfig) -> Result<Self> {
        let client = reqwest::Client::new();

        let store = Self {
            url: config.url.clone(),
            api_key: Some(config.api_key.clone()),
            collection: config.index_name.clone(),
            client,
        };

        // Ensure collection exists
        store.ensure_collection().await?;

        info!("Qdrant vector store initialized");
        Ok(store)
    }

    /// Ensure collection exists
    async fn ensure_collection(&self) -> Result<()> {
        let url = format!("{}/collections/{}", self.url, self.collection);
        let mut request = self.client.get(&url);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to check collection: {}", e)))?;

        if response.status() == 404 {
            // Collection doesn't exist, create it
            self.create_collection().await?;
        } else if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Failed to check collection: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Create collection
    async fn create_collection(&self) -> Result<()> {
        let url = format!("{}/collections/{}", self.url, self.collection);
        let payload = serde_json::json!({
            "vectors": {
                "size": 1536, // Default OpenAI embedding size
                "distance": "Cosine"
            }
        });

        let mut request = self.client.put(&url).json(&payload);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to create collection: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Failed to create collection: {}",
                response.status()
            )));
        }

        info!("Created Qdrant collection: {}", self.collection);
        Ok(())
    }

    /// Store a vector
    pub async fn store(
        &self,
        id: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let url = format!("{}/collections/{}/points", self.url, self.collection);
        let payload = serde_json::json!({
            "points": [{
                "id": id,
                "vector": vector,
                "payload": metadata.unwrap_or_default()
            }]
        });

        let mut request = self.client.put(&url).json(&payload);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to store vector: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Failed to store vector: {}",
                response.status()
            )));
        }

        debug!("Stored vector: {}", id);
        Ok(())
    }

    /// Search for similar vectors
    pub async fn search(
        &self,
        query_vector: &[f32],
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SearchResult>> {
        let url = format!("{}/collections/{}/points/search", self.url, self.collection);
        let mut payload = serde_json::json!({
            "vector": query_vector,
            "limit": limit,
            "with_payload": true,
            "with_vector": false
        });

        if let Some(threshold) = threshold {
            payload["score_threshold"] = serde_json::Number::from_f64(threshold as f64)
                .map(serde_json::Value::Number)
                .ok_or_else(|| {
                    GatewayError::Storage(format!(
                        "Invalid score threshold: {} (NaN/Infinity not allowed)",
                        threshold
                    ))
                })?;
        }

        let mut request = self.client.post(&url).json(&payload);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to search vectors: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Failed to search vectors: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            GatewayError::Storage(format!("Failed to parse search response: {}", e))
        })?;

        let mut search_results = Vec::new();
        if let Some(points) = result["result"].as_array() {
            for point in points {
                if let (Some(id), Some(score)) = (point["id"].as_str(), point["score"].as_f64()) {
                    search_results.push(SearchResult {
                        id: id.to_string(),
                        score: score as f32,
                        metadata: point["payload"].clone().into(),
                        vector: None,
                    });
                }
            }
        }

        Ok(search_results)
    }

    /// Delete a vector
    pub async fn delete(&self, id: &str) -> Result<()> {
        let url = format!("{}/collections/{}/points/delete", self.url, self.collection);
        let payload = serde_json::json!({
            "points": [id]
        });

        let mut request = self.client.post(&url).json(&payload);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to delete vector: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Failed to delete vector: {}",
                response.status()
            )));
        }

        debug!("Deleted vector: {}", id);
        Ok(())
    }

    /// Get a vector by ID
    pub async fn get(&self, id: &str) -> Result<Option<VectorPoint>> {
        let url = format!("{}/collections/{}/points/{}", self.url, self.collection, id);
        let mut request = self.client.get(&url);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to get vector: {}", e)))?;

        if response.status() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Failed to get vector: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to parse get response: {}", e)))?;

        if let Some(point) = result["result"].as_object()
            && let (Some(id), Some(vector)) = (point["id"].as_str(), point["vector"].as_array())
        {
            let vector_data: Vec<f32> = vector
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();

            return Ok(Some(VectorPoint {
                id: id.to_string(),
                vector: vector_data,
                metadata: point["payload"].clone().into(),
            }));
        }

        Ok(None)
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/", self.url);
        let mut request = self.client.get(&url);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Qdrant health check failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Qdrant health check failed: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Close connections
    pub async fn close(&self) -> Result<()> {
        // HTTP client doesn't need explicit closing
        Ok(())
    }

    /// Batch store vectors
    pub async fn batch_store(&self, points: &[VectorPoint]) -> Result<()> {
        let url = format!("{}/collections/{}/points", self.url, self.collection);
        let qdrant_points: Vec<serde_json::Value> = points
            .iter()
            .map(|point| {
                serde_json::json!({
                    "id": point.id,
                    "vector": point.vector,
                    "payload": point.metadata.clone().unwrap_or_default()
                })
            })
            .collect();

        let payload = serde_json::json!({
            "points": qdrant_points
        });

        let mut request = self.client.put(&url).json(&payload);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to batch store vectors: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Failed to batch store vectors: {}",
                response.status()
            )));
        }

        debug!("Batch stored {} vectors", points.len());
        Ok(())
    }

    /// Count vectors in collection
    pub async fn count(&self) -> Result<u64> {
        let url = format!("{}/collections/{}", self.url, self.collection);
        let mut request = self.client.get(&url);

        if let Some(api_key) = &self.api_key {
            request = request.header("api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GatewayError::Storage(format!("Failed to get collection info: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::Storage(format!(
                "Failed to get collection info: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            GatewayError::Storage(format!("Failed to parse collection info: {}", e))
        })?;

        if let Some(count) = result["result"]["points_count"].as_u64() {
            Ok(count)
        } else {
            Ok(0)
        }
    }
}
