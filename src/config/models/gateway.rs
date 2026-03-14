//! Main gateway configuration

#![allow(missing_docs)]

use super::auth::AuthConfig;
use super::cache::CacheConfig;
use super::enterprise::EnterpriseConfig;
use super::monitoring::MonitoringConfig;
use super::provider::ProviderConfig;
use super::router::GatewayRouterConfig;
use super::server::ServerConfig;
use super::storage::StorageConfig;
use crate::core::types::config::rate_limit::RateLimitConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

const ENV_HOST: &str = "LITELLM_HOST";
const ENV_PORT: &str = "LITELLM_PORT";
const ENV_WORKERS: &str = "LITELLM_WORKERS";
const ENV_TIMEOUT: &str = "LITELLM_TIMEOUT";
const ENV_DATABASE_URL: &str = "LITELLM_DATABASE_URL";
const ENV_DATABASE_MAX_CONNECTIONS: &str = "LITELLM_DATABASE_MAX_CONNECTIONS";
const ENV_DATABASE_CONNECTION_TIMEOUT: &str = "LITELLM_DATABASE_CONNECTION_TIMEOUT";
const ENV_DATABASE_SSL: &str = "LITELLM_DATABASE_SSL";
const ENV_DATABASE_ENABLED: &str = "LITELLM_DATABASE_ENABLED";
const ENV_REDIS_URL: &str = "LITELLM_REDIS_URL";
const ENV_REDIS_ENABLED: &str = "LITELLM_REDIS_ENABLED";
const ENV_REDIS_MAX_CONNECTIONS: &str = "LITELLM_REDIS_MAX_CONNECTIONS";
const ENV_REDIS_CONNECTION_TIMEOUT: &str = "LITELLM_REDIS_CONNECTION_TIMEOUT";
const ENV_REDIS_CLUSTER: &str = "LITELLM_REDIS_CLUSTER";
const ENV_ENABLE_JWT: &str = "LITELLM_ENABLE_JWT";
const ENV_ENABLE_API_KEY: &str = "LITELLM_ENABLE_API_KEY";
const ENV_JWT_SECRET: &str = "LITELLM_JWT_SECRET";
const ENV_JWT_EXPIRATION: &str = "LITELLM_JWT_EXPIRATION";
const ENV_API_KEY_HEADER: &str = "LITELLM_API_KEY_HEADER";
const ENV_PROVIDERS: &str = "LITELLM_PROVIDERS";
const ENV_PRICING_SOURCE: &str = "LITELLM_PRICING_SOURCE";
const ENV_CACHE_ENABLED: &str = "LITELLM_CACHE_ENABLED";
const ENV_RATE_LIMIT_ENABLED: &str = "LITELLM_RATE_LIMIT_ENABLED";
const ENV_ENTERPRISE_ENABLED: &str = "LITELLM_ENTERPRISE_ENABLED";
const DEFAULT_PRICING_SOURCE: &str = "config/model_prices_extended.json";

fn env_var(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_env<T>(key: &str) -> crate::utils::error::gateway_error::Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let Some(raw) = env_var(key) else {
        return Ok(None);
    };

    raw.parse::<T>().map(Some).map_err(|error| {
        crate::utils::error::gateway_error::GatewayError::Config(format!(
            "Invalid value for {}: {}",
            key, error
        ))
    })
}

fn parse_env_bool(key: &str) -> crate::utils::error::gateway_error::Result<Option<bool>> {
    let Some(raw) = env_var(key) else {
        return Ok(None);
    };

    let value = match raw.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => {
            return Err(crate::utils::error::gateway_error::GatewayError::Config(
                format!("Invalid boolean value for {}: {}", key, raw),
            ));
        }
    };
    Ok(Some(value))
}

fn parse_env_list(key: &str) -> Option<Vec<String>> {
    env_var(key).map(|raw| {
        raw.split(',')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    })
}

fn required_env(key: &str) -> crate::utils::error::gateway_error::Result<String> {
    env_var(key).ok_or_else(|| {
        crate::utils::error::gateway_error::GatewayError::Config(format!(
            "Missing required env var: {}",
            key
        ))
    })
}

fn provider_env_key(provider_name: &str) -> String {
    provider_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn provider_env_name(provider_name: &str, field: &str) -> String {
    format!(
        "LITELLM_PROVIDER_{}_{}",
        provider_env_key(provider_name),
        field
    )
}

fn load_providers_from_env() -> crate::utils::error::gateway_error::Result<Vec<ProviderConfig>> {
    let provider_names = parse_env_list(ENV_PROVIDERS).ok_or_else(|| {
        crate::utils::error::gateway_error::GatewayError::Config(format!(
            "{} must be set with at least one provider name",
            ENV_PROVIDERS
        ))
    })?;

    if provider_names.is_empty() {
        return Err(crate::utils::error::gateway_error::GatewayError::Config(
            format!("{} must contain at least one provider name", ENV_PROVIDERS),
        ));
    }

    let mut providers = Vec::with_capacity(provider_names.len());

    for name in provider_names {
        let type_key = provider_env_name(&name, "TYPE");
        let api_key_key = provider_env_name(&name, "API_KEY");
        let provider_type = required_env(&type_key)?;
        let selector = provider_type.to_lowercase();
        let skip_api_key = crate::core::providers::registry::get_definition(&selector)
            .map(|def| def.skip_api_key)
            .unwrap_or(false);
        let api_key = if skip_api_key {
            env_var(&api_key_key).unwrap_or_default()
        } else {
            required_env(&api_key_key)?
        };

        let mut provider = ProviderConfig {
            name: name.clone(),
            provider_type,
            api_key,
            ..ProviderConfig::default()
        };

        if let Some(base_url) = env_var(&provider_env_name(&name, "BASE_URL")) {
            provider.base_url = Some(base_url);
        }
        if let Some(api_version) = env_var(&provider_env_name(&name, "API_VERSION")) {
            provider.api_version = Some(api_version);
        }
        if let Some(organization) = env_var(&provider_env_name(&name, "ORGANIZATION")) {
            provider.organization = Some(organization);
        }
        if let Some(project) = env_var(&provider_env_name(&name, "PROJECT")) {
            provider.project = Some(project);
        }

        if let Some(weight) = parse_env::<f32>(&provider_env_name(&name, "WEIGHT"))? {
            provider.weight = weight;
        }
        if let Some(rpm) = parse_env::<u32>(&provider_env_name(&name, "RPM"))? {
            provider.rpm = rpm;
        }
        if let Some(tpm) = parse_env::<u32>(&provider_env_name(&name, "TPM"))? {
            provider.tpm = tpm;
        }
        if let Some(max_concurrent_requests) =
            parse_env::<u32>(&provider_env_name(&name, "MAX_CONCURRENT_REQUESTS"))?
        {
            provider.max_concurrent_requests = max_concurrent_requests;
        }
        if let Some(timeout) = parse_env::<u64>(&provider_env_name(&name, "TIMEOUT"))? {
            provider.timeout = timeout;
        }
        if let Some(max_retries) = parse_env::<u32>(&provider_env_name(&name, "MAX_RETRIES"))? {
            provider.max_retries = max_retries;
        }

        if let Some(enabled) = parse_env_bool(&provider_env_name(&name, "ENABLED"))? {
            provider.enabled = enabled;
        }
        if let Some(models) = parse_env_list(&provider_env_name(&name, "MODELS")) {
            provider.models = models;
        }
        if let Some(tags) = parse_env_list(&provider_env_name(&name, "TAGS")) {
            provider.tags = tags;
        }

        providers.push(provider);
    }

    Ok(providers)
}

/// Pricing source configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayPricingConfig {
    /// Optional pricing source path/URL used by PricingService::new
    #[serde(default = "default_pricing_source")]
    pub source: Option<String>,
}

impl Default for GatewayPricingConfig {
    fn default() -> Self {
        Self {
            source: default_pricing_source(),
        }
    }
}

fn default_pricing_source() -> Option<String> {
    Some(DEFAULT_PRICING_SOURCE.to_string())
}

/// Main gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Configuration schema version
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    /// Server configuration
    pub server: ServerConfig,
    /// Provider configurations
    pub providers: Vec<ProviderConfig>,
    /// Router configuration
    pub router: GatewayRouterConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Authentication configuration
    pub auth: AuthConfig,
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
    /// Caching configuration
    #[serde(default)]
    pub cache: CacheConfig,
    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    /// Enterprise features configuration
    #[serde(default)]
    pub enterprise: EnterpriseConfig,
    /// Pricing configuration
    #[serde(default)]
    pub pricing: GatewayPricingConfig,
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            server: ServerConfig::default(),
            providers: Vec::new(),
            router: GatewayRouterConfig::default(),
            storage: StorageConfig::default(),
            auth: AuthConfig::default(),
            monitoring: MonitoringConfig::default(),
            cache: CacheConfig::default(),
            rate_limit: RateLimitConfig::default(),
            enterprise: EnterpriseConfig::default(),
            pricing: GatewayPricingConfig::default(),
        }
    }
}

impl GatewayConfig {
    pub fn from_env() -> crate::utils::error::gateway_error::Result<Self> {
        let mut config = Self::default();

        if let Some(host) = env_var(ENV_HOST) {
            config.server.host = host;
        }
        if let Some(port) = parse_env::<u16>(ENV_PORT)? {
            config.server.port = port;
        }
        if let Some(workers) = parse_env::<usize>(ENV_WORKERS)? {
            config.server.workers = Some(workers);
        }
        if let Some(timeout) = parse_env::<u64>(ENV_TIMEOUT)? {
            config.server.timeout = timeout;
        }

        if let Some(database_url) = env_var(ENV_DATABASE_URL) {
            config.storage.database.url = database_url;
        }
        if let Some(max_connections) = parse_env::<u32>(ENV_DATABASE_MAX_CONNECTIONS)? {
            config.storage.database.max_connections = max_connections;
        }
        if let Some(connection_timeout) = parse_env::<u64>(ENV_DATABASE_CONNECTION_TIMEOUT)? {
            config.storage.database.connection_timeout = connection_timeout;
        }
        if let Some(ssl) = parse_env_bool(ENV_DATABASE_SSL)? {
            config.storage.database.ssl = ssl;
        }
        if let Some(enabled) = parse_env_bool(ENV_DATABASE_ENABLED)? {
            config.storage.database.enabled = enabled;
        }

        if let Some(redis_url) = env_var(ENV_REDIS_URL) {
            config.storage.redis.url = redis_url;
        }
        if let Some(enabled) = parse_env_bool(ENV_REDIS_ENABLED)? {
            config.storage.redis.enabled = enabled;
        }
        if let Some(max_connections) = parse_env::<u32>(ENV_REDIS_MAX_CONNECTIONS)? {
            config.storage.redis.max_connections = max_connections;
        }
        if let Some(connection_timeout) = parse_env::<u64>(ENV_REDIS_CONNECTION_TIMEOUT)? {
            config.storage.redis.connection_timeout = connection_timeout;
        }
        if let Some(cluster) = parse_env_bool(ENV_REDIS_CLUSTER)? {
            config.storage.redis.cluster = cluster;
        }

        if let Some(enable_jwt) = parse_env_bool(ENV_ENABLE_JWT)? {
            config.auth.enable_jwt = enable_jwt;
        }
        if let Some(enable_api_key) = parse_env_bool(ENV_ENABLE_API_KEY)? {
            config.auth.enable_api_key = enable_api_key;
        }
        if let Some(jwt_secret) = env_var(ENV_JWT_SECRET) {
            config.auth.jwt_secret = jwt_secret;
        } else if config.auth.enable_jwt {
            return Err(crate::utils::error::gateway_error::GatewayError::Config(
                format!(
                    "{} is required when {} is enabled",
                    ENV_JWT_SECRET, ENV_ENABLE_JWT
                ),
            ));
        }
        if let Some(jwt_expiration) = parse_env::<u64>(ENV_JWT_EXPIRATION)? {
            config.auth.jwt_expiration = jwt_expiration;
        }
        if let Some(api_key_header) = env_var(ENV_API_KEY_HEADER) {
            config.auth.api_key_header = api_key_header;
        }

        config.providers = load_providers_from_env()?;

        if let Some(pricing_source) = env_var(ENV_PRICING_SOURCE) {
            config.pricing.source = Some(pricing_source);
        }

        if let Some(enabled) = parse_env_bool(ENV_CACHE_ENABLED)? {
            config.cache.enabled = enabled;
        }
        if let Some(enabled) = parse_env_bool(ENV_RATE_LIMIT_ENABLED)? {
            config.rate_limit.enabled = enabled;
        }
        if let Some(enabled) = parse_env_bool(ENV_ENTERPRISE_ENABLED)? {
            config.enterprise.enabled = enabled;
        }

        Ok(config)
    }
}

impl GatewayConfig {
    /// Merge two configurations, with other taking precedence
    pub fn merge(mut self, other: Self) -> Self {
        self.server = self.server.merge(other.server);

        // Merge providers (other takes precedence for same names)
        let mut provider_map: HashMap<String, ProviderConfig> = self
            .providers
            .into_iter()
            .map(|p| (p.name.clone(), p))
            .collect();

        for provider in other.providers {
            provider_map.insert(provider.name.clone(), provider);
        }

        self.providers = provider_map.into_values().collect();
        self.router = self.router.merge(other.router);
        self.storage = self.storage.merge(other.storage);
        self.auth = self.auth.merge(other.auth);
        self.monitoring = self.monitoring.merge(other.monitoring);
        self.cache = self.cache.merge(other.cache);
        self.rate_limit = self.rate_limit.merge(other.rate_limit);
        self.enterprise = self.enterprise.merge(other.enterprise);
        self.pricing = other.pricing;

        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        crate::config::validation::Validate::validate(self)
    }

    /// Get provider by name
    pub fn get_provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.name == name)
    }

    /// Get providers by type
    pub fn get_providers_by_type(&self, provider_type: &str) -> Vec<&ProviderConfig> {
        self.providers
            .iter()
            .filter(|p| p.provider_type == provider_type)
            .collect()
    }

    /// Get providers by tag
    pub fn get_providers_by_tag(&self, tag: &str) -> Vec<&ProviderConfig> {
        self.providers
            .iter()
            .filter(|p| p.tags.contains(&tag.to_string()))
            .collect()
    }

    /// Check if a feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        match feature {
            "jwt_auth" => self.auth.enable_jwt,
            "api_key_auth" => self.auth.enable_api_key,
            "rbac" => self.auth.rbac.enabled,
            "metrics" => self.monitoring.metrics.enabled,
            "tracing" => self.monitoring.tracing.enabled,
            "health_checks" => true, // Always enabled
            "caching" => self.cache.enabled,
            "semantic_cache" => self.cache.semantic_cache,
            "rate_limiting" => self.rate_limit.enabled,
            "enterprise" => self.enterprise.enabled,
            "sso" => self.enterprise.sso.is_some(),
            "audit_logging" => self.enterprise.audit_logging,
            "advanced_analytics" => self.enterprise.advanced_analytics,
            _ => false,
        }
    }

    /// Get environment-specific configuration
    pub fn for_environment(&self, env: &str) -> Self {
        let mut config = self.clone();

        match env {
            "development" => {
                config.server.dev_mode = true;
                config.monitoring.tracing.enabled = true;
            }
            "production" => {
                config.server.dev_mode = false;
                config.monitoring.metrics.enabled = true;
                config.monitoring.tracing.enabled = true;
            }
            "testing" => {
                config.server.dev_mode = true;
                config.cache.enabled = false;
                config.rate_limit.enabled = false;
            }
            _ => {}
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::enterprise::SsoConfig;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const TEST_ENV_KEYS: [&str; 21] = [
        ENV_HOST,
        ENV_PORT,
        ENV_WORKERS,
        ENV_TIMEOUT,
        ENV_DATABASE_URL,
        ENV_ENABLE_JWT,
        ENV_ENABLE_API_KEY,
        ENV_JWT_SECRET,
        ENV_JWT_EXPIRATION,
        ENV_API_KEY_HEADER,
        ENV_PROVIDERS,
        ENV_PRICING_SOURCE,
        ENV_CACHE_ENABLED,
        ENV_RATE_LIMIT_ENABLED,
        ENV_ENTERPRISE_ENABLED,
        "LITELLM_PROVIDER_OPENAI_TYPE",
        "LITELLM_PROVIDER_OPENAI_API_KEY",
        "LITELLM_PROVIDER_OPENAI_BASE_URL",
        "LITELLM_PROVIDER_OPENAI_MODELS",
        "LITELLM_PROVIDER_OPENAI_TAGS",
        "LITELLM_PROVIDER_OPENAI_MAX_RETRIES",
    ];

    fn clear_test_env() {
        for key in TEST_ENV_KEYS {
            unsafe { env::remove_var(key) };
        }
    }

    fn create_test_provider(name: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: "openai".to_string(),
            api_key: "test-api-key".to_string(),
            ..ProviderConfig::default()
        }
    }

    fn create_valid_config() -> GatewayConfig {
        let mut config = GatewayConfig {
            providers: vec![create_test_provider("test-provider")],
            ..Default::default()
        };
        config.storage.database.enabled = true;
        config.storage.database.url = "postgres://localhost/test".to_string();
        config.auth.jwt_secret = "StrongJwtSecretWithMixedCaseAndNumbers1234!".to_string();
        config
    }

    // ==================== GatewayConfig Default Tests ====================

    #[test]
    fn test_gateway_config_default() {
        let config = GatewayConfig::default();
        assert!(config.providers.is_empty());
        assert_eq!(config.server.port, 8000);
    }

    #[test]
    fn test_gateway_config_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_HOST, "127.0.0.1");
            env::set_var(ENV_PORT, "18080");
            env::set_var(
                ENV_DATABASE_URL,
                "postgresql://env-user:env-pass@localhost/env-db",
            );
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
            env::set_var(
                "LITELLM_PROVIDER_OPENAI_BASE_URL",
                "https://api.openai.com/v1",
            );
            env::set_var("LITELLM_PROVIDER_OPENAI_MODELS", "gpt-4o,gpt-4.1");
            env::set_var("LITELLM_PROVIDER_OPENAI_TAGS", "prod,primary");
            env::set_var("LITELLM_PROVIDER_OPENAI_MAX_RETRIES", "5");
            env::set_var(ENV_PRICING_SOURCE, "/tmp/pricing-test.json");
        }

        let config = match GatewayConfig::from_env() {
            Ok(config) => config,
            Err(error) => panic!("expected GatewayConfig::from_env() to succeed: {}", error),
        };
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 18080);
        assert_eq!(
            config.storage.database.url,
            "postgresql://env-user:env-pass@localhost/env-db"
        );
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].name, "openai");
        assert_eq!(config.providers[0].provider_type, "openai");
        assert_eq!(config.providers[0].api_key, "sk-test-key");
        assert_eq!(
            config.providers[0].base_url,
            Some("https://api.openai.com/v1".to_string())
        );
        assert_eq!(
            config.providers[0].models,
            vec!["gpt-4o".to_string(), "gpt-4.1".to_string()]
        );
        assert_eq!(
            config.providers[0].tags,
            vec!["prod".to_string(), "primary".to_string()]
        );
        assert_eq!(config.providers[0].max_retries, 5);
        assert_eq!(
            config.pricing.source,
            Some("/tmp/pricing-test.json".to_string())
        );

        clear_test_env();
    }

    #[test]
    fn test_gateway_config_from_env_allows_local_provider_without_api_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "vllm");
            env::set_var("LITELLM_PROVIDER_VLLM_TYPE", "vllm");
        }

        let config = match GatewayConfig::from_env() {
            Ok(config) => config,
            Err(error) => panic!("expected GatewayConfig::from_env() to succeed: {}", error),
        };
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].provider_type, "vllm");
        assert_eq!(config.providers[0].api_key, "");

        clear_test_env();
    }

    #[test]
    fn test_gateway_config_from_env_requires_providers() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
        }

        let result = GatewayConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(ENV_PROVIDERS));

        clear_test_env();
    }

    #[test]
    fn test_gateway_config_from_env_invalid_port() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_PORT, "invalid-port");
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
        }

        let result = GatewayConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(ENV_PORT));

        clear_test_env();
    }

    // ==================== GatewayConfig Validation Tests ====================

    #[test]
    fn test_gateway_config_validate_success() {
        let config = create_valid_config();
        let result = config.validate();
        if let Err(e) = &result {
            eprintln!("Validation error: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_gateway_config_validate_empty_schema_version() {
        let mut config = create_valid_config();
        config.schema_version = "".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Schema version"));
    }

    #[test]
    fn test_gateway_config_validate_unsupported_schema_version() {
        let mut config = create_valid_config();
        config.schema_version = "2.0".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported schema version"));
    }

    #[test]
    fn test_gateway_config_default_schema_version() {
        let config = GatewayConfig::default();
        assert_eq!(config.schema_version, "1.0");
    }

    #[test]
    fn test_gateway_config_validate_port_zero() {
        let mut config = create_valid_config();
        config.server.port = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("port"));
    }

    #[test]
    fn test_gateway_config_validate_no_providers() {
        let mut config = create_valid_config();
        config.providers = vec![];
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("provider"));
    }

    #[test]
    fn test_gateway_config_validate_empty_provider_name() {
        let mut config = create_valid_config();
        config.providers[0].name = "".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("name"));
    }

    #[test]
    fn test_gateway_config_validate_unsupported_provider_type() {
        let mut config = create_valid_config();
        config.providers[0].provider_type = "unsupported_provider".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not supported"));
    }

    #[test]
    fn test_gateway_config_validate_duplicate_provider_names() {
        let mut config = create_valid_config();
        config.providers.push(create_test_provider("test-provider"));
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate"));
    }

    #[test]
    fn test_gateway_config_validate_empty_api_key() {
        let mut config = create_valid_config();
        config.providers[0].api_key = "".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }

    #[test]
    fn test_gateway_config_validate_local_provider_without_api_key() {
        let mut config = create_valid_config();
        config.providers[0].provider_type = "vllm".to_string();
        config.providers[0].api_key = "".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_gateway_config_validate_empty_database_url() {
        let mut config = create_valid_config();
        config.storage.database.url = "".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Database URL"));
    }

    #[test]
    fn test_gateway_config_validate_empty_jwt_secret() {
        let mut config = create_valid_config();
        config.auth.jwt_secret = "".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("JWT secret"));
    }

    // ==================== GatewayConfig Provider Lookup Tests ====================

    #[test]
    fn test_gateway_config_get_provider_found() {
        let config = create_valid_config();
        let provider = config.get_provider("test-provider");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name, "test-provider");
    }

    #[test]
    fn test_gateway_config_get_provider_not_found() {
        let config = create_valid_config();
        let provider = config.get_provider("non-existent");
        assert!(provider.is_none());
    }

    #[test]
    fn test_gateway_config_get_providers_by_type() {
        let mut config = create_valid_config();
        let mut anthropic_provider = create_test_provider("anthropic-provider");
        anthropic_provider.provider_type = "anthropic".to_string();
        config.providers.push(anthropic_provider);

        let openai_providers = config.get_providers_by_type("openai");
        assert_eq!(openai_providers.len(), 1);

        let anthropic_providers = config.get_providers_by_type("anthropic");
        assert_eq!(anthropic_providers.len(), 1);
    }

    #[test]
    fn test_gateway_config_get_providers_by_tag() {
        let mut config = create_valid_config();
        config.providers[0].tags = vec!["production".to_string(), "fast".to_string()];

        let mut tagged_provider = create_test_provider("tagged-provider");
        tagged_provider.tags = vec!["staging".to_string()];
        config.providers.push(tagged_provider);

        let prod_providers = config.get_providers_by_tag("production");
        assert_eq!(prod_providers.len(), 1);

        let staging_providers = config.get_providers_by_tag("staging");
        assert_eq!(staging_providers.len(), 1);

        let no_tag_providers = config.get_providers_by_tag("nonexistent");
        assert!(no_tag_providers.is_empty());
    }

    // ==================== GatewayConfig Feature Check Tests ====================

    #[test]
    fn test_gateway_config_is_feature_enabled_jwt_auth() {
        let mut config = GatewayConfig::default();
        config.auth.enable_jwt = true;
        assert!(config.is_feature_enabled("jwt_auth"));

        config.auth.enable_jwt = false;
        assert!(!config.is_feature_enabled("jwt_auth"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_api_key_auth() {
        let mut config = GatewayConfig::default();
        config.auth.enable_api_key = true;
        assert!(config.is_feature_enabled("api_key_auth"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_rbac() {
        let mut config = GatewayConfig::default();
        config.auth.rbac.enabled = true;
        assert!(config.is_feature_enabled("rbac"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_metrics() {
        let mut config = GatewayConfig::default();
        config.monitoring.metrics.enabled = true;
        assert!(config.is_feature_enabled("metrics"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_tracing() {
        let mut config = GatewayConfig::default();
        config.monitoring.tracing.enabled = true;
        assert!(config.is_feature_enabled("tracing"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_health_checks() {
        let config = GatewayConfig::default();
        assert!(config.is_feature_enabled("health_checks"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_caching() {
        let mut config = GatewayConfig::default();
        config.cache.enabled = true;
        assert!(config.is_feature_enabled("caching"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_semantic_cache() {
        let mut config = GatewayConfig::default();
        config.cache.semantic_cache = true;
        assert!(config.is_feature_enabled("semantic_cache"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_rate_limiting() {
        let mut config = GatewayConfig::default();
        config.rate_limit.enabled = true;
        assert!(config.is_feature_enabled("rate_limiting"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_enterprise() {
        let mut config = GatewayConfig::default();
        config.enterprise.enabled = true;
        assert!(config.is_feature_enabled("enterprise"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_sso() {
        let mut config = GatewayConfig::default();
        config.enterprise.sso = Some(SsoConfig {
            provider: "okta".to_string(),
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            redirect_url: "https://redirect.example.com".to_string(),
            settings: std::collections::HashMap::new(),
        });
        assert!(config.is_feature_enabled("sso"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_audit_logging() {
        let mut config = GatewayConfig::default();
        config.enterprise.audit_logging = true;
        assert!(config.is_feature_enabled("audit_logging"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_advanced_analytics() {
        let mut config = GatewayConfig::default();
        config.enterprise.advanced_analytics = true;
        assert!(config.is_feature_enabled("advanced_analytics"));
    }

    #[test]
    fn test_gateway_config_is_feature_enabled_unknown() {
        let config = GatewayConfig::default();
        assert!(!config.is_feature_enabled("unknown_feature"));
    }

    // ==================== GatewayConfig Environment Tests ====================

    #[test]
    fn test_gateway_config_for_environment_development() {
        let config = GatewayConfig::default();
        let dev_config = config.for_environment("development");
        assert!(dev_config.server.dev_mode);
        assert!(dev_config.monitoring.tracing.enabled);
    }

    #[test]
    fn test_gateway_config_for_environment_production() {
        let config = GatewayConfig::default();
        let prod_config = config.for_environment("production");
        assert!(!prod_config.server.dev_mode);
        assert!(prod_config.monitoring.metrics.enabled);
        assert!(prod_config.monitoring.tracing.enabled);
    }

    #[test]
    fn test_gateway_config_for_environment_testing() {
        let config = GatewayConfig::default();
        let test_config = config.for_environment("testing");
        assert!(test_config.server.dev_mode);
        assert!(!test_config.cache.enabled);
        assert!(!test_config.rate_limit.enabled);
    }

    #[test]
    fn test_gateway_config_for_environment_unknown() {
        let config = GatewayConfig::default();
        let unknown_config = config.for_environment("unknown");
        assert_eq!(unknown_config.server.dev_mode, config.server.dev_mode);
    }

    // ==================== GatewayConfig Merge Tests ====================

    #[test]
    fn test_gateway_config_merge_providers() {
        let base = create_valid_config();
        let mut other = GatewayConfig {
            providers: vec![create_test_provider("other-provider")],
            ..Default::default()
        };
        other.storage.database.url = "postgres://other".to_string();
        other.auth.jwt_secret = "other-secret".to_string();

        let merged = base.merge(other);
        assert_eq!(merged.providers.len(), 2);
    }

    #[test]
    fn test_gateway_config_merge_provider_override() {
        let base = create_valid_config();
        let mut other = GatewayConfig::default();
        let mut override_provider = create_test_provider("test-provider");
        override_provider.api_key = "new-api-key".to_string();
        other.providers = vec![override_provider];

        let merged = base.merge(other);
        assert_eq!(merged.providers.len(), 1);
        assert_eq!(merged.providers[0].api_key, "new-api-key");
    }

    // ==================== GatewayConfig Serialization Tests ====================

    #[test]
    fn test_gateway_config_serialization() {
        let config = create_valid_config();
        let json = serde_json::to_value(&config).unwrap();
        assert!(json["server"].is_object());
        assert!(json["providers"].is_array());
    }

    #[test]
    fn test_gateway_config_clone() {
        let config = create_valid_config();
        let cloned = config.clone();
        assert_eq!(config.providers.len(), cloned.providers.len());
    }

    // ==================== Environment Variable Feature Flag Tests ====================

    #[test]
    fn test_gateway_config_from_env_cache_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
            env::set_var(ENV_CACHE_ENABLED, "true");
        }

        let config = GatewayConfig::from_env().unwrap();
        assert!(config.cache.enabled);

        clear_test_env();
    }

    #[test]
    fn test_gateway_config_from_env_rate_limit_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
            env::set_var(ENV_RATE_LIMIT_ENABLED, "1");
        }

        let config = GatewayConfig::from_env().unwrap();
        assert!(config.rate_limit.enabled);

        clear_test_env();
    }

    #[test]
    fn test_gateway_config_from_env_enterprise_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
            env::set_var(ENV_ENTERPRISE_ENABLED, "yes");
        }

        let config = GatewayConfig::from_env().unwrap();
        assert!(config.enterprise.enabled);

        clear_test_env();
    }

    #[test]
    fn test_gateway_config_from_env_all_features_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
            env::set_var(ENV_CACHE_ENABLED, "true");
            env::set_var(ENV_RATE_LIMIT_ENABLED, "true");
            env::set_var(ENV_ENTERPRISE_ENABLED, "true");
        }

        let config = GatewayConfig::from_env().unwrap();
        assert!(config.cache.enabled);
        assert!(config.rate_limit.enabled);
        assert!(config.enterprise.enabled);

        clear_test_env();
    }

    #[test]
    fn test_gateway_config_from_env_features_disabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
            env::set_var(ENV_CACHE_ENABLED, "false");
            env::set_var(ENV_RATE_LIMIT_ENABLED, "0");
            env::set_var(ENV_ENTERPRISE_ENABLED, "no");
        }

        let config = GatewayConfig::from_env().unwrap();
        assert!(!config.cache.enabled);
        assert!(!config.rate_limit.enabled);
        assert!(!config.enterprise.enabled);

        clear_test_env();
    }

    #[test]
    fn test_gateway_config_from_env_invalid_cache_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_test_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "true");
            env::set_var(ENV_JWT_SECRET, "StrongJwtSecretWithMixedCaseAndNumbers1234");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test-key");
            env::set_var(ENV_CACHE_ENABLED, "invalid");
        }

        let result = GatewayConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(ENV_CACHE_ENABLED));

        clear_test_env();
    }
}
