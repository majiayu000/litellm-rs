//! Request and response types for API key management endpoints

use crate::core::keys::{
    KeyInfo, KeyPermissions, KeyRateLimits, KeyStatus, KeyUsageStats, VerifyKeyResult,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ==================== Request Types ====================

/// Request to create a new API key
#[derive(Debug, Clone, Deserialize)]
pub struct CreateKeyRequest {
    /// Human-readable name for the key
    pub name: String,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,

    /// Associated user ID
    #[serde(default)]
    pub user_id: Option<Uuid>,

    /// Associated team ID
    #[serde(default)]
    pub team_id: Option<Uuid>,

    /// Associated budget ID
    #[serde(default)]
    pub budget_id: Option<Uuid>,

    /// Key permissions
    #[serde(default)]
    pub permissions: Option<KeyPermissions>,

    /// Rate limits
    #[serde(default)]
    pub rate_limits: Option<KeyRateLimits>,

    /// Expiration date (ISO 8601 format)
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,

    /// Custom metadata
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Request to update an API key
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateKeyRequest {
    /// Update the name
    #[serde(default)]
    pub name: Option<String>,

    /// Update the description
    #[serde(default)]
    pub description: Option<Option<String>>,

    /// Update permissions
    #[serde(default)]
    pub permissions: Option<KeyPermissions>,

    /// Update rate limits
    #[serde(default)]
    pub rate_limits: Option<KeyRateLimits>,

    /// Update budget ID
    #[serde(default)]
    pub budget_id: Option<Option<Uuid>>,

    /// Update expiration date
    #[serde(default)]
    pub expires_at: Option<Option<DateTime<Utc>>>,

    /// Update metadata
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Request to verify an API key
#[derive(Debug, Clone, Deserialize)]
pub struct VerifyKeyRequest {
    /// The raw API key to verify
    pub key: String,
}

/// Query parameters for listing keys
#[derive(Debug, Clone, Deserialize)]
pub struct ListKeysQuery {
    /// Filter by status
    #[serde(default)]
    pub status: Option<KeyStatus>,

    /// Filter by user ID
    #[serde(default)]
    pub user_id: Option<Uuid>,

    /// Filter by team ID
    #[serde(default)]
    pub team_id: Option<Uuid>,

    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: u32,

    /// Items per page
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}

fn default_limit() -> u32 {
    20
}

// ==================== Response Types ====================

/// Response after creating a new API key
#[derive(Debug, Clone, Serialize)]
pub struct CreateKeyResponse {
    /// The key ID
    pub id: Uuid,

    /// The raw API key (only shown once!)
    pub key: String,

    /// Key information
    pub info: KeyInfo,

    /// Warning message
    pub warning: String,
}

/// Response for a single key
#[derive(Debug, Clone, Serialize)]
pub struct KeyResponse {
    /// Key information
    pub key: KeyInfo,
}

/// Response for listing keys
#[derive(Debug, Clone, Serialize)]
pub struct ListKeysResponse {
    /// List of keys
    pub keys: Vec<KeyInfo>,

    /// Pagination info
    pub pagination: PaginationInfo,
}

/// Response for key usage statistics
#[derive(Debug, Clone, Serialize)]
pub struct KeyUsageResponse {
    /// Key ID
    pub key_id: Uuid,

    /// Usage statistics
    pub usage: KeyUsageStats,
}

/// Response for key rotation
#[derive(Debug, Clone, Serialize)]
pub struct RotateKeyResponse {
    /// Old key ID (now revoked)
    pub old_key_id: Uuid,

    /// New key ID
    pub new_key_id: Uuid,

    /// The new raw API key (only shown once!)
    pub new_key: String,

    /// New key information
    pub info: KeyInfo,

    /// Warning message
    pub warning: String,
}

/// Response for key verification
#[derive(Debug, Clone, Serialize)]
pub struct VerifyKeyResponse {
    /// Verification result
    #[serde(flatten)]
    pub result: VerifyKeyResult,
}

/// Response for key revocation
#[derive(Debug, Clone, Serialize)]
pub struct RevokeKeyResponse {
    /// Key ID that was revoked
    pub key_id: Uuid,

    /// Status after revocation
    pub status: KeyStatus,

    /// Message
    pub message: String,
}

/// Pagination information
#[derive(Debug, Clone, Serialize)]
pub struct PaginationInfo {
    /// Current page
    pub page: u32,

    /// Items per page
    pub limit: u32,

    /// Total items
    pub total: u64,

    /// Total pages
    pub pages: u32,

    /// Has next page
    pub has_next: bool,

    /// Has previous page
    pub has_prev: bool,
}

impl PaginationInfo {
    /// Create pagination info from count and query parameters
    pub fn new(total: u64, page: u32, limit: u32) -> Self {
        let pages = ((total as f64) / (limit as f64)).ceil() as u32;
        Self {
            page,
            limit,
            total,
            pages,
            has_next: page < pages,
            has_prev: page > 1,
        }
    }
}

/// Error response for key operations
#[derive(Debug, Clone, Serialize)]
pub struct KeyErrorResponse {
    /// Error message
    pub error: String,

    /// Error code
    pub code: String,

    /// Additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl KeyErrorResponse {
    /// Create a new error response
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
            details: None,
        }
    }

    /// Create a not found error
    pub fn not_found(resource: &str) -> Self {
        Self::new(format!("{} not found", resource), "NOT_FOUND")
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(message, "VALIDATION_ERROR")
    }

    /// Create a conflict error
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(message, "CONFLICT")
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(message, "INTERNAL_ERROR")
    }

    /// Create a forbidden error
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(message, "FORBIDDEN")
    }

    /// Create an unauthorized error
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(message, "UNAUTHORIZED")
    }
}

#[cfg(test)]
mod types_tests {
    use super::*;

    #[test]
    fn test_create_key_request_deserialize() {
        let json = r#"{"name": "Test Key"}"#;
        let req: CreateKeyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Test Key");
        assert!(req.description.is_none());
    }

    #[test]
    fn test_create_key_request_full_deserialize() {
        let json = r#"{
            "name": "Full Key",
            "description": "A test key",
            "user_id": "550e8400-e29b-41d4-a716-446655440000",
            "permissions": {
                "allowed_models": ["gpt-4"],
                "is_admin": false
            }
        }"#;
        let req: CreateKeyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Full Key");
        assert!(req.description.is_some());
        assert!(req.user_id.is_some());
        assert!(req.permissions.is_some());
    }

    #[test]
    fn test_pagination_info() {
        let info = PaginationInfo::new(100, 2, 20);
        assert_eq!(info.pages, 5);
        assert!(info.has_next);
        assert!(info.has_prev);

        let first_page = PaginationInfo::new(100, 1, 20);
        assert!(!first_page.has_prev);
        assert!(first_page.has_next);

        let last_page = PaginationInfo::new(100, 5, 20);
        assert!(last_page.has_prev);
        assert!(!last_page.has_next);
    }

    #[test]
    fn test_error_response() {
        let err = KeyErrorResponse::not_found("API key");
        assert!(err.error.contains("not found"));
        assert_eq!(err.code, "NOT_FOUND");

        let validation_err = KeyErrorResponse::validation("Name is required");
        assert_eq!(validation_err.code, "VALIDATION_ERROR");
    }

    #[test]
    fn test_list_keys_query_defaults() {
        let json = r#"{}"#;
        let query: ListKeysQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.page, 1);
        assert_eq!(query.limit, 20);
        assert!(query.status.is_none());
    }
}
