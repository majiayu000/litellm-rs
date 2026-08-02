//! Amazon S3 storage implementation

use crate::config::models::file_storage::S3Config;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashSet;
#[cfg(feature = "s3")]
use tracing::debug;
use tracing::info;
use uuid::Uuid;

#[cfg(feature = "s3")]
use aws_config;
#[cfg(feature = "s3")]
use aws_sdk_s3 as aws_s3;

use super::types::{FileMetadata, FileOwnerScope, StoredFileMetadata};

const OWNER_METADATA_KEY: &str = "litellm-owner";

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
    #[cfg(all(test, feature = "s3"))]
    pub(super) fn test_endpoint(endpoint: String) -> Self {
        use aws_s3::config::{Credentials, Region};
        let config = aws_s3::Config::builder()
            .behavior_version_latest()
            .region(Region::new("us-east-1"))
            .credentials_provider(Credentials::new("test", "test", None, None, "fixture"))
            .retry_config(aws_config::retry::RetryConfig::standard().with_max_attempts(1))
            .endpoint_url(endpoint)
            .force_path_style(true)
            .build();
        Self {
            bucket: "test-bucket".into(),
            _region: "us-east-1".into(),
            client: Some(aws_s3::Client::from_conf(config)),
        }
    }

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
        self.store_with_purpose(filename, content, None).await
    }

    /// Store a file to S3 with optional OpenAI purpose metadata.
    #[allow(unused_variables)]
    pub async fn store_with_purpose(
        &self,
        filename: &str,
        content: &[u8],
        purpose: Option<&str>,
    ) -> Result<String> {
        self.store_envelope(filename, content, purpose, None).await
    }

    /// Store a file with a trusted, non-optional owner.
    #[allow(unused_variables)]
    pub(crate) async fn store_owned_with_purpose(
        &self,
        filename: &str,
        content: &[u8],
        purpose: Option<&str>,
        owner: FileOwnerScope,
    ) -> Result<String> {
        self.store_envelope(filename, content, purpose, Some(owner))
            .await
    }

    #[allow(unused_variables)]
    async fn store_envelope(
        &self,
        filename: &str,
        content: &[u8],
        purpose: Option<&str>,
        owner: Option<FileOwnerScope>,
    ) -> Result<String> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                use aws_s3::primitives::ByteStream;

                let file_id = Self::file_id_for_object(&Uuid::new_v4().to_string(), filename);
                let mut request = client
                    .put_object()
                    .bucket(&self.bucket)
                    .key(&file_id)
                    .content_type(Self::detect_content_type(filename))
                    .metadata("filename", filename)
                    .body(ByteStream::from(content.to_vec()));

                if let Some(purpose) = Self::normalize_purpose(purpose) {
                    request = request.metadata("purpose", purpose);
                }
                if let Some(owner) = owner.as_ref() {
                    request = request.metadata(OWNER_METADATA_KEY, Self::encode_owner(owner)?);
                }

                request
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
                        if Self::head_error_is_not_found(&e) {
                            Ok(false)
                        } else {
                            Err(GatewayError::Internal(format!(
                                "S3 exists check failed: {}",
                                e
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
        Ok(self.metadata_with_owner(file_id).await?.public)
    }

    /// Get public metadata and the strict internal owner envelope.
    #[allow(unused_variables)]
    pub(crate) async fn metadata_with_owner(&self, file_id: &str) -> Result<StoredFileMetadata> {
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                let head = client
                    .head_object()
                    .bucket(&self.bucket)
                    .key(file_id)
                    .send()
                    .await
                    .map_err(|error| {
                        if Self::head_error_is_not_found(&error) {
                            GatewayError::not_found("File not found")
                        } else {
                            GatewayError::internal(format!("S3 metadata fetch failed: {error}"))
                        }
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

                let filename = head
                    .metadata()
                    .and_then(|metadata| metadata.get("filename").cloned())
                    .unwrap_or_else(|| Self::filename_from_file_id(file_id));

                let checksum = head.e_tag().unwrap_or("").trim_matches('"').to_string();
                let purpose = head
                    .metadata()
                    .and_then(|metadata| metadata.get("purpose").cloned());

                let public = FileMetadata {
                    id: file_id.to_string(),
                    filename,
                    content_type,
                    size,
                    created_at,
                    purpose,
                    checksum,
                };
                let owner = Self::decode_owner(
                    head.metadata()
                        .and_then(|metadata| metadata.get(OWNER_METADATA_KEY))
                        .map(String::as_str),
                )?;
                Ok(match owner {
                    Some(owner) => StoredFileMetadata::owned(public, owner),
                    None => StoredFileMetadata::legacy(public),
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
        if limit == Some(0) {
            return Ok(Vec::new());
        }
        #[cfg(feature = "s3")]
        {
            if let Some(client) = &self.client {
                let mut keys = Vec::new();
                let mut continuation: Option<String> = None;
                let mut seen_tokens = HashSet::new();

                loop {
                    let remaining = limit.map(|limit| limit.saturating_sub(keys.len()));
                    if remaining == Some(0) {
                        break;
                    }
                    let page_size = remaining.unwrap_or(1000).min(1000) as i32;
                    let mut request = client
                        .list_objects_v2()
                        .bucket(&self.bucket)
                        .max_keys(page_size);
                    if let Some(prefix) = prefix {
                        request = request.prefix(prefix);
                    }
                    if let Some(token) = continuation.as_deref() {
                        request = request.continuation_token(token);
                    }

                    let page = request.send().await.map_err(|error| {
                        GatewayError::internal(format!("S3 list failed: {error}"))
                    })?;
                    for object in page.contents() {
                        if let Some(key) = object.key() {
                            keys.push(key.to_string());
                            if limit.is_some_and(|limit| keys.len() >= limit) {
                                break;
                            }
                        }
                    }

                    if !page.is_truncated().unwrap_or(false) {
                        break;
                    }
                    let next = Self::validate_next_token(
                        page.next_continuation_token(),
                        &mut seen_tokens,
                    )?;
                    continuation = Some(next);
                }

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
    fn file_id_for_object(object_id: &str, _filename: &str) -> String {
        object_id.to_string()
    }

    #[cfg_attr(not(feature = "s3"), allow(dead_code))]
    fn filename_from_file_id(file_id: &str) -> String {
        file_id.rsplit('/').next().unwrap_or(file_id).to_string()
    }

    #[cfg_attr(not(feature = "s3"), allow(dead_code))]
    fn detect_content_type(filename: &str) -> String {
        super::local::LocalStorage::detect_content_type(filename)
    }

    #[cfg_attr(not(feature = "s3"), allow(dead_code))]
    fn normalize_purpose(purpose: Option<&str>) -> Option<String> {
        purpose
            .map(str::trim)
            .filter(|purpose| !purpose.is_empty())
            .map(ToOwned::to_owned)
    }

    fn encode_owner(owner: &FileOwnerScope) -> Result<String> {
        let (scope, id) = match owner {
            FileOwnerScope::Team(id) => ("team", id),
            FileOwnerScope::User(id) => ("user", id),
            FileOwnerScope::ApiKey(id) => ("api_key", id),
        };
        serde_json::to_string(&serde_json::json!({
            "version": 1,
            "scope": scope,
            "id": id,
        }))
        .map_err(|error| GatewayError::internal(format!("Failed to encode file owner: {error}")))
    }

    fn decode_owner(raw: Option<&str>) -> Result<Option<FileOwnerScope>> {
        let Some(raw) = raw else {
            return Ok(None);
        };
        let value: serde_json::Value = serde_json::from_str(raw)
            .map_err(|_| GatewayError::internal("Invalid S3 file owner metadata"))?;
        let object = value
            .as_object()
            .ok_or_else(|| GatewayError::internal("Invalid S3 file owner metadata"))?;
        if object.len() != 3 || object.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        {
            return Err(GatewayError::internal("Invalid S3 file owner metadata"));
        }
        let scope = object
            .get("scope")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| GatewayError::internal("Invalid S3 file owner metadata"))?;
        let id = object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| GatewayError::internal("Invalid S3 file owner metadata"))?;
        match scope {
            "team" => Ok(Some(FileOwnerScope::Team(id))),
            "user" => Ok(Some(FileOwnerScope::User(id))),
            "api_key" => Ok(Some(FileOwnerScope::ApiKey(id))),
            _ => Err(GatewayError::internal("Invalid S3 file owner metadata")),
        }
    }

    #[cfg(feature = "s3")]
    fn head_error_is_not_found(
        error: &aws_s3::error::SdkError<aws_s3::operation::head_object::HeadObjectError>,
    ) -> bool {
        let modeled = error
            .as_service_error()
            .is_some_and(|service| service.is_not_found());
        let status = error
            .raw_response()
            .map(|response| response.status().as_u16());
        Self::is_canonical_not_found(modeled, status)
    }

    fn is_canonical_not_found(modeled: bool, raw_status: Option<u16>) -> bool {
        modeled || raw_status == Some(404)
    }

    fn validate_next_token(
        token: Option<&str>,
        seen_tokens: &mut HashSet<String>,
    ) -> Result<String> {
        let token = token.filter(|token| !token.is_empty()).ok_or_else(|| {
            GatewayError::internal("S3 truncated page omitted continuation token")
        })?;
        if !seen_tokens.insert(token.to_string()) {
            return Err(GatewayError::internal(
                "S3 continuation token repeated before listing completed",
            ));
        }
        Ok(token.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{FileOwnerScope, HashSet, S3Storage};
    use uuid::Uuid;

    #[test]
    fn store_identifier_is_route_safe() {
        let object_id = "550e8400-e29b-41d4-a716-446655440000";
        let file_id = S3Storage::file_id_for_object(object_id, "batch.jsonl");

        assert_eq!(file_id, "550e8400-e29b-41d4-a716-446655440000");
        assert!(!file_id.contains('/'));
    }

    #[test]
    fn metadata_filename_is_derived_from_returned_s3_file_id() {
        let file_id = "550e8400-e29b-41d4-a716-446655440000/nested/batch.jsonl";

        assert_eq!(S3Storage::filename_from_file_id(file_id), "batch.jsonl");
    }

    #[test]
    fn gh1130_s3_owner_metadata_is_strict_and_versioned() {
        for owner in [
            FileOwnerScope::Team(Uuid::new_v4()),
            FileOwnerScope::User(Uuid::new_v4()),
            FileOwnerScope::ApiKey(Uuid::new_v4()),
        ] {
            let encoded = S3Storage::encode_owner(&owner).unwrap();
            assert_eq!(
                S3Storage::decode_owner(Some(&encoded)).unwrap(),
                Some(owner)
            );
        }
        assert_eq!(S3Storage::decode_owner(None).unwrap(), None);
        for invalid in [
            "null",
            "{}",
            r#"{"version":2,"scope":"team","id":"00000000-0000-0000-0000-000000000000"}"#,
            r#"{"version":1,"scope":"team","id":null}"#,
            r#"{"version":1,"scope":"team","id":"00000000-0000-0000-0000-000000000000","extra":true}"#,
        ] {
            assert!(S3Storage::decode_owner(Some(invalid)).is_err());
        }
    }

    #[test]
    fn gh1130_head_only_maps_modeled_or_raw_404_to_not_found() {
        assert!(S3Storage::is_canonical_not_found(true, None));
        assert!(S3Storage::is_canonical_not_found(false, Some(404)));
        for status in [
            None,
            Some(400),
            Some(401),
            Some(403),
            Some(408),
            Some(429),
            Some(500),
        ] {
            assert!(!S3Storage::is_canonical_not_found(false, status));
        }
    }

    #[test]
    fn gh1130_pagination_rejects_missing_empty_and_repeated_tokens() {
        let mut seen = HashSet::new();
        assert!(S3Storage::validate_next_token(None, &mut seen).is_err());
        assert!(S3Storage::validate_next_token(Some(""), &mut seen).is_err());
        assert_eq!(
            S3Storage::validate_next_token(Some("next"), &mut seen).unwrap(),
            "next"
        );
        assert!(S3Storage::validate_next_token(Some("next"), &mut seen).is_err());
    }
}
