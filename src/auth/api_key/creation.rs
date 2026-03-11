//! API key creation and verification
//!
//! This module provides methods for creating and verifying API keys.

use super::types::{ApiKeyVerification, CreateApiKeyRequest};
use crate::core::models::user::types::User;
use crate::core::models::{ApiKey, Metadata, UsageStats};
use crate::storage::StorageLayer;
use crate::utils::auth::crypto::keys::{extract_api_key_prefix, generate_api_key, hash_api_key};
use crate::utils::error::gateway_error::Result;
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// API key handler for authentication and management
#[derive(Debug, Clone)]
pub struct ApiKeyHandler {
    /// Storage layer for persistence
    pub(super) storage: Arc<StorageLayer>,
}

impl ApiKeyHandler {
    /// Create a new API key handler
    pub async fn new(storage: Arc<StorageLayer>) -> Result<Self> {
        Ok(Self { storage })
    }

    /// Create a new API key
    pub async fn create_key(
        &self,
        user_id: Option<Uuid>,
        team_id: Option<Uuid>,
        name: String,
        permissions: Vec<String>,
    ) -> Result<(ApiKey, String)> {
        info!("Creating API key: {}", name);

        // Generate API key
        let raw_key = generate_api_key();
        let key_hash = hash_api_key(&raw_key);
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
        info!("Creating API key with options: {}", request.name);

        // Generate API key
        let raw_key = generate_api_key();
        let key_hash = hash_api_key(&raw_key);
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

    /// Verify an API key
    pub async fn verify_key(&self, raw_key: &str) -> Result<Option<(ApiKey, Option<User>)>> {
        debug!("Verifying API key");

        // Hash the provided key
        let key_hash = hash_api_key(raw_key);

        // Find API key in database
        let api_key = match self.storage.db().find_api_key_by_hash(&key_hash).await? {
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
        let key_hash = hash_api_key(raw_key);

        let api_key = match self.storage.db().find_api_key_by_hash(&key_hash).await? {
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

    /// Update last used timestamp
    pub(super) async fn update_last_used(&self, key_id: Uuid) -> Result<()> {
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
