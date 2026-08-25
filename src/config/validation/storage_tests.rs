use super::trait_def::Validate;
use crate::config::models::storage::{DatabaseConfig, RedisConfig};

#[test]
fn database_validation_skips_when_disabled() {
    let config = DatabaseConfig {
        enabled: false,
        url: String::new(),
        max_connections: 0,
        connection_timeout: 0,
        ssl: false,
        auto_migrate: false,
        auto_migrate_configured: false,
        fallback_to_sqlite: false,
        allow_degraded: false,
    };
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn redis_validation_skips_when_disabled() {
    let config = RedisConfig {
        enabled: false,
        url: String::new(),
        max_connections: 0,
        connection_timeout: 0,
        cluster: false,
        allow_degraded: false,
    };
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn redis_validation_rejects_unimplemented_cluster_mode_with_actionable_remedy() {
    let config = RedisConfig {
        enabled: true,
        url: "redis://localhost:6379".to_string(),
        max_connections: 10,
        connection_timeout: 5,
        cluster: true,
        allow_degraded: false,
    };

    let error = Validate::validate(&config).unwrap_err();

    assert!(error.contains("cluster"), "got: {error}");
    assert!(error.contains("not implemented"), "got: {error}");
    assert!(
        error.contains("storage.redis.cluster=false"),
        "got: {error}"
    );
}

#[test]
fn redis_validation_accepts_standalone_when_enabled() {
    let config = RedisConfig {
        enabled: true,
        url: "redis://localhost:6379".to_string(),
        max_connections: 10,
        connection_timeout: 5,
        cluster: false,
        allow_degraded: false,
    };
    assert!(Validate::validate(&config).is_ok());
}
