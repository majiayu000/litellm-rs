//! Local file system storage implementation

use crate::utils::error::gateway_error::{GatewayError, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info};
use uuid::Uuid;

use super::types::FileMetadata;

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
        let file_id = Uuid::new_v4().to_string();
        let file_path = self.get_file_path(&file_id);

        // Create subdirectories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                GatewayError::Internal(format!("Failed to create directory: {}", e))
            })?;
        }

        // Write file content
        let mut file = fs::File::create(&file_path)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to create file: {}", e)))?;

        file.write_all(content)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to write file: {}", e)))?;

        // Store metadata
        let metadata = FileMetadata {
            id: file_id.clone(),
            filename: filename.to_string(),
            content_type: Self::detect_content_type(filename),
            size: content.len() as u64,
            created_at: chrono::Utc::now(),
            checksum: Self::calculate_checksum(content),
        };

        self.store_metadata(&file_id, &metadata).await?;

        debug!("File stored: {} -> {}", filename, file_id);
        Ok(file_id)
    }

    /// Retrieve file content
    pub async fn get(&self, file_id: &str) -> Result<Vec<u8>> {
        let file_path = self.get_file_path(file_id);

        if !file_path.exists() {
            return Err(GatewayError::NotFound(format!(
                "File not found: {}",
                file_id
            )));
        }

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
        let file_path = self.get_file_path(file_id);
        Ok(file_path.exists())
    }

    /// Get file metadata
    pub async fn metadata(&self, file_id: &str) -> Result<FileMetadata> {
        let metadata_path = self.get_metadata_path(file_id);

        if !metadata_path.exists() {
            return Err(GatewayError::NotFound(format!(
                "File metadata not found: {}",
                file_id
            )));
        }

        let content = fs::read_to_string(&metadata_path)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to read metadata: {}", e)))?;

        let metadata: FileMetadata = serde_json::from_str(&content)
            .map_err(|e| GatewayError::Internal(format!("Failed to parse metadata: {}", e)))?;

        Ok(metadata)
    }

    /// List files
    pub async fn list(&self, prefix: Option<&str>, limit: Option<usize>) -> Result<Vec<String>> {
        let mut files = Vec::new();
        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to read entry: {}", e)))?
        {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip metadata files
            if file_name.ends_with(".meta") {
                continue;
            }

            // Apply prefix filter
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
                break;
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

    /// Store file metadata
    async fn store_metadata(&self, file_id: &str, metadata: &FileMetadata) -> Result<()> {
        let metadata_path = self.get_metadata_path(file_id);

        if let Some(parent) = metadata_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                GatewayError::Internal(format!("Failed to create metadata directory: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(metadata)
            .map_err(|e| GatewayError::Internal(format!("Failed to serialize metadata: {}", e)))?;

        fs::write(&metadata_path, content)
            .await
            .map_err(|e| GatewayError::Internal(format!("Failed to write metadata: {}", e)))?;

        Ok(())
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
}
