//! Virtual key request types

use super::types::{Permission, RateLimits};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Virtual key creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    /// Key alias (optional)
    pub key_alias: Option<String>,
    /// User ID
    pub user_id: String,
    /// Team ID (optional)
    pub team_id: Option<String>,
    /// Models to allow
    pub models: Vec<String>,
    /// Maximum budget
    pub max_budget: Option<f64>,
    /// Budget duration
    pub budget_duration: Option<String>,
    /// Rate limits
    pub rate_limits: Option<RateLimits>,
    /// Permissions
    pub permissions: Vec<Permission>,
    /// Metadata
    pub metadata: HashMap<String, String>,
    /// Expiration time
    pub expires_at: Option<DateTime<Utc>>,
    /// Tags
    pub tags: Vec<String>,
}

/// Virtual key update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateKeyRequest {
    /// Key alias
    pub key_alias: Option<String>,
    /// Models to allow
    pub models: Option<Vec<String>>,
    /// Maximum budget
    pub max_budget: Option<f64>,
    /// Budget duration
    pub budget_duration: Option<String>,
    /// Rate limits
    pub rate_limits: Option<RateLimits>,
    /// Permissions
    pub permissions: Option<Vec<Permission>>,
    /// Metadata
    pub metadata: Option<HashMap<String, String>>,
    /// Expiration time
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether key is active
    pub is_active: Option<bool>,
    /// Tags
    pub tags: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    // ==================== Helper Functions ====================

    fn create_test_rate_limits() -> RateLimits {
        RateLimits {
            rpm: Some(60),
            rph: Some(3600),
            rpd: Some(86400),
            tpm: Some(100000),
            tph: Some(6000000),
            tpd: Some(144000000),
            max_parallel_requests: Some(10),
        }
    }

    // ==================== CreateKeyRequest Tests ====================

    #[test]
    fn test_create_key_request_minimal() {
        let request = CreateKeyRequest {
            key_alias: None,
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec![],
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: vec![],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec![],
        };

        assert_eq!(request.user_id, "user-001");
        assert!(request.key_alias.is_none());
        assert!(request.models.is_empty());
        assert!(request.permissions.is_empty());
    }

    #[test]
    fn test_create_key_request_full() {
        let expires = Utc::now() + Duration::days(365);
        let mut metadata = HashMap::new();
        metadata.insert("env".to_string(), "production".to_string());

        let request = CreateKeyRequest {
            key_alias: Some("my-api-key".to_string()),
            user_id: "user-001".to_string(),
            team_id: Some("team-001".to_string()),
            models: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
            max_budget: Some(100.0),
            budget_duration: Some("1m".to_string()),
            rate_limits: Some(create_test_rate_limits()),
            permissions: vec![Permission::ChatCompletion, Permission::Embedding],
            metadata,
            expires_at: Some(expires),
            tags: vec!["production".to_string(), "api".to_string()],
        };

        assert_eq!(request.key_alias, Some("my-api-key".to_string()));
        assert_eq!(request.user_id, "user-001");
        assert_eq!(request.team_id, Some("team-001".to_string()));
        assert_eq!(request.models.len(), 2);
        assert_eq!(request.max_budget, Some(100.0));
        assert_eq!(request.permissions.len(), 2);
        assert_eq!(request.tags.len(), 2);
    }

    #[test]
    fn test_create_key_request_with_models() {
        let request = CreateKeyRequest {
            key_alias: None,
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec![
                "gpt-4".to_string(),
                "gpt-3.5-turbo".to_string(),
                "claude-3-opus".to_string(),
            ],
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: vec![],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec![],
        };

        assert_eq!(request.models.len(), 3);
        assert!(request.models.contains(&"gpt-4".to_string()));
        assert!(request.models.contains(&"claude-3-opus".to_string()));
    }

    #[test]
    fn test_create_key_request_with_budget() {
        let request = CreateKeyRequest {
            key_alias: None,
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec![],
            max_budget: Some(500.0),
            budget_duration: Some("1w".to_string()),
            rate_limits: None,
            permissions: vec![],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec![],
        };

        assert_eq!(request.max_budget, Some(500.0));
        assert_eq!(request.budget_duration, Some("1w".to_string()));
    }

    #[test]
    fn test_create_key_request_with_rate_limits() {
        let request = CreateKeyRequest {
            key_alias: None,
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec![],
            max_budget: None,
            budget_duration: None,
            rate_limits: Some(create_test_rate_limits()),
            permissions: vec![],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec![],
        };

        assert!(request.rate_limits.is_some());
        let limits = request.rate_limits.unwrap();
        assert_eq!(limits.rpm, Some(60));
        assert_eq!(limits.tpm, Some(100000));
    }

    #[test]
    fn test_create_key_request_with_permissions() {
        let request = CreateKeyRequest {
            key_alias: None,
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec![],
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: vec![
                Permission::ChatCompletion,
                Permission::TextCompletion,
                Permission::Embedding,
                Permission::Admin,
            ],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec![],
        };

        assert_eq!(request.permissions.len(), 4);
        assert!(request.permissions.contains(&Permission::Admin));
    }

    #[test]
    fn test_create_key_request_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("environment".to_string(), "production".to_string());
        metadata.insert("version".to_string(), "v1".to_string());
        metadata.insert("owner".to_string(), "team-a".to_string());

        let request = CreateKeyRequest {
            key_alias: None,
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec![],
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: vec![],
            metadata,
            expires_at: None,
            tags: vec![],
        };

        assert_eq!(request.metadata.len(), 3);
        assert_eq!(
            request.metadata.get("environment"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn test_create_key_request_with_expiration() {
        let expires = Utc::now() + Duration::days(30);

        let request = CreateKeyRequest {
            key_alias: None,
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec![],
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: vec![],
            metadata: HashMap::new(),
            expires_at: Some(expires),
            tags: vec![],
        };

        assert!(request.expires_at.is_some());
        assert!(request.expires_at.unwrap() > Utc::now());
    }

    #[test]
    fn test_create_key_request_clone() {
        let request = CreateKeyRequest {
            key_alias: Some("test-key".to_string()),
            user_id: "user-001".to_string(),
            team_id: Some("team-001".to_string()),
            models: vec!["gpt-4".to_string()],
            max_budget: Some(100.0),
            budget_duration: Some("1m".to_string()),
            rate_limits: None,
            permissions: vec![Permission::ChatCompletion],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec!["test".to_string()],
        };

        let cloned = request.clone();
        assert_eq!(cloned.key_alias, request.key_alias);
        assert_eq!(cloned.user_id, request.user_id);
        assert_eq!(cloned.models, request.models);
    }

    #[test]
    fn test_create_key_request_debug() {
        let request = CreateKeyRequest {
            key_alias: Some("debug-key".to_string()),
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec![],
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: vec![],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec![],
        };

        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("CreateKeyRequest"));
        assert!(debug_str.contains("debug-key"));
    }

    #[test]
    fn test_create_key_request_serialization() {
        let request = CreateKeyRequest {
            key_alias: Some("ser-key".to_string()),
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec!["gpt-4".to_string()],
            max_budget: Some(100.0),
            budget_duration: None,
            rate_limits: None,
            permissions: vec![Permission::ChatCompletion],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec![],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("ser-key"));
        assert!(json.contains("user-001"));
        assert!(json.contains("gpt-4"));

        let parsed: CreateKeyRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key_alias, request.key_alias);
        assert_eq!(parsed.user_id, request.user_id);
    }

    // ==================== UpdateKeyRequest Tests ====================

    #[test]
    fn test_update_key_request_minimal() {
        let request = UpdateKeyRequest {
            key_alias: None,
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: None,
        };

        assert!(request.key_alias.is_none());
        assert!(request.models.is_none());
        assert!(request.is_active.is_none());
    }

    #[test]
    fn test_update_key_request_full() {
        let expires = Utc::now() + Duration::days(90);
        let mut metadata = HashMap::new();
        metadata.insert("updated".to_string(), "true".to_string());

        let request = UpdateKeyRequest {
            key_alias: Some("updated-key".to_string()),
            models: Some(vec!["gpt-4".to_string()]),
            max_budget: Some(200.0),
            budget_duration: Some("1m".to_string()),
            rate_limits: Some(create_test_rate_limits()),
            permissions: Some(vec![Permission::ChatCompletion]),
            metadata: Some(metadata),
            expires_at: Some(expires),
            is_active: Some(true),
            tags: Some(vec!["updated".to_string()]),
        };

        assert_eq!(request.key_alias, Some("updated-key".to_string()));
        assert_eq!(request.max_budget, Some(200.0));
        assert_eq!(request.is_active, Some(true));
    }

    #[test]
    fn test_update_key_request_partial_update() {
        let request = UpdateKeyRequest {
            key_alias: Some("new-alias".to_string()),
            models: None,
            max_budget: Some(300.0),
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: None,
        };

        assert_eq!(request.key_alias, Some("new-alias".to_string()));
        assert!(request.models.is_none());
        assert_eq!(request.max_budget, Some(300.0));
    }

    #[test]
    fn test_update_key_request_deactivate() {
        let request = UpdateKeyRequest {
            key_alias: None,
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: Some(false),
            tags: None,
        };

        assert_eq!(request.is_active, Some(false));
    }

    #[test]
    fn test_update_key_request_update_models() {
        let request = UpdateKeyRequest {
            key_alias: None,
            models: Some(vec![
                "gpt-4".to_string(),
                "gpt-4-turbo".to_string(),
                "claude-3-opus".to_string(),
            ]),
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: None,
        };

        assert!(request.models.is_some());
        let models = request.models.unwrap();
        assert_eq!(models.len(), 3);
    }

    #[test]
    fn test_update_key_request_update_rate_limits() {
        let request = UpdateKeyRequest {
            key_alias: None,
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: Some(RateLimits {
                rpm: Some(120),
                rph: None,
                rpd: None,
                tpm: Some(200000),
                tph: None,
                tpd: None,
                max_parallel_requests: Some(20),
            }),
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: None,
        };

        assert!(request.rate_limits.is_some());
        let limits = request.rate_limits.unwrap();
        assert_eq!(limits.rpm, Some(120));
        assert_eq!(limits.tpm, Some(200000));
    }

    #[test]
    fn test_update_key_request_update_permissions() {
        let request = UpdateKeyRequest {
            key_alias: None,
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: Some(vec![
                Permission::ChatCompletion,
                Permission::Admin,
                Permission::KeyManagement,
            ]),
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: None,
        };

        assert!(request.permissions.is_some());
        let perms = request.permissions.unwrap();
        assert_eq!(perms.len(), 3);
        assert!(perms.contains(&Permission::Admin));
    }

    #[test]
    fn test_update_key_request_update_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("status".to_string(), "upgraded".to_string());
        metadata.insert("tier".to_string(), "premium".to_string());

        let request = UpdateKeyRequest {
            key_alias: None,
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: Some(metadata),
            expires_at: None,
            is_active: None,
            tags: None,
        };

        assert!(request.metadata.is_some());
        let meta = request.metadata.unwrap();
        assert_eq!(meta.len(), 2);
        assert_eq!(meta.get("tier"), Some(&"premium".to_string()));
    }

    #[test]
    fn test_update_key_request_extend_expiration() {
        let new_expires = Utc::now() + Duration::days(365);

        let request = UpdateKeyRequest {
            key_alias: None,
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: Some(new_expires),
            is_active: None,
            tags: None,
        };

        assert!(request.expires_at.is_some());
        assert!(request.expires_at.unwrap() > Utc::now());
    }

    #[test]
    fn test_update_key_request_clone() {
        let request = UpdateKeyRequest {
            key_alias: Some("clone-test".to_string()),
            models: Some(vec!["gpt-4".to_string()]),
            max_budget: Some(100.0),
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: Some(true),
            tags: None,
        };

        let cloned = request.clone();
        assert_eq!(cloned.key_alias, request.key_alias);
        assert_eq!(cloned.max_budget, request.max_budget);
        assert_eq!(cloned.is_active, request.is_active);
    }

    #[test]
    fn test_update_key_request_debug() {
        let request = UpdateKeyRequest {
            key_alias: Some("debug-test".to_string()),
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: None,
        };

        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("UpdateKeyRequest"));
        assert!(debug_str.contains("debug-test"));
    }

    #[test]
    fn test_update_key_request_serialization() {
        let request = UpdateKeyRequest {
            key_alias: Some("ser-test".to_string()),
            models: Some(vec!["gpt-4".to_string()]),
            max_budget: Some(150.0),
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: Some(true),
            tags: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("ser-test"));
        assert!(json.contains("150"));

        let parsed: UpdateKeyRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key_alias, request.key_alias);
        assert_eq!(parsed.max_budget, request.max_budget);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_key_lifecycle_create_and_update() {
        // Step 1: Create key request
        let create_request = CreateKeyRequest {
            key_alias: Some("lifecycle-key".to_string()),
            user_id: "user-001".to_string(),
            team_id: None,
            models: vec!["gpt-3.5-turbo".to_string()],
            max_budget: Some(50.0),
            budget_duration: None,
            rate_limits: None,
            permissions: vec![Permission::ChatCompletion],
            metadata: HashMap::new(),
            expires_at: None,
            tags: vec!["test".to_string()],
        };

        assert_eq!(create_request.max_budget, Some(50.0));
        assert_eq!(create_request.models.len(), 1);

        // Step 2: Upgrade key
        let update_request = UpdateKeyRequest {
            key_alias: None,
            models: Some(vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()]),
            max_budget: Some(200.0),
            budget_duration: None,
            rate_limits: Some(create_test_rate_limits()),
            permissions: Some(vec![Permission::ChatCompletion, Permission::Embedding]),
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: None,
        };

        assert_eq!(update_request.max_budget, Some(200.0));
        assert_eq!(update_request.models.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_budget_duration_formats() {
        let durations = vec!["1d", "1w", "1m", "3m", "1y"];

        for duration in durations {
            let request = CreateKeyRequest {
                key_alias: None,
                user_id: "user-001".to_string(),
                team_id: None,
                models: vec![],
                max_budget: Some(100.0),
                budget_duration: Some(duration.to_string()),
                rate_limits: None,
                permissions: vec![],
                metadata: HashMap::new(),
                expires_at: None,
                tags: vec![],
            };

            assert_eq!(request.budget_duration, Some(duration.to_string()));
        }
    }

    #[test]
    fn test_permission_combinations() {
        let basic_perms = [Permission::ChatCompletion];
        let standard_perms = [
            Permission::ChatCompletion,
            Permission::TextCompletion,
            Permission::Embedding,
        ];
        let admin_perms = [
            Permission::ChatCompletion,
            Permission::TextCompletion,
            Permission::Embedding,
            Permission::Admin,
            Permission::KeyManagement,
        ];

        assert_eq!(basic_perms.len(), 1);
        assert_eq!(standard_perms.len(), 3);
        assert_eq!(admin_perms.len(), 5);
        assert!(admin_perms.contains(&Permission::Admin));
    }

    #[test]
    fn test_empty_update_request() {
        let request = UpdateKeyRequest {
            key_alias: None,
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: None,
        };

        // All fields are None - valid empty update
        assert!(request.key_alias.is_none());
        assert!(request.models.is_none());
        assert!(request.max_budget.is_none());
        assert!(request.budget_duration.is_none());
        assert!(request.rate_limits.is_none());
        assert!(request.permissions.is_none());
        assert!(request.metadata.is_none());
        assert!(request.expires_at.is_none());
        assert!(request.is_active.is_none());
        assert!(request.tags.is_none());
    }

    #[test]
    fn test_update_tags() {
        let request = UpdateKeyRequest {
            key_alias: None,
            models: None,
            max_budget: None,
            budget_duration: None,
            rate_limits: None,
            permissions: None,
            metadata: None,
            expires_at: None,
            is_active: None,
            tags: Some(vec!["production".to_string(), "high-priority".to_string()]),
        };

        assert!(request.tags.is_some());
        let tags = request.tags.unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"production".to_string()));
    }
}
