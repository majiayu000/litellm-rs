//! Virtual key database operations
//!
//! Stub implementations — database schema for virtual keys is not yet migrated.
//! Each method returns `GatewayError::NotImplemented` until the migration lands.

use crate::core::virtual_keys::VirtualKey;
use crate::utils::error::gateway_error::{GatewayError, Result};

use super::types::SeaOrmDatabase;

impl SeaOrmDatabase {
    /// Store a new virtual key in the database.
    pub async fn store_virtual_key(&self, _key: &VirtualKey) -> Result<()> {
        Err(GatewayError::not_implemented(
            "virtual_keys: store_virtual_key not yet implemented",
        ))
    }

    /// Retrieve a virtual key by its hash.
    pub async fn get_virtual_key(&self, _key_hash: &str) -> Result<Option<VirtualKey>> {
        Err(GatewayError::not_implemented(
            "virtual_keys: get_virtual_key not yet implemented",
        ))
    }

    /// Update the usage statistics (last_used_at, usage_count) for a virtual key.
    pub async fn update_virtual_key_usage(&self, _key: &VirtualKey) -> Result<()> {
        Err(GatewayError::not_implemented(
            "virtual_keys: update_virtual_key_usage not yet implemented",
        ))
    }

    /// Add `cost` to the recorded spend for the given key ID.
    pub async fn update_key_spend(&self, _key_id: &str, _cost: f64) -> Result<()> {
        Err(GatewayError::not_implemented(
            "virtual_keys: update_key_spend not yet implemented",
        ))
    }

    /// List all virtual keys owned by a user.
    pub async fn list_user_keys(&self, _user_id: &str) -> Result<Vec<VirtualKey>> {
        Err(GatewayError::not_implemented(
            "virtual_keys: list_user_keys not yet implemented",
        ))
    }

    /// Retrieve a virtual key by its opaque key ID.
    pub async fn get_virtual_key_by_id(&self, _key_id: &str) -> Result<Option<VirtualKey>> {
        Err(GatewayError::not_implemented(
            "virtual_keys: get_virtual_key_by_id not yet implemented",
        ))
    }

    /// Persist all mutable fields of a virtual key (full update).
    pub async fn update_virtual_key(&self, _key: &VirtualKey) -> Result<()> {
        Err(GatewayError::not_implemented(
            "virtual_keys: update_virtual_key not yet implemented",
        ))
    }

    /// Remove a virtual key from the database by its key ID.
    pub async fn delete_virtual_key(&self, _key_id: &str) -> Result<()> {
        Err(GatewayError::not_implemented(
            "virtual_keys: delete_virtual_key not yet implemented",
        ))
    }

    /// Return all virtual keys whose budget reset timestamp has passed.
    pub async fn get_keys_with_expired_budgets(&self) -> Result<Vec<VirtualKey>> {
        Err(GatewayError::not_implemented(
            "virtual_keys: get_keys_with_expired_budgets not yet implemented",
        ))
    }
}
