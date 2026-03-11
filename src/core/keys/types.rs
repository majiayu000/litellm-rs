//! API Key types and data structures
//!
//! This module contains all types related to API key management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of an API key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KeyStatus {
    /// Key is active and can be used
    #[default]
    Active,
    /// Key has been revoked and cannot be used
    Revoked,
    /// Key has expired
    Expired,
}

impl std::fmt::Display for KeyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Revoked => write!(f, "revoked"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

/// Permissions associated with an API key
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyPermissions {
    /// List of allowed model patterns (supports wildcards like "gpt-*")
    #[serde(default)]
    pub allowed_models: Vec<String>,

    /// List of allowed API endpoints (e.g., "/v1/chat/completions")
    #[serde(default)]
    pub allowed_endpoints: Vec<String>,

    /// Maximum tokens allowed per request (0 = unlimited)
    #[serde(default)]
    pub max_tokens_per_request: Option<u32>,

    /// Whether the key has admin privileges
    #[serde(default)]
    pub is_admin: bool,

    /// Custom permission strings
    #[serde(default)]
    pub custom_permissions: Vec<String>,
}

impl KeyPermissions {
    /// Create new permissions with all access
    pub fn full_access() -> Self {
        Self {
            allowed_models: vec!["*".to_string()],
            allowed_endpoints: vec!["*".to_string()],
            max_tokens_per_request: None,
            is_admin: false,
            custom_permissions: vec![],
        }
    }

    /// Create admin permissions
    pub fn admin() -> Self {
        Self {
            allowed_models: vec!["*".to_string()],
            allowed_endpoints: vec!["*".to_string()],
            max_tokens_per_request: None,
            is_admin: true,
            custom_permissions: vec!["admin".to_string()],
        }
    }

    /// Check if a model is allowed
    pub fn is_model_allowed(&self, model: &str) -> bool {
        if self.allowed_models.is_empty() {
            return true;
        }

        for pattern in &self.allowed_models {
            if pattern == "*" {
                return true;
            }
            if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if model.starts_with(prefix) {
                    return true;
                }
            } else if pattern == model {
                return true;
            }
        }
        false
    }

    /// Check if an endpoint is allowed
    pub fn is_endpoint_allowed(&self, endpoint: &str) -> bool {
        if self.allowed_endpoints.is_empty() {
            return true;
        }

        for pattern in &self.allowed_endpoints {
            if pattern == "*" {
                return true;
            }
            if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if endpoint.starts_with(prefix) {
                    return true;
                }
            } else if pattern == endpoint {
                return true;
            }
        }
        false
    }
}

/// Rate limits for an API key
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyRateLimits {
    /// Requests per minute (0 = unlimited)
    #[serde(default)]
    pub requests_per_minute: Option<u32>,

    /// Tokens per minute (0 = unlimited)
    #[serde(default)]
    pub tokens_per_minute: Option<u32>,

    /// Requests per day (0 = unlimited)
    #[serde(default)]
    pub requests_per_day: Option<u32>,

    /// Tokens per day (0 = unlimited)
    #[serde(default)]
    pub tokens_per_day: Option<u32>,

    /// Maximum concurrent requests (0 = unlimited)
    #[serde(default)]
    pub max_concurrent_requests: Option<u32>,
}

impl KeyRateLimits {
    /// Create unlimited rate limits
    pub fn unlimited() -> Self {
        Self::default()
    }

    /// Create standard rate limits
    pub fn standard() -> Self {
        Self {
            requests_per_minute: Some(60),
            tokens_per_minute: Some(100_000),
            requests_per_day: Some(10_000),
            tokens_per_day: Some(1_000_000),
            max_concurrent_requests: Some(10),
        }
    }

    /// Create premium rate limits
    pub fn premium() -> Self {
        Self {
            requests_per_minute: Some(600),
            tokens_per_minute: Some(1_000_000),
            requests_per_day: Some(100_000),
            tokens_per_day: Some(10_000_000),
            max_concurrent_requests: Some(100),
        }
    }
}

/// Usage statistics for an API key
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyUsageStats {
    /// Total requests made with this key
    pub total_requests: u64,

    /// Total tokens consumed
    pub total_tokens: u64,

    /// Total cost incurred
    pub total_cost: f64,

    /// Requests today
    pub requests_today: u32,

    /// Tokens today
    pub tokens_today: u32,

    /// Cost today
    pub cost_today: f64,

    /// Last reset date (for daily counters)
    pub last_reset: DateTime<Utc>,
}

impl KeyUsageStats {
    /// Create new usage stats
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            total_tokens: 0,
            total_cost: 0.0,
            requests_today: 0,
            tokens_today: 0,
            cost_today: 0.0,
            last_reset: Utc::now(),
        }
    }

    /// Record usage
    pub fn record_usage(&mut self, tokens: u64, cost: f64) {
        self.total_requests += 1;
        self.total_tokens += tokens;
        self.total_cost += cost;
        self.requests_today += 1;
        self.tokens_today += tokens as u32;
        self.cost_today += cost;
    }

    /// Reset daily counters if needed
    pub fn reset_daily_if_needed(&mut self) {
        let now = Utc::now();
        if now.date_naive() != self.last_reset.date_naive() {
            self.requests_today = 0;
            self.tokens_today = 0;
            self.cost_today = 0.0;
            self.last_reset = now;
        }
    }
}

/// A managed API key with full metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedApiKey {
    /// Unique identifier for the key
    pub id: Uuid,

    /// Hash of the API key (never store raw key)
    pub key_hash: String,

    /// Key prefix for identification (e.g., "gw-abc...xyz")
    pub key_prefix: String,

    /// Human-readable name for the key
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Associated user ID
    pub user_id: Option<Uuid>,

    /// Associated team ID
    pub team_id: Option<Uuid>,

    /// Associated budget ID for spend tracking
    pub budget_id: Option<Uuid>,

    /// Key permissions
    pub permissions: KeyPermissions,

    /// Rate limits
    pub rate_limits: KeyRateLimits,

    /// Current status
    pub status: KeyStatus,

    /// Expiration date (None = never expires)
    pub expires_at: Option<DateTime<Utc>>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Last used timestamp
    pub last_used_at: Option<DateTime<Utc>>,

    /// Usage statistics
    pub usage_stats: KeyUsageStats,

    /// Custom metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl ManagedApiKey {
    /// Check if the key is valid for use
    pub fn is_valid(&self) -> bool {
        if self.status != KeyStatus::Active {
            return false;
        }

        if let Some(expires_at) = self.expires_at
            && Utc::now() > expires_at
        {
            return false;
        }

        true
    }

    /// Get the effective status (checking expiration)
    pub fn effective_status(&self) -> KeyStatus {
        if self.status == KeyStatus::Revoked {
            return KeyStatus::Revoked;
        }

        if let Some(expires_at) = self.expires_at
            && Utc::now() > expires_at
        {
            return KeyStatus::Expired;
        }

        self.status
    }
}

/// Configuration for creating a new API key
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateKeyConfig {
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
    pub permissions: KeyPermissions,

    /// Rate limits
    #[serde(default)]
    pub rate_limits: KeyRateLimits,

    /// Expiration date (None = never expires)
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,

    /// Custom metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Configuration for updating an API key
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateKeyConfig {
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

/// Result of verifying a key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyKeyResult {
    /// Whether the key is valid
    pub valid: bool,

    /// Key information (if found and valid)
    pub key: Option<KeyInfo>,

    /// Reason for invalidity (if not valid)
    pub invalid_reason: Option<String>,
}

/// Public information about a key (safe to expose)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    /// Key ID
    pub id: Uuid,

    /// Key prefix (masked key)
    pub key_prefix: String,

    /// Key name
    pub name: String,

    /// Description
    pub description: Option<String>,

    /// User ID
    pub user_id: Option<Uuid>,

    /// Team ID
    pub team_id: Option<Uuid>,

    /// Current status
    pub status: KeyStatus,

    /// Permissions
    pub permissions: KeyPermissions,

    /// Rate limits
    pub rate_limits: KeyRateLimits,

    /// Expiration date
    pub expires_at: Option<DateTime<Utc>>,

    /// Creation date
    pub created_at: DateTime<Utc>,

    /// Last used
    pub last_used_at: Option<DateTime<Utc>>,

    /// Usage statistics
    pub usage_stats: KeyUsageStats,
}

impl From<&ManagedApiKey> for KeyInfo {
    fn from(key: &ManagedApiKey) -> Self {
        Self {
            id: key.id,
            key_prefix: key.key_prefix.clone(),
            name: key.name.clone(),
            description: key.description.clone(),
            user_id: key.user_id,
            team_id: key.team_id,
            status: key.effective_status(),
            permissions: key.permissions.clone(),
            rate_limits: key.rate_limits.clone(),
            expires_at: key.expires_at,
            created_at: key.created_at,
            last_used_at: key.last_used_at,
            usage_stats: key.usage_stats.clone(),
        }
    }
}

#[cfg(test)]
mod type_tests {
    use super::*;

    #[test]
    fn test_key_status_display() {
        assert_eq!(KeyStatus::Active.to_string(), "active");
        assert_eq!(KeyStatus::Revoked.to_string(), "revoked");
        assert_eq!(KeyStatus::Expired.to_string(), "expired");
    }

    #[test]
    fn test_key_status_default() {
        assert_eq!(KeyStatus::default(), KeyStatus::Active);
    }

    #[test]
    fn test_permissions_model_allowed_wildcard() {
        let perms = KeyPermissions {
            allowed_models: vec!["*".to_string()],
            ..Default::default()
        };
        assert!(perms.is_model_allowed("gpt-4"));
        assert!(perms.is_model_allowed("claude-3"));
    }

    #[test]
    fn test_permissions_model_allowed_pattern() {
        let perms = KeyPermissions {
            allowed_models: vec!["gpt-*".to_string()],
            ..Default::default()
        };
        assert!(perms.is_model_allowed("gpt-4"));
        assert!(perms.is_model_allowed("gpt-3.5-turbo"));
        assert!(!perms.is_model_allowed("claude-3"));
    }

    #[test]
    fn test_permissions_model_allowed_exact() {
        let perms = KeyPermissions {
            allowed_models: vec!["gpt-4".to_string()],
            ..Default::default()
        };
        assert!(perms.is_model_allowed("gpt-4"));
        assert!(!perms.is_model_allowed("gpt-4-turbo"));
    }

    #[test]
    fn test_permissions_empty_allows_all() {
        let perms = KeyPermissions::default();
        assert!(perms.is_model_allowed("any-model"));
        assert!(perms.is_endpoint_allowed("/any/endpoint"));
    }

    #[test]
    fn test_rate_limits_standard() {
        let limits = KeyRateLimits::standard();
        assert_eq!(limits.requests_per_minute, Some(60));
        assert_eq!(limits.tokens_per_minute, Some(100_000));
    }

    #[test]
    fn test_usage_stats_record() {
        let mut stats = KeyUsageStats::new();
        stats.record_usage(100, 0.01);
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.total_tokens, 100);
        assert!((stats.total_cost - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn test_managed_key_validity() {
        let key = ManagedApiKey {
            id: Uuid::new_v4(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test...1234".to_string(),
            name: "Test Key".to_string(),
            description: None,
            user_id: None,
            team_id: None,
            budget_id: None,
            permissions: KeyPermissions::default(),
            rate_limits: KeyRateLimits::default(),
            status: KeyStatus::Active,
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            usage_stats: KeyUsageStats::new(),
            metadata: serde_json::Value::Null,
        };
        assert!(key.is_valid());
    }

    #[test]
    fn test_managed_key_expired() {
        let key = ManagedApiKey {
            id: Uuid::new_v4(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test...1234".to_string(),
            name: "Test Key".to_string(),
            description: None,
            user_id: None,
            team_id: None,
            budget_id: None,
            permissions: KeyPermissions::default(),
            rate_limits: KeyRateLimits::default(),
            status: KeyStatus::Active,
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            usage_stats: KeyUsageStats::new(),
            metadata: serde_json::Value::Null,
        };
        assert!(!key.is_valid());
        assert_eq!(key.effective_status(), KeyStatus::Expired);
    }

    #[test]
    fn test_managed_key_revoked() {
        let key = ManagedApiKey {
            id: Uuid::new_v4(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test...1234".to_string(),
            name: "Test Key".to_string(),
            description: None,
            user_id: None,
            team_id: None,
            budget_id: None,
            permissions: KeyPermissions::default(),
            rate_limits: KeyRateLimits::default(),
            status: KeyStatus::Revoked,
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            usage_stats: KeyUsageStats::new(),
            metadata: serde_json::Value::Null,
        };
        assert!(!key.is_valid());
    }

    #[test]
    fn test_key_info_from_managed_key() {
        let key = ManagedApiKey {
            id: Uuid::new_v4(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test...1234".to_string(),
            name: "Test Key".to_string(),
            description: Some("A test key".to_string()),
            user_id: Some(Uuid::new_v4()),
            team_id: None,
            budget_id: None,
            permissions: KeyPermissions::full_access(),
            rate_limits: KeyRateLimits::standard(),
            status: KeyStatus::Active,
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
            usage_stats: KeyUsageStats::new(),
            metadata: serde_json::Value::Null,
        };

        let info = KeyInfo::from(&key);
        assert_eq!(info.id, key.id);
        assert_eq!(info.name, key.name);
        assert_eq!(info.status, KeyStatus::Active);
    }
}
