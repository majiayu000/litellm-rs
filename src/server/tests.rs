//! Tests for server module
//!
//! This module contains all tests for the server components.

use crate::config::models::gateway::GATEWAY_ENV_LOCK;
use crate::server::HttpServer;
#[cfg(test)]
use crate::server::builder::{ServerBuilder, load_default_config_or_env, load_explicit_config};
use crate::server::types::ServerRequestMetrics;
use crate::utils::error::gateway_error::GatewayError;
use std::io::Write;

const GATEWAY_ENV_KEYS: &[&str] = &[
    "LITELLM_HOST",
    "LITELLM_PORT",
    "LITELLM_WORKERS",
    "LITELLM_TIMEOUT",
    "LITELLM_DATABASE_URL",
    "LITELLM_DATABASE_MAX_CONNECTIONS",
    "LITELLM_DATABASE_CONNECTION_TIMEOUT",
    "LITELLM_DATABASE_SSL",
    "LITELLM_DATABASE_ENABLED",
    "LITELLM_DATABASE_AUTO_MIGRATE",
    "LITELLM_REDIS_URL",
    "LITELLM_REDIS_ENABLED",
    "LITELLM_REDIS_MAX_CONNECTIONS",
    "LITELLM_REDIS_CONNECTION_TIMEOUT",
    "LITELLM_REDIS_CLUSTER",
    "LITELLM_ENABLE_JWT",
    "LITELLM_ENABLE_API_KEY",
    "LITELLM_JWT_SECRET",
    "LITELLM_JWT_EXPIRATION",
    "LITELLM_API_KEY_HEADER",
    "LITELLM_PROVIDERS",
    "LITELLM_PRICING_SOURCE",
    "LITELLM_CACHE_ENABLED",
    "LITELLM_RATE_LIMIT_ENABLED",
    "LITELLM_ENTERPRISE_ENABLED",
    "LITELLM_PROVIDER_OPENAI_TYPE",
    "LITELLM_PROVIDER_OPENAI_API_KEY",
    "LITELLM_PROVIDER_OPENAI_BASE_URL",
    "LITELLM_PROVIDER_OPENAI_API_VERSION",
    "LITELLM_PROVIDER_OPENAI_ORGANIZATION",
    "LITELLM_PROVIDER_OPENAI_PROJECT",
    "LITELLM_PROVIDER_OPENAI_WEIGHT",
    "LITELLM_PROVIDER_OPENAI_RPM",
    "LITELLM_PROVIDER_OPENAI_TPM",
    "LITELLM_PROVIDER_OPENAI_MAX_CONCURRENT_REQUESTS",
    "LITELLM_PROVIDER_OPENAI_TIMEOUT",
    "LITELLM_PROVIDER_OPENAI_MAX_RETRIES",
    "LITELLM_PROVIDER_OPENAI_ENABLED",
    "LITELLM_PROVIDER_OPENAI_MODELS",
    "LITELLM_PROVIDER_OPENAI_TAGS",
];

struct EnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
}

fn valid_programmatic_config() -> crate::config::Config {
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
    config
}

impl EnvGuard {
    fn with_minimal_gateway_config() -> Self {
        let saved = GATEWAY_ENV_KEYS
            .iter()
            .map(|key| (*key, std::env::var(key).ok()))
            .collect();

        for key in GATEWAY_ENV_KEYS {
            unsafe { std::env::remove_var(key) };
        }

        unsafe {
            std::env::set_var("LITELLM_PROVIDERS", "openai");
            std::env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            std::env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
        }

        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

#[tokio::test]
async fn test_server_builder_requires_config() {
    let result = ServerBuilder::new().build().await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("builder without configuration should fail"),
    };

    match error {
        GatewayError::Config(message) => assert_eq!(message, "Configuration is required"),
        other => panic!("expected config error, got: {other:?}"),
    }
}

#[tokio::test]
async fn server_builder_rejects_redis_cluster_before_client_initialization() {
    let mut config = crate::config::Config::default();
    config.gateway.storage.redis.enabled = true;
    config.gateway.storage.redis.cluster = true;

    let error = match ServerBuilder::new().with_config(config).build().await {
        Err(error) => error,
        Ok(_) => panic!("server builder must reject unsupported Redis cluster mode"),
    };

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
async fn server_builder_rejects_invalid_programmatic_config() {
    let mut config = valid_programmatic_config();
    config.gateway.auth.enable_jwt = true;
    config.gateway.auth.jwt_secret = "short".to_string();

    let error = match ServerBuilder::new().with_config(config).build().await {
        Err(error) => error,
        Ok(_) => panic!("builder must reject invalid auth before initialization"),
    };
    assert!(error.to_string().contains("JWT secret"));
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn gateway_rejects_invalid_programmatic_config() {
    let mut config = valid_programmatic_config();
    config.gateway.auth.enable_jwt = true;
    config.gateway.auth.jwt_secret = "short".to_string();

    let error = match crate::Gateway::new(config).await {
        Err(error) => error,
        Ok(_) => panic!("gateway must reject invalid auth before initialization"),
    };
    assert!(error.to_string().contains("JWT secret"));
}

#[tokio::test]
async fn explicit_config_path_missing_file_fails_without_env_fallback() {
    let temp_dir = match tempfile::tempdir() {
        Ok(temp_dir) => temp_dir,
        Err(error) => panic!("failed to create temp dir: {error}"),
    };
    let missing_config = temp_dir.path().join("missing-gateway.yaml");

    let error = match load_explicit_config(&missing_config).await {
        Err(error) => error,
        Ok(_) => panic!("missing explicit config path should fail"),
    };
    let message = error.to_string();

    assert!(message.contains("Failed to load explicit configuration file"));
    assert!(message.contains("Failed to read config file"));
    assert!(!message.contains("environment"));
}

#[tokio::test]
async fn explicit_config_path_parse_error_fails_without_env_fallback() {
    let mut config_file = match tempfile::NamedTempFile::new() {
        Ok(config_file) => config_file,
        Err(error) => panic!("failed to create temp file: {error}"),
    };
    if let Err(error) = config_file.write_all(b"server: [\n") {
        panic!("failed to write invalid config: {error}");
    }

    let error = match load_explicit_config(config_file.path()).await {
        Err(error) => error,
        Ok(_) => panic!("invalid explicit config path should fail"),
    };
    let message = error.to_string();

    assert!(message.contains("Failed to load explicit configuration file"));
    assert!(message.contains("Failed to parse config"));
    assert!(!message.contains("environment"));
}

#[tokio::test]
async fn default_config_path_can_fall_back_to_env_when_file_is_missing() {
    let _env_lock = GATEWAY_ENV_LOCK.lock().await;
    let _env = EnvGuard::with_minimal_gateway_config();
    let temp_dir = match tempfile::tempdir() {
        Ok(temp_dir) => temp_dir,
        Err(error) => panic!("failed to create temp dir: {error}"),
    };
    let missing_config = temp_dir.path().join("missing-gateway.yaml");

    let config = match load_default_config_or_env(&missing_config).await {
        Ok(config) => config,
        Err(error) => panic!("default config path should fall back to env: {error}"),
    };

    assert_eq!(config.providers().len(), 1);
    assert_eq!(config.providers()[0].name, "openai");
}

#[test]
fn test_request_metrics_creation() {
    let metrics = ServerRequestMetrics {
        request_id: "req-123".to_string(),
        method: "GET".to_string(),
        path: "/health".to_string(),
        status_code: 200,
        response_time_ms: 50,
        request_size: 0,
        response_size: 100,
        user_agent: Some("test-agent".to_string()),
        client_ip: Some("127.0.0.1".to_string()),
        user_id: None,
        api_key_id: None,
    };

    assert_eq!(metrics.request_id, "req-123");
    assert_eq!(metrics.method, "GET");
    assert_eq!(metrics.status_code, 200);
}

#[tokio::test]
async fn enabled_audit_logging_is_constructed_in_gateway_state() {
    let mut config = valid_programmatic_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.enterprise.audit_logging = true;

    let server = HttpServer::new(&config)
        .await
        .unwrap_or_else(|error| panic!("audit-enabled server must initialize: {error}"));

    assert!(server.state().audit_logger.is_enabled());
    assert!(server.state().audit_logger.should_log_path("/v1/models"));
    assert!(!server.state().audit_logger.should_log_path("/health"));
}
