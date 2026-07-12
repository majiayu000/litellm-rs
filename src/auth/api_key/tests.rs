//! Tests for API key functionality
//!
//! This module contains unit tests for API key management.

#[cfg(test)]
use super::creation::ApiKeyHandler;
#[cfg(test)]
use crate::auth::api_key::types::{ApiKeyVerification, CreateApiKeyRequest};
#[cfg(test)]
use crate::config::models::storage::{RedisConfig, StorageConfig};
use crate::core::models::{ApiKey, Metadata, RateLimits, UsageStats};
#[cfg(test)]
use crate::storage::{DependencyStatus, StorageLayer, redis::RedisPool};
use chrono::{Duration, Utc};
#[cfg(test)]
use std::sync::Arc;
use uuid::Uuid;

// ==================== CreateApiKeyRequest Tests ====================

#[test]
fn test_create_api_key_request() {
    let request = CreateApiKeyRequest {
        name: "Test Key".to_string(),
        user_id: Some(Uuid::new_v4()),
        team_id: None,
        permissions: vec!["read".to_string(), "write".to_string()],
        rate_limits: None,
        expires_at: None,
    };

    assert_eq!(request.name, "Test Key");
    assert!(request.user_id.is_some());
    assert_eq!(request.permissions.len(), 2);
}

#[test]
fn test_create_api_key_request_with_team() {
    let user_id = Uuid::new_v4();
    let team_id = Uuid::new_v4();

    let request = CreateApiKeyRequest {
        name: "Team Key".to_string(),
        user_id: Some(user_id),
        team_id: Some(team_id),
        permissions: vec!["api.chat".to_string()],
        rate_limits: None,
        expires_at: None,
    };

    assert_eq!(request.team_id, Some(team_id));
    assert_eq!(request.user_id, Some(user_id));
}

#[test]
fn test_create_api_key_request_with_rate_limits() {
    let rate_limits = RateLimits {
        rpm: Some(100),
        tpm: Some(50000),
        rpd: Some(10000),
        tpd: Some(1000000),
        concurrent: Some(10),
    };

    let request = CreateApiKeyRequest {
        name: "Rate Limited Key".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec!["api.chat".to_string()],
        rate_limits: Some(rate_limits.clone()),
        expires_at: None,
    };

    assert!(request.rate_limits.is_some());
    let limits = request.rate_limits.unwrap();
    assert_eq!(limits.rpm, Some(100));
    assert_eq!(limits.tpd, Some(1000000));
}

#[test]
fn test_create_api_key_request_with_expiration() {
    let expires_at = Utc::now() + Duration::days(30);

    let request = CreateApiKeyRequest {
        name: "Expiring Key".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec!["api.chat".to_string()],
        rate_limits: None,
        expires_at: Some(expires_at),
    };

    assert!(request.expires_at.is_some());
    assert!(request.expires_at.unwrap() > Utc::now());
}

#[test]
fn test_create_api_key_request_empty_permissions() {
    let request = CreateApiKeyRequest {
        name: "No Permissions Key".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec![],
        rate_limits: None,
        expires_at: None,
    };

    assert!(request.permissions.is_empty());
}

#[test]
fn test_create_api_key_request_clone() {
    let request = CreateApiKeyRequest {
        name: "Clone Test".to_string(),
        user_id: Some(Uuid::new_v4()),
        team_id: None,
        permissions: vec!["read".to_string()],
        rate_limits: None,
        expires_at: None,
    };

    let cloned = request.clone();
    assert_eq!(request.name, cloned.name);
    assert_eq!(request.user_id, cloned.user_id);
    assert_eq!(request.permissions, cloned.permissions);
}

// ==================== ApiKeyVerification Tests ====================

#[test]
fn test_api_key_verification_valid() {
    let verification = ApiKeyVerification {
        api_key: ApiKey {
            metadata: Metadata::new(),
            name: "Test Key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test".to_string(),
            user_id: None,
            team_id: None,
            permissions: vec!["read".to_string()],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        },
        user: None,
        is_valid: true,
        invalid_reason: None,
    };

    assert!(verification.is_valid);
    assert!(verification.invalid_reason.is_none());
    assert_eq!(verification.api_key.name, "Test Key");
}

#[test]
fn test_api_key_verification_invalid_inactive() {
    let verification = ApiKeyVerification {
        api_key: ApiKey {
            metadata: Metadata::new(),
            name: "Inactive Key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test".to_string(),
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
        invalid_reason: Some("API key is inactive".to_string()),
    };

    assert!(!verification.is_valid);
    assert!(verification.invalid_reason.is_some());
    assert_eq!(verification.invalid_reason.unwrap(), "API key is inactive");
}

#[test]
fn test_api_key_verification_invalid_expired() {
    let expired_at = Utc::now() - Duration::days(1);

    let verification = ApiKeyVerification {
        api_key: ApiKey {
            metadata: Metadata::new(),
            name: "Expired Key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test".to_string(),
            user_id: None,
            team_id: None,
            permissions: vec![],
            rate_limits: None,
            expires_at: Some(expired_at),
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        },
        user: None,
        is_valid: false,
        invalid_reason: Some("API key is expired".to_string()),
    };

    assert!(!verification.is_valid);
    assert!(verification.api_key.expires_at.unwrap() < Utc::now());
}

#[test]
fn test_api_key_verification_not_found() {
    let verification = ApiKeyVerification {
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
    };

    assert!(!verification.is_valid);
    assert_eq!(verification.invalid_reason.unwrap(), "API key not found");
}

#[test]
fn test_api_key_verification_clone() {
    let verification = ApiKeyVerification {
        api_key: ApiKey {
            metadata: Metadata::new(),
            name: "Clone Test".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test".to_string(),
            user_id: None,
            team_id: None,
            permissions: vec!["read".to_string()],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        },
        user: None,
        is_valid: true,
        invalid_reason: None,
    };

    let cloned = verification.clone();
    assert_eq!(verification.is_valid, cloned.is_valid);
    assert_eq!(verification.api_key.name, cloned.api_key.name);
}

// ==================== ApiKey Model Tests ====================

#[test]
fn test_api_key_creation() {
    let api_key = ApiKey {
        metadata: Metadata::new(),
        name: "Test Key".to_string(),
        key_hash: "hash123".to_string(),
        key_prefix: "gw-abcd".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec!["read".to_string(), "write".to_string()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };

    assert_eq!(api_key.name, "Test Key");
    assert!(api_key.is_active);
    assert_eq!(api_key.permissions.len(), 2);
}

#[test]
fn test_api_key_with_user_and_team() {
    let user_id = Uuid::new_v4();
    let team_id = Uuid::new_v4();

    let api_key = ApiKey {
        metadata: Metadata::new(),
        name: "User Team Key".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-test".to_string(),
        user_id: Some(user_id),
        team_id: Some(team_id),
        permissions: vec!["api.chat".to_string()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };

    assert_eq!(api_key.user_id, Some(user_id));
    assert_eq!(api_key.team_id, Some(team_id));
}

#[test]
fn test_api_key_with_rate_limits() {
    let rate_limits = RateLimits {
        rpm: Some(60),
        tpm: Some(100000),
        rpd: Some(5000),
        tpd: Some(500000),
        concurrent: Some(5),
    };

    let api_key = ApiKey {
        metadata: Metadata::new(),
        name: "Rate Limited".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-test".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec![],
        rate_limits: Some(rate_limits),
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };

    assert!(api_key.rate_limits.is_some());
    let limits = api_key.rate_limits.unwrap();
    assert_eq!(limits.rpm, Some(60));
    assert_eq!(limits.concurrent, Some(5));
}

#[test]
fn test_api_key_with_last_used() {
    let last_used = Utc::now() - Duration::hours(1);

    let api_key = ApiKey {
        metadata: Metadata::new(),
        name: "Recently Used".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-test".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec![],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: Some(last_used),
        usage_stats: UsageStats::default(),
    };

    assert!(api_key.last_used_at.is_some());
    assert!(api_key.last_used_at.unwrap() < Utc::now());
}

#[test]
fn test_api_key_permissions_check() {
    let api_key = ApiKey {
        metadata: Metadata::new(),
        name: "Permission Test".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-test".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec![
            "api.chat".to_string(),
            "api.embeddings".to_string(),
            "api.images".to_string(),
        ],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };

    assert!(api_key.permissions.contains(&"api.chat".to_string()));
    assert!(api_key.permissions.contains(&"api.embeddings".to_string()));
    assert!(api_key.permissions.contains(&"api.images".to_string()));
    assert!(!api_key.permissions.contains(&"admin".to_string()));
}

// ==================== UsageStats Tests ====================

#[test]
fn test_usage_stats_default() {
    let stats = UsageStats::default();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.total_tokens, 0);
    assert_eq!(stats.total_cost, 0.0);
    assert_eq!(stats.requests_today, 0);
    assert_eq!(stats.tokens_today, 0);
    assert_eq!(stats.cost_today, 0.0);
    assert_eq!(stats.unpriced_requests, 0);
    assert_eq!(stats.unpriced_tokens, 0);
    assert_eq!(stats.unpriced_cost, 0.0);
    assert!(stats.last_unpriced_at.is_none());
}

#[test]
fn test_usage_stats_with_values() {
    let stats = UsageStats {
        total_requests: 1000,
        total_tokens: 500000,
        total_cost: 25.50,
        requests_today: 100,
        tokens_today: 50000,
        cost_today: 2.50,
        last_reset: Utc::now(),
        ..UsageStats::default()
    };

    assert_eq!(stats.total_requests, 1000);
    assert_eq!(stats.total_tokens, 500000);
    assert!((stats.total_cost - 25.50).abs() < f64::EPSILON);
    assert_eq!(stats.requests_today, 100);
}

// ==================== RateLimits Tests ====================

#[test]
fn test_rate_limits_all_set() {
    let limits = RateLimits {
        rpm: Some(100),
        tpm: Some(50000),
        rpd: Some(10000),
        tpd: Some(1000000),
        concurrent: Some(10),
    };

    assert_eq!(limits.rpm, Some(100));
    assert_eq!(limits.rpd, Some(10000));
    assert_eq!(limits.tpm, Some(50000));
    assert_eq!(limits.tpd, Some(1000000));
    assert_eq!(limits.concurrent, Some(10));
}

#[test]
fn test_rate_limits_partial() {
    let limits = RateLimits {
        rpm: Some(60),
        tpm: None,
        rpd: None,
        tpd: Some(100000),
        concurrent: None,
    };

    assert!(limits.rpm.is_some());
    assert!(limits.rpd.is_none());
    assert!(limits.tpd.is_some());
}

#[test]
fn test_rate_limits_clone() {
    let limits = RateLimits {
        rpm: Some(100),
        tpm: Some(50000),
        rpd: Some(10000),
        tpd: Some(1000000),
        concurrent: Some(10),
    };

    let cloned = limits.clone();
    assert_eq!(limits.rpm, cloned.rpm);
    assert_eq!(limits.tpd, cloned.tpd);
}

// ==================== Metadata Tests ====================

#[test]
fn test_metadata_new() {
    let metadata = Metadata::new();
    assert!(!metadata.id.is_nil());
    assert!(metadata.created_at <= Utc::now());
    assert!(metadata.updated_at <= Utc::now());
}

#[test]
fn test_metadata_timestamps() {
    let metadata = Metadata::new();
    // Created and updated should be approximately equal for new metadata
    let diff = metadata.updated_at - metadata.created_at;
    assert!(diff.num_seconds() < 1);
}

// ==================== Key Prefix Tests ====================

#[test]
fn test_key_prefix_format() {
    let api_key = ApiKey {
        metadata: Metadata::new(),
        name: "Prefix Test".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-abcd1234".to_string(),
        user_id: None,
        team_id: None,
        permissions: vec![],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };

    assert!(api_key.key_prefix.starts_with("gw-"));
}

#[test]
fn test_key_prefix_different_formats() {
    // Test various valid prefix formats
    let prefixes = vec!["gw-test", "gw-abc123", "gw-UPPER", "gw-mixed123ABC"];

    for prefix in prefixes {
        let api_key = ApiKey {
            metadata: Metadata::new(),
            name: "Test".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: prefix.to_string(),
            user_id: None,
            team_id: None,
            permissions: vec![],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        };

        assert_eq!(api_key.key_prefix, prefix);
    }
}

// ==================== Revocation Consistency Tests ====================

#[tokio::test]
async fn revoked_key_is_rejected_when_cache_delete_fails() {
    let Some((admin_pool, redis_url)) = live_redis_pool().await else {
        return;
    };
    let base_handler = test_api_key_handler().await;
    let (api_key, raw_key) = base_handler
        .create_key(
            None,
            None,
            "acl-denied-cache-delete".to_string(),
            vec!["api.chat".to_string()],
        )
        .await
        .expect("test API key should be stored");
    let cache_key = format!("api_key:hash:{}", api_key.key_hash);
    let username = format!("gh959_{}", Uuid::new_v4().simple());
    let password = format!("gh959-{}", Uuid::new_v4().simple());
    let restricted_config = restricted_redis_config(&redis_url, &username, &password);

    configure_delete_denied_user(&admin_pool, &username, &password, &cache_key).await;
    let scenario = async {
        let restricted_pool = RedisPool::new(&restricted_config)
            .await
            .map_err(|error| error.to_string())?;
        let handler = handler_with_redis(&base_handler, restricted_pool)
            .await
            .map_err(|error| error.to_string())?;
        let serialized = serde_json::to_string(&api_key).map_err(|error| error.to_string())?;
        handler
            .storage
            .cache_set(&cache_key, &serialized, None)
            .await
            .map_err(|error| error.to_string())?;
        let delete_error = handler
            .storage
            .cache_delete(&cache_key)
            .await
            .err()
            .ok_or_else(|| "restricted user unexpectedly deleted the cache key".to_string())?;

        handler
            .revoke_key(api_key.metadata.id)
            .await
            .map_err(|error| error.to_string())?;
        let stored = handler
            .storage
            .db()
            .find_api_key_by_hash(&api_key.key_hash)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "revoked key was not stored".to_string())?;
        let cached = handler
            .storage
            .cache_get(&cache_key)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "denied deletion removed the stale snapshot".to_string())?;
        let stale_snapshot: ApiKey =
            serde_json::from_str(&cached).map_err(|error| error.to_string())?;
        let live_result = handler
            .verify_key(&raw_key)
            .await
            .map_err(|error| error.to_string())?;
        let detailed_result = handler
            .verify_key_detailed(&raw_key)
            .await
            .map_err(|error| error.to_string())?;

        Ok::<_, String>((
            delete_error.to_string(),
            stored.is_active,
            stale_snapshot.is_active,
            live_result.is_some(),
            detailed_result.is_valid,
            detailed_result.invalid_reason,
        ))
    }
    .await;

    cleanup_delete_denied_user(&admin_pool, &username, &cache_key).await;

    let (delete_error, stored_active, stale_active, live_valid, detailed_valid, invalid_reason) =
        scenario.expect("revocation scenario should complete before cleanup");
    assert!(
        delete_error.to_ascii_lowercase().contains("noperm"),
        "cache deletion should fail because Redis denied DEL: {delete_error}"
    );
    assert!(!stored_active, "database revoke must be committed");
    assert!(
        stale_active,
        "the denied cache deletion must leave the old active snapshot readable"
    );
    assert!(
        !live_valid,
        "live authentication must ignore the stale active snapshot"
    );
    assert!(!detailed_valid);
    assert_eq!(invalid_reason.as_deref(), Some("API key is inactive"));
}

#[cfg(test)]
async fn test_api_key_handler() -> ApiKeyHandler {
    let mut config = StorageConfig::default();
    config.database.enabled = false;
    config.redis.enabled = false;
    let storage = StorageLayer::new(&config)
        .await
        .expect("test storage should initialize");
    ApiKeyHandler::new(Arc::new(storage), None)
        .await
        .expect("API key handler should initialize")
}

#[cfg(test)]
async fn handler_with_redis(
    base_handler: &ApiKeyHandler,
    redis: RedisPool,
) -> crate::utils::error::gateway_error::Result<ApiKeyHandler> {
    let mut storage = (*base_handler.storage).clone();
    storage.redis = Arc::new(redis);
    storage.redis_status = DependencyStatus::Healthy;
    ApiKeyHandler::new(Arc::new(storage), None).await
}

#[cfg(test)]
async fn live_redis_pool() -> Option<(RedisPool, String)> {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let config = RedisConfig {
        url: redis_url.clone(),
        enabled: true,
        max_connections: 2,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    };

    match RedisPool::new(&config).await {
        Ok(pool) => match pool.health_check().await {
            Ok(()) => Some((pool, redis_url)),
            Err(error) => unavailable_live_redis(&redis_url, error),
        },
        Err(error) => unavailable_live_redis(&redis_url, error),
    }
}

#[cfg(test)]
fn unavailable_live_redis(
    redis_url: &str,
    error: crate::utils::error::gateway_error::GatewayError,
) -> Option<(RedisPool, String)> {
    if std::env::var("CI").is_ok() {
        panic!("Redis should be reachable in CI at {redis_url}: {error}");
    }
    eprintln!("Skipping API-key revocation Redis test: {error}");
    None
}

#[cfg(test)]
fn restricted_redis_config(redis_url: &str, username: &str, password: &str) -> RedisConfig {
    let mut url = url::Url::parse(redis_url).expect("REDIS_URL should be valid");
    url.set_username(username)
        .expect("Redis ACL username should be valid");
    url.set_password(Some(password))
        .expect("Redis ACL password should be valid");
    RedisConfig {
        url: url.to_string(),
        enabled: true,
        max_connections: 2,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    }
}

#[cfg(test)]
async fn configure_delete_denied_user(
    admin_pool: &RedisPool,
    username: &str,
    password: &str,
    cache_key: &str,
) {
    let mut connection = admin_pool
        .get_connection()
        .await
        .expect("admin Redis connection should be available");
    let connection = connection
        .conn
        .as_mut()
        .expect("live Redis connection should not be no-op");
    let _: () = redis::cmd("ACL")
        .arg("SETUSER")
        .arg(username)
        .arg("reset")
        .arg("on")
        .arg(format!(">{password}"))
        .arg(format!("~{cache_key}"))
        .arg("+@connection")
        .arg("+get")
        .arg("+set")
        .arg("-del")
        .query_async(connection)
        .await
        .expect("test should create a Redis user that cannot delete the cache key");
}

#[cfg(test)]
async fn cleanup_delete_denied_user(admin_pool: &RedisPool, username: &str, cache_key: &str) {
    let mut connection = admin_pool
        .get_connection()
        .await
        .expect("admin Redis cleanup connection should be available");
    let connection = connection
        .conn
        .as_mut()
        .expect("live Redis cleanup connection should not be no-op");
    let _: i64 = redis::cmd("DEL")
        .arg(cache_key)
        .query_async(&mut *connection)
        .await
        .expect("admin should remove the stale test cache key");
    let _: i64 = redis::cmd("ACL")
        .arg("DELUSER")
        .arg(username)
        .query_async(connection)
        .await
        .expect("admin should remove the restricted test user");
}
