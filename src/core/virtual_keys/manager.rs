//! Virtual key manager implementation

use super::requests::{CreateKeyRequest, UpdateKeyRequest};
use super::types::{KeyGenerationSettings, RateLimitState, VirtualKey};
use crate::storage::database::Database;
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};
use uuid::Uuid;

/// Consolidated key manager data - single lock for cache and rate limiting
#[derive(Debug, Default)]
struct KeyManagerData {
    /// In-memory cache for frequently accessed keys
    cache: HashMap<String, VirtualKey>,
    /// Rate limiting tracker
    rate_limits: HashMap<String, RateLimitState>,
}

/// Virtual key manager
pub struct VirtualKeyManager {
    /// Database connection
    database: Arc<Database>,
    /// Consolidated key data - single lock for cache and rate limiting
    key_data: Arc<RwLock<KeyManagerData>>,
    /// Key generation settings
    key_settings: KeyGenerationSettings,
}

impl VirtualKeyManager {
    /// Create a new virtual key manager
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        Ok(Self {
            database,
            key_data: Arc::new(RwLock::new(KeyManagerData::default())),
            key_settings: KeyGenerationSettings::default(),
        })
    }

    /// Create a new virtual key
    pub async fn create_key(&self, request: CreateKeyRequest) -> Result<(String, VirtualKey)> {
        info!("Creating virtual key for user: {}", request.user_id);

        // Generate new API key
        let api_key = self.generate_api_key();
        let key_hash = self.hash_key(&api_key);

        // Create virtual key
        let virtual_key = VirtualKey {
            key_id: Uuid::new_v4().to_string(),
            key_hash: key_hash.clone(),
            key_alias: request.key_alias,
            user_id: request.user_id,
            team_id: request.team_id,
            organization_id: None,
            models: request.models,
            max_budget: request.max_budget.or(self.key_settings.default_budget),
            spend: 0.0,
            budget_duration: request.budget_duration.clone(),
            budget_reset_at: self.calculate_budget_reset(&request.budget_duration),
            rate_limits: request
                .rate_limits
                .or(self.key_settings.default_rate_limits.clone()),
            permissions: if request.permissions.is_empty() {
                self.key_settings.default_permissions.clone()
            } else {
                request.permissions
            },
            metadata: request.metadata,
            expires_at: request.expires_at,
            is_active: true,
            created_at: Utc::now(),
            last_used_at: None,
            usage_count: 0,
            tags: request.tags,
        };

        // Store in database
        self.database.store_virtual_key(&virtual_key).await?;

        // Cache the key
        {
            let mut data = self.key_data.write().await;
            data.cache.insert(key_hash, virtual_key.clone());
        }

        info!("Virtual key created successfully: {}", virtual_key.key_id);
        Ok((api_key, virtual_key))
    }

    /// Validate and retrieve virtual key
    pub async fn validate_key(&self, api_key: &str) -> Result<VirtualKey> {
        let key_hash = self.hash_key(api_key);

        // Check cache first
        {
            let data = self.key_data.read().await;
            if let Some(key) = data.cache.get(&key_hash) {
                if self.is_key_valid(key) {
                    return Ok(key.clone());
                }
            }
        }

        // Load from database
        let mut virtual_key = self
            .database
            .get_virtual_key(&key_hash)
            .await?
            .ok_or_else(|| GatewayError::Auth("Invalid API key".to_string()))?;

        // Validate key
        if !self.is_key_valid(&virtual_key) {
            return Err(GatewayError::Auth(
                "API key is expired or inactive".to_string(),
            ));
        }

        // Update last used
        virtual_key.last_used_at = Some(Utc::now());
        virtual_key.usage_count += 1;

        // Update in database (async)
        let db = self.database.clone();
        let key_for_update = virtual_key.clone();
        tokio::spawn(async move {
            if let Err(e) = db.update_virtual_key_usage(&key_for_update).await {
                error!("Failed to update key usage: {}", e);
            }
        });

        // Update cache
        {
            let mut data = self.key_data.write().await;
            data.cache.insert(key_hash, virtual_key.clone());
        }

        Ok(virtual_key)
    }

    /// Check rate limits for a key
    pub async fn check_rate_limits(
        &self,
        key: &VirtualKey,
        tokens_requested: u32,
    ) -> Result<()> {
        if let Some(rate_limits) = &key.rate_limits {
            let mut data = self.key_data.write().await;
            let state = data
                .rate_limits
                .entry(key.key_id.clone())
                .or_insert_with(|| RateLimitState {
                    request_count: 0,
                    token_count: 0,
                    window_start: Utc::now(),
                    parallel_requests: 0,
                });

            let now = Utc::now();

            // Reset window if needed (1 minute window)
            if now.signed_duration_since(state.window_start) > Duration::minutes(1) {
                state.request_count = 0;
                state.token_count = 0;
                state.window_start = now;
            }

            // Check RPM
            if let Some(rpm) = rate_limits.rpm {
                if state.request_count >= rpm {
                    return Err(GatewayError::RateLimit(format!(
                        "Rate limit exceeded: {} requests per minute",
                        rpm
                    )));
                }
            }

            // Check TPM
            if let Some(tpm) = rate_limits.tpm {
                if state.token_count + tokens_requested > tpm {
                    return Err(GatewayError::RateLimit(format!(
                        "Token rate limit exceeded: {} tokens per minute",
                        tpm
                    )));
                }
            }

            // Check parallel requests
            if let Some(max_parallel) = rate_limits.max_parallel_requests {
                if state.parallel_requests >= max_parallel {
                    return Err(GatewayError::RateLimit(format!(
                        "Too many parallel requests: max {}",
                        max_parallel
                    )));
                }
            }

            // Update counters
            state.request_count += 1;
            state.token_count += tokens_requested;
            state.parallel_requests += 1;
        }

        Ok(())
    }

    /// Record request completion (for parallel request tracking)
    pub async fn record_request_completion(&self, key_id: &str) {
        let mut data = self.key_data.write().await;
        if let Some(state) = data.rate_limits.get_mut(key_id) {
            if state.parallel_requests > 0 {
                state.parallel_requests -= 1;
            }
        }
    }

    /// Check budget limits
    pub async fn check_budget(&self, key: &VirtualKey, cost: f64) -> Result<()> {
        if let Some(max_budget) = key.max_budget {
            if key.spend + cost > max_budget {
                return Err(GatewayError::BudgetExceeded(format!(
                    "Budget exceeded: ${:.2} + ${:.2} > ${:.2}",
                    key.spend, cost, max_budget
                )));
            }
        }
        Ok(())
    }

    /// Update key spend
    pub async fn update_spend(&self, key_id: &str, cost: f64) -> Result<()> {
        self.database.update_key_spend(key_id, cost).await?;

        // Update cache
        {
            let mut data = self.key_data.write().await;
            for (_, key) in data.cache.iter_mut() {
                if key.key_id == key_id {
                    key.spend += cost;
                    break;
                }
            }
        }

        Ok(())
    }

    /// List keys for a user
    pub async fn list_user_keys(&self, user_id: &str) -> Result<Vec<VirtualKey>> {
        self.database.list_user_keys(user_id).await
    }

    /// Update virtual key
    pub async fn update_key(&self, key_id: &str, request: UpdateKeyRequest) -> Result<VirtualKey> {
        let mut key = self
            .database
            .get_virtual_key_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::NotFound("Virtual key not found".to_string()))?;

        // Update fields
        if let Some(alias) = request.key_alias {
            key.key_alias = Some(alias);
        }
        if let Some(models) = request.models {
            key.models = models;
        }
        if let Some(budget) = request.max_budget {
            key.max_budget = Some(budget);
        }
        if let Some(duration) = request.budget_duration {
            key.budget_duration = Some(duration.clone());
            key.budget_reset_at = self.calculate_budget_reset(&Some(duration));
        }
        if let Some(rate_limits) = request.rate_limits {
            key.rate_limits = Some(rate_limits);
        }
        if let Some(permissions) = request.permissions {
            key.permissions = permissions;
        }
        if let Some(metadata) = request.metadata {
            key.metadata = metadata;
        }
        if let Some(expires_at) = request.expires_at {
            key.expires_at = Some(expires_at);
        }
        if let Some(is_active) = request.is_active {
            key.is_active = is_active;
        }
        if let Some(tags) = request.tags {
            key.tags = tags;
        }

        // Update in database
        self.database.update_virtual_key(&key).await?;

        // Update cache
        {
            let mut data = self.key_data.write().await;
            data.cache.insert(key.key_hash.clone(), key.clone());
        }

        Ok(key)
    }

    /// Delete virtual key
    pub async fn delete_key(&self, key_id: &str) -> Result<()> {
        let key = self
            .database
            .get_virtual_key_by_id(key_id)
            .await?
            .ok_or_else(|| GatewayError::NotFound("Virtual key not found".to_string()))?;

        // Delete from database
        self.database.delete_virtual_key(key_id).await?;

        // Remove from cache and rate limits
        {
            let mut data = self.key_data.write().await;
            data.cache.remove(&key.key_hash);
            data.rate_limits.remove(key_id);
        }

        info!("Virtual key deleted: {}", key_id);
        Ok(())
    }

    /// Generate a new API key
    fn generate_api_key(&self) -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::rng();

        let random_string: String = (0..self.key_settings.key_length)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();

        format!("{}{}", self.key_settings.key_prefix, random_string)
    }

    /// Hash an API key
    fn hash_key(&self, key: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Check if a key is valid
    fn is_key_valid(&self, key: &VirtualKey) -> bool {
        if !key.is_active {
            return false;
        }

        if let Some(expires_at) = key.expires_at {
            if Utc::now() > expires_at {
                return false;
            }
        }

        true
    }

    /// Calculate budget reset time
    fn calculate_budget_reset(&self, duration: &Option<String>) -> Option<chrono::DateTime<Utc>> {
        duration.as_ref().and_then(|d| {
            let now = Utc::now();
            match d.as_str() {
                "1d" => Some(now + Duration::days(1)),
                "1w" => Some(now + Duration::weeks(1)),
                "1m" => Some(now + Duration::days(30)),
                _ => None,
            }
        })
    }

    /// Reset budgets for expired keys
    pub async fn reset_expired_budgets(&self) -> Result<()> {
        let keys_to_reset = self.database.get_keys_with_expired_budgets().await?;

        for mut key in keys_to_reset {
            key.spend = 0.0;
            key.budget_reset_at = self.calculate_budget_reset(&key.budget_duration);

            self.database.update_virtual_key(&key).await?;

            // Update cache
            {
                let mut data = self.key_data.write().await;
                data.cache.insert(key.key_hash.clone(), key);
            }
        }

        Ok(())
    }
}
