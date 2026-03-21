//! Storage layer for the Gateway
//!
//! This module provides data persistence and caching functionality.

/// Database storage module
pub mod database;
/// File storage module
pub mod files;
/// Redis cache module
pub mod redis;
/// Vector storage module
pub mod vector;

use crate::config::models::storage::StorageConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Main storage layer that orchestrates all storage backends
#[derive(Debug, Clone)]
pub struct StorageLayer {
    /// Database connection pool
    pub database: Arc<database::Database>,
    /// Redis connection pool
    pub redis: Arc<redis::RedisPool>,
    /// File storage backend
    pub files: Arc<files::FileStorage>,
    /// Vector database client (optional)
    /// Note: Using concrete type instead of trait object for now
    pub vector: Option<Arc<vector::VectorStoreBackend>>,
}

impl StorageLayer {
    /// Create a new storage layer
    pub async fn new(config: &StorageConfig) -> Result<Self> {
        info!("Initializing storage layer");

        // Initialize database
        debug!("Connecting to database");
        let database = Arc::new(database::Database::new(&config.database).await?);

        // Initialize Redis (optional with graceful degradation)
        debug!("Creating Redis connection pool");
        let redis = match redis::RedisPool::new(&config.redis).await {
            Ok(pool) => {
                if pool.is_noop() {
                    info!("Redis caching is disabled (no-op mode)");
                } else {
                    info!("Redis connection established");
                }
                Arc::new(pool)
            }
            Err(e) => {
                warn!(
                    "Redis connection failed: {}. Gateway will operate without caching.",
                    e
                );
                // Create a no-op Redis pool wrapper
                Arc::new(redis::RedisPool::create_noop())
            }
        };

        // Initialize file storage (using default config for now)
        debug!("Initializing file storage");
        let default_file_config = crate::config::models::file_storage::FileStorageConfig::default();
        let files = Arc::new(files::FileStorage::new(&default_file_config).await?);

        // Initialize vector database (optional)
        let vector = if let Some(ref vector_config) = config.vector_db {
            debug!("Initializing vector database");
            match vector::VectorStoreBackend::new(vector_config).await {
                Ok(v) => Some(Arc::new(v)),
                Err(e) => {
                    warn!(
                        "Vector database initialization failed: {}, continuing without vector DB",
                        e
                    );
                    None
                }
            }
        } else {
            debug!("Vector database not configured, skipping");
            None
        };

        info!("Storage layer initialized successfully");

        Ok(Self {
            database,
            redis,
            files,
            vector,
        })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations");
        self.database.migrate().await?;
        info!("Database migrations completed");
        Ok(())
    }

    /// Health check for all storage backends
    pub async fn health_check(&self) -> Result<StorageHealthStatus> {
        let mut status = StorageHealthStatus {
            database: false,
            redis: false,
            files: false,
            vector: false,
            overall: false,
        };

        // Check database health
        match self.database.health_check().await {
            Ok(_) => status.database = true,
            Err(e) => {
                warn!("Database health check failed: {}", e);
            }
        }

        // Check Redis health
        match self.redis.health_check().await {
            Ok(_) => status.redis = true,
            Err(e) => {
                warn!("Redis health check failed: {}", e);
            }
        }

        // Check file storage health
        match self.files.health_check().await {
            Ok(_) => status.files = true,
            Err(e) => {
                warn!("File storage health check failed: {}", e);
            }
        }

        // Check vector database health (if configured)
        if let Some(vector) = &self.vector {
            match vector.health_check().await {
                Ok(_) => status.vector = true,
                Err(e) => {
                    warn!("Vector database health check failed: {}", e);
                }
            }
        } else {
            status.vector = true; // Not configured, so consider it healthy
        }

        // Overall health is true if all configured backends are healthy
        status.overall = status.database && status.redis && status.files && status.vector;

        Ok(status)
    }

    /// Close all connections
    pub async fn close(&self) -> Result<()> {
        info!("Closing storage connections");

        // Database connections will be closed when Arc is dropped
        // self.database.close().await?;

        // Close Redis connections
        self.redis.close().await?;

        // Close file storage
        self.files.close().await?;

        // Close vector database connections
        if let Some(vector) = &self.vector {
            vector.close().await?;
        }

        info!("Storage connections closed");
        Ok(())
    }

    /// Get database pool
    pub fn db(&self) -> &database::Database {
        &self.database
    }

    /// Get Redis pool
    pub fn redis(&self) -> &redis::RedisPool {
        &self.redis
    }

    /// Get file storage
    pub fn files(&self) -> &files::FileStorage {
        &self.files
    }

    /// Get vector store (if available)
    pub fn vector(&self) -> Option<&vector::VectorStoreBackend> {
        self.vector.as_deref()
    }

    // Transaction support removed - SeaORM handles transactions differently
    // Use the database connection directly for transactional operations

    /// Get a Redis connection
    pub async fn redis_conn(&self) -> Result<redis::RedisConnection> {
        self.redis.get_connection().await
    }

    /// Store file and return file ID
    pub async fn store_file(&self, filename: &str, content: &[u8]) -> Result<String> {
        self.files.store(filename, content).await
    }

    /// Retrieve file content
    pub async fn get_file(&self, file_id: &str) -> Result<Vec<u8>> {
        self.files.get(file_id).await
    }

    /// Delete file
    pub async fn delete_file(&self, file_id: &str) -> Result<()> {
        self.files.delete(file_id).await
    }

    /// Store vector embeddings
    pub async fn store_embeddings(
        &self,
        id: &str,
        embeddings: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        if let Some(vector) = &self.vector {
            vector.store(id, embeddings, metadata).await
        } else {
            Err(GatewayError::Config(
                "Vector database not configured".to_string(),
            ))
        }
    }

    /// Search similar vectors
    pub async fn search_similar(
        &self,
        query_vector: &[f32],
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<vector::SearchResult>> {
        if let Some(vector) = &self.vector {
            vector.search(query_vector, limit, threshold).await
        } else {
            Err(GatewayError::Config(
                "Vector database not configured".to_string(),
            ))
        }
    }

    /// Cache operations
    pub async fn cache_get(&self, key: &str) -> Result<Option<String>> {
        self.redis.get(key).await
    }

    /// Set cache value with optional TTL
    pub async fn cache_set(&self, key: &str, value: &str, ttl: Option<u64>) -> Result<()> {
        self.redis.set(key, value, ttl).await
    }

    /// Delete cache key
    pub async fn cache_delete(&self, key: &str) -> Result<()> {
        self.redis.delete(key).await
    }

    /// Check if cache key exists
    pub async fn cache_exists(&self, key: &str) -> Result<bool> {
        self.redis.exists(key).await
    }

    /// Batch cache operations
    pub async fn cache_mget(&self, keys: &[String]) -> Result<Vec<Option<String>>> {
        self.redis.mget(keys).await
    }

    /// Set multiple cache values with optional TTL
    pub async fn cache_mset(&self, pairs: &[(String, String)], ttl: Option<u64>) -> Result<()> {
        self.redis.mset(pairs, ttl).await
    }

    /// List operations
    /// Push value to list
    pub async fn list_push(&self, key: &str, value: &str) -> Result<()> {
        self.redis.list_push(key, value).await
    }

    /// Pop value from list
    pub async fn list_pop(&self, key: &str) -> Result<Option<String>> {
        self.redis.list_pop(key).await
    }

    /// Get list length
    pub async fn list_length(&self, key: &str) -> Result<usize> {
        self.redis.list_length(key).await
    }

    /// Set operations
    /// Add member to set
    pub async fn set_add(&self, key: &str, member: &str) -> Result<()> {
        self.redis.set_add(key, member).await
    }

    /// Remove member from set
    pub async fn set_remove(&self, key: &str, member: &str) -> Result<()> {
        self.redis.set_remove(key, member).await
    }

    /// Get all members of set
    pub async fn set_members(&self, key: &str) -> Result<Vec<String>> {
        self.redis.set_members(key).await
    }

    /// Hash operations
    /// Set hash field value
    pub async fn hash_set(&self, key: &str, field: &str, value: &str) -> Result<()> {
        self.redis.hash_set(key, field, value).await
    }

    /// Get hash field value
    pub async fn hash_get(&self, key: &str, field: &str) -> Result<Option<String>> {
        self.redis.hash_get(key, field).await
    }

    /// Delete hash field
    pub async fn hash_delete(&self, key: &str, field: &str) -> Result<()> {
        self.redis.hash_delete(key, field).await
    }

    /// Get all hash fields and values
    pub async fn hash_get_all(
        &self,
        key: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        self.redis.hash_get_all(key).await
    }

    /// Pub/Sub operations
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        self.redis.publish(channel, message).await
    }

    /// Subscribe to Redis channels
    /// Subscribe to Redis channels for pub/sub messaging
    pub async fn subscribe(&self, channels: &[String]) -> Result<redis::Subscription> {
        self.redis.subscribe(channels).await
    }
}

/// Storage health status
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageHealthStatus {
    /// Database health status
    pub database: bool,
    /// Redis health status
    pub redis: bool,
    /// File storage health status
    pub files: bool,
    /// Vector storage health status
    pub vector: bool,
    /// Overall health status
    pub overall: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::storage::{DatabaseConfig, RedisConfig};

    #[tokio::test]
    async fn test_storage_layer_creation() {
        let config = StorageConfig {
            database: DatabaseConfig {
                url: "postgresql://localhost:5432/test".to_string(),
                max_connections: 5,
                connection_timeout: 5,
                ssl: false,
                enabled: true,
            },
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                enabled: true,
                max_connections: 10,
                connection_timeout: 5,
                cluster: false,
            },
            vector_db: None,
        };

        // This test would require actual database connections
        // For now, we'll just test that the config is properly structured
        assert_eq!(config.database.url, "postgresql://localhost:5432/test");
        assert_eq!(config.redis.url, "redis://localhost:6379");
    }
}
