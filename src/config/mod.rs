//! Configuration management for the Gateway
//!
//! This module handles loading, validation, and management of all gateway configuration.
//! Canonical server-side models live under `crate::config::models::*`.

pub mod builder;
pub mod models;
pub mod validation;

pub use validation::Validate;

use crate::config::models::auth::AuthConfig;
use crate::config::models::gateway::GatewayConfig;
use crate::config::models::monitoring::MonitoringConfig;
use crate::config::models::provider::ProviderConfig;
use crate::config::models::router::GatewayRouterConfig;
use crate::config::models::server::ServerConfig;
use crate::config::models::storage::StorageConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use regex::Regex;
use std::path::Path;
use tracing::{debug, info, warn};

/// Canonical alias for gateway server runtime configuration.
pub type GatewayServerConfig = crate::config::models::server::ServerConfig;
/// Canonical alias for gateway provider runtime configuration.
pub type GatewayProviderConfig = crate::config::models::provider::ProviderConfig;

/// Substitutes `${VAR_NAME}` and `$VAR_NAME` patterns in a string with
/// corresponding environment variable values. Unresolved variables are left as-is
/// and a warning is emitted.
fn substitute_env_vars(input: &str) -> String {
    // Match ${VAR_NAME} first, then bare $VAR_NAME
    let re = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)")
        .expect("static regex is valid");

    re.replace_all(input, |caps: &regex::Captures<'_>| {
        // Group 1: ${VAR_NAME}, group 2: $VAR_NAME
        let var_name = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or("");

        match std::env::var(var_name) {
            Ok(val) => val,
            Err(_) => {
                warn!(
                    "Config env var '{}' is not set; leaving placeholder as-is",
                    var_name
                );
                caps[0].to_string()
            }
        }
    })
    .into_owned()
}

/// Main configuration struct for the Gateway
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Gateway configuration
    pub gateway: GatewayConfig,
}

impl Config {
    /// Load configuration from file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("Loading configuration from: {:?}", path);

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| GatewayError::Config(format!("Failed to read config file: {}", e)))?;

        let content = substitute_env_vars(&content);

        let gateway: GatewayConfig = serde_yml::from_str(&content)
            .map_err(|e| GatewayError::Config(format!("Failed to parse config: {}", e)))?;

        let config = Self { gateway };

        // Configuration
        config.validate()?;

        debug!("Configuration loaded successfully");
        Ok(config)
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        info!("Loading configuration from environment variables");

        let gateway = GatewayConfig::from_env()?;
        let config = Self { gateway };

        config.validate()?;
        Ok(config)
    }

    /// Get server configuration
    pub fn server(&self) -> &ServerConfig {
        &self.gateway.server
    }

    /// Get providers configuration
    pub fn providers(&self) -> &[ProviderConfig] {
        &self.gateway.providers
    }

    /// Get router settings
    pub fn router(&self) -> &GatewayRouterConfig {
        &self.gateway.router
    }

    /// Get storage configuration
    pub fn storage(&self) -> &StorageConfig {
        &self.gateway.storage
    }

    /// Get auth configuration
    pub fn auth(&self) -> &AuthConfig {
        &self.gateway.auth
    }

    /// Get monitoring configuration
    pub fn monitoring(&self) -> &MonitoringConfig {
        &self.gateway.monitoring
    }

    /// Validate the entire configuration
    pub fn validate(&self) -> Result<()> {
        debug!("Validating configuration");

        // Single validation entry-point via Validate trait implementations.
        validation::Validate::validate(&self.gateway)
            .map_err(|e| GatewayError::Config(format!("Gateway config error: {}", e)))?;

        // Warn about insecure configurations
        crate::config::models::auth::warn_insecure_config(&self.gateway.auth);

        debug!("Configuration validation completed");
        Ok(())
    }

    /// Merge with another configuration (other takes precedence)
    pub fn merge(mut self, other: Self) -> Self {
        self.gateway = self.gateway.merge(other.gateway);
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.gateway)
            .map_err(|e| GatewayError::Config(format!("Failed to serialize config to JSON: {}", e)))
    }

    /// Convert to YAML string
    pub fn to_yaml(&self) -> Result<String> {
        serde_yml::to_string(&self.gateway)
            .map_err(|e| GatewayError::Config(format!("Failed to serialize config to YAML: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_substitute_env_vars_braces() {
        // SAFETY: test-only mutation of env, not run in parallel with other tests touching this var
        unsafe { std::env::set_var("TEST_HOST", "example.com") };
        let result = substitute_env_vars("host: ${TEST_HOST}");
        assert_eq!(result, "host: example.com");
        unsafe { std::env::remove_var("TEST_HOST") };
    }

    #[test]
    fn test_substitute_env_vars_bare() {
        // SAFETY: test-only mutation of env, not run in parallel with other tests touching this var
        unsafe { std::env::set_var("TEST_PORT", "9000") };
        let result = substitute_env_vars("port: $TEST_PORT");
        assert_eq!(result, "port: 9000");
        unsafe { std::env::remove_var("TEST_PORT") };
    }

    #[test]
    fn test_substitute_env_vars_missing_leaves_placeholder() {
        // SAFETY: test-only removal, unique var name avoids interference
        unsafe { std::env::remove_var("DEFINITELY_NOT_SET_XYZ") };
        let result = substitute_env_vars("key: ${DEFINITELY_NOT_SET_XYZ}");
        assert_eq!(result, "key: ${DEFINITELY_NOT_SET_XYZ}");
    }

    #[tokio::test]
    async fn test_config_from_file() {
        let config_content = r#"
server:
  host: "127.0.0.1"
  port: 8080
  workers: 4

providers:
  - name: "openai"
    provider_type: "openai"
    api_key: "test-key"
    api_base: "https://api.openai.com/v1"

router:
  strategy: "round_robin"
  circuit_breaker:
    failure_threshold: 5
    recovery_timeout: 30

storage:
  database:
    url: "postgresql://localhost/gateway"
  redis:
    url: "redis://localhost:6379"

auth:
  jwt_secret: "test-secret-that-is-at-least-32-characters-long-for-security"
  api_key_header: "Authorization"

monitoring:
  metrics:
    enabled: true
    port: 9090
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = Config::from_file(temp_file.path()).await.unwrap();

        assert_eq!(config.server().host, "127.0.0.1");
        assert_eq!(config.server().port, 8080);
        assert_eq!(config.providers().len(), 1);
        assert_eq!(config.providers()[0].name, "openai");
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();

        let json = config.to_json().unwrap();
        assert!(!json.is_empty());

        let yaml = config.to_yaml().unwrap();
        assert!(!yaml.is_empty());
    }
}
