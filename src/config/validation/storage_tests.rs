use super::trait_def::Validate;
#[cfg(all(feature = "storage", not(feature = "sqlite")))]
use crate::config::models::storage::StorageConfig;
use crate::config::models::storage::{DatabaseConfig, RedisConfig};

#[cfg(any(not(feature = "storage"), feature = "sqlite"))]
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

#[cfg(feature = "sqlite")]
#[test]
fn database_validation_accepts_sqlite_when_feature_enabled() {
    let config = DatabaseConfig {
        enabled: true,
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        connection_timeout: 1,
        ssl: false,
        auto_migrate: true,
        auto_migrate_configured: false,
        fallback_to_sqlite: false,
        allow_degraded: false,
    };

    assert!(Validate::validate(&config).is_ok());
}

#[cfg(not(feature = "sqlite"))]
#[test]
fn database_validation_rejects_sqlite_when_feature_disabled() {
    let config = DatabaseConfig {
        enabled: true,
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        connection_timeout: 1,
        ssl: false,
        auto_migrate: true,
        auto_migrate_configured: false,
        fallback_to_sqlite: false,
        allow_degraded: false,
    };

    assert!(Validate::validate(&config).is_err());
}

#[cfg(not(feature = "postgres"))]
#[test]
fn database_validation_rejects_postgres_when_feature_disabled() {
    let config = DatabaseConfig {
        enabled: true,
        url: "postgresql://localhost/litellm".to_string(),
        max_connections: 1,
        connection_timeout: 1,
        ssl: false,
        auto_migrate: false,
        auto_migrate_configured: false,
        fallback_to_sqlite: false,
        allow_degraded: false,
    };

    let error = config.validate().unwrap_err();
    assert!(error.contains("`postgres` feature"), "got: {error}");
}

#[cfg(all(feature = "storage", not(feature = "sqlite")))]
#[test]
fn database_validation_rejects_sqlite_dependent_runtime_modes() {
    let disabled = DatabaseConfig::default();
    let error = Validate::validate(&disabled).unwrap_err();
    assert!(error.to_string().contains("enabled=false"), "got: {error}");
    assert!(error.contains("`sqlite` feature"), "got: {error}");
    let storage = StorageConfig {
        database: disabled,
        ..StorageConfig::default()
    };
    let error = Validate::validate(&storage).unwrap_err();
    assert!(error.contains("enabled=false"), "got: {error}");
    let mut config = crate::config::Config::default();
    config
        .gateway
        .providers
        .push(crate::config::models::provider::ProviderConfig {
            name: "test-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            ..Default::default()
        });
    let error = config.validate().unwrap_err();
    assert!(error.to_string().contains("enabled=false"), "got: {error}");

    let fallback = DatabaseConfig {
        enabled: true,
        url: "postgresql://localhost/litellm".to_string(),
        max_connections: 1,
        connection_timeout: 1,
        ssl: false,
        auto_migrate: false,
        auto_migrate_configured: false,
        fallback_to_sqlite: true,
        allow_degraded: false,
    };
    let error = Validate::validate(&fallback).unwrap_err();
    assert!(error.contains("fallback_to_sqlite=true"), "got: {error}");
    assert!(error.contains("`sqlite` feature"), "got: {error}");
}

#[cfg(feature = "postgres")]
#[test]
fn database_validation_accepts_postgres_when_feature_enabled() {
    let config = DatabaseConfig {
        enabled: true,
        url: "postgresql://localhost/litellm".to_string(),
        max_connections: 1,
        connection_timeout: 1,
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
fn redis_validation_accepts_cluster_mode_when_url_is_valid() {
    let config = RedisConfig {
        enabled: true,
        url: "redis://localhost:6379".to_string(),
        max_connections: 10,
        connection_timeout: 5,
        cluster: true,
        allow_degraded: false,
    };

    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn redis_validation_rejects_cluster_mode_with_empty_url() {
    let config = RedisConfig {
        enabled: true,
        url: String::new(),
        max_connections: 10,
        connection_timeout: 5,
        cluster: true,
        allow_degraded: false,
    };

    let error = Validate::validate(&config).unwrap_err();
    assert!(error.contains("Redis URL cannot be empty"), "got: {error}");
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
