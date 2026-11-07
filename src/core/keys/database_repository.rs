//! Database-backed API key repository implementation.

use super::db_mapping::{from_domain_api_key, to_domain_api_key};
use super::db_update::apply_update_config;
use super::repository::KeyRepository;
use super::types::{KeyStatus, ManagedApiKey, UpdateKeyConfig, UsageRecord};
use crate::storage::StorageLayer;
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DatabaseKeyRepository {
    storage: Arc<StorageLayer>,
}

impl DatabaseKeyRepository {
    pub fn new(storage: Arc<StorageLayer>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl KeyRepository for DatabaseKeyRepository {
    async fn create(&self, key: ManagedApiKey, max_budget: Option<f64>) -> Result<ManagedApiKey> {
        let domain_key = to_domain_api_key(&key)?;
        let stored = self
            .storage
            .db()
            .create_api_key_with_budget(&domain_key, key.budget_id.zip(max_budget))
            .await?;
        Ok(from_domain_api_key(&stored))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ManagedApiKey>> {
        let key = self.storage.db().find_api_key_by_id(id).await?;
        Ok(key.map(|k| from_domain_api_key(&k)))
    }

    async fn find_by_hash(&self, key_hash: &str) -> Result<Option<ManagedApiKey>> {
        let key = self.storage.db().find_api_key_by_hash(key_hash).await?;
        Ok(key.map(|k| from_domain_api_key(&k)))
    }

    async fn update(&self, id: Uuid, config: UpdateKeyConfig) -> Result<ManagedApiKey> {
        let mut domain_key = self
            .storage
            .db()
            .find_api_key_by_id(id)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key not found"))?;

        let max_budget = config.max_budget;
        let mut config = config;
        let existing_budget = from_domain_api_key(&domain_key).budget_id;
        if max_budget.is_some() && config.budget_id.is_none() {
            config.budget_id = Some(Some(existing_budget.unwrap_or_else(Uuid::new_v4)));
        }
        let expected_version = domain_key.metadata.version;
        apply_update_config(&mut domain_key, config)?;
        // The SQL writer compares the version read before applying this update.
        domain_key.metadata.version = expected_version;
        let budget_id = from_domain_api_key(&domain_key).budget_id;
        let updated = self
            .storage
            .db()
            .update_api_key_with_budget(&domain_key, budget_id.zip(max_budget))
            .await?;
        Ok(from_domain_api_key(&updated))
    }

    async fn update_status(&self, id: Uuid, status: KeyStatus) -> Result<()> {
        match status {
            KeyStatus::Active => self.storage.db().set_api_key_active(id, true).await,
            KeyStatus::Revoked => self.storage.db().set_api_key_active(id, false).await,
            KeyStatus::Expired => Err(GatewayError::validation(
                "Cannot set key status to expired manually",
            )),
        }
    }

    async fn update_last_used(&self, id: Uuid) -> Result<()> {
        self.storage.db().update_api_key_last_used(id).await
    }

    async fn update_usage(&self, id: Uuid, record: UsageRecord) -> Result<()> {
        self.storage
            .db()
            .update_api_key_usage(
                id,
                record.requests,
                record.tokens,
                record.cost,
                record.unpriced,
                record.pricing_policy.as_deref(),
            )
            .await
    }

    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<ManagedApiKey>> {
        let keys = self.storage.db().list_api_keys_by_user(user_id).await?;
        Ok(keys.into_iter().map(|k| from_domain_api_key(&k)).collect())
    }

    async fn list_by_team(&self, team_id: Uuid) -> Result<Vec<ManagedApiKey>> {
        let keys = self.storage.db().list_api_keys_by_team(team_id).await?;
        Ok(keys.into_iter().map(|k| from_domain_api_key(&k)).collect())
    }

    async fn list_all(
        &self,
        status: Option<KeyStatus>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ManagedApiKey>> {
        let keys = self
            .storage
            .db()
            .list_api_keys(status, limit, offset)
            .await?;
        Ok(keys.into_iter().map(|k| from_domain_api_key(&k)).collect())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        self.storage.db().delete_api_key(id).await
    }

    async fn delete_expired(&self) -> Result<u64> {
        self.storage.db().delete_expired_api_keys().await
    }

    async fn count(&self, status: Option<KeyStatus>) -> Result<u64> {
        self.storage.db().count_api_keys(status).await
    }
}
