//! API key creation and verification
//!
//! This module provides methods for creating and verifying API keys.

use super::types::{ApiKeyVerification, CreateApiKeyRequest};
use crate::core::models::user::types::User;
use crate::core::models::{ApiKey, Metadata, UsageStats};
use crate::storage::StorageLayer;
use crate::utils::auth::crypto::keys::{extract_api_key_prefix, generate_api_key, hash_api_key};
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Known valid permission strings for API keys.
///
/// These match the default permissions defined in [`crate::auth::rbac::system`].
const VALID_PERMISSIONS: &[&str] = &[
    "*",
    "users.read",
    "users.write",
    "users.delete",
    "teams.read",
    "teams.write",
    "teams.delete",
    "api.chat",
    "api.embeddings",
    "api.images",
    "api_keys.read",
    "api_keys.write",
    "api_keys.delete",
    "analytics.read",
    "system.admin",
];

/// Validate name and permissions for API key creation.
fn validate_create_key_input(name: &str, permissions: &[String]) -> Result<()> {
    // Validate name length (1-255 chars)
    if name.is_empty() {
        return Err(GatewayError::Validation(
            "API key name must not be empty".to_string(),
        ));
    }
    if name.len() > 255 {
        return Err(GatewayError::Validation(
            "API key name must not exceed 255 characters".to_string(),
        ));
    }

    // Validate name contains no control characters
    if name.chars().any(|c| c.is_control()) {
        return Err(GatewayError::Validation(
            "API key name must not contain control characters".to_string(),
        ));
    }

    // Validate permissions against known set
    for perm in permissions {
        if !VALID_PERMISSIONS.contains(&perm.as_str()) {
            return Err(GatewayError::Validation(format!(
                "Unknown permission: '{}'. Valid permissions: {}",
                perm,
                VALID_PERMISSIONS[1..].join(", "),
            )));
        }
    }

    Ok(())
}

/// Minimum interval between DB writes for the same key's last_used timestamp.
const LAST_USED_THROTTLE: Duration = Duration::from_secs(5 * 60);

/// TTL for cached API keys in Redis (seconds).
const API_KEY_CACHE_TTL: u64 = 300;

/// Build the Redis cache key for an API key hash.
fn api_key_cache_key(key_hash: &str) -> String {
    format!("api_key:hash:{}", key_hash)
}

/// API key handler for authentication and management
#[derive(Debug, Clone)]
pub struct ApiKeyHandler {
    /// Storage layer for persistence
    pub(super) storage: Arc<StorageLayer>,
    /// Tracks when each key's `last_used_at` was last persisted to the DB.
    last_used_cache: Arc<DashMap<Uuid, Instant>>,
    /// Optional HMAC secret for key hashing. When set, uses HMAC-SHA256
    /// instead of plain SHA-256.
    hmac_secret: Option<String>,
}

impl ApiKeyHandler {
    /// Create a new API key handler
    pub async fn new(storage: Arc<StorageLayer>, hmac_secret: Option<String>) -> Result<Self> {
        Ok(Self {
            storage,
            hmac_secret,
            last_used_cache: Arc::new(DashMap::new()),
        })
    }

    /// Get the HMAC secret as an Option<&str> for passing to hash_api_key.
    fn hmac_secret(&self) -> Option<&str> {
        self.hmac_secret.as_deref()
    }

    /// Create a new API key
    pub async fn create_key(
        &self,
        user_id: Option<Uuid>,
        team_id: Option<Uuid>,
        name: String,
        permissions: Vec<String>,
    ) -> Result<(ApiKey, String)> {
        validate_create_key_input(&name, &permissions)?;

        info!("Creating API key: {}", name);

        // Generate API key
        let raw_key = generate_api_key();
        let key_hash = hash_api_key(&raw_key, self.hmac_secret());
        let key_prefix = extract_api_key_prefix(&raw_key);

        // Create API key object
        let api_key = ApiKey {
            metadata: Metadata::new(),
            name,
            key_hash,
            key_prefix,
            user_id,
            team_id,
            permissions,
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        };

        // Store in database
        let stored_key = self.storage.db().create_api_key(&api_key).await?;

        info!("API key created successfully: {}", stored_key.metadata.id);
        Ok((stored_key, raw_key))
    }

    /// Create API key with full options
    pub async fn create_key_with_options(
        &self,
        request: CreateApiKeyRequest,
    ) -> Result<(ApiKey, String)> {
        validate_create_key_input(&request.name, &request.permissions)?;

        info!("Creating API key with options: {}", request.name);

        // Generate API key
        let raw_key = generate_api_key();
        let key_hash = hash_api_key(&raw_key, self.hmac_secret());
        let key_prefix = extract_api_key_prefix(&raw_key);

        // Create API key object
        let api_key = ApiKey {
            metadata: Metadata::new(),
            name: request.name,
            key_hash,
            key_prefix,
            user_id: request.user_id,
            team_id: request.team_id,
            permissions: request.permissions,
            rate_limits: request.rate_limits,
            expires_at: request.expires_at,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        };

        // Store in database
        let stored_key = self.storage.db().create_api_key(&api_key).await?;

        info!("API key created successfully: {}", stored_key.metadata.id);
        Ok((stored_key, raw_key))
    }

    /// Look up an API key by hash, checking Redis cache first and falling back
    /// to PostgreSQL. On a cache miss the result is populated into Redis with a
    /// 5-minute TTL.
    async fn find_api_key_cached(&self, key_hash: &str) -> Result<Option<ApiKey>> {
        let cache_key = api_key_cache_key(key_hash);

        // 1. Try Redis cache
        match self.storage.cache_get(&cache_key).await {
            Ok(Some(cached)) => {
                debug!("API key cache hit");
                match serde_json::from_str::<ApiKey>(&cached) {
                    Ok(api_key) => return Ok(Some(api_key)),
                    Err(e) => {
                        warn!(
                            "Failed to deserialize cached API key, falling back to DB: {}",
                            e
                        );
                        // Stale/corrupt entry – delete and continue to DB
                        if let Err(del_err) = self.storage.cache_delete(&cache_key).await {
                            warn!("Failed to delete corrupt API key cache entry: {}", del_err);
                        }
                    }
                }
            }
            Ok(None) => {
                debug!("API key cache miss");
            }
            Err(e) => {
                // Redis unavailable – degrade gracefully
                warn!("Redis cache_get failed, falling back to DB: {}", e);
            }
        }

        // 2. Fall back to PostgreSQL
        let api_key = self.storage.db().find_api_key_by_hash(key_hash).await?;

        // 3. Populate cache on DB hit
        if let Some(ref key) = api_key
            && let Ok(serialized) = serde_json::to_string(key)
            && let Err(e) = self
                .storage
                .cache_set(&cache_key, &serialized, Some(API_KEY_CACHE_TTL))
                .await
        {
            warn!("Failed to populate API key cache: {}", e);
        }

        Ok(api_key)
    }

    /// Invalidate the Redis cache entry for the given key hash.
    pub(super) async fn invalidate_api_key_cache(&self, key_hash: &str) {
        let cache_key = api_key_cache_key(key_hash);
        if let Err(e) = self.storage.cache_delete(&cache_key).await {
            warn!(
                "Failed to invalidate API key cache for {}: {}",
                cache_key, e
            );
        }
    }

    /// Verify an API key
    pub async fn verify_key(&self, raw_key: &str) -> Result<Option<(ApiKey, Option<User>)>> {
        debug!("Verifying API key");

        // Hash the provided key
        let key_hash = hash_api_key(raw_key, self.hmac_secret());

        // Find API key (cache-aside: Redis → PostgreSQL → populate Redis)
        let api_key = match self.find_api_key_cached(&key_hash).await? {
            Some(key) => key,
            None => {
                debug!("API key not found");
                return Ok(None);
            }
        };

        // Check if key is active
        if !api_key.is_active {
            debug!("API key is inactive");
            return Ok(None);
        }

        // Check if key is expired
        if let Some(expires_at) = api_key.expires_at
            && Utc::now() > expires_at
        {
            debug!("API key is expired");
            return Ok(None);
        }

        // Get associated user if any
        let user = if let Some(user_id) = api_key.user_id {
            self.storage.db().find_user_by_id(user_id).await?
        } else {
            None
        };

        // Update last used timestamp
        self.update_last_used(api_key.metadata.id).await?;

        debug!("API key verified successfully");
        Ok(Some((api_key, user)))
    }

    /// Verify API key with detailed result
    pub async fn verify_key_detailed(&self, raw_key: &str) -> Result<ApiKeyVerification> {
        let key_hash = hash_api_key(raw_key, self.hmac_secret());

        let api_key = match self.find_api_key_cached(&key_hash).await? {
            Some(key) => key,
            None => {
                return Ok(ApiKeyVerification {
                    api_key: ApiKey {
                        metadata: Metadata::new(),
                        name: "".to_string(),
                        key_hash: "".to_string(),
                        key_prefix: "".to_string(),
                        user_id: None,
                        team_id: None,
                        permissions: vec![],
                        rate_limits: None,
                        expires_at: None,
                        is_active: false,
                        last_used_at: None,
                        usage_stats: UsageStats::default(),
                    },
                    user: None,
                    is_valid: false,
                    invalid_reason: Some("API key not found".to_string()),
                });
            }
        };

        // Check if key is active
        if !api_key.is_active {
            return Ok(ApiKeyVerification {
                api_key,
                user: None,
                is_valid: false,
                invalid_reason: Some("API key is inactive".to_string()),
            });
        }

        // Check if key is expired
        if let Some(expires_at) = api_key.expires_at
            && Utc::now() > expires_at
        {
            return Ok(ApiKeyVerification {
                api_key,
                user: None,
                is_valid: false,
                invalid_reason: Some("API key is expired".to_string()),
            });
        }

        // Get associated user if any
        let user = if let Some(user_id) = api_key.user_id {
            self.storage.db().find_user_by_id(user_id).await?
        } else {
            None
        };

        // Check if user is active (if associated)
        if let Some(ref user) = user
            && !user.is_active()
        {
            return Ok(ApiKeyVerification {
                api_key,
                user: Some(user.clone()),
                is_valid: false,
                invalid_reason: Some("Associated user is inactive".to_string()),
            });
        }

        // Update last used timestamp
        self.update_last_used(api_key.metadata.id).await?;

        Ok(ApiKeyVerification {
            api_key,
            user,
            is_valid: true,
            invalid_reason: None,
        })
    }

    /// Update last used timestamp, throttled to at most once per 5 minutes per key.
    pub(super) async fn update_last_used(&self, key_id: Uuid) -> Result<()> {
        let now = Instant::now();

        // Skip the DB write if we persisted this key's last_used within the throttle window.
        if let Some(last_persisted) = self.last_used_cache.get(&key_id)
            && now.duration_since(*last_persisted) < LAST_USED_THROTTLE
        {
            return Ok(());
        }

        self.last_used_cache.insert(key_id, now);

        // Use a background task to avoid blocking the request
        let storage = self.storage.clone();
        tokio::spawn(async move {
            if let Err(e) = storage.db().update_api_key_last_used(key_id).await {
                warn!("Failed to update API key last used timestamp: {}", e);
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== validate_create_key_input ====================

    #[test]
    fn test_valid_name_and_permissions() {
        let result = validate_create_key_input("My API Key", &["api.chat".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_name_is_rejected() {
        let result = validate_create_key_input("", &[]);
        assert!(matches!(result, Err(GatewayError::Validation(_))));
        if let Err(GatewayError::Validation(msg)) = result {
            assert!(msg.contains("empty"), "unexpected message: {msg}");
        }
    }

    #[test]
    fn test_name_at_max_length_is_accepted() {
        let name = "a".repeat(255);
        let result = validate_create_key_input(&name, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_name_exceeding_max_length_is_rejected() {
        let name = "a".repeat(256);
        let result = validate_create_key_input(&name, &[]);
        assert!(matches!(result, Err(GatewayError::Validation(_))));
        if let Err(GatewayError::Validation(msg)) = result {
            assert!(msg.contains("255"), "expected '255' in message: {msg}");
        }
    }

    #[test]
    fn test_name_with_control_character_is_rejected() {
        let name = "bad\x00name";
        let result = validate_create_key_input(name, &[]);
        assert!(matches!(result, Err(GatewayError::Validation(_))));
        if let Err(GatewayError::Validation(msg)) = result {
            assert!(
                msg.contains("control"),
                "expected 'control' in message: {msg}"
            );
        }
    }

    #[test]
    fn test_name_with_newline_is_rejected() {
        let result = validate_create_key_input("bad\nname", &[]);
        assert!(matches!(result, Err(GatewayError::Validation(_))));
    }

    #[test]
    fn test_empty_permissions_are_accepted() {
        let result = validate_create_key_input("My Key", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wildcard_permission_is_accepted() {
        let result = validate_create_key_input("My Key", &["*".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_all_known_permissions_are_accepted() {
        let perms: Vec<String> = VALID_PERMISSIONS.iter().map(|p| p.to_string()).collect();
        let result = validate_create_key_input("My Key", &perms);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unknown_permission_is_rejected() {
        let result = validate_create_key_input("My Key", &["invalid.perm".to_string()]);
        assert!(matches!(result, Err(GatewayError::Validation(_))));
        if let Err(GatewayError::Validation(msg)) = result {
            assert!(
                msg.contains("invalid.perm"),
                "expected permission name in message: {msg}"
            );
        }
    }

    #[test]
    fn test_mixed_valid_and_invalid_permissions_are_rejected() {
        let perms = vec!["api.chat".to_string(), "not.a.perm".to_string()];
        let result = validate_create_key_input("My Key", &perms);
        assert!(matches!(result, Err(GatewayError::Validation(_))));
    }
}
