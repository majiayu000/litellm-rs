//! Amazon S3 storage implementation

use crate::config::models::file_storage::S3Config;
use crate::utils::error::gateway_error::{GatewayError, Result};
#[cfg(feature = "s3")]
use tracing::debug;
use tracing::info;
#[cfg(feature = "s3")]
use uuid::Uuid;

#[cfg(feature = "s3")]
use aws_config;
#[cfg(feature = "s3")]
use aws_sdk_s3 as aws_s3;

use super::types::FileMetadata;

/// S3 file storage
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "s3"), allow(dead_code))]
pub struct S3Storage {
    bucket: String,
    _region: String,
    #[cfg(feature = "s3")]
    client: Option<aws_s3::Client>,
    #[cfg(not(feature = "s3"))]
    client: Option<()>, // Placeholder when S3 feature is disabled
}

impl S3Storage {
    /// Create a new S3 storage instance
    ///
    /// Credential resolution order:
    /// - If `access_key_id` and `secret_access_key` are both non-empty,
    ///   use them as static credentials.
    /// - Otherwise, fall back to the AWS default provider chain
    ///   (env vars, shared config, IMDS, container credentials).
    ///
    /// `endpoint` may be set to point at S3-compatible services
    /// (MinIO, Tigris, Cloudflare R2). When set, path-style addressing
    /// is forced.
    pub async fn new(config: &S3Config) -> Result<Self> {
        info!(
            "S3 file storage initialized: bucket={}, region={}",
            config.bucket, config.region
        );

        #[cfg(feature = "s3")]
        {
            use aws_s3::config::{Credentials, Region};

            let use_static_creds =
                !config.access_key_id.is_empty() && !config.secret_access_key.is_empty();
            info!(
                "S3 client config: endpoint={:?}, credentials={}",
                config.endpoint,
                if use_static_creds {
                    "static"
                } else {
                    "default-chain"
                }
            );

            let region = Region::new(config.region.clone());
            let mut loader =
                aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region);

            if use_static_creds {
                let creds = Credentials::new(
                    &config.access_key_id,
                    &config.secret_access_key,
                    None,
                    None,
                    "litellm-rs-s3-config",
                );
                loader = loader.credentials_provider(creds);
            }

            let aws_config = loader.load().await;

            let mut s3_builder = aws_s3::config::Builder::from(&aws_config);
            if let Some(endpoint) = &config.endpoint {
                s3_builder = s3_builder.endpoint_url(endpoint).force_path_style(true);
            }
            let client = aws_s3::Client::from_conf(s3_builder.build());

            Ok(Self {
                bucket: config.bucket.clone(),
                _region: config.region.clone(),
                client: Some(client),
            })
        }

        #[cfg(not(feature = "s3"))]
        {
            Ok(Self {
                bucket: config.bucket.clone(),
                _region: config.region.clone(),
                client: None,
            })
        }
    }

    /// Store a file to S3
    #[allow(unused_variables)]
    pub async fn store(&self, filename: &str, content: &[u8]) -> Result<String> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                use aws_s3::primitives::ByteStream;

                let file_id = Self::file_id_for_object(&Uuid::new_v4().to_string(), filename);

                client
                    .put_object()
                    .bucket(&self.bucket)
                    .key(&file_id)
                    .body(ByteStream::from(content.to_vec()))
                    .send()
                    .await
                    .map_err(|e| GatewayError::Internal(format!("S3 upload failed: {}", e)))?;

                debug!("File uploaded to S3: {}", file_id);
                Ok(file_id)
            } else {
                Err(GatewayError::Internal(
                    "S3 client not initialized".to_string(),
                ))
            }
        }

        #[cfg(not(feature = "s3"))]
        {
            Err(GatewayError::Internal("S3 feature not enabled".to_string()))
        }
    }

    /// Retrieve file content from S3
    #[allow(unused_variables)]
    pub async fn get(&self, file_id: &str) -> Result<Vec<u8>> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                let result = client
                    .get_object()
                    .bucket(&self.bucket)
                    .key(file_id)
                    .send()
                    .await
                    .map_err(|e| GatewayError::Internal(format!("S3 download failed: {}", e)))?;

                let bytes = result.body.collect().await.map_err(|e| {
                    GatewayError::Internal(format!("Failed to read S3 content: {}", e))
                })?;

                Ok(bytes.to_vec())
            } else {
                Err(GatewayError::Internal(
                    "S3 client not initialized".to_string(),
                ))
            }
        }

        #[cfg(not(feature = "s3"))]
        {
            Err(GatewayError::Internal("S3 feature not enabled".to_string()))
        }
    }

    /// Delete a file from S3
    #[allow(unused_variables)]
    pub async fn delete(&self, file_id: &str) -> Result<()> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                client
                    .delete_object()
                    .bucket(&self.bucket)
                    .key(file_id)
                    .send()
                    .await
                    .map_err(|e| GatewayError::Internal(format!("S3 deletion failed: {}", e)))?;

                debug!("File deleted from S3: {}", file_id);
                Ok(())
            } else {
                Err(GatewayError::Internal(
                    "S3 client not initialized".to_string(),
                ))
            }
        }

        #[cfg(not(feature = "s3"))]
        {
            Err(GatewayError::Internal("S3 feature not enabled".to_string()))
        }
    }

    /// Check if file exists in S3
    #[allow(unused_variables)]
    pub async fn exists(&self, file_id: &str) -> Result<bool> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                match client
                    .head_object()
                    .bucket(&self.bucket)
                    .key(file_id)
                    .send()
                    .await
                {
                    Ok(_) => Ok(true),
                    Err(e) => {
                        let service_err = e.into_service_error();
                        if service_err.is_not_found() {
                            Ok(false)
                        } else {
                            Err(GatewayError::Internal(format!(
                                "S3 exists check failed: {}",
                                service_err
                            )))
                        }
                    }
                }
            } else {
                Err(GatewayError::Internal(
                    "S3 client not initialized".to_string(),
                ))
            }
        }

        #[cfg(not(feature = "s3"))]
        {
            Err(GatewayError::Internal("S3 feature not enabled".to_string()))
        }
    }

    /// Get file metadata from S3
    #[allow(unused_variables)]
    pub async fn metadata(&self, file_id: &str) -> Result<FileMetadata> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                let head = client
                    .head_object()
                    .bucket(&self.bucket)
                    .key(file_id)
                    .send()
                    .await
                    .map_err(|e| {
                        GatewayError::Internal(format!("S3 metadata fetch failed: {}", e))
                    })?;

                let content_type = head
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let size = head.content_length().unwrap_or(0) as u64;
                let created_at = head
                    .last_modified()
                    .and_then(|t| chrono::DateTime::from_timestamp(t.secs(), t.subsec_nanos()))
                    .unwrap_or_else(chrono::Utc::now);

                let filename = Self::filename_from_file_id(file_id);

                let checksum = head.e_tag().unwrap_or("").trim_matches('"').to_string();

                Ok(FileMetadata {
                    id: file_id.to_string(),
                    filename,
                    content_type,
                    size,
                    created_at,
                    checksum,
                })
            } else {
                Err(GatewayError::Internal(
                    "S3 client not initialized".to_string(),
                ))
            }
        }

        #[cfg(not(feature = "s3"))]
        {
            Err(GatewayError::Internal("S3 feature not enabled".to_string()))
        }
    }

    /// List files in S3 with optional prefix and limit
    #[allow(unused_variables)]
    pub async fn list(&self, prefix: Option<&str>, limit: Option<usize>) -> Result<Vec<String>> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                let mut request = client.list_objects_v2().bucket(&self.bucket);

                if let Some(prefix) = prefix {
                    request = request.prefix(prefix);
                }

                if let Some(limit) = limit {
                    request = request.max_keys(limit as i32);
                }

                let result = request
                    .send()
                    .await
                    .map_err(|e| GatewayError::Internal(format!("S3 list failed: {}", e)))?;

                let keys = result
                    .contents()
                    .iter()
                    .filter_map(|obj| obj.key().map(|k| k.to_string()))
                    .collect();

                Ok(keys)
            } else {
                Err(GatewayError::Internal(
                    "S3 client not initialized".to_string(),
                ))
            }
        }

        #[cfg(not(feature = "s3"))]
        {
            Err(GatewayError::Internal("S3 feature not enabled".to_string()))
        }
    }

    /// Health check via head_bucket
    pub async fn health_check(&self) -> Result<()> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                client
                    .head_bucket()
                    .bucket(&self.bucket)
                    .send()
                    .await
                    .map_err(|e| {
                        GatewayError::Internal(format!("S3 health check failed: {}", e))
                    })?;

                Ok(())
            } else {
                Err(GatewayError::Internal(
                    "S3 client not initialized".to_string(),
                ))
            }
        }

        #[cfg(not(feature = "s3"))]
        {
            Err(GatewayError::Internal("S3 feature not enabled".to_string()))
        }
    }

    /// Close storage (placeholder implementation)
    pub async fn close(&self) -> Result<()> {
        Ok(())
    }

    #[cfg_attr(not(feature = "s3"), allow(dead_code))]
    fn file_id_for_object(object_id: &str, filename: &str) -> String {
        format!("{}/{}", object_id, filename)
    }

    #[cfg_attr(not(feature = "s3"), allow(dead_code))]
    fn filename_from_file_id(file_id: &str) -> String {
        file_id.rsplit('/').next().unwrap_or(file_id).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::S3Storage;

    #[test]
    fn store_identifier_matches_s3_object_key() {
        let object_id = "550e8400-e29b-41d4-a716-446655440000";
        let file_id = S3Storage::file_id_for_object(object_id, "batch.jsonl");

        assert_eq!(file_id, "550e8400-e29b-41d4-a716-446655440000/batch.jsonl");
    }

    #[test]
    fn metadata_filename_is_derived_from_returned_s3_file_id() {
        let file_id = "550e8400-e29b-41d4-a716-446655440000/nested/batch.jsonl";

        assert_eq!(S3Storage::filename_from_file_id(file_id), "batch.jsonl");
    }
}
