//! Configuration management for the Gateway
//!
//! This module handles loading, validation, and management of all gateway configuration.
//! Canonical server-side models live under `crate::config::models::*`.

pub mod builder;
#[cfg(test)]
mod kubernetes_tests;
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
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;
use tracing::{debug, info};

const REDACTED_SECRET: &str = "[REDACTED]";

/// Canonical alias for gateway server runtime configuration.
pub type GatewayServerConfig = crate::config::models::server::ServerConfig;
/// Canonical alias for gateway provider runtime configuration.
pub type GatewayProviderConfig = crate::config::models::provider::ProviderConfig;

/// Substitutes `${VAR_NAME}` and shell-style `$VAR_NAME` patterns in a string
/// with corresponding environment variable values.
///
/// Missing braced variables are treated as configuration errors so explicit
/// placeholders cannot accidentally reach runtime as literal secrets or URLs.
/// Bare `$VAR_NAME` keeps legacy convenience substitution, but unresolved bare
/// tokens are left literal to avoid rejecting ordinary dollar-containing values.
fn substitute_env_vars(input: &str) -> Result<String> {
    let env_re =
        Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|(^|[^A-Za-z0-9_$])\$([A-Za-z_][A-Za-z0-9_]*)")
            .expect("static regex is valid");
    let mut missing_vars = BTreeSet::new();
    let mut substituted = String::with_capacity(input.len());

    for line in input.split_inclusive('\n') {
        let (line_body, line_ending) = split_line_ending(line);
        let (config_text, comment) = split_yaml_comment(line_body);
        substituted.push_str(&substitute_env_vars_in_segment(
            config_text,
            &env_re,
            &mut missing_vars,
        ));
        substituted.push_str(comment);
        substituted.push_str(line_ending);
    }

    if !missing_vars.is_empty() {
        let missing = missing_vars.into_iter().collect::<Vec<_>>().join(", ");
        return Err(GatewayError::Config(format!(
            "Missing environment variables referenced by config: {}",
            missing
        )));
    }

    Ok(substituted)
}

fn substitute_env_vars_in_segment(
    segment: &str,
    env_re: &Regex,
    missing_vars: &mut BTreeSet<String>,
) -> String {
    env_re
        .replace_all(segment, |caps: &regex::Captures<'_>| {
            if let Some(var_match) = caps.get(1) {
                let var_name = var_match.as_str();
                return match std::env::var(var_name) {
                    Ok(val) => val,
                    Err(_) => {
                        missing_vars.insert(var_name.to_string());
                        caps[0].to_string()
                    }
                };
            }

            let prefix = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let var_name = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            match std::env::var(var_name) {
                Ok(val) => format!("{}{}", prefix, val),
                Err(_) => caps[0].to_string(),
            }
        })
        .into_owned()
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, "\n")
    } else {
        (line, "")
    }
}

fn split_yaml_comment(line: &str) -> (&str, &str) {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut double_quote_escaped = false;
    let mut single_quote_escaped = false;
    let mut previous_char = None;

    for (idx, ch) in line.char_indices() {
        if in_double_quote {
            if double_quote_escaped {
                double_quote_escaped = false;
            } else if ch == '\\' {
                double_quote_escaped = true;
            } else if ch == '"' {
                in_double_quote = false;
            }
            previous_char = Some(ch);
            continue;
        }

        if in_single_quote {
            if single_quote_escaped {
                single_quote_escaped = false;
            } else if ch == '\'' && line[idx + ch.len_utf8()..].starts_with('\'') {
                single_quote_escaped = true;
            } else if ch == '\'' {
                in_single_quote = false;
            }
            previous_char = Some(ch);
            continue;
        }

        match ch {
            '\'' => in_single_quote = true,
            '"' => in_double_quote = true,
            '#' if previous_char.is_none_or(char::is_whitespace) => return line.split_at(idx),
            _ => {}
        }

        previous_char = Some(ch);
    }

    (line, "")
}

/// Main configuration struct for the Gateway
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Gateway configuration
    pub gateway: GatewayConfig,
}

/// A configuration value plus presence metadata for layered merges.
///
/// Runtime configuration structs remain source-compatible and contain only
/// their established public fields. This overlay carries the otherwise-lost
/// distinction between an omitted `storage.redis.cluster` value and an
/// explicit `false`. It is an intermediate layer and is not finally validated
/// until callers merge it into a runtime [`Config`] and validate that result.
#[derive(Debug, Clone)]
pub struct ConfigOverlay {
    config: Config,
    redis_cluster: Option<bool>,
    env_presence: Option<models::gateway::env_config::GatewayEnvPresence>,
}

impl ConfigOverlay {
    /// Create a programmatic overlay from an existing configuration.
    pub fn from_config(config: Config) -> Self {
        let redis_cluster = config.gateway.storage.redis.cluster.then_some(true);
        Self {
            config,
            redis_cluster,
            env_presence: None,
        }
    }

    /// Explicitly override Redis cluster mode in this overlay.
    pub fn with_redis_cluster(mut self, cluster: bool) -> Self {
        self.config.gateway.storage.redis.cluster = cluster;
        self.redis_cluster = Some(cluster);
        self
    }

    /// Discard overlay metadata without performing final configuration validation.
    pub fn into_config(self) -> Config {
        self.config
    }
}

#[derive(Default, Deserialize)]
struct GatewayConfigPresence {
    #[serde(default)]
    storage: StorageConfigPresence,
}

#[derive(Default, Deserialize)]
struct StorageConfigPresence {
    #[serde(default)]
    redis: RedisConfigPresence,
}

#[derive(Default, Deserialize)]
struct RedisConfigPresence {
    #[serde(default)]
    cluster: Option<bool>,
}

impl Config {
    /// Load configuration from file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = Self::overlay_from_file(path).await?.into_config();
        config.validate()?;
        debug!("Configuration loaded successfully");
        Ok(config)
    }

    /// Parse a presence-aware, not-yet-finally-validated overlay from a file.
    pub async fn overlay_from_file<P: AsRef<Path>>(path: P) -> Result<ConfigOverlay> {
        let path = path.as_ref();
        info!("Loading configuration from: {:?}", path);

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| GatewayError::Config(format!("Failed to read config file: {}", e)))?;

        let content = substitute_env_vars(&content)?;

        let gateway: GatewayConfig = serde_yml::from_str(&content)
            .map_err(|e| GatewayError::Config(format!("Failed to parse config: {}", e)))?;
        let presence: GatewayConfigPresence = serde_yml::from_str(&content)
            .map_err(|e| GatewayError::Config(format!("Failed to parse config: {}", e)))?;

        let config = Self { gateway };

        Ok(ConfigOverlay {
            config,
            redis_cluster: presence.storage.redis.cluster,
            env_presence: None,
        })
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let config = Self::overlay_from_env()?.into_config();
        config.validate()?;
        Ok(config)
    }

    /// Parse a presence-aware, not-yet-finally-validated environment overlay.
    pub fn overlay_from_env() -> Result<ConfigOverlay> {
        info!("Loading configuration from environment variables");

        let layer = GatewayConfig::from_env_with_redis_cluster_presence()?;
        let gateway = layer.gateway;
        let config = Self { gateway };

        Ok(ConfigOverlay {
            config,
            redis_cluster: layer.redis_cluster,
            env_presence: Some(layer.presence),
        })
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

    /// Merge another materialized configuration using legacy default-aware semantics.
    ///
    /// Use [`Self::merge_overlay`] when an explicit
    /// `storage.redis.cluster=false` must take precedence.
    pub fn merge(mut self, other: Self) -> Self {
        self.gateway = self.gateway.merge(other.gateway);
        self
    }

    /// Merge an overlay while preserving explicit `storage.redis.cluster` values.
    ///
    /// The merged configuration must be validated before runtime use.
    pub fn merge_overlay(mut self, overlay: ConfigOverlay) -> Self {
        self.gateway = match overlay.env_presence {
            Some(presence) => self.gateway.merge_env_overlay(
                overlay.config.gateway,
                overlay.redis_cluster,
                &presence,
            ),
            None => self
                .gateway
                .merge_with_redis_cluster_override(overlay.config.gateway, overlay.redis_cluster),
        };
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.sanitized_gateway_for_export())
            .map_err(|e| GatewayError::Config(format!("Failed to serialize config to JSON: {}", e)))
    }

    /// Convert to YAML string
    pub fn to_yaml(&self) -> Result<String> {
        serde_yml::to_string(&self.sanitized_gateway_for_export())
            .map_err(|e| GatewayError::Config(format!("Failed to serialize config to YAML: {}", e)))
    }

    fn sanitized_gateway_for_export(&self) -> GatewayConfig {
        let mut gateway = self.gateway.clone();

        for provider in &mut gateway.providers {
            redact_string(&mut provider.api_key);
        }

        redact_string(&mut gateway.auth.jwt_secret);
        redact_optional_string(&mut gateway.auth.api_key_hmac_secret);

        if let Some(s3) = &mut gateway.storage.files.s3 {
            redact_string(&mut s3.access_key_id);
            redact_string(&mut s3.secret_access_key);
        }

        if let Some(vector_db) = &mut gateway.storage.vector_db {
            redact_string(&mut vector_db.api_key);
        }

        if let Some(sso) = &mut gateway.enterprise.sso {
            redact_string(&mut sso.client_secret);
        }

        gateway
    }
}

fn redact_string(value: &mut String) {
    if !value.is_empty() {
        *value = REDACTED_SECRET.to_string();
    }
}

fn redact_optional_string(value: &mut Option<String>) {
    if let Some(secret) = value
        && !secret.is_empty()
    {
        *secret = REDACTED_SECRET.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::enterprise::SsoConfig;
    use crate::config::models::file_storage::{S3Config, VectorDbConfig};
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_substitute_env_vars_braces() {
        // SAFETY: test-only mutation of env, not run in parallel with other tests touching this var
        unsafe { std::env::set_var("TEST_HOST", "example.com") };
        let result = substitute_env_vars("host: ${TEST_HOST}").unwrap();
        assert_eq!(result, "host: example.com");
        unsafe { std::env::remove_var("TEST_HOST") };
    }

    #[test]
    fn test_substitute_env_vars_bare() {
        // SAFETY: test-only mutation of env, not run in parallel with other tests touching this var
        unsafe { std::env::set_var("TEST_PORT", "9000") };
        let result = substitute_env_vars("port: $TEST_PORT").unwrap();
        assert_eq!(result, "port: 9000");
        unsafe { std::env::remove_var("TEST_PORT") };
    }

    #[test]
    fn test_substitute_env_vars_missing_fails_with_var_name() {
        // SAFETY: test-only removal, unique var name avoids interference
        unsafe { std::env::remove_var("DEFINITELY_NOT_SET_XYZ") };
        let err = substitute_env_vars("key: ${DEFINITELY_NOT_SET_XYZ}").unwrap_err();
        assert!(err.to_string().contains("DEFINITELY_NOT_SET_XYZ"));
    }

    #[test]
    fn test_substitute_env_vars_missing_lists_each_var_once() {
        // SAFETY: test-only removal, unique var names avoid interference
        unsafe {
            std::env::remove_var("DEFINITELY_NOT_SET_A");
            std::env::remove_var("DEFINITELY_NOT_SET_B");
        };
        let err = substitute_env_vars(
            "a: ${DEFINITELY_NOT_SET_A}\nb: ${DEFINITELY_NOT_SET_B}\nagain: ${DEFINITELY_NOT_SET_A}",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("DEFINITELY_NOT_SET_A"));
        assert!(err.contains("DEFINITELY_NOT_SET_B"));
        assert_eq!(err.matches("DEFINITELY_NOT_SET_A").count(), 1);
    }

    #[test]
    fn test_substitute_env_vars_unresolved_bare_is_literal() {
        // SAFETY: test-only removal, unique var name avoids interference
        unsafe { std::env::remove_var("DEFINITELY_NOT_SET_LITERAL_TOKEN") };
        let result =
            substitute_env_vars("password: pa$word\nkey: $DEFINITELY_NOT_SET_LITERAL_TOKEN")
                .unwrap();

        assert_eq!(
            result,
            "password: pa$word\nkey: $DEFINITELY_NOT_SET_LITERAL_TOKEN"
        );
    }

    #[test]
    fn test_substitute_env_vars_ignores_yaml_comments() {
        // SAFETY: test-only env mutations use unique var names
        unsafe {
            std::env::set_var("COMMENT_REAL_VALUE", "ok");
            std::env::remove_var("DEFINITELY_NOT_SET_COMMENT_ONLY_A");
            std::env::remove_var("DEFINITELY_NOT_SET_COMMENT_ONLY_B");
        };

        let result = substitute_env_vars(
            "# set ${DEFINITELY_NOT_SET_COMMENT_ONLY_A}\nkey: ${COMMENT_REAL_VALUE} # ${DEFINITELY_NOT_SET_COMMENT_ONLY_B}\nurl: \"https://example.test/#fragment\"\nsingle: 'it''s # literal'",
        )
        .unwrap();

        assert_eq!(
            result,
            "# set ${DEFINITELY_NOT_SET_COMMENT_ONLY_A}\nkey: ok # ${DEFINITELY_NOT_SET_COMMENT_ONLY_B}\nurl: \"https://example.test/#fragment\"\nsingle: 'it''s # literal'"
        );
        unsafe { std::env::remove_var("COMMENT_REAL_VALUE") };
    }

    #[test]
    fn test_substitute_env_vars_does_not_expand_env_value_again() {
        // SAFETY: test-only env mutations use unique var names
        unsafe {
            std::env::set_var("OUTER_SECRET_WITH_DOLLAR", "pa$INNER_SECRET_TOKEN");
            std::env::set_var("INNER_SECRET_TOKEN", "expanded");
        };

        let result =
            substitute_env_vars("secret: ${OUTER_SECRET_WITH_DOLLAR}\nnext: $INNER_SECRET_TOKEN")
                .unwrap();

        assert_eq!(result, "secret: pa$INNER_SECRET_TOKEN\nnext: expanded");
        unsafe {
            std::env::remove_var("OUTER_SECRET_WITH_DOLLAR");
            std::env::remove_var("INNER_SECRET_TOKEN");
        };
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
    base_url: "https://api.openai.com/v1"

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
  jwt_secret: "TestSecretThatIsAtLeast32CharsLong123!"
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

    #[tokio::test]
    async fn test_config_from_file_rejects_missing_env_var() {
        let config_content = r#"
server:
  host: "127.0.0.1"
  port: 8080

providers:
  - name: "openai"
    provider_type: "openai"
    api_key: "${DEFINITELY_NOT_SET_CONFIG_FILE_VAR}"

router:
  strategy: "round_robin"

storage:
  database:
    url: "postgresql://localhost/gateway"

auth:
  jwt_secret: "TestSecretThatIsAtLeast32CharsLong123!"

monitoring:
  metrics:
    enabled: true
"#;

        unsafe { std::env::remove_var("DEFINITELY_NOT_SET_CONFIG_FILE_VAR") };

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let err = Config::from_file(temp_file.path()).await.unwrap_err();

        assert!(
            err.to_string()
                .contains("DEFINITELY_NOT_SET_CONFIG_FILE_VAR")
        );
    }

    #[test]
    fn test_gateway_config_rejects_unknown_top_level_field() {
        let err = serde_yml::from_str::<GatewayConfig>(
            r#"
schema_version: "1.0"
server: {}
providers: []
router: {}
storage:
  database:
    url: "postgresql://localhost/test"
  redis:
    url: "redis://localhost:6379"
auth: {}
monitoring: {}
typo_field: true
"#,
        )
        .unwrap_err()
        .to_string();

        assert!(
            err.contains("unknown field") || err.contains("typo_field"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_gateway_yaml_example_matches_config_schema() {
        use crate::config::models::gateway::UnpricedModelPolicy;

        let example_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/gateway.yaml.example");
        let content = std::fs::read_to_string(example_path).unwrap();
        let content = content
            .replace("${OPENAI_API_KEY}", "sk-test-openai")
            .replace("${ANTHROPIC_API_KEY}", "sk-ant-test")
            .replace(
                "${LITELLM_JWT_SECRET}",
                "StrongJwtSecretWithMixedCaseAndNumbers1234!",
            );

        let gateway: GatewayConfig = serde_yml::from_str(&content).unwrap();
        Config {
            gateway: gateway.clone(),
        }
        .validate()
        .unwrap();
        assert_eq!(
            gateway.pricing.unpriced_model_policy,
            UnpricedModelPolicy::Reject
        );
        assert_eq!(gateway.pricing.unpriced_fallback_cost_per_1k_tokens, None);
    }

    #[test]
    fn test_gateway_dev_yaml_example_matches_config_schema_and_prices_all_models() {
        use crate::config::models::gateway::UnpricedModelPolicy;
        use crate::core::pricing::embedded_default_pricing_models;
        use crate::core::pricing_service::DEFAULT_PRICING_SOURCE;

        let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config/gateway.dev.yaml.example");
        let content = std::fs::read_to_string(example_path).unwrap();
        let gateway: GatewayConfig = serde_yml::from_str(&content).unwrap();

        Config {
            gateway: gateway.clone(),
        }
        .validate()
        .unwrap();
        assert_eq!(
            gateway.pricing.source.as_deref(),
            Some(DEFAULT_PRICING_SOURCE)
        );
        assert!(!gateway.pricing.allow_degraded);

        let priced_models = embedded_default_pricing_models().unwrap();
        let unpriced_models: Vec<&str> = gateway
            .providers
            .iter()
            .filter(|provider| provider.enabled)
            .flat_map(|provider| provider.models.iter())
            .filter(|model| !priced_models.contains_key(*model))
            .map(String::as_str)
            .collect();

        assert!(
            unpriced_models.is_empty()
                || (gateway.pricing.unpriced_model_policy == UnpricedModelPolicy::AllowUnpriced
                    && gateway
                        .pricing
                        .unpriced_fallback_cost_per_1k_tokens
                        .is_some()),
            "enabled dev models {unpriced_models:?} need embedded prices or an explicit fallback"
        );
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

    #[test]
    fn test_config_serialization_redacts_secrets() {
        let provider_secret = "issue677-provider-secret-sentinel";
        let jwt_secret = "Issue677JwtSecretSentinelWithMixedCase123!";
        let hmac_secret = "issue677-hmac-secret-sentinel";
        let s3_access_key_id = "issue677-s3-access-key-id-sentinel";
        let s3_secret_access_key = "issue677-s3-secret-access-key-sentinel";
        let vector_api_key = "issue677-vector-api-key-sentinel";
        let sso_client_secret = "issue677-sso-client-secret-sentinel";

        let mut config = Config::default();
        config.gateway.providers.push(ProviderConfig {
            name: "openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: provider_secret.to_string(),
            ..ProviderConfig::default()
        });
        config.gateway.auth.jwt_secret = jwt_secret.to_string();
        config.gateway.auth.api_key_hmac_secret = Some(hmac_secret.to_string());
        config.gateway.storage.files.s3 = Some(S3Config {
            bucket: "bucket".to_string(),
            region: "us-east-1".to_string(),
            access_key_id: s3_access_key_id.to_string(),
            secret_access_key: s3_secret_access_key.to_string(),
            endpoint: None,
        });
        config.gateway.storage.vector_db = Some(VectorDbConfig {
            db_type: "pinecone".to_string(),
            url: "https://vector.example.test".to_string(),
            api_key: vector_api_key.to_string(),
            index_name: "default".to_string(),
            allow_degraded: false,
        });
        config.gateway.enterprise.sso = Some(SsoConfig {
            provider: "okta".to_string(),
            client_id: "client-id".to_string(),
            client_secret: sso_client_secret.to_string(),
            redirect_url: "https://gateway.example.test/callback".to_string(),
            settings: std::collections::HashMap::new(),
        });

        let json = match config.to_json() {
            Ok(json) => json,
            Err(error) => panic!("JSON export failed: {error}"),
        };
        let yaml = match config.to_yaml() {
            Ok(yaml) => yaml,
            Err(error) => panic!("YAML export failed: {error}"),
        };

        for exported in [&json, &yaml] {
            assert!(!exported.contains(provider_secret));
            assert!(!exported.contains(jwt_secret));
            assert!(!exported.contains(hmac_secret));
            assert!(!exported.contains(s3_access_key_id));
            assert!(!exported.contains(s3_secret_access_key));
            assert!(!exported.contains(vector_api_key));
            assert!(!exported.contains(sso_client_secret));
            assert!(exported.contains(REDACTED_SECRET));
        }

        assert_eq!(config.gateway.providers[0].api_key, provider_secret);
        assert_eq!(config.gateway.auth.jwt_secret, jwt_secret);
        assert_eq!(
            config.gateway.auth.api_key_hmac_secret.as_deref(),
            Some(hmac_secret)
        );
        let s3 = match config.gateway.storage.files.s3.as_ref() {
            Some(s3) => s3,
            None => panic!("S3 config missing after export"),
        };
        assert_eq!(s3.access_key_id, s3_access_key_id);
        assert_eq!(s3.secret_access_key, s3_secret_access_key);
        let vector_db = match config.gateway.storage.vector_db.as_ref() {
            Some(vector_db) => vector_db,
            None => panic!("Vector DB config missing after export"),
        };
        assert_eq!(vector_db.api_key, vector_api_key);
        let sso = match config.gateway.enterprise.sso.as_ref() {
            Some(sso) => sso,
            None => panic!("SSO config missing after export"),
        };
        assert_eq!(sso.client_secret, sso_client_secret);
    }

    #[test]
    fn test_config_serialization_preserves_empty_optional_secret() {
        let mut config = Config::default();
        config.gateway.auth.api_key_hmac_secret = Some(String::new());

        let exported = config.sanitized_gateway_for_export();

        assert_eq!(exported.auth.api_key_hmac_secret.as_deref(), Some(""));
    }
}
