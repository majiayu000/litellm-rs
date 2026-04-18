//! API Key Repository
//!
//! This module provides the storage abstraction for API keys.

use super::types::{KeyStatus, ManagedApiKey, UpdateKeyConfig};
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Repository trait for API key storage operations
#[async_trait]
pub trait KeyRepository: Send + Sync {
    /// Store a new API key
    async fn create(&self, key: ManagedApiKey) -> Result<ManagedApiKey>;

    /// Find a key by ID
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ManagedApiKey>>;

    /// Find a key by its hash
    async fn find_by_hash(&self, key_hash: &str) -> Result<Option<ManagedApiKey>>;

    /// Update a key
    async fn update(&self, id: Uuid, config: UpdateKeyConfig) -> Result<ManagedApiKey>;

    /// Update key status
    async fn update_status(&self, id: Uuid, status: KeyStatus) -> Result<()>;

    /// Update last used timestamp
    async fn update_last_used(&self, id: Uuid) -> Result<()>;

    /// Update usage statistics
    async fn update_usage(&self, id: Uuid, tokens: u64, cost: f64) -> Result<()>;

    /// List keys by user ID
    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<ManagedApiKey>>;

    /// List keys by team ID
    async fn list_by_team(&self, team_id: Uuid) -> Result<Vec<ManagedApiKey>>;

    /// List all keys with optional filters
    async fn list_all(
        &self,
        status: Option<KeyStatus>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ManagedApiKey>>;

    /// Delete a key permanently
    async fn delete(&self, id: Uuid) -> Result<()>;

    /// Delete expired keys
    async fn delete_expired(&self) -> Result<u64>;

    /// Count total keys
    async fn count(&self, status: Option<KeyStatus>) -> Result<u64>;
}

/// In-memory implementation of KeyRepository for testing and development
#[derive(Debug, Default)]
pub struct InMemoryKeyRepository {
    /// Storage for keys by ID
    keys_by_id: DashMap<Uuid, ManagedApiKey>,
    /// Index for looking up keys by hash
    hash_index: DashMap<String, Uuid>,
}

impl InMemoryKeyRepository {
    /// Create a new in-memory repository
    pub fn new() -> Self {
        Self {
            keys_by_id: DashMap::new(),
            hash_index: DashMap::new(),
        }
    }

    /// Create a new in-memory repository wrapped in Arc
    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self::new())
    }
}

#[async_trait]
impl KeyRepository for InMemoryKeyRepository {
    async fn create(&self, key: ManagedApiKey) -> Result<ManagedApiKey> {
        // Check if hash already exists
        if self.hash_index.contains_key(&key.key_hash) {
            return Err(GatewayError::conflict("API key already exists"));
        }

        // Store the key
        self.hash_index.insert(key.key_hash.clone(), key.id);
        self.keys_by_id.insert(key.id, key.clone());

        Ok(key)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ManagedApiKey>> {
        Ok(self.keys_by_id.get(&id).map(|r| r.value().clone()))
    }

    async fn find_by_hash(&self, key_hash: &str) -> Result<Option<ManagedApiKey>> {
        if let Some(id) = self.hash_index.get(key_hash) {
            return self.find_by_id(*id).await;
        }
        Ok(None)
    }

    async fn update(&self, id: Uuid, config: UpdateKeyConfig) -> Result<ManagedApiKey> {
        let mut entry = self
            .keys_by_id
            .get_mut(&id)
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        let key = entry.value_mut();

        if let Some(name) = config.name {
            key.name = name;
        }
        if let Some(description) = config.description {
            key.description = description;
        }
        if let Some(permissions) = config.permissions {
            key.permissions = permissions;
        }
        if let Some(rate_limits) = config.rate_limits {
            key.rate_limits = rate_limits;
        }
        if let Some(budget_id) = config.budget_id {
            key.budget_id = budget_id;
        }
        if let Some(expires_at) = config.expires_at {
            key.expires_at = expires_at;
        }
        if let Some(metadata) = config.metadata {
            key.metadata = metadata;
        }

        key.updated_at = Utc::now();

        Ok(key.clone())
    }

    async fn update_status(&self, id: Uuid, status: KeyStatus) -> Result<()> {
        let mut entry = self
            .keys_by_id
            .get_mut(&id)
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        entry.value_mut().status = status;
        entry.value_mut().updated_at = Utc::now();

        Ok(())
    }

    async fn update_last_used(&self, id: Uuid) -> Result<()> {
        if let Some(mut entry) = self.keys_by_id.get_mut(&id) {
            entry.value_mut().last_used_at = Some(Utc::now());
        }
        Ok(())
    }

    async fn update_usage(&self, id: Uuid, tokens: u64, cost: f64) -> Result<()> {
        if let Some(mut entry) = self.keys_by_id.get_mut(&id) {
            let key = entry.value_mut();
            key.usage_stats.reset_daily_if_needed();
            key.usage_stats.record_usage(tokens, cost);
            key.last_used_at = Some(Utc::now());
        }
        Ok(())
    }

    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<ManagedApiKey>> {
        let keys: Vec<ManagedApiKey> = self
            .keys_by_id
            .iter()
            .filter(|r| r.value().user_id == Some(user_id))
            .map(|r| r.value().clone())
            .collect();
        Ok(keys)
    }

    async fn list_by_team(&self, team_id: Uuid) -> Result<Vec<ManagedApiKey>> {
        let keys: Vec<ManagedApiKey> = self
            .keys_by_id
            .iter()
            .filter(|r| r.value().team_id == Some(team_id))
            .map(|r| r.value().clone())
            .collect();
        Ok(keys)
    }

    async fn list_all(
        &self,
        status: Option<KeyStatus>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ManagedApiKey>> {
        let mut keys: Vec<ManagedApiKey> = self
            .keys_by_id
            .iter()
            .filter(|r| {
                if let Some(s) = status {
                    r.value().effective_status() == s
                } else {
                    true
                }
            })
            .map(|r| r.value().clone())
            .collect();

        // Sort by created_at descending
        keys.sort_by_key(|b| std::cmp::Reverse(b.created_at));

        // Apply pagination
        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(100);

        Ok(keys.into_iter().skip(offset).take(limit).collect())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        if let Some((_, key)) = self.keys_by_id.remove(&id) {
            self.hash_index.remove(&key.key_hash);
        }
        Ok(())
    }

    async fn delete_expired(&self) -> Result<u64> {
        let now = Utc::now();
        let mut deleted = 0u64;

        let expired_ids: Vec<Uuid> = self
            .keys_by_id
            .iter()
            .filter(|r| {
                if let Some(expires_at) = r.value().expires_at {
                    expires_at < now
                } else {
                    false
                }
            })
            .map(|r| *r.key())
            .collect();

        for id in expired_ids {
            if let Some((_, key)) = self.keys_by_id.remove(&id) {
                self.hash_index.remove(&key.key_hash);
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    async fn count(&self, status: Option<KeyStatus>) -> Result<u64> {
        let count = self
            .keys_by_id
            .iter()
            .filter(|r| {
                if let Some(s) = status {
                    r.value().effective_status() == s
                } else {
                    true
                }
            })
            .count();
        Ok(count as u64)
    }
}

#[cfg(test)]
mod repository_tests {
    use super::*;
    use crate::core::keys::types::{KeyPermissions, KeyRateLimits, KeyUsageStats};

    fn create_test_key() -> ManagedApiKey {
        ManagedApiKey {
            id: Uuid::new_v4(),
            key_hash: format!("hash_{}", Uuid::new_v4()),
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
        }
    }

    #[tokio::test]
    async fn test_create_key() {
        let repo = InMemoryKeyRepository::new();
        let key = create_test_key();
        let id = key.id;

        let created = repo.create(key).await.unwrap();
        assert_eq!(created.id, id);

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_find_by_hash() {
        let repo = InMemoryKeyRepository::new();
        let key = create_test_key();
        let hash = key.key_hash.clone();

        repo.create(key).await.unwrap();

        let found = repo.find_by_hash(&hash).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().key_hash, hash);
    }

    #[tokio::test]
    async fn test_update_key() {
        let repo = InMemoryKeyRepository::new();
        let key = create_test_key();
        let id = key.id;

        repo.create(key).await.unwrap();

        let config = UpdateKeyConfig {
            name: Some("Updated Name".to_string()),
            ..Default::default()
        };

        let updated = repo.update(id, config).await.unwrap();
        assert_eq!(updated.name, "Updated Name");
    }

    #[tokio::test]
    async fn test_update_status() {
        let repo = InMemoryKeyRepository::new();
        let key = create_test_key();
        let id = key.id;

        repo.create(key).await.unwrap();
        repo.update_status(id, KeyStatus::Revoked).await.unwrap();

        let found = repo.find_by_id(id).await.unwrap().unwrap();
        assert_eq!(found.status, KeyStatus::Revoked);
    }

    #[tokio::test]
    async fn test_list_by_user() {
        let repo = InMemoryKeyRepository::new();
        let user_id = Uuid::new_v4();

        let mut key1 = create_test_key();
        key1.user_id = Some(user_id);
        repo.create(key1).await.unwrap();

        let mut key2 = create_test_key();
        key2.user_id = Some(user_id);
        repo.create(key2).await.unwrap();

        let mut key3 = create_test_key();
        key3.user_id = Some(Uuid::new_v4());
        repo.create(key3).await.unwrap();

        let keys = repo.list_by_user(user_id).await.unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_key() {
        let repo = InMemoryKeyRepository::new();
        let key = create_test_key();
        let id = key.id;
        let hash = key.key_hash.clone();

        repo.create(key).await.unwrap();
        repo.delete(id).await.unwrap();

        assert!(repo.find_by_id(id).await.unwrap().is_none());
        assert!(repo.find_by_hash(&hash).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_expired() {
        let repo = InMemoryKeyRepository::new();

        // Create an expired key
        let mut key1 = create_test_key();
        key1.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        repo.create(key1).await.unwrap();

        // Create a valid key
        let key2 = create_test_key();
        let valid_id = key2.id;
        repo.create(key2).await.unwrap();

        let deleted = repo.delete_expired().await.unwrap();
        assert_eq!(deleted, 1);

        // Valid key should still exist
        assert!(repo.find_by_id(valid_id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_count() {
        let repo = InMemoryKeyRepository::new();

        repo.create(create_test_key()).await.unwrap();
        repo.create(create_test_key()).await.unwrap();

        let count = repo.count(None).await.unwrap();
        assert_eq!(count, 2);

        let active_count = repo.count(Some(KeyStatus::Active)).await.unwrap();
        assert_eq!(active_count, 2);
    }

    #[tokio::test]
    async fn test_update_usage() {
        let repo = InMemoryKeyRepository::new();
        let key = create_test_key();
        let id = key.id;

        repo.create(key).await.unwrap();
        repo.update_usage(id, 100, 0.01).await.unwrap();

        let found = repo.find_by_id(id).await.unwrap().unwrap();
        assert_eq!(found.usage_stats.total_requests, 1);
        assert_eq!(found.usage_stats.total_tokens, 100);
    }

    #[tokio::test]
    async fn test_duplicate_hash_rejected() {
        let repo = InMemoryKeyRepository::new();
        let mut key1 = create_test_key();
        let hash = "same_hash".to_string();
        key1.key_hash = hash.clone();

        repo.create(key1).await.unwrap();

        let mut key2 = create_test_key();
        key2.key_hash = hash;

        let result = repo.create(key2).await;
        assert!(result.is_err());
    }
}
