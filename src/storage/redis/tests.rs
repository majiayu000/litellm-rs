//! Redis module tests

use super::pool::RedisPool;
use crate::config::models::storage::RedisConfig;
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
async fn test_redis_pool_creation_returns_error_for_unreachable_endpoint() {
    let config = RedisConfig {
        url: "redis://127.0.0.1:1".to_string(),
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
    };

    let result = RedisPool::new(&config).await;
    assert!(matches!(result, Err(GatewayError::Redis(_))));
}

#[tokio::test]
async fn test_redis_pool_disabled_is_noop() {
    let config = RedisConfig {
        url: "redis://127.0.0.1:1".to_string(),
        enabled: false,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
    };

    let pool = RedisPool::new(&config)
        .await
        .expect("Disabled redis config should create no-op pool");
    assert!(pool.is_noop());
}

async fn live_redis_pool() -> Option<RedisPool> {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let config = RedisConfig {
        url: redis_url.clone(),
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
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
