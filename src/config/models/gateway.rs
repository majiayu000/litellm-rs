//! Main gateway configuration

#![allow(missing_docs)]

use super::auth::AuthConfig;
use super::cache::CacheConfig;
use super::enterprise::EnterpriseConfig;
use super::monitoring::MonitoringConfig;
use super::provider::ProviderConfig;
use super::rate_limit::RateLimitConfig;
use super::router::GatewayRouterConfig;
use super::server::ServerConfig;
use super::storage::StorageConfig;
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
    // Try to resolve relative to the executable directory for cwd-independent deployment
    if let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|dir| dir.join("config/model_prices_extended.json"))
        })
        .and_then(|p| p.to_str().map(str::to_string))
    {
        return Some(path);
    }
    // Fallback: relative path (works when cwd is the repo root)
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
#[path = "gateway_tests.rs"]
mod tests;
