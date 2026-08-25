//! Redis module tests

use super::pool::RedisPool;
use crate::config::models::storage::{RedisConfig, StorageConfig};
use crate::storage::StorageLayer;
use crate::utils::error::gateway_error::GatewayError;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_sanitize_url() {
    let url = "redis://user:password@localhost:6379/0";
    let sanitized = RedisPool::sanitize_url(url);
    assert!(sanitized.contains("user:***@localhost"));
    assert!(!sanitized.contains("password"));
}

#[tokio::test]
async fn test_redis_set_get_roundtrip_with_live_pool() {
    let Some(pool) = live_redis_pool().await else {
        return;
    };

    let key = unique_test_key("roundtrip");
    let value = "value-from-integration-test";

    pool.set(&key, value, Some(30))
        .await
        .expect("set should write to redis");

    let cached = pool.get(&key).await.expect("get should read from redis");
    assert_eq!(cached.as_deref(), Some(value));

    let exists = pool
        .exists(&key)
        .await
        .expect("exists should succeed for written key");
    assert!(exists);

    pool.delete(&key).await.expect("delete should remove key");
    let exists_after_delete = pool
        .exists(&key)
        .await
        .expect("exists should succeed after delete");
    assert!(!exists_after_delete);
}

#[tokio::test]
async fn test_redis_delete_by_prefix_with_live_pool() {
    let Some(pool) = live_redis_pool().await else {
        return;
    };

    let prefix = unique_test_key("prefix");
    let first = format!("{prefix}:1");
    let second = format!("{prefix}:2");

    pool.set(&first, "one", Some(30))
        .await
        .expect("first set should write to redis");
    pool.set(&second, "two", Some(30))
        .await
        .expect("second set should write to redis");

    let deleted = pool
        .delete_by_prefix(&prefix)
        .await
        .expect("prefix delete should remove matching redis keys");
    assert_eq!(deleted, 2);
    assert!(!pool.exists(&first).await.expect("exists should succeed"));
    assert!(!pool.exists(&second).await.expect("exists should succeed"));
}

#[tokio::test]
async fn test_redis_pool_creation_returns_error_for_unreachable_endpoint() {
    let config = RedisConfig {
        url: "redis://127.0.0.1:1".to_string(),
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    };

    let result = RedisPool::new(&config).await;
    assert!(matches!(result, Err(GatewayError::Storage(_))));
}

#[tokio::test]
async fn test_redis_pool_disabled_is_noop() {
    let config = RedisConfig {
        url: "redis://127.0.0.1:1".to_string(),
        enabled: false,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    };

    let pool = RedisPool::new(&config)
        .await
        .expect("Disabled redis config should create no-op pool");
    assert!(pool.is_noop());
}

#[tokio::test]
async fn cluster_mode_is_rejected_before_standalone_connection_attempt() {
    let config = RedisConfig {
        url: "redis://127.0.0.1:1".to_string(),
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: true,
        allow_degraded: true,
    };

    let error = RedisPool::new(&config)
        .await
        .expect_err("cluster mode must fail instead of degrading or connecting");
    let message = error.to_string();

    assert!(
        message.contains("storage.redis.cluster=true"),
        "got: {message}"
    );
    assert!(
        message.contains("storage.redis.cluster=false"),
        "got: {message}"
    );
}

#[tokio::test]
async fn storage_rejects_cluster_mode_before_optional_dependency_initialization() {
    let mut config = StorageConfig::default();
    config.redis.enabled = true;
    config.redis.cluster = true;
    config.redis.allow_degraded = true;

    let error = StorageLayer::new(&config)
        .await
        .expect_err("cluster mode must be rejected rather than degraded");

    assert!(matches!(error, GatewayError::Config(_)));
    assert!(error.to_string().contains("storage.redis.cluster=false"));
}

#[tokio::test]
async fn test_redis_delete_by_prefix_disabled_is_noop() {
    let pool = RedisPool::create_noop();

    let deleted = pool
        .delete_by_prefix("litellm-rs:test:")
        .await
        .expect("disabled redis prefix delete should not fail");
    assert_eq!(deleted, 0);
}

async fn live_redis_pool() -> Option<RedisPool> {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let config = RedisConfig {
        url: redis_url.clone(),
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    };

    match RedisPool::new(&config).await {
        Ok(pool) => match pool.health_check().await {
            Ok(()) => Some(pool),
            Err(err) => {
                if std::env::var("CI").is_ok() {
                    panic!("Redis should pass health check in CI at {redis_url}: {err}");
                }

                eprintln!("Skipping live Redis integration test: {err}");
                None
            }
        },
        Err(err) => {
            if std::env::var("CI").is_ok() {
                panic!("Redis should be reachable in CI at {redis_url}: {err}");
            }

            eprintln!("Skipping live Redis integration test: {err}");
            None
        }
    }
}

fn unique_test_key(suffix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    format!("litellm-rs:test:{suffix}:{nanos}")
}
