//! API key management operations
//!
//! This module provides methods for managing API keys (update, revoke, list, etc.).

use super::creation::ApiKeyHandler;
use super::types::CreateApiKeyRequest;
use crate::core::models::{ApiKey, RateLimits, UsageStats};
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};
use uuid::Uuid;

impl ApiKeyHandler {
    /// Look up an API key by ID and invalidate its Redis cache entry.
    /// Errors are logged but not propagated so the caller's primary
    /// mutation is never blocked by a cache failure.
    async fn invalidate_cache_for_key_id(&self, key_id: Uuid) {
        match self.storage.db().find_api_key_by_id(key_id).await {
            Ok(Some(key)) => self.invalidate_api_key_cache(&key.key_hash).await,
            Ok(None) => {
                debug!("No API key found for cache invalidation: {}", key_id);
            }
            Err(e) => {
                warn!("Failed to look up API key for cache invalidation: {}", e);
            }
        }
    }

    /// Revoke an API key
    pub async fn revoke_key(&self, key_id: Uuid) -> Result<()> {
        info!("Revoking API key: {}", key_id);

        // Fetch hash before mutation so we can invalidate cache
        self.invalidate_cache_for_key_id(key_id).await;

        self.storage.db().deactivate_api_key(key_id).await?;

        info!("API key revoked successfully: {}", key_id);
        Ok(())
    }

    /// List API keys for a user
    pub async fn list_user_keys(&self, user_id: Uuid) -> Result<Vec<ApiKey>> {
        debug!("Listing API keys for user: {}", user_id);
        self.storage.db().list_api_keys_by_user(user_id).await
    }

    /// List API keys for a team
    pub async fn list_team_keys(&self, team_id: Uuid) -> Result<Vec<ApiKey>> {
        debug!("Listing API keys for team: {}", team_id);
        self.storage.db().list_api_keys_by_team(team_id).await
    }

    /// Update API key permissions
    pub async fn update_permissions(&self, key_id: Uuid, permissions: Vec<String>) -> Result<()> {
        info!("Updating permissions for API key: {}", key_id);

        self.storage
            .db()
            .update_api_key_permissions(key_id, &permissions)
            .await?;

        self.invalidate_cache_for_key_id(key_id).await;

        info!("API key permissions updated successfully: {}", key_id);
        Ok(())
    }

    /// Update API key rate limits
    pub async fn update_rate_limits(
        &self,
        key_id: Uuid,
        rate_limits: Option<RateLimits>,
    ) -> Result<()> {
        info!("Updating rate limits for API key: {}", key_id);

        if let Some(ref limits) = rate_limits {
            self.storage
                .db()
                .update_api_key_rate_limits(key_id, limits)
                .await?;
        }

        self.invalidate_cache_for_key_id(key_id).await;

        info!("API key rate limits updated successfully: {}", key_id);
        Ok(())
    }

    /// Update API key expiration
    pub async fn update_expiration(
        &self,
        key_id: Uuid,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        info!("Updating expiration for API key: {}", key_id);

        self.storage
            .db()
            .update_api_key_expiration(key_id, expires_at)
            .await?;

        self.invalidate_cache_for_key_id(key_id).await;

        info!("API key expiration updated successfully: {}", key_id);
        Ok(())
    }

    /// Record API key usage
    pub async fn record_usage(
        &self,
        key_id: Uuid,
        requests: u64,
        tokens: u64,
        cost: f64,
    ) -> Result<()> {
        debug!("Recording usage for API key: {}", key_id);

        self.storage
            .db()
            .update_api_key_usage(key_id, requests, tokens, cost)
            .await?;

        // Invalidate cached API key so subsequent reads see updated usage stats
        self.invalidate_cache_for_key_id(key_id).await;

        Ok(())
    }

    /// Get API key usage statistics
    pub async fn get_usage_stats(&self, key_id: Uuid) -> Result<UsageStats> {
        debug!("Getting usage stats for API key: {}", key_id);

        let api_key = self
            .storage
            .db()
            .find_api_key_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        Ok(api_key.usage_stats)
    }

    /// Get API key by ID
    pub async fn get_key(&self, key_id: Uuid) -> Result<Option<ApiKey>> {
        self.storage.db().find_api_key_by_id(key_id).await
    }

    /// Regenerate API key (creates new key, deactivates old one)
    pub async fn regenerate_key(&self, key_id: Uuid) -> Result<(ApiKey, String)> {
        info!("Regenerating API key: {}", key_id);

        // Get existing key
        let old_key = self
            .storage
            .db()
            .find_api_key_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        // Create new key with same properties
        let request = CreateApiKeyRequest {
            name: old_key.name.clone(),
            user_id: old_key.user_id,
            team_id: old_key.team_id,
            permissions: old_key.permissions.clone(),
            rate_limits: old_key.rate_limits.clone(),
            expires_at: old_key.expires_at,
        };

        let (new_key, raw_key) = self.create_key_with_options(request).await?;

        // Deactivate old key
        self.revoke_key(key_id).await?;

        info!(
            "API key regenerated successfully: {} -> {}",
            key_id, new_key.metadata.id
        );
        Ok((new_key, raw_key))
    }

    /// Cleanup expired API keys
    pub async fn cleanup_expired_keys(&self) -> Result<u64> {
        info!("Cleaning up expired API keys");

        // Delete expired API keys from database
        let count = self.storage.db().delete_expired_api_keys().await?;

        info!("Cleaned up {} expired API keys", count);
        Ok(count)
    }
}
