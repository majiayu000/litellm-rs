//! File storage types and enums

use super::{LocalStorage, S3Storage};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

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

/// Canonical single-owner scope persisted with newly uploaded HTTP files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "scope",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum FileOwnerScope {
    Team(Uuid),
    User(Uuid),
    ApiKey(Uuid),
}

/// Presence state for the persisted owner field.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum OwnerField {
    #[default]
    Absent,
    Present(FileOwnerScope),
}

impl OwnerField {
    fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    pub(crate) fn as_scope(&self) -> Option<&FileOwnerScope> {
        match self {
            Self::Absent => None,
            Self::Present(scope) => Some(scope),
        }
    }
}

fn serialize_owner_field<S>(
    owner: &OwnerField,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match owner {
        OwnerField::Present(scope) => scope.serialize(serializer),
        OwnerField::Absent => serializer.serialize_unit(),
    }
}

fn deserialize_owner_field<'de, D>(deserializer: D) -> std::result::Result<OwnerField, D::Error>
where
    D: Deserializer<'de>,
{
    FileOwnerScope::deserialize(deserializer).map(OwnerField::Present)
}

/// Internal persisted metadata envelope. Public API metadata remains unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredFileMetadata {
    #[serde(flatten)]
    pub(crate) public: FileMetadata,
    #[serde(
        default,
        skip_serializing_if = "OwnerField::is_absent",
        serialize_with = "serialize_owner_field",
        deserialize_with = "deserialize_owner_field"
    )]
    pub(crate) owner: OwnerField,
}

impl StoredFileMetadata {
    pub(crate) fn legacy(public: FileMetadata) -> Self {
        Self {
            public,
            owner: OwnerField::Absent,
        }
    }

    pub(crate) fn owned(public: FileMetadata, owner: FileOwnerScope) -> Self {
        Self {
            public,
            owner: OwnerField::Present(owner),
        }
    }

    pub(crate) fn owner(&self) -> Option<&FileOwnerScope> {
        self.owner.as_scope()
    }
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

    #[test]
    fn gh1130_owner_wire_is_single_adjacent_tag_uuid_scope() {
        let team_id = Uuid::new_v4();
        let encoded = serde_json::to_value(FileOwnerScope::Team(team_id)).unwrap();
        assert_eq!(encoded, serde_json::json!({"scope": "team", "id": team_id}));

        for scope in [
            FileOwnerScope::Team(Uuid::new_v4()),
            FileOwnerScope::User(Uuid::new_v4()),
            FileOwnerScope::ApiKey(Uuid::new_v4()),
        ] {
            let round_trip: FileOwnerScope =
                serde_json::from_value(serde_json::to_value(&scope).unwrap()).unwrap();
            assert_eq!(round_trip, scope);
        }
    }

    #[test]
    fn gh1130_only_physically_missing_owner_is_legacy() {
        let public = FileMetadata {
            id: "legacy".to_string(),
            filename: "legacy.jsonl".to_string(),
            content_type: "application/json".to_string(),
            size: 2,
            created_at: Utc::now(),
            purpose: Some("batch".to_string()),
            checksum: "checksum".to_string(),
        };
        let legacy_json = serde_json::to_value(&public).unwrap();
        let legacy: StoredFileMetadata = serde_json::from_value(legacy_json).unwrap();
        assert_eq!(legacy.owner, OwnerField::Absent);

        let mut explicit_null = serde_json::to_value(&public).unwrap();
        explicit_null
            .as_object_mut()
            .unwrap()
            .insert("owner".to_string(), serde_json::Value::Null);
        assert!(serde_json::from_value::<StoredFileMetadata>(explicit_null).is_err());

        for malformed in [
            serde_json::json!({"scope": "team"}),
            serde_json::json!({"scope": "unknown", "id": Uuid::new_v4()}),
            serde_json::json!({"scope": "user", "id": "not-a-uuid"}),
            serde_json::json!({
                "scope": "api_key",
                "id": Uuid::new_v4(),
                "extra": true
            }),
        ] {
            let mut value = serde_json::to_value(&public).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert("owner".to_string(), malformed);
            assert!(serde_json::from_value::<StoredFileMetadata>(value).is_err());
        }
    }
}
