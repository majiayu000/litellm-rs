//! FileStorage enum implementation with dispatch methods

use crate::config::models::file_storage::FileStorageConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use tracing::info;

use super::local::LocalStorage;
use super::s3::S3Storage;
use super::types::{FileMetadata, FileStorage};

impl FileStorage {
    /// Create a new file storage instance
    pub async fn new(config: &FileStorageConfig) -> Result<Self> {
        info!("Initializing file storage: {}", config.storage_type);

        match config.storage_type.as_str() {
            "local" => {
                let default_path;
                let path = match config.local_path.as_ref() {
                    Some(p) => p.as_str(),
                    None => {
                        default_path = super::default_data_path();
                        default_path.to_str().unwrap_or("/tmp/litellm-rs/data")
                    }
                };
                Ok(FileStorage::Local(LocalStorage::new(path).await?))
            }
            "s3" => {
                let s3_config = config.s3.as_ref().ok_or_else(|| {
                    GatewayError::Config("S3 configuration not specified".to_string())
                })?;
                Ok(FileStorage::S3(S3Storage::new(s3_config).await?))
            }
            _ => Err(GatewayError::Config(format!(
                "Unsupported storage type: {}",
                config.storage_type
            ))),
        }
    }

    /// Store a file and return its ID
    pub async fn store(&self, filename: &str, content: &[u8]) -> Result<String> {
        match self {
            FileStorage::Local(storage) => storage.store(filename, content).await,
            FileStorage::S3(storage) => storage.store(filename, content).await,
        }
    }

    /// Retrieve file content by ID
    pub async fn get(&self, file_id: &str) -> Result<Vec<u8>> {
        match self {
            FileStorage::Local(storage) => storage.get(file_id).await,
            FileStorage::S3(storage) => storage.get(file_id).await,
        }
    }

    /// Delete a file by ID
    pub async fn delete(&self, file_id: &str) -> Result<()> {
        match self {
            FileStorage::Local(storage) => storage.delete(file_id).await,
            FileStorage::S3(storage) => storage.delete(file_id).await,
        }
    }

    /// Check if a file exists
    pub async fn exists(&self, file_id: &str) -> Result<bool> {
        match self {
            FileStorage::Local(storage) => storage.exists(file_id).await,
            FileStorage::S3(storage) => storage.exists(file_id).await,
        }
    }

    /// Get file metadata
    pub async fn metadata(&self, file_id: &str) -> Result<FileMetadata> {
        match self {
            FileStorage::Local(storage) => storage.metadata(file_id).await,
            FileStorage::S3(storage) => storage.metadata(file_id).await,
        }
    }

    /// List files with pagination
    pub async fn list(&self, prefix: Option<&str>, limit: Option<usize>) -> Result<Vec<String>> {
        match self {
            FileStorage::Local(storage) => storage.list(prefix, limit).await,
            FileStorage::S3(storage) => storage.list(prefix, limit).await,
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        match self {
            FileStorage::Local(storage) => storage.health_check().await,
            FileStorage::S3(storage) => storage.health_check().await,
        }
    }

    /// Close storage connections
    pub async fn close(&self) -> Result<()> {
        match self {
            FileStorage::Local(storage) => storage.close().await,
            FileStorage::S3(storage) => storage.close().await,
        }
    }
}
