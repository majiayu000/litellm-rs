use super::database_repository::DatabaseKeyRepository;
use super::db_mapping::{from_domain_api_key, to_domain_api_key};
use super::types::{KeyPermissions, KeyRateLimits, KeyStatus, KeyUsageStats, ManagedApiKey};
use super::{CreateKeyConfig, KeyManager};
use crate::auth::{AuthMethod, AuthSystem};
use crate::core::models::{ApiKey, Metadata, UsageStats};
use crate::core::types::context::RequestContext;
use crate::storage::StorageLayer;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

fn sample_managed_key() -> ManagedApiKey {
    ManagedApiKey {
        id: Uuid::new_v4(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-abc...xyz".to_string(),
        name: "Test Key".to_string(),
        description: Some("desc".to_string()),
        user_id: Some(Uuid::new_v4()),
        team_id: Some(Uuid::new_v4()),
        budget_id: Some(Uuid::new_v4()),
        permissions: KeyPermissions {
            allowed_models: vec!["gpt-*".to_string()],
            allowed_endpoints: vec!["/v1/chat/*".to_string()],
            max_tokens_per_request: Some(1024),
            is_admin: true,
            custom_permissions: vec!["api.chat".to_string()],
        },
        rate_limits: KeyRateLimits {
            requests_per_minute: Some(10),
            tokens_per_minute: Some(1000),
            requests_per_day: Some(100),
            tokens_per_day: Some(10000),
            max_concurrent_requests: Some(2),
        },
        status: KeyStatus::Active,
        expires_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_used_at: None,
        usage_stats: KeyUsageStats::new(),
        metadata: serde_json::json!({"tenant":"acme"}),
    }
}

#[test]
fn test_roundtrip_preserves_core_keys_fields() {
    let managed = sample_managed_key();
    let domain = to_domain_api_key(&managed).unwrap();
    let roundtrip = from_domain_api_key(&domain);

    assert_eq!(roundtrip.description, managed.description);
    assert_eq!(roundtrip.budget_id, managed.budget_id);
    assert_eq!(
        roundtrip.permissions.allowed_models,
        managed.permissions.allowed_models
    );
    assert_eq!(
        roundtrip.permissions.allowed_endpoints,
        managed.permissions.allowed_endpoints
    );
    assert_eq!(
        roundtrip.permissions.max_tokens_per_request,
        managed.permissions.max_tokens_per_request
    );
    assert_eq!(roundtrip.permissions.is_admin, managed.permissions.is_admin);
    assert_eq!(
        roundtrip.permissions.custom_permissions,
        managed.permissions.custom_permissions
    );
    assert_eq!(roundtrip.metadata, managed.metadata);
}

#[test]
fn test_derive_permissions_without_namespace() {
    let domain = ApiKey {
        metadata: Metadata::new(),
        name: "legacy".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-legacy".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec!["system.admin".to_string(), "api.chat".to_string()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };

    let managed = from_domain_api_key(&domain);
    assert!(managed.permissions.is_admin);
    assert_eq!(managed.permissions.custom_permissions, domain.permissions);
    assert_eq!(managed.metadata, serde_json::Value::Null);
}

#[tokio::test]
async fn test_key_manager_and_auth_system_share_db_source_of_truth() {
    let mut config = crate::config::Config::default();
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;

    let storage = Arc::new(
        StorageLayer::new(&config.gateway.storage)
            .await
            .expect("failed to create storage"),
    );
    storage.migrate().await.expect("failed to run migrations");

    let manager = KeyManager::new(DatabaseKeyRepository::new(storage.clone()))
        .with_hmac_secret(config.gateway.auth.api_key_hmac_secret.clone());
    let auth = AuthSystem::new(&config.gateway.auth, storage.clone())
        .await
        .expect("failed to create auth system");

    let create = CreateKeyConfig {
        name: "shared-db-key".to_string(),
        ..Default::default()
    };
    let (key_id, raw_key) = manager.generate_key(create).await.unwrap();

    let auth_result = auth
        .authenticate(AuthMethod::ApiKey(raw_key), RequestContext::new())
        .await
        .expect("auth failed unexpectedly");

    assert!(auth_result.success);
    assert_eq!(auth_result.context.api_key_id(), Some(key_id));
}
