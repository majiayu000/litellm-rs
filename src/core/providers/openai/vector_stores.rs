//! OpenAI Vector Stores Module
//!
//! Vector stores functionality for Assistants API following the unified architecture

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::providers::unified_provider::ProviderError;

/// OpenAI Vector Store creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIVectorStoreRequest {
    /// The file IDs to be used for the vector store
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_ids: Option<Vec<String>>,

    /// The name of the vector store
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The expiration policy for the vector store
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<ExpirationPolicy>,

    /// The chunking strategy to use for the data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<ChunkingStrategy>,

    /// Set of key-value pairs for metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Vector Store expiration policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpirationPolicy {
    /// The anchor timestamp after which the expiration policy applies
    pub anchor: ExpirationAnchor,
    /// The number of days after the anchor time that the vector store will expire
    pub days: u32,
}

/// Expiration anchor options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpirationAnchor {
    LastActiveAt,
}

/// Chunking strategy for processing files
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChunkingStrategy {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "static")]
    Static {
        /// The maximum number of tokens in each chunk
        max_chunk_size_tokens: u32,
        /// The number of tokens that overlap between chunks
        chunk_overlap_tokens: u32,
    },
}

/// OpenAI Vector Store response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIVectorStore {
    /// The identifier of the vector store
    pub id: String,

    /// The object type (always "vector_store")
    pub object: String,

    /// The Unix timestamp for when the vector store was created
    pub created_at: i64,

    /// The name of the vector store
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The total number of bytes used by the files in the vector store
    pub usage_bytes: u64,

    /// The status of the vector store
    pub status: VectorStoreStatus,

    /// Files count by status
    pub file_counts: FileCounts,

    /// The last time the vector store was active
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<i64>,

    /// The expiration policy for the vector store
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<ExpirationPolicy>,

    /// The Unix timestamp for when the vector store will expire
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,

    /// Set of key-value pairs for metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Vector store status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreStatus {
    Expired,
    InProgress,
    Completed,
}

/// File counts by status
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileCounts {
    /// The number of files that are currently being processed
    pub in_progress: u32,
    /// The number of files that have been successfully processed
    pub completed: u32,
    /// The number of files that have failed to process
    pub failed: u32,
    /// The number of files that have been cancelled
    pub cancelled: u32,
    /// The total number of files
    pub total: u32,
}

/// Vector Store File request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreFileRequest {
    /// The file ID to add to the vector store
    pub file_id: String,

    /// The chunking strategy to use for this file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<ChunkingStrategy>,
}

/// Vector Store File response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreFile {
    /// The identifier of the vector store file
    pub id: String,

    /// The object type (always "vector_store.file")
    pub object: String,

    /// The total vector store usage in bytes
    pub usage_bytes: u64,

    /// The Unix timestamp for when the vector store file was created
    pub created_at: i64,

    /// The ID of the vector store that the file is attached to
    pub vector_store_id: String,

    /// The status of the vector store file
    pub status: VectorStoreFileStatus,

    /// The last error associated with this vector store file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<VectorStoreFileError>,

    /// The chunking strategy used to chunk the file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<ChunkingStrategy>,
}

/// Vector store file status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreFileStatus {
    InProgress,
    Completed,
    Cancelled,
    Failed,
}

/// Vector store file error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreFileError {
    /// The error code
    pub code: String,
    /// The error message
    pub message: String,
}

/// Vector Store File Batch request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreFileBatchRequest {
    /// A list of file IDs to add to the vector store
    pub file_ids: Vec<String>,

    /// The chunking strategy to use for the files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<ChunkingStrategy>,
}

/// Vector Store File Batch response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreFileBatch {
    /// The identifier of the vector store file batch
    pub id: String,

    /// The object type (always "vector_store.files_batch")
    pub object: String,

    /// The Unix timestamp for when the vector store file batch was created
    pub created_at: i64,

    /// The ID of the vector store that the file batch is attached to
    pub vector_store_id: String,

    /// The status of the vector store file batch
    pub status: VectorStoreFileBatchStatus,

    /// The file counts for this batch
    pub file_counts: FileCounts,
}

/// Vector store file batch status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreFileBatchStatus {
    InProgress,
    Completed,
    Cancelled,
    Failed,
}

/// Vector stores utilities
pub struct OpenAIVectorStoreUtils;

impl OpenAIVectorStoreUtils {
    /// Create vector store request
    pub fn create_vector_store_request(
        name: Option<String>,
        file_ids: Option<Vec<String>>,
        expires_after_days: Option<u32>,
    ) -> OpenAIVectorStoreRequest {
        OpenAIVectorStoreRequest {
            file_ids,
            name,
            expires_after: expires_after_days.map(|days| ExpirationPolicy {
                anchor: ExpirationAnchor::LastActiveAt,
                days,
            }),
            chunking_strategy: Some(ChunkingStrategy::Auto),
            metadata: None,
        }
    }

    /// Create file batch request
    pub fn create_file_batch_request(
        file_ids: Vec<String>,
        chunking_strategy: Option<ChunkingStrategy>,
    ) -> VectorStoreFileBatchRequest {
        VectorStoreFileBatchRequest {
            file_ids,
            chunking_strategy: Some(chunking_strategy.unwrap_or(ChunkingStrategy::Auto)),
        }
    }

    /// Validate vector store request
    pub fn validate_request(request: &OpenAIVectorStoreRequest) -> Result<(), ProviderError> {
        // Check file IDs
        if let Some(file_ids) = &request.file_ids {
            if file_ids.is_empty() {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: "file_ids cannot be empty when provided".to_string(),
                });
            }

            if file_ids.len() > 10000 {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: "Cannot attach more than 10,000 files to a vector store".to_string(),
                });
            }

            // Check for duplicates
            let mut unique_ids = std::collections::HashSet::new();
            for file_id in file_ids {
                if !unique_ids.insert(file_id) {
                    return Err(ProviderError::InvalidRequest {
                        provider: "openai",
                        message: format!(
                            "Duplicate file IDs are not allowed. Duplicate file ID: {}",
                            file_id
                        ),
                    });
                }
            }
        }

        // Check name length
        if let Some(name) = &request.name
            && name.len() > 256
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Vector store name must be 256 characters or less".to_string(),
            });
        }

        // Check expiration policy
        if let Some(expires_after) = &request.expires_after
            && (expires_after.days == 0 || expires_after.days > 365)
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Expiration days must be between 1 and 365".to_string(),
            });
        }

        // Validate chunking strategy
        if let Some(ChunkingStrategy::Static {
            max_chunk_size_tokens,
            chunk_overlap_tokens,
        }) = &request.chunking_strategy
        {
            if *max_chunk_size_tokens == 0 || *max_chunk_size_tokens > 4096 {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: "max_chunk_size_tokens must be between 1 and 4096".to_string(),
                });
            }

            if *chunk_overlap_tokens >= *max_chunk_size_tokens {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: "chunk_overlap_tokens must be less than max_chunk_size_tokens"
                        .to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate file batch request
    pub fn validate_file_batch_request(
        request: &VectorStoreFileBatchRequest,
    ) -> Result<(), ProviderError> {
        if request.file_ids.is_empty() {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "file_ids cannot be empty".to_string(),
            });
        }

        if request.file_ids.len() > 500 {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Cannot batch more than 500 files at once".to_string(),
            });
        }

        // Check for duplicates
        let mut unique_ids = std::collections::HashSet::new();
        for file_id in &request.file_ids {
            if !unique_ids.insert(file_id) {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: format!(
                        "Duplicate file IDs are not allowed. Duplicate file ID: {}",
                        file_id
                    ),
                });
            }
        }

        Ok(())
    }

    /// Get recommended chunking strategy for file type
    pub fn get_recommended_chunking_strategy(file_extension: &str) -> ChunkingStrategy {
        match file_extension.to_lowercase().as_str() {
            "pdf" | "docx" | "doc" => ChunkingStrategy::Static {
                max_chunk_size_tokens: 800,
                chunk_overlap_tokens: 400,
            },
            "txt" | "md" => ChunkingStrategy::Static {
                max_chunk_size_tokens: 512,
                chunk_overlap_tokens: 256,
            },
            "json" | "jsonl" => ChunkingStrategy::Static {
                max_chunk_size_tokens: 1024,
                chunk_overlap_tokens: 100,
            },
            _ => ChunkingStrategy::Auto,
        }
    }

    /// Calculate estimated storage cost
    pub fn estimate_storage_cost(file_size_bytes: u64) -> f64 {
        // OpenAI charges $0.10 per GB per day for vector storage
        let gb_size = file_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        gb_size * 0.10
    }

    /// Check if vector store is ready
    pub fn is_vector_store_ready(vector_store: &OpenAIVectorStore) -> bool {
        matches!(vector_store.status, VectorStoreStatus::Completed)
            && vector_store.file_counts.in_progress == 0
            && vector_store.file_counts.failed == 0
    }

    /// Get vector store health status
    pub fn get_health_status(vector_store: &OpenAIVectorStore) -> VectorStoreHealth {
        match vector_store.status {
            VectorStoreStatus::Completed => {
                if vector_store.file_counts.failed > 0 {
                    VectorStoreHealth::PartiallyHealthy
                } else {
                    VectorStoreHealth::Healthy
                }
            }
            VectorStoreStatus::InProgress => VectorStoreHealth::Processing,
            VectorStoreStatus::Expired => VectorStoreHealth::Expired,
        }
    }
}

/// Vector store health status
#[derive(Debug, Clone, PartialEq)]
pub enum VectorStoreHealth {
    Healthy,
    PartiallyHealthy,
    Processing,
    Expired,
}

/// Default implementations
impl Default for ChunkingStrategy {
    fn default() -> Self {
        ChunkingStrategy::Auto
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_vector_store_request() {
        let request = OpenAIVectorStoreUtils::create_vector_store_request(
            Some("Test Store".to_string()),
            Some(vec!["file-123".to_string(), "file-456".to_string()]),
            Some(30),
        );

        assert_eq!(request.name, Some("Test Store".to_string()));
        assert_eq!(
            request.file_ids,
            Some(vec!["file-123".to_string(), "file-456".to_string()])
        );
        assert!(matches!(
            request.expires_after,
            Some(ExpirationPolicy { days: 30, .. })
        ));
    }

    #[test]
    fn test_validate_request() {
        let valid_request = OpenAIVectorStoreUtils::create_vector_store_request(
            Some("Valid Store".to_string()),
            Some(vec!["file-123".to_string()]),
            Some(7),
        );
        assert!(OpenAIVectorStoreUtils::validate_request(&valid_request).is_ok());

        // Test empty file_ids
        let mut empty_files = valid_request.clone();
        empty_files.file_ids = Some(vec![]);
        assert!(OpenAIVectorStoreUtils::validate_request(&empty_files).is_err());

        // Test long name
        let mut long_name = valid_request.clone();
        long_name.name = Some("a".repeat(300));
        assert!(OpenAIVectorStoreUtils::validate_request(&long_name).is_err());

        // Test invalid expiration
        let mut invalid_expiration = valid_request.clone();
        invalid_expiration.expires_after = Some(ExpirationPolicy {
            anchor: ExpirationAnchor::LastActiveAt,
            days: 0,
        });
        assert!(OpenAIVectorStoreUtils::validate_request(&invalid_expiration).is_err());
    }

    #[test]
    fn test_validate_file_batch_request() {
        let valid_request = OpenAIVectorStoreUtils::create_file_batch_request(
            vec!["file-123".to_string(), "file-456".to_string()],
            None,
        );
        assert!(OpenAIVectorStoreUtils::validate_file_batch_request(&valid_request).is_ok());

        // Test empty file_ids
        let empty_request = VectorStoreFileBatchRequest {
            file_ids: vec![],
            chunking_strategy: None,
        };
        assert!(OpenAIVectorStoreUtils::validate_file_batch_request(&empty_request).is_err());

        // Test duplicate file_ids
        let duplicate_request = VectorStoreFileBatchRequest {
            file_ids: vec!["file-123".to_string(), "file-123".to_string()],
            chunking_strategy: None,
        };
        assert!(OpenAIVectorStoreUtils::validate_file_batch_request(&duplicate_request).is_err());
    }

    #[test]
    fn test_get_recommended_chunking_strategy() {
        let pdf_strategy = OpenAIVectorStoreUtils::get_recommended_chunking_strategy("pdf");
        if let ChunkingStrategy::Static {
            max_chunk_size_tokens,
            ..
        } = pdf_strategy
        {
            assert_eq!(max_chunk_size_tokens, 800);
        } else {
            panic!("Expected Static chunking strategy for PDF");
        }

        let unknown_strategy = OpenAIVectorStoreUtils::get_recommended_chunking_strategy("unknown");
        assert!(matches!(unknown_strategy, ChunkingStrategy::Auto));
    }

    #[test]
    fn test_estimate_storage_cost() {
        let cost = OpenAIVectorStoreUtils::estimate_storage_cost(1024 * 1024 * 1024); // 1GB
        assert_eq!(cost, 0.10);

        let cost_half_gb = OpenAIVectorStoreUtils::estimate_storage_cost(512 * 1024 * 1024); // 0.5GB
        assert_eq!(cost_half_gb, 0.05);
    }

    #[test]
    fn test_is_vector_store_ready() {
        let ready_store = OpenAIVectorStore {
            id: "vs-123".to_string(),
            object: "vector_store".to_string(),
            created_at: 1234567890,
            name: Some("Test Store".to_string()),
            usage_bytes: 1024,
            status: VectorStoreStatus::Completed,
            file_counts: FileCounts {
                in_progress: 0,
                completed: 5,
                failed: 0,
                cancelled: 0,
                total: 5,
            },
            last_active_at: Some(1234567890),
            expires_after: None,
            expires_at: None,
            metadata: None,
        };

        assert!(OpenAIVectorStoreUtils::is_vector_store_ready(&ready_store));

        let mut not_ready = ready_store.clone();
        not_ready.status = VectorStoreStatus::InProgress;
        assert!(!OpenAIVectorStoreUtils::is_vector_store_ready(&not_ready));
    }

    #[test]
    fn test_get_health_status() {
        let healthy_store = OpenAIVectorStore {
            id: "vs-123".to_string(),
            object: "vector_store".to_string(),
            created_at: 1234567890,
            name: None,
            usage_bytes: 1024,
            status: VectorStoreStatus::Completed,
            file_counts: FileCounts {
                in_progress: 0,
                completed: 5,
                failed: 0,
                cancelled: 0,
                total: 5,
            },
            last_active_at: None,
            expires_after: None,
            expires_at: None,
            metadata: None,
        };

        assert_eq!(
            OpenAIVectorStoreUtils::get_health_status(&healthy_store),
            VectorStoreHealth::Healthy
        );

        let mut partially_healthy = healthy_store.clone();
        partially_healthy.file_counts.failed = 1;
        assert_eq!(
            OpenAIVectorStoreUtils::get_health_status(&partially_healthy),
            VectorStoreHealth::PartiallyHealthy
        );

        let mut processing = healthy_store.clone();
        processing.status = VectorStoreStatus::InProgress;
        assert_eq!(
            OpenAIVectorStoreUtils::get_health_status(&processing),
            VectorStoreHealth::Processing
        );

        let mut expired = healthy_store.clone();
        expired.status = VectorStoreStatus::Expired;
        assert_eq!(
            OpenAIVectorStoreUtils::get_health_status(&expired),
            VectorStoreHealth::Expired
        );
    }
}
