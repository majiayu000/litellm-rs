//! API Key Manager
//!
//! This module provides the main API key management functionality.

use super::repository::KeyRepository;
use super::types::{
    CreateKeyConfig, KeyInfo, KeyStatus, KeyUsageStats, ManagedApiKey, UpdateKeyConfig,
    VerifyKeyResult,
};
use crate::utils::auth::crypto::keys::{extract_api_key_prefix, generate_api_key, hash_api_key};
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// API Key Manager for handling all key operations
#[derive(Clone)]
pub struct KeyManager {
    /// Repository for key storage
    repository: Arc<dyn KeyRepository>,
}

impl std::fmt::Debug for KeyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyManager")
            .field("repository", &"<KeyRepository>")
            .finish()
    }
}

impl KeyManager {
    /// Create a new KeyManager with the given repository
    pub fn new<R: KeyRepository + 'static>(repository: R) -> Self {
        Self {
            repository: Arc::new(repository),
        }
    }

    /// Create a new KeyManager with an Arc repository
    pub fn with_arc_repository(repository: Arc<dyn KeyRepository>) -> Self {
        Self { repository }
    }

    /// Generate a new API key
    ///
    /// Returns a tuple of (key_id, raw_key). The raw_key should be shown to the user
    /// only once and never stored. Only the hash is stored.
    pub async fn generate_key(&self, config: CreateKeyConfig) -> Result<(Uuid, String)> {
        info!("Generating new API key: {}", config.name);

        // Validate configuration
        self.validate_create_config(&config)?;

        // Generate the raw key
        let raw_key = generate_api_key();
        let key_hash = hash_api_key(&raw_key);
        let key_prefix = extract_api_key_prefix(&raw_key);

        let now = Utc::now();

        // Create the managed key
        let managed_key = ManagedApiKey {
            id: Uuid::new_v4(),
            key_hash,
            key_prefix,
            name: config.name,
            description: config.description,
            user_id: config.user_id,
            team_id: config.team_id,
            budget_id: config.budget_id,
            permissions: config.permissions,
            rate_limits: config.rate_limits,
            status: KeyStatus::Active,
            expires_at: config.expires_at,
            created_at: now,
            updated_at: now,
            last_used_at: None,
            usage_stats: KeyUsageStats::new(),
            metadata: config.metadata,
        };

        let key_id = managed_key.id;

        // Store the key
        self.repository.create(managed_key).await?;

        info!("API key generated successfully: {}", key_id);

        // Return key_id and raw_key (raw_key should only be shown once)
        Ok((key_id, raw_key))
    }

    /// Validate a raw API key
    ///
    /// Returns verification result with key info if valid.
    pub async fn validate_key(&self, raw_key: &str) -> Result<VerifyKeyResult> {
        debug!("Validating API key");

        // Hash the provided key
        let key_hash = hash_api_key(raw_key);

        // Find key by hash
        let key = match self.repository.find_by_hash(&key_hash).await? {
            Some(k) => k,
            None => {
                debug!("API key not found");
                return Ok(VerifyKeyResult {
                    valid: false,
                    key: None,
                    invalid_reason: Some("API key not found".to_string()),
                });
            }
        };

        // Check if key is active
        if key.status == KeyStatus::Revoked {
            debug!("API key is revoked");
            return Ok(VerifyKeyResult {
                valid: false,
                key: Some(KeyInfo::from(&key)),
                invalid_reason: Some("API key has been revoked".to_string()),
            });
        }

        // Check expiration
        if let Some(expires_at) = key.expires_at
            && Utc::now() > expires_at
        {
            debug!("API key is expired");
            return Ok(VerifyKeyResult {
                valid: false,
                key: Some(KeyInfo::from(&key)),
                invalid_reason: Some("API key has expired".to_string()),
            });
        }

        // Update last used (fire and forget)
        let repo = self.repository.clone();
        let key_id = key.id;
        tokio::spawn(async move {
            if let Err(e) = repo.update_last_used(key_id).await {
                warn!("Failed to update last used timestamp: {}", e);
            }
        });

        debug!("API key validated successfully");
        Ok(VerifyKeyResult {
            valid: true,
            key: Some(KeyInfo::from(&key)),
            invalid_reason: None,
        })
    }

    /// Revoke an API key
    pub async fn revoke_key(&self, key_id: Uuid) -> Result<()> {
        info!("Revoking API key: {}", key_id);

        // Verify key exists
        let key = self
            .repository
            .find_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        if key.status == KeyStatus::Revoked {
            return Err(GatewayError::conflict("API key is already revoked"));
        }

        self.repository
            .update_status(key_id, KeyStatus::Revoked)
            .await?;

        info!("API key revoked successfully: {}", key_id);
        Ok(())
    }

    /// Rotate an API key (generate new key, revoke old one)
    ///
    /// Returns a tuple of (new_key_id, new_raw_key). The new_raw_key should be shown
    /// to the user only once.
    pub async fn rotate_key(&self, key_id: Uuid) -> Result<(Uuid, String)> {
        info!("Rotating API key: {}", key_id);

        // Get existing key
        let old_key = self
            .repository
            .find_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        if old_key.status == KeyStatus::Revoked {
            return Err(GatewayError::conflict("Cannot rotate a revoked key"));
        }

        // Create new key with same configuration
        let config = CreateKeyConfig {
            name: format!("{} (rotated)", old_key.name),
            description: old_key.description,
            user_id: old_key.user_id,
            team_id: old_key.team_id,
            budget_id: old_key.budget_id,
            permissions: old_key.permissions,
            rate_limits: old_key.rate_limits,
            expires_at: old_key.expires_at,
            metadata: old_key.metadata,
        };

        let (new_key_id, new_raw_key) = self.generate_key(config).await?;

        // Revoke the old key
        self.repository
            .update_status(key_id, KeyStatus::Revoked)
            .await?;

        info!("API key rotated successfully: {} -> {}", key_id, new_key_id);
        Ok((new_key_id, new_raw_key))
    }

    /// Update an API key's configuration
    pub async fn update_key(&self, key_id: Uuid, config: UpdateKeyConfig) -> Result<KeyInfo> {
        info!("Updating API key: {}", key_id);

        // Verify key exists and is not revoked
        let existing = self
            .repository
            .find_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        if existing.status == KeyStatus::Revoked {
            return Err(GatewayError::conflict("Cannot update a revoked key"));
        }

        let updated = self.repository.update(key_id, config).await?;

        info!("API key updated successfully: {}", key_id);
        Ok(KeyInfo::from(&updated))
    }

    /// Get key information by ID
    pub async fn get_key(&self, key_id: Uuid) -> Result<Option<KeyInfo>> {
        let key = self.repository.find_by_id(key_id).await?;
        Ok(key.map(|k| KeyInfo::from(&k)))
    }

    /// Get key usage statistics
    pub async fn get_usage_stats(&self, key_id: Uuid) -> Result<KeyUsageStats> {
        let key = self
            .repository
            .find_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        Ok(key.usage_stats)
    }

    /// Record usage for a key
    pub async fn record_usage(&self, key_id: Uuid, tokens: u64, cost: f64) -> Result<()> {
        self.repository.update_usage(key_id, tokens, cost).await
    }

    /// List keys for a user
    pub async fn list_user_keys(&self, user_id: Uuid) -> Result<Vec<KeyInfo>> {
        let keys = self.repository.list_by_user(user_id).await?;
        Ok(keys.iter().map(KeyInfo::from).collect())
    }

    /// List keys for a team
    pub async fn list_team_keys(&self, team_id: Uuid) -> Result<Vec<KeyInfo>> {
        let keys = self.repository.list_by_team(team_id).await?;
        Ok(keys.iter().map(KeyInfo::from).collect())
    }

    /// List all keys with optional filtering
    pub async fn list_keys(
        &self,
        status: Option<KeyStatus>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<KeyInfo>> {
        let keys = self.repository.list_all(status, limit, offset).await?;
        Ok(keys.iter().map(KeyInfo::from).collect())
    }

    /// Delete a key permanently (use with caution)
    pub async fn delete_key(&self, key_id: Uuid) -> Result<()> {
        info!("Deleting API key permanently: {}", key_id);

        // Verify key exists
        self.repository
            .find_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        self.repository.delete(key_id).await?;

        info!("API key deleted permanently: {}", key_id);
        Ok(())
    }

    /// Cleanup expired keys
    pub async fn cleanup_expired_keys(&self) -> Result<u64> {
        info!("Cleaning up expired API keys");
        let deleted = self.repository.delete_expired().await?;
        info!("Cleaned up {} expired API keys", deleted);
        Ok(deleted)
    }

    /// Count keys
    pub async fn count_keys(&self, status: Option<KeyStatus>) -> Result<u64> {
        self.repository.count(status).await
    }

    /// Validate create configuration
    fn validate_create_config(&self, config: &CreateKeyConfig) -> Result<()> {
        if config.name.is_empty() {
            return Err(GatewayError::validation("Key name cannot be empty"));
        }

        if config.name.len() > 255 {
            return Err(GatewayError::validation(
                "Key name cannot exceed 255 characters",
            ));
        }

        if let Some(ref desc) = config.description
            && desc.len() > 1000
        {
            return Err(GatewayError::validation(
                "Key description cannot exceed 1000 characters",
            ));
        }

        // Check expiration is in the future
        if let Some(expires_at) = config.expires_at
            && expires_at <= Utc::now()
        {
            return Err(GatewayError::validation(
                "Expiration date must be in the future",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod manager_tests {
    use super::*;
    use crate::core::keys::repository::InMemoryKeyRepository;
    use crate::core::keys::types::{KeyPermissions, KeyRateLimits};

    fn create_manager() -> KeyManager {
        KeyManager::new(InMemoryKeyRepository::new())
    }

    #[tokio::test]
    async fn test_generate_key() {
        let manager = create_manager();

        let config = CreateKeyConfig {
            name: "Test Key".to_string(),
            ..Default::default()
        };

        let (key_id, raw_key) = manager.generate_key(config).await.unwrap();

        assert!(!raw_key.is_empty());
        assert!(raw_key.starts_with("gw-"));

        // Verify key was stored
        let key_info = manager.get_key(key_id).await.unwrap();
        assert!(key_info.is_some());
        assert_eq!(key_info.unwrap().name, "Test Key");
    }

    #[tokio::test]
    async fn test_validate_key() {
        let manager = create_manager();

        let config = CreateKeyConfig {
            name: "Validation Test".to_string(),
            ..Default::default()
        };

        let (_, raw_key) = manager.generate_key(config).await.unwrap();

        let result = manager.validate_key(&raw_key).await.unwrap();
        assert!(result.valid);
        assert!(result.key.is_some());
    }

    #[tokio::test]
    async fn test_validate_invalid_key() {
        let manager = create_manager();

        let result = manager.validate_key("invalid-key").await.unwrap();
        assert!(!result.valid);
        assert!(result.invalid_reason.is_some());
    }

    #[tokio::test]
    async fn test_revoke_key() {
        let manager = create_manager();

        let config = CreateKeyConfig {
            name: "Revoke Test".to_string(),
            ..Default::default()
        };

        let (key_id, raw_key) = manager.generate_key(config).await.unwrap();

        manager.revoke_key(key_id).await.unwrap();

        // Key should no longer be valid
        let result = manager.validate_key(&raw_key).await.unwrap();
        assert!(!result.valid);
        assert!(result.invalid_reason.as_ref().unwrap().contains("revoked"));
    }

    #[tokio::test]
    async fn test_rotate_key() {
        let manager = create_manager();

        let config = CreateKeyConfig {
            name: "Rotate Test".to_string(),
            permissions: KeyPermissions::full_access(),
            rate_limits: KeyRateLimits::standard(),
            ..Default::default()
        };

        let (old_key_id, old_raw_key) = manager.generate_key(config).await.unwrap();

        let (new_key_id, new_raw_key) = manager.rotate_key(old_key_id).await.unwrap();

        // Old key should be revoked
        let old_result = manager.validate_key(&old_raw_key).await.unwrap();
        assert!(!old_result.valid);

        // New key should be valid
        let new_result = manager.validate_key(&new_raw_key).await.unwrap();
        assert!(new_result.valid);

        // New key should have same permissions
        let new_key = manager.get_key(new_key_id).await.unwrap().unwrap();
        assert!(new_key.name.contains("rotated"));
    }

    #[tokio::test]
    async fn test_update_key() {
        let manager = create_manager();

        let config = CreateKeyConfig {
            name: "Update Test".to_string(),
            ..Default::default()
        };

        let (key_id, _) = manager.generate_key(config).await.unwrap();

        let update = UpdateKeyConfig {
            name: Some("Updated Name".to_string()),
            ..Default::default()
        };

        let updated = manager.update_key(key_id, update).await.unwrap();
        assert_eq!(updated.name, "Updated Name");
    }

    #[tokio::test]
    async fn test_list_user_keys() {
        let manager = create_manager();
        let user_id = Uuid::new_v4();

        for i in 0..3 {
            let config = CreateKeyConfig {
                name: format!("User Key {}", i),
                user_id: Some(user_id),
                ..Default::default()
            };
            manager.generate_key(config).await.unwrap();
        }

        let keys = manager.list_user_keys(user_id).await.unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[tokio::test]
    async fn test_expired_key_validation() {
        let manager = create_manager();

        // We need to create a key and then manually expire it
        // Since we can't create an already-expired key, we'll update it

        let config = CreateKeyConfig {
            name: "Expiring Key".to_string(),
            expires_at: Some(Utc::now() + chrono::Duration::seconds(1)),
            ..Default::default()
        };

        let (_, raw_key) = manager.generate_key(config).await.unwrap();

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let result = manager.validate_key(&raw_key).await.unwrap();
        assert!(!result.valid);
        assert!(result.invalid_reason.as_ref().unwrap().contains("expired"));
    }

    #[tokio::test]
    async fn test_validation_empty_name() {
        let manager = create_manager();

        let config = CreateKeyConfig {
            name: "".to_string(),
            ..Default::default()
        };

        let result = manager.generate_key(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_key() {
        let manager = create_manager();

        let config = CreateKeyConfig {
            name: "Delete Test".to_string(),
            ..Default::default()
        };

        let (key_id, _) = manager.generate_key(config).await.unwrap();

        manager.delete_key(key_id).await.unwrap();

        let key = manager.get_key(key_id).await.unwrap();
        assert!(key.is_none());
    }

    #[tokio::test]
    async fn test_count_keys() {
        let manager = create_manager();

        for i in 0..5 {
            let config = CreateKeyConfig {
                name: format!("Count Key {}", i),
                ..Default::default()
            };
            manager.generate_key(config).await.unwrap();
        }

        let count = manager.count_keys(None).await.unwrap();
        assert_eq!(count, 5);
    }
}
