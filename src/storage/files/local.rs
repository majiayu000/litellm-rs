//! Local file system storage implementation

use crate::utils::error::gateway_error::{GatewayError, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info};
use uuid::Uuid;

use super::types::{FileMetadata, FileOwnerScope, StoredFileMetadata};

const STAGING_DIRECTORY: &str = ".staging";

/// Local file storage
#[derive(Debug, Clone)]
pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    /// Create a new local storage instance
    pub async fn new(base_path: &str) -> Result<Self> {
        let path = PathBuf::from(base_path);

        // Create directory if it doesn't exist
        if !path.exists() {
            fs::create_dir_all(&path).await.map_err(|e| {
                GatewayError::Internal(format!("Failed to create storage directory: {}", e))
            })?;
        }

        info!("Local file storage initialized at: {}", path.display());
        Ok(Self { base_path: path })
    }

    /// Store a file
    pub async fn store(&self, filename: &str, content: &[u8]) -> Result<String> {
        self.store_with_purpose(filename, content, None).await
    }

    /// Store a file with optional OpenAI purpose metadata.
    pub async fn store_with_purpose(
        &self,
        filename: &str,
        content: &[u8],
        purpose: Option<&str>,
    ) -> Result<String> {
        let file_id = Uuid::new_v4().to_string();
        let metadata =
            StoredFileMetadata::legacy(Self::build_metadata(&file_id, filename, content, purpose));
        self.store_envelope_with_id(&file_id, content, &metadata)
            .await
    }

    /// Store a file whose owner has already been resolved from trusted auth state.
    pub(crate) async fn store_owned_with_purpose(
        &self,
        filename: &str,
        content: &[u8],
        purpose: Option<&str>,
        owner: FileOwnerScope,
    ) -> Result<String> {
        let file_id = Uuid::new_v4().to_string();
        let metadata = StoredFileMetadata::owned(
            Self::build_metadata(&file_id, filename, content, purpose),
            owner,
        );
        self.store_envelope_with_id(&file_id, content, &metadata)
            .await
    }

    fn build_metadata(
        file_id: &str,
        filename: &str,
        content: &[u8],
        purpose: Option<&str>,
    ) -> FileMetadata {
        FileMetadata {
            id: file_id.to_string(),
            filename: filename.to_string(),
            content_type: Self::detect_content_type(filename),
            size: content.len() as u64,
            created_at: chrono::Utc::now(),
            purpose: Self::normalize_purpose(purpose),
            checksum: Self::calculate_checksum(content),
        }
    }

    /// Stage content and complete metadata outside the list-visible namespace.
    ///
    /// Metadata is published first; the final content rename is the only commit
    /// point visible to `list`, `get`, and `metadata_with_owner`.
    async fn store_envelope_with_id(
        &self,
        file_id: &str,
        content: &[u8],
        metadata: &StoredFileMetadata,
    ) -> Result<String> {
        Self::validate_file_id(file_id)?;
        let final_content = self.get_file_path(file_id);
        let final_metadata = self.get_metadata_path(file_id);
        let final_parent = final_content
            .parent()
            .ok_or_else(|| GatewayError::internal("File storage path has no parent directory"))?;
        fs::create_dir_all(final_parent)
            .await
            .map_err(|error| GatewayError::internal(format!("Failed to create shard: {error}")))?;

        let staging_dir = self.base_path.join(STAGING_DIRECTORY).join(file_id);
        fs::create_dir_all(&staging_dir).await.map_err(|error| {
            GatewayError::internal(format!("Failed to create file staging directory: {error}"))
        })?;
        let staged_content = staging_dir.join("content");
        let staged_metadata = staging_dir.join("metadata");

        let result = async {
            Self::write_complete(&staged_content, content).await?;
            let encoded = serde_json::to_vec_pretty(metadata).map_err(|error| {
                GatewayError::internal(format!("Failed to serialize file metadata: {error}"))
            })?;
            Self::write_complete(&staged_metadata, &encoded).await?;

            fs::rename(&staged_metadata, &final_metadata)
                .await
                .map_err(|error| {
                    GatewayError::internal(format!("Failed to publish file metadata: {error}"))
                })?;
            if let Err(error) = fs::rename(&staged_content, &final_content).await {
                if let Err(cleanup_error) = fs::remove_file(&final_metadata).await {
                    debug!(
                        "Failed to clean unpublished metadata sidecar: {}",
                        cleanup_error
                    );
                }
                return Err(GatewayError::internal(format!(
                    "Failed to publish file content: {error}"
                )));
            }
            Ok(())
        }
        .await;

        if let Err(cleanup_error) = fs::remove_dir_all(&staging_dir).await
            && cleanup_error.kind() != std::io::ErrorKind::NotFound
        {
            debug!("Failed to clean file staging directory: {}", cleanup_error);
        }
        result?;

        debug!("File stored: {} -> {}", metadata.public.filename, file_id);
        Ok(file_id.to_string())
    }

    async fn write_complete(path: &Path, content: &[u8]) -> Result<()> {
        let mut file = fs::File::create(path).await.map_err(|error| {
            GatewayError::internal(format!("Failed to create staged file: {error}"))
        })?;
        file.write_all(content).await.map_err(|error| {
            GatewayError::internal(format!("Failed to write staged file: {error}"))
        })?;
        file.flush().await.map_err(|error| {
            GatewayError::internal(format!("Failed to flush staged file: {error}"))
        })?;
        file.sync_all().await.map_err(|error| {
            GatewayError::internal(format!("Failed to sync staged file: {error}"))
        })?;
        Ok(())
    }

    /// Retrieve file content
    pub async fn get(&self, file_id: &str) -> Result<Vec<u8>> {
        Self::validate_file_id(file_id)?;
        self.metadata_with_owner(file_id).await?;
        let file_path = self.get_file_path(file_id);

        let mut file = fs::File::open(&file_path)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to open file: {}", e)))?;

        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to read file: {}", e)))?;

        Ok(content)
    }

    /// Delete a file
    pub async fn delete(&self, file_id: &str) -> Result<()> {
        Self::validate_file_id(file_id)?;
        let file_path = self.get_file_path(file_id);
        let metadata_path = self.get_metadata_path(file_id);

        // Delete file
        if file_path.exists() {
            fs::remove_file(&file_path)
                .await
                .map_err(|e| GatewayError::Internal(format!("Failed to delete file: {}", e)))?;
        }

        // Delete metadata
        if metadata_path.exists() {
            fs::remove_file(&metadata_path)
                .await
                .map_err(|e| GatewayError::Internal(format!("Failed to delete metadata: {}", e)))?;
        }

        debug!("File deleted: {}", file_id);
        Ok(())
    }

    /// Check if file exists
    pub async fn exists(&self, file_id: &str) -> Result<bool> {
        Self::validate_file_id(file_id)?;
        let file_path = self.get_file_path(file_id);
        if !file_path.exists() {
            return Ok(false);
        }
        self.metadata_with_owner(file_id).await?;
        Ok(true)
    }

    /// Get file metadata
    pub async fn metadata(&self, file_id: &str) -> Result<FileMetadata> {
        Ok(self.metadata_with_owner(file_id).await?.public)
    }

    /// Get complete internal metadata while preserving owner presence.
    pub(crate) async fn metadata_with_owner(&self, file_id: &str) -> Result<StoredFileMetadata> {
        Self::validate_file_id(file_id)?;
        let file_path = self.get_file_path(file_id);
        if !file_path.exists() {
            return Err(GatewayError::not_found("File not found"));
        }
        let metadata_path = self.get_metadata_path(file_id);

        if !metadata_path.exists() {
            return Err(GatewayError::internal(
                "Committed file is missing required metadata",
            ));
        }

        let content = fs::read_to_string(&metadata_path)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to read metadata: {}", e)))?;

        let metadata: StoredFileMetadata = serde_json::from_str(&content)
            .map_err(|e| GatewayError::Internal(format!("Failed to parse metadata: {}", e)))?;

        Ok(metadata)
    }

    /// List files
    pub async fn list(&self, prefix: Option<&str>, limit: Option<usize>) -> Result<Vec<String>> {
        if limit == Some(0) {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        let mut subdirs = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to read directory: {}", e)))?;

        while let Some(subdir) = subdirs
            .next_entry()
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to read entry: {}", e)))?
        {
            let subdir_name = subdir.file_name().to_string_lossy().to_string();
            if subdir_name.len() != 2 || !subdir_name.chars().all(|c| c.is_ascii_alphanumeric()) {
                continue;
            }

            let file_type = subdir
                .file_type()
                .await
                .map_err(|e| GatewayError::Internal(format!("Failed to read entry type: {}", e)))?;
            if !file_type.is_dir() {
                continue;
            }

            let mut entries = fs::read_dir(subdir.path())
                .await
                .map_err(|e| GatewayError::Internal(format!("Failed to read directory: {}", e)))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| GatewayError::Internal(format!("Failed to read entry: {}", e)))?
            {
                let file_type = entry.file_type().await.map_err(|e| {
                    GatewayError::Internal(format!("Failed to read entry type: {}", e))
                })?;
                if !file_type.is_file() {
                    continue;
                }

                let file_name = entry.file_name().to_string_lossy().to_string();

                // Skip metadata files
                if file_name.ends_with(".meta") {
                    continue;
                }

                // Apply prefix filter to the stored file id, not to shard paths.
                if let Some(prefix) = prefix
                    && !file_name.starts_with(prefix)
                {
                    continue;
                }

                files.push(file_name);

                // Apply limit
                if let Some(limit) = limit
                    && files.len() >= limit
                {
                    return Ok(files);
                }
            }
        }

        Ok(files)
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        // Check if base directory is accessible
        if !self.base_path.exists() {
            return Err(GatewayError::Internal(
                "Storage directory does not exist".to_string(),
            ));
        }

        // Try to write a test file
        let test_file = self.base_path.join(".health_check");
        fs::write(&test_file, b"health_check")
            .await
            .map_err(|e| GatewayError::Internal(format!("Storage not writable: {}", e)))?;

        // Clean up test file
        let _ = fs::remove_file(&test_file).await;

        Ok(())
    }

    /// Close storage (no-op for local storage)
    pub async fn close(&self) -> Result<()> {
        Ok(())
    }

    /// Validate a caller-supplied file_id before joining it onto base_path.
    ///
    /// `Path::join` does not normalize `..`, so without validation a value
    /// like `"../../etc/passwd"` would escape `base_path`. `store()` only
    /// ever generates `Uuid::new_v4()` ids, so we constrain accepted ids
    /// to the UUID alphabet (lowercase hex + hyphen) plus a length bound.
    fn validate_file_id(file_id: &str) -> Result<()> {
        if file_id.is_empty() || file_id.len() > 64 {
            return Err(GatewayError::Validation(format!(
                "Invalid file_id length ({}); must be 1..=64 chars",
                file_id.len()
            )));
        }
        if !file_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(GatewayError::Validation(format!(
                "Invalid file_id: {file_id:?}; only alphanumeric ASCII and '-' allowed"
            )));
        }
        Ok(())
    }

    /// Get file path for a given file ID
    fn get_file_path(&self, file_id: &str) -> PathBuf {
        // Use first two characters as subdirectory for better distribution
        let subdir = &file_id[..2.min(file_id.len())];
        self.base_path.join(subdir).join(file_id)
    }

    /// Get metadata path for a given file ID
    fn get_metadata_path(&self, file_id: &str) -> PathBuf {
        let subdir = &file_id[..2.min(file_id.len())];
        self.base_path
            .join(subdir)
            .join(format!("{}.meta", file_id))
    }

    /// Detect content type from filename
    pub(crate) fn detect_content_type(filename: &str) -> String {
        match Path::new(filename).extension().and_then(|ext| ext.to_str()) {
            Some("txt") => "text/plain".to_string(),
            Some("json") => "application/json".to_string(),
            Some("xml") => "application/xml".to_string(),
            Some("html") => "text/html".to_string(),
            Some("css") => "text/css".to_string(),
            Some("js") => "application/javascript".to_string(),
            Some("png") => "image/png".to_string(),
            Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
            Some("gif") => "image/gif".to_string(),
            Some("pdf") => "application/pdf".to_string(),
            Some("zip") => "application/zip".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }

    /// Calculate file checksum
    fn calculate_checksum(content: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    fn normalize_purpose(purpose: Option<&str>) -> Option<String> {
        purpose
            .map(str::trim)
            .filter(|purpose| !purpose.is_empty())
            .map(ToOwned::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== detect_content_type Tests ====================

    #[test]
    fn test_detect_content_type_txt() {
        assert_eq!(LocalStorage::detect_content_type("file.txt"), "text/plain");
    }

    #[test]
    fn test_detect_content_type_json() {
        assert_eq!(
            LocalStorage::detect_content_type("data.json"),
            "application/json"
        );
    }

    #[test]
    fn test_detect_content_type_xml() {
        assert_eq!(
            LocalStorage::detect_content_type("config.xml"),
            "application/xml"
        );
    }

    #[test]
    fn test_detect_content_type_html() {
        assert_eq!(LocalStorage::detect_content_type("index.html"), "text/html");
    }

    #[test]
    fn test_detect_content_type_css() {
        assert_eq!(LocalStorage::detect_content_type("styles.css"), "text/css");
    }

    #[test]
    fn test_detect_content_type_js() {
        assert_eq!(
            LocalStorage::detect_content_type("script.js"),
            "application/javascript"
        );
    }

    #[test]
    fn test_detect_content_type_png() {
        assert_eq!(LocalStorage::detect_content_type("image.png"), "image/png");
    }

    #[test]
    fn test_detect_content_type_jpg() {
        assert_eq!(LocalStorage::detect_content_type("photo.jpg"), "image/jpeg");
    }

    #[test]
    fn test_detect_content_type_jpeg() {
        assert_eq!(
            LocalStorage::detect_content_type("photo.jpeg"),
            "image/jpeg"
        );
    }

    #[test]
    fn test_detect_content_type_gif() {
        assert_eq!(LocalStorage::detect_content_type("anim.gif"), "image/gif");
    }

    #[test]
    fn test_detect_content_type_pdf() {
        assert_eq!(
            LocalStorage::detect_content_type("document.pdf"),
            "application/pdf"
        );
    }

    #[test]
    fn test_detect_content_type_zip() {
        assert_eq!(
            LocalStorage::detect_content_type("archive.zip"),
            "application/zip"
        );
    }

    #[test]
    fn test_detect_content_type_unknown() {
        assert_eq!(
            LocalStorage::detect_content_type("file.xyz"),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_detect_content_type_no_extension() {
        assert_eq!(
            LocalStorage::detect_content_type("README"),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_detect_content_type_multiple_dots() {
        assert_eq!(
            LocalStorage::detect_content_type("file.backup.json"),
            "application/json"
        );
    }

    #[test]
    fn test_detect_content_type_uppercase() {
        // Extensions are case-sensitive in this impl
        assert_eq!(
            LocalStorage::detect_content_type("file.TXT"),
            "application/octet-stream"
        );
    }

    // ==================== calculate_checksum Tests ====================

    #[test]
    fn test_calculate_checksum_basic() {
        let content = b"Hello, World!";
        let checksum = LocalStorage::calculate_checksum(content);
        // SHA256 produces 64 hex characters
        assert_eq!(checksum.len(), 64);
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_calculate_checksum_empty() {
        let content = b"";
        let checksum = LocalStorage::calculate_checksum(content);
        assert_eq!(checksum.len(), 64);
        // Known SHA256 hash for empty input
        assert_eq!(
            checksum,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_calculate_checksum_consistency() {
        let content = b"test content";
        let checksum1 = LocalStorage::calculate_checksum(content);
        let checksum2 = LocalStorage::calculate_checksum(content);
        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_calculate_checksum_different_content() {
        let content1 = b"content A";
        let content2 = b"content B";
        let checksum1 = LocalStorage::calculate_checksum(content1);
        let checksum2 = LocalStorage::calculate_checksum(content2);
        assert_ne!(checksum1, checksum2);
    }

    #[test]
    fn test_calculate_checksum_binary_data() {
        let content = &[0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
        let checksum = LocalStorage::calculate_checksum(content);
        assert_eq!(checksum.len(), 64);
    }

    #[test]
    fn test_calculate_checksum_large_content() {
        let content = vec![0u8; 1024 * 1024]; // 1MB
        let checksum = LocalStorage::calculate_checksum(&content);
        assert_eq!(checksum.len(), 64);
    }

    // ==================== get_file_path Tests ====================

    #[test]
    fn test_get_file_path_structure() {
        let storage = LocalStorage {
            base_path: PathBuf::from("/tmp/storage"),
        };
        let file_id = "ab12345";
        let path = storage.get_file_path(file_id);

        // Should use first 2 chars as subdir
        assert!(path.to_string_lossy().contains("/ab/"));
        assert!(path.to_string_lossy().ends_with("ab12345"));
    }

    #[test]
    fn test_get_file_path_short_id() {
        let storage = LocalStorage {
            base_path: PathBuf::from("/tmp/storage"),
        };
        let file_id = "a";
        let path = storage.get_file_path(file_id);

        // Should handle short IDs gracefully
        assert!(path.to_string_lossy().contains("/a/"));
    }

    // ==================== get_metadata_path Tests ====================

    #[test]
    fn test_get_metadata_path_structure() {
        let storage = LocalStorage {
            base_path: PathBuf::from("/tmp/storage"),
        };
        let file_id = "cd67890";
        let path = storage.get_metadata_path(file_id);

        assert!(path.to_string_lossy().contains("/cd/"));
        assert!(path.to_string_lossy().ends_with("cd67890.meta"));
    }

    // ==================== validate_file_id Tests ====================

    #[test]
    fn test_validate_file_id_accepts_uuid() {
        assert!(LocalStorage::validate_file_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(LocalStorage::validate_file_id("abc123").is_ok());
    }

    #[test]
    fn test_validate_file_id_rejects_traversal() {
        for bad in [
            "../../etc/passwd",
            "..",
            "../foo",
            "foo/../bar",
            "abc/def",
            "abc\\def",
            "abc\0def",
            "",
            "  ",
            "abc:def",
            "file with space",
            ".hidden",
        ] {
            assert!(
                LocalStorage::validate_file_id(bad).is_err(),
                "should reject {bad:?}"
            );
        }
    }

    #[test]
    fn test_validate_file_id_rejects_overlong() {
        let long = "a".repeat(65);
        assert!(LocalStorage::validate_file_id(&long).is_err());
    }

    #[tokio::test]
    async fn gh1130_owned_metadata_round_trips_without_changing_public_shape() {
        let temp = tempfile::TempDir::new().unwrap();
        let storage = LocalStorage::new(temp.path().to_str().unwrap())
            .await
            .unwrap();
        let owner = FileOwnerScope::Team(Uuid::new_v4());
        let file_id = storage
            .store_owned_with_purpose("batch.jsonl", b"{}\n", Some("batch"), owner.clone())
            .await
            .unwrap();

        let stored = storage.metadata_with_owner(&file_id).await.unwrap();
        assert_eq!(stored.owner(), Some(&owner));
        let public_json = serde_json::to_value(storage.metadata(&file_id).await.unwrap()).unwrap();
        assert!(public_json.get("owner").is_none());

        let reopened = LocalStorage::new(temp.path().to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(
            reopened
                .metadata_with_owner(&file_id)
                .await
                .unwrap()
                .owner(),
            Some(&owner)
        );
    }

    #[tokio::test]
    async fn gh1130_failed_staging_and_meta_only_records_are_not_visible() {
        let temp = tempfile::TempDir::new().unwrap();
        let storage = LocalStorage::new(temp.path().to_str().unwrap())
            .await
            .unwrap();
        let file_id = Uuid::new_v4().to_string();
        let metadata = StoredFileMetadata::owned(
            LocalStorage::build_metadata(&file_id, "batch.jsonl", b"{}\n", Some("batch")),
            FileOwnerScope::User(Uuid::new_v4()),
        );

        let staging_metadata = temp
            .path()
            .join(STAGING_DIRECTORY)
            .join(&file_id)
            .join("metadata");
        fs::create_dir_all(&staging_metadata).await.unwrap();
        assert!(
            storage
                .store_envelope_with_id(&file_id, b"{}\n", &metadata)
                .await
                .is_err()
        );
        assert!(!storage.get_file_path(&file_id).exists());
        assert!(!storage.get_metadata_path(&file_id).exists());

        let meta_only_id = Uuid::new_v4().to_string();
        let meta_path = storage.get_metadata_path(&meta_only_id);
        fs::create_dir_all(meta_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&meta_path, serde_json::to_vec(&metadata).unwrap())
            .await
            .unwrap();

        let reopened = LocalStorage::new(temp.path().to_str().unwrap())
            .await
            .unwrap();
        assert!(reopened.list(None, None).await.unwrap().is_empty());
        assert!(matches!(
            reopened.metadata_with_owner(&meta_only_id).await,
            Err(GatewayError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn gh1130_content_without_metadata_is_explicit_corruption() {
        let temp = tempfile::TempDir::new().unwrap();
        let storage = LocalStorage::new(temp.path().to_str().unwrap())
            .await
            .unwrap();
        let file_id = Uuid::new_v4().to_string();
        let content_path = storage.get_file_path(&file_id);
        fs::create_dir_all(content_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(content_path, b"orphan").await.unwrap();

        assert!(matches!(
            storage.metadata_with_owner(&file_id).await,
            Err(GatewayError::Internal(_))
        ));
    }
}
