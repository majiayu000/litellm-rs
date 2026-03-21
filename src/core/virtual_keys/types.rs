//! Virtual key types and data structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Virtual key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualKey {
    /// Unique key identifier
    pub key_id: String,
    /// The actual API key (hashed)
    pub key_hash: String,
    /// Human-readable key alias
    pub key_alias: Option<String>,
    /// User ID who owns this key
    pub user_id: String,
    /// Team ID (if applicable)
    pub team_id: Option<String>,
    /// Organization ID
    pub organization_id: Option<String>,
    /// Models this key can access
    pub models: Vec<String>,
    /// Maximum spend limit
    pub max_budget: Option<f64>,
    /// Current spend
    pub spend: f64,
    /// Budget duration (e.g., "1d", "1w", "1m")
    pub budget_duration: Option<String>,
    /// Budget reset timestamp
    pub budget_reset_at: Option<DateTime<Utc>>,
    /// Rate limits
    pub rate_limits: Option<RateLimits>,
    /// Key permissions
    pub permissions: Vec<Permission>,
    /// Key metadata
    pub metadata: HashMap<String, String>,
    /// Key expiration
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether key is active
    pub is_active: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last used timestamp
    pub last_used_at: Option<DateTime<Utc>>,
    /// Usage count
    pub usage_count: u64,
    /// Tags for organization
    pub tags: Vec<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    /// Requests per minute
    pub rpm: Option<u32>,
    /// Requests per hour
    pub rph: Option<u32>,
    /// Requests per day
    pub rpd: Option<u32>,
    /// Tokens per minute
    pub tpm: Option<u32>,
    /// Tokens per hour
    pub tph: Option<u32>,
    /// Tokens per day
    pub tpd: Option<u32>,
    /// Maximum parallel requests
    pub max_parallel_requests: Option<u32>,
}

/// Permission types for virtual keys
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Permission {
    /// Can make chat completion requests
    ChatCompletion,
    /// Can make text completion requests
    TextCompletion,
    /// Can make embedding requests
    Embedding,
    /// Can make image generation requests
    ImageGeneration,
    /// Can access specific models
    ModelAccess(String),
    /// Can access admin endpoints
    Admin,
    /// Can create other keys
    KeyManagement,
    /// Can view usage statistics
    ViewUsage,
    /// Can modify team settings
    TeamManagement,
    /// Custom permission
    Custom(String),
}

/// Rate limit state tracking
#[derive(Debug, Clone)]
pub struct RateLimitState {
    /// Request count in current window
    pub request_count: u32,
    /// Token count in current window
    pub token_count: u32,
    /// Window start time
    pub window_start: DateTime<Utc>,
    /// Current parallel requests
    pub parallel_requests: u32,
}

/// Key generation settings
#[derive(Debug, Clone)]
pub struct KeyGenerationSettings {
    /// Key length
    pub key_length: usize,
    /// Key prefix
    pub key_prefix: String,
    /// Default permissions
    pub default_permissions: Vec<Permission>,
    /// Default budget
    pub default_budget: Option<f64>,
    /// Default rate limits
    pub default_rate_limits: Option<RateLimits>,
}

impl Default for KeyGenerationSettings {
    fn default() -> Self {
        Self {
            key_length: 32,
            key_prefix: "sk-".to_string(),
            default_permissions: vec![
                Permission::ChatCompletion,
                Permission::TextCompletion,
                Permission::Embedding,
            ],
            default_budget: Some(100.0),
            default_rate_limits: Some(RateLimits {
                rpm: Some(60),
                rph: Some(3600),
                rpd: Some(86400),
                tpm: Some(100000),
                tph: Some(6000000),
                tpd: Some(144000000),
                max_parallel_requests: Some(10),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // ==================== Helper Functions ====================

    fn create_test_virtual_key() -> VirtualKey {
        VirtualKey {
            key_id: "key-12345".to_string(),
            key_hash: "abc123def456".to_string(),
            key_alias: Some("test-key".to_string()),
            user_id: "user-001".to_string(),
            team_id: Some("team-001".to_string()),
            organization_id: Some("org-001".to_string()),
            models: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
            max_budget: Some(100.0),
            spend: 25.0,
            budget_duration: Some("1m".to_string()),
            budget_reset_at: Some(Utc::now() + chrono::Duration::days(30)),
            rate_limits: Some(RateLimits {
                rpm: Some(60),
                rph: Some(3600),
                rpd: Some(86400),
                tpm: Some(100000),
                tph: Some(6000000),
                tpd: Some(144000000),
                max_parallel_requests: Some(10),
            }),
            permissions: vec![Permission::ChatCompletion, Permission::Embedding],
            metadata: HashMap::new(),
            expires_at: Some(Utc::now() + chrono::Duration::days(365)),
            is_active: true,
            created_at: Utc::now(),
            last_used_at: Some(Utc::now()),
            usage_count: 100,
            tags: vec!["production".to_string(), "api".to_string()],
        }
    }

    // ==================== VirtualKey Tests ====================

    #[test]
    fn test_virtual_key_creation() {
        let key = create_test_virtual_key();

        assert_eq!(key.key_id, "key-12345");
        assert_eq!(key.user_id, "user-001");
        assert_eq!(key.spend, 25.0);
        assert!(key.is_active);
        assert_eq!(key.usage_count, 100);
    }

    #[test]
    fn test_virtual_key_with_minimal_fields() {
        let key = VirtualKey {
            key_id: "key-minimal".to_string(),
            key_hash: "hash".to_string(),
            key_alias: None,
            user_id: "user".to_string(),
            team_id: None,
            organization_id: None,
            models: vec![],
            max_budget: None,
            spend: 0.0,
            budget_duration: None,
            budget_reset_at: None,
            rate_limits: None,
            permissions: vec![],
            metadata: HashMap::new(),
            expires_at: None,
            is_active: true,
            created_at: Utc::now(),
            last_used_at: None,
            usage_count: 0,
            tags: vec![],
        };

        assert!(key.key_alias.is_none());
        assert!(key.team_id.is_none());
        assert!(key.max_budget.is_none());
        assert!(key.rate_limits.is_none());
        assert!(key.models.is_empty());
    }

    #[test]
    fn test_virtual_key_clone() {
        let key = create_test_virtual_key();
        let cloned = key.clone();

        assert_eq!(cloned.key_id, key.key_id);
        assert_eq!(cloned.user_id, key.user_id);
        assert_eq!(cloned.spend, key.spend);
        assert_eq!(cloned.models, key.models);
    }

    #[test]
    fn test_virtual_key_debug() {
        let key = create_test_virtual_key();
        let debug_str = format!("{:?}", key);

        assert!(debug_str.contains("VirtualKey"));
        assert!(debug_str.contains("key-12345"));
    }

    #[test]
    fn test_virtual_key_serialization() {
        let key = create_test_virtual_key();
        let json = serde_json::to_string(&key).unwrap();

        assert!(json.contains("key-12345"));
        assert!(json.contains("user-001"));
        assert!(json.contains("gpt-4"));

        let parsed: VirtualKey = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key_id, key.key_id);
        assert_eq!(parsed.spend, key.spend);
    }

    #[test]
    fn test_virtual_key_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("environment".to_string(), "production".to_string());
        metadata.insert("version".to_string(), "v1".to_string());

        let key = VirtualKey {
            metadata,
            ..create_test_virtual_key()
        };

        assert_eq!(key.metadata.len(), 2);
        assert_eq!(
            key.metadata.get("environment"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn test_virtual_key_budget_remaining() {
        let key = create_test_virtual_key();

        let remaining = key.max_budget.map(|b| b - key.spend);
        assert_eq!(remaining, Some(75.0));
    }

    #[test]
    fn test_virtual_key_expired() {
        let mut key = create_test_virtual_key();
        key.expires_at = Some(Utc::now() - chrono::Duration::days(1));

        let is_expired = key.expires_at.map(|e| Utc::now() > e).unwrap_or(false);
        assert!(is_expired);
    }

    #[test]
    fn test_virtual_key_not_expired() {
        let key = create_test_virtual_key();

        let is_expired = key.expires_at.map(|e| Utc::now() > e).unwrap_or(false);
        assert!(!is_expired);
    }

    // ==================== RateLimits Tests ====================

    #[test]
    fn test_rate_limits_creation() {
        let limits = RateLimits {
            rpm: Some(60),
            rph: Some(3600),
            rpd: Some(86400),
            tpm: Some(100000),
            tph: Some(6000000),
            tpd: Some(144000000),
            max_parallel_requests: Some(10),
        };

        assert_eq!(limits.rpm, Some(60));
        assert_eq!(limits.tpm, Some(100000));
        assert_eq!(limits.max_parallel_requests, Some(10));
    }

    #[test]
    fn test_rate_limits_partial() {
        let limits = RateLimits {
            rpm: Some(30),
            rph: None,
            rpd: None,
            tpm: Some(50000),
            tph: None,
            tpd: None,
            max_parallel_requests: Some(5),
        };

        assert_eq!(limits.rpm, Some(30));
        assert!(limits.rph.is_none());
        assert!(limits.rpd.is_none());
    }

    #[test]
    fn test_rate_limits_clone() {
        let limits = RateLimits {
            rpm: Some(60),
            rph: Some(3600),
            rpd: Some(86400),
            tpm: Some(100000),
            tph: Some(6000000),
            tpd: Some(144000000),
            max_parallel_requests: Some(10),
        };

        let cloned = limits.clone();
        assert_eq!(cloned.rpm, limits.rpm);
        assert_eq!(cloned.tpm, limits.tpm);
    }

    #[test]
    fn test_rate_limits_debug() {
        let limits = RateLimits {
            rpm: Some(60),
            rph: None,
            rpd: None,
            tpm: Some(100000),
            tph: None,
            tpd: None,
            max_parallel_requests: Some(10),
        };

        let debug_str = format!("{:?}", limits);
        assert!(debug_str.contains("RateLimits"));
        assert!(debug_str.contains("60"));
    }

    #[test]
    fn test_rate_limits_serialization() {
        let limits = RateLimits {
            rpm: Some(60),
            rph: Some(3600),
            rpd: Some(86400),
            tpm: Some(100000),
            tph: Some(6000000),
            tpd: Some(144000000),
            max_parallel_requests: Some(10),
        };

        let json = serde_json::to_string(&limits).unwrap();
        assert!(json.contains("rpm"));
        assert!(json.contains("60"));

        let parsed: RateLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.rpm, Some(60));
    }

    #[test]
    fn test_rate_limits_all_none() {
        let limits = RateLimits {
            rpm: None,
            rph: None,
            rpd: None,
            tpm: None,
            tph: None,
            tpd: None,
            max_parallel_requests: None,
        };

        assert!(limits.rpm.is_none());
        assert!(limits.tpm.is_none());
        assert!(limits.max_parallel_requests.is_none());
    }

    // ==================== Permission Tests ====================

    #[test]
    fn test_permission_variants() {
        let permissions = vec![
            Permission::ChatCompletion,
            Permission::TextCompletion,
            Permission::Embedding,
            Permission::ImageGeneration,
            Permission::ModelAccess("gpt-4".to_string()),
            Permission::Admin,
            Permission::KeyManagement,
            Permission::ViewUsage,
            Permission::TeamManagement,
            Permission::Custom("special".to_string()),
        ];

        assert_eq!(permissions.len(), 10);
    }

    #[test]
    fn test_permission_equality() {
        let p1 = Permission::ChatCompletion;
        let p2 = Permission::ChatCompletion;
        let p3 = Permission::Embedding;

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_permission_model_access() {
        let p1 = Permission::ModelAccess("gpt-4".to_string());
        let p2 = Permission::ModelAccess("gpt-4".to_string());
        let p3 = Permission::ModelAccess("claude-3".to_string());

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_permission_custom() {
        let p1 = Permission::Custom("read-only".to_string());
        let p2 = Permission::Custom("read-only".to_string());
        let p3 = Permission::Custom("write".to_string());

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_permission_clone() {
        let perm = Permission::ModelAccess("gpt-4".to_string());
        let cloned = perm.clone();

        assert_eq!(perm, cloned);
    }

    #[test]
    fn test_permission_debug() {
        let perm = Permission::ChatCompletion;
        let debug_str = format!("{:?}", perm);

        assert!(debug_str.contains("ChatCompletion"));
    }

    #[test]
    fn test_permission_serialization() {
        let perm = Permission::ChatCompletion;
        let json = serde_json::to_string(&perm).unwrap();

        assert!(json.contains("ChatCompletion"));

        let parsed: Permission = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Permission::ChatCompletion);
    }

    #[test]
    fn test_permission_model_access_serialization() {
        let perm = Permission::ModelAccess("gpt-4".to_string());
        let json = serde_json::to_string(&perm).unwrap();

        assert!(json.contains("ModelAccess"));
        assert!(json.contains("gpt-4"));

        let parsed: Permission = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, perm);
    }

    // ==================== RateLimitState Tests ====================

    #[test]
    fn test_rate_limit_state_creation() {
        let state = RateLimitState {
            request_count: 10,
            token_count: 5000,
            window_start: Utc::now(),
            parallel_requests: 2,
        };

        assert_eq!(state.request_count, 10);
        assert_eq!(state.token_count, 5000);
        assert_eq!(state.parallel_requests, 2);
    }

    #[test]
    fn test_rate_limit_state_clone() {
        let state = RateLimitState {
            request_count: 50,
            token_count: 25000,
            window_start: Utc::now(),
            parallel_requests: 5,
        };

        let cloned = state.clone();
        assert_eq!(cloned.request_count, state.request_count);
        assert_eq!(cloned.token_count, state.token_count);
    }

    #[test]
    fn test_rate_limit_state_debug() {
        let state = RateLimitState {
            request_count: 100,
            token_count: 50000,
            window_start: Utc::now(),
            parallel_requests: 10,
        };

        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("RateLimitState"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_rate_limit_state_zero_values() {
        let state = RateLimitState {
            request_count: 0,
            token_count: 0,
            window_start: Utc::now(),
            parallel_requests: 0,
        };

        assert_eq!(state.request_count, 0);
        assert_eq!(state.token_count, 0);
        assert_eq!(state.parallel_requests, 0);
    }

    // ==================== KeyGenerationSettings Tests ====================

    #[test]
    fn test_key_generation_settings_default() {
        let settings = KeyGenerationSettings::default();

        assert_eq!(settings.key_length, 32);
        assert_eq!(settings.key_prefix, "sk-");
        assert_eq!(settings.default_permissions.len(), 3);
        assert_eq!(settings.default_budget, Some(100.0));
        assert!(settings.default_rate_limits.is_some());
    }

    #[test]
    fn test_key_generation_settings_default_permissions() {
        let settings = KeyGenerationSettings::default();

        assert!(
            settings
                .default_permissions
                .contains(&Permission::ChatCompletion)
        );
        assert!(
            settings
                .default_permissions
                .contains(&Permission::TextCompletion)
        );
        assert!(
            settings
                .default_permissions
                .contains(&Permission::Embedding)
        );
    }

    #[test]
    fn test_key_generation_settings_default_rate_limits() {
        let settings = KeyGenerationSettings::default();
        let limits = settings.default_rate_limits.unwrap();

        assert_eq!(limits.rpm, Some(60));
        assert_eq!(limits.rph, Some(3600));
        assert_eq!(limits.rpd, Some(86400));
        assert_eq!(limits.tpm, Some(100000));
        assert_eq!(limits.max_parallel_requests, Some(10));
    }

    #[test]
    fn test_key_generation_settings_custom() {
        let settings = KeyGenerationSettings {
            key_length: 64,
            key_prefix: "api-".to_string(),
            default_permissions: vec![Permission::Admin],
            default_budget: Some(500.0),
            default_rate_limits: None,
        };

        assert_eq!(settings.key_length, 64);
        assert_eq!(settings.key_prefix, "api-");
        assert_eq!(settings.default_budget, Some(500.0));
        assert!(settings.default_rate_limits.is_none());
    }

    #[test]
    fn test_key_generation_settings_clone() {
        let settings = KeyGenerationSettings::default();
        let cloned = settings.clone();

        assert_eq!(cloned.key_length, settings.key_length);
        assert_eq!(cloned.key_prefix, settings.key_prefix);
    }

    #[test]
    fn test_key_generation_settings_debug() {
        let settings = KeyGenerationSettings::default();
        let debug_str = format!("{:?}", settings);

        assert!(debug_str.contains("KeyGenerationSettings"));
        assert!(debug_str.contains("sk-"));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_virtual_key_with_permissions_check() {
        let key = create_test_virtual_key();

        let has_chat = key.permissions.contains(&Permission::ChatCompletion);
        let has_admin = key.permissions.contains(&Permission::Admin);

        assert!(has_chat);
        assert!(!has_admin);
    }

    #[test]
    fn test_virtual_key_model_access_check() {
        let key = create_test_virtual_key();

        let can_access_gpt4 = key.models.contains(&"gpt-4".to_string());
        let can_access_claude = key.models.contains(&"claude-3".to_string());

        assert!(can_access_gpt4);
        assert!(!can_access_claude);
    }

    #[test]
    fn test_rate_limit_state_window_check() {
        let state = RateLimitState {
            request_count: 30,
            token_count: 15000,
            window_start: Utc::now() - chrono::Duration::seconds(30),
            parallel_requests: 3,
        };

        let window_age_secs = (Utc::now() - state.window_start).num_seconds();
        assert!(window_age_secs >= 30);
        assert!(window_age_secs < 60);
    }

    #[test]
    fn test_rate_limit_enforcement_simulation() {
        let limits = RateLimits {
            rpm: Some(60),
            rph: None,
            rpd: None,
            tpm: Some(100000),
            tph: None,
            tpd: None,
            max_parallel_requests: Some(10),
        };

        let state = RateLimitState {
            request_count: 55,
            token_count: 90000,
            window_start: Utc::now(),
            parallel_requests: 8,
        };

        // Check if under limits
        let under_rpm = limits.rpm.map(|l| state.request_count < l).unwrap_or(true);
        let under_tpm = limits.tpm.map(|l| state.token_count < l).unwrap_or(true);
        let under_parallel = limits
            .max_parallel_requests
            .map(|l| state.parallel_requests < l)
            .unwrap_or(true);

        assert!(under_rpm);
        assert!(under_tpm);
        assert!(under_parallel);
    }

    #[test]
    fn test_rate_limit_exceeded_simulation() {
        let limits = RateLimits {
            rpm: Some(60),
            rph: None,
            rpd: None,
            tpm: Some(100000),
            tph: None,
            tpd: None,
            max_parallel_requests: Some(10),
        };

        let state = RateLimitState {
            request_count: 65,
            token_count: 110000,
            window_start: Utc::now(),
            parallel_requests: 12,
        };

        // Check if over limits
        let over_rpm = limits
            .rpm
            .map(|l| state.request_count >= l)
            .unwrap_or(false);
        let over_tpm = limits.tpm.map(|l| state.token_count >= l).unwrap_or(false);
        let over_parallel = limits
            .max_parallel_requests
            .map(|l| state.parallel_requests >= l)
            .unwrap_or(false);

        assert!(over_rpm);
        assert!(over_tpm);
        assert!(over_parallel);
    }

    #[test]
    fn test_budget_check_simulation() {
        let key = create_test_virtual_key();

        let cost = 10.0;
        let would_exceed = key
            .max_budget
            .map(|b| key.spend + cost > b)
            .unwrap_or(false);

        assert!(!would_exceed); // 25 + 10 = 35 < 100
    }

    #[test]
    fn test_budget_exceeded_simulation() {
        let mut key = create_test_virtual_key();
        key.spend = 95.0;

        let cost = 10.0;
        let would_exceed = key
            .max_budget
            .map(|b| key.spend + cost > b)
            .unwrap_or(false);

        assert!(would_exceed); // 95 + 10 = 105 > 100
    }

    #[test]
    fn test_key_validity_check() {
        let key = create_test_virtual_key();

        let is_valid = key.is_active && key.expires_at.map(|e| Utc::now() < e).unwrap_or(true);

        assert!(is_valid);
    }

    #[test]
    fn test_inactive_key_validity_check() {
        let mut key = create_test_virtual_key();
        key.is_active = false;

        let is_valid = key.is_active && key.expires_at.map(|e| Utc::now() < e).unwrap_or(true);

        assert!(!is_valid);
    }

    #[test]
    fn test_key_with_all_permissions() {
        let key = VirtualKey {
            permissions: vec![
                Permission::ChatCompletion,
                Permission::TextCompletion,
                Permission::Embedding,
                Permission::ImageGeneration,
                Permission::Admin,
                Permission::KeyManagement,
                Permission::ViewUsage,
                Permission::TeamManagement,
            ],
            ..create_test_virtual_key()
        };

        assert_eq!(key.permissions.len(), 8);
        assert!(key.permissions.contains(&Permission::Admin));
    }

    #[test]
    fn test_key_tags_filtering() {
        let key = create_test_virtual_key();

        let is_production = key.tags.contains(&"production".to_string());
        let is_staging = key.tags.contains(&"staging".to_string());

        assert!(is_production);
        assert!(!is_staging);
    }
}
