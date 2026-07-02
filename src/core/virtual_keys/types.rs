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
#[path = "types_tests.rs"]
mod tests;
