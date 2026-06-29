//! File storage types and enums

use super::{LocalStorage, S3Storage};

/// File storage backend
#[derive(Debug, Clone)]
pub enum FileStorage {
    /// Local file system storage
    Local(LocalStorage),
    /// Amazon S3 storage
    S3(S3Storage),
}

/// File metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileMetadata {
    /// File ID
    pub id: String,
    /// Original filename
    pub filename: String,
    /// MIME content type
    pub content_type: String,
    /// File size in bytes
    pub size: u64,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// OpenAI file purpose, when supplied at upload time
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// File checksum
    pub checksum: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_file_metadata_structure() {
        let metadata = FileMetadata {
            id: "file-123".to_string(),
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 1024,
            created_at: Utc::now(),
            purpose: Some("assistants".to_string()),
            checksum: "abc123".to_string(),
        };

        assert_eq!(metadata.id, "file-123");
        assert_eq!(metadata.filename, "test.txt");
        assert_eq!(metadata.content_type, "text/plain");
        assert_eq!(metadata.size, 1024);
        assert_eq!(metadata.purpose.as_deref(), Some("assistants"));
        assert_eq!(metadata.checksum, "abc123");
    }

    #[test]
    fn test_file_metadata_clone() {
        let metadata = FileMetadata {
            id: "file-123".to_string(),
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 1024,
            created_at: Utc::now(),
            purpose: None,
            checksum: "abc123".to_string(),
        };

        let cloned = metadata.clone();
        assert_eq!(metadata.id, cloned.id);
        assert_eq!(metadata.filename, cloned.filename);
        assert_eq!(metadata.size, cloned.size);
    }

    #[test]
    fn test_file_metadata_serialization() {
        let metadata = FileMetadata {
            id: "file-456".to_string(),
            filename: "document.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            size: 2048,
            created_at: Utc::now(),
            purpose: Some("batch".to_string()),
            checksum: "def456".to_string(),
        };

        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["id"], "file-456");
        assert_eq!(json["filename"], "document.pdf");
        assert_eq!(json["content_type"], "application/pdf");
        assert_eq!(json["size"], 2048);
        assert_eq!(json["purpose"], "batch");
    }

    #[test]
    fn test_file_metadata_deserialization() {
        let json = r#"{
            "id": "file-789",
            "filename": "image.png",
            "content_type": "image/png",
            "size": 4096,
            "created_at": "2024-01-01T00:00:00Z",
            "purpose": "fine-tune",
            "checksum": "ghi789"
        }"#;

        let metadata: FileMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.id, "file-789");
        assert_eq!(metadata.filename, "image.png");
        assert_eq!(metadata.content_type, "image/png");
        assert_eq!(metadata.size, 4096);
        assert_eq!(metadata.purpose.as_deref(), Some("fine-tune"));
        assert_eq!(metadata.checksum, "ghi789");
    }

    #[test]
    fn test_file_metadata_deserializes_legacy_without_purpose() {
        let json = r#"{
            "id": "file-legacy",
            "filename": "legacy.jsonl",
            "content_type": "application/json",
            "size": 128,
            "created_at": "2024-01-01T00:00:00Z",
            "checksum": "legacy"
        }"#;

        let metadata: FileMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.id, "file-legacy");
        assert_eq!(metadata.purpose, None);
    }

    #[test]
    fn test_file_metadata_zero_size() {
        let metadata = FileMetadata {
            id: "empty-file".to_string(),
            filename: "empty.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 0,
            created_at: Utc::now(),
            purpose: None,
            checksum: "empty".to_string(),
        };

        assert_eq!(metadata.size, 0);
    }

    #[test]
    fn test_file_metadata_large_size() {
        let metadata = FileMetadata {
            id: "large-file".to_string(),
            filename: "large.bin".to_string(),
            content_type: "application/octet-stream".to_string(),
            size: u64::MAX,
            created_at: Utc::now(),
            purpose: None,
            checksum: "large".to_string(),
        };

        assert_eq!(metadata.size, u64::MAX);
    }
}
