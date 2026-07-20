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
use std::str::FromStr;

use crate::core::net::ProviderEndpointAccess;

const ENV_HOST: &str = "LITELLM_HOST";
const ENV_PORT: &str = "LITELLM_PORT";
const ENV_WORKERS: &str = "LITELLM_WORKERS";
const ENV_TIMEOUT: &str = "LITELLM_TIMEOUT";
const ENV_DATABASE_URL: &str = "LITELLM_DATABASE_URL";
const ENV_DATABASE_MAX_CONNECTIONS: &str = "LITELLM_DATABASE_MAX_CONNECTIONS";
const ENV_DATABASE_CONNECTION_TIMEOUT: &str = "LITELLM_DATABASE_CONNECTION_TIMEOUT";
const ENV_DATABASE_SSL: &str = "LITELLM_DATABASE_SSL";
const ENV_DATABASE_ENABLED: &str = "LITELLM_DATABASE_ENABLED";
const ENV_DATABASE_AUTO_MIGRATE: &str = "LITELLM_DATABASE_AUTO_MIGRATE";
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
const ENV_UNPRICED_MODEL_POLICY: &str = "LITELLM_UNPRICED_MODEL_POLICY";
const ENV_UNPRICED_FALLBACK_COST_PER_1K_TOKENS: &str =
    "LITELLM_UNPRICED_FALLBACK_COST_PER_1K_TOKENS";
const ENV_CACHE_ENABLED: &str = "LITELLM_CACHE_ENABLED";
const ENV_RATE_LIMIT_ENABLED: &str = "LITELLM_RATE_LIMIT_ENABLED";
const ENV_ENTERPRISE_ENABLED: &str = "LITELLM_ENTERPRISE_ENABLED";
const DEFAULT_PRICING_SOURCE: &str = crate::core::pricing_service::DEFAULT_PRICING_SOURCE;

#[cfg(test)]
pub(crate) static GATEWAY_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
        let endpoint_access_key = provider_env_name(&name, "ENDPOINT_ACCESS");
        match env::var(&endpoint_access_key) {
            Ok(raw) => {
                provider.endpoint_access =
                    raw.parse::<ProviderEndpointAccess>().map_err(|error| {
                        crate::utils::error::gateway_error::GatewayError::Config(format!(
                            "Invalid value for {endpoint_access_key}: {error}"
                        ))
                    })?;
            }
            Err(env::VarError::NotPresent) => {}
            Err(error) => {
                return Err(crate::utils::error::gateway_error::GatewayError::Config(
                    format!("Invalid value for {endpoint_access_key}: {error}"),
                ));
            }
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
            provider.models = models.into_iter().map(Into::into).collect();
        }
        if let Some(tags) = parse_env_list(&provider_env_name(&name, "TAGS")) {
            provider.tags = tags;
        }

        providers.push(provider);
    }

    Ok(providers)
}

/// Request-time behavior when provider/model pricing is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UnpricedModelPolicy {
    /// Reject requests before provider execution when pricing cannot be proven.
    #[default]
    Reject,
    /// Allow requests and require settlement paths to mark unpriced usage.
    AllowUnpriced,
}

impl FromStr for UnpricedModelPolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "reject" => Ok(Self::Reject),
            "allow_unpriced" => Ok(Self::AllowUnpriced),
            other => Err(format!(
                "unsupported unpriced model policy '{}'; expected reject or allow_unpriced",
                other
            )),
        }
    }
}

/// Pricing source configuration
#[derive(Debug, Clone, Serialize)]
pub struct GatewayPricingConfig {
    /// Optional pricing source path/URL used by PricingService::new
    pub source: Option<String>,
    /// When the pricing source is configured and the initial load fails, allow
    /// the gateway to keep running without pricing data instead of failing
    /// startup.
    ///
    /// Defaults to `false` so a configured-but-broken pricing source is
    /// surfaced at startup. A `true` value documents that the gateway may
    /// serve traffic without cost accounting until pricing data is refreshed.
    pub allow_degraded: bool,
    /// Request-time policy for provider/model combinations missing pricing.
    ///
    /// Defaults to `reject`. `allow_degraded` only controls initial pricing
    /// source load failures; it does not allow unpriced requests by itself.
    pub unpriced_model_policy: UnpricedModelPolicy,
    /// Optional fallback price used by `allow_unpriced` request-time policy.
    ///
    /// This is a per-1k usage unit price, not a fixed per-request amount.
    pub unpriced_fallback_cost_per_1k_tokens: Option<f64>,
    #[serde(skip)]
    merge_fields: GatewayPricingMergeFields,
}

#[derive(Debug, Clone, Copy, Default)]
struct GatewayPricingMergeFields {
    source: bool,
    allow_degraded: bool,
    unpriced_model_policy: bool,
    unpriced_fallback_cost_per_1k_tokens: bool,
}

#[derive(Default)]
enum ConfigField<T> {
    #[default]
    Missing,
    Present(T),
}

impl<'de, T> Deserialize<'de> for ConfigField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::Present)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewayPricingConfigWire {
    #[serde(default)]
    source: ConfigField<Option<String>>,
    #[serde(default)]
    allow_degraded: ConfigField<bool>,
    #[serde(default)]
    unpriced_model_policy: ConfigField<UnpricedModelPolicy>,
    #[serde(default)]
    unpriced_fallback_cost_per_1k_tokens: ConfigField<Option<f64>>,
}

impl<'de> Deserialize<'de> for GatewayPricingConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = GatewayPricingConfigWire::deserialize(deserializer)?;
        let mut merge_fields = GatewayPricingMergeFields::default();

        let source = match wire.source {
            ConfigField::Present(source) => {
                merge_fields.source = true;
                source
            }
            ConfigField::Missing => default_pricing_source(),
        };

        let allow_degraded = match wire.allow_degraded {
            ConfigField::Present(allow_degraded) => {
                merge_fields.allow_degraded = true;
                allow_degraded
            }
            ConfigField::Missing => false,
        };

        let unpriced_model_policy = match wire.unpriced_model_policy {
            ConfigField::Present(policy) => {
                merge_fields.unpriced_model_policy = true;
                policy
            }
            ConfigField::Missing => UnpricedModelPolicy::default(),
        };

        let unpriced_fallback_cost_per_1k_tokens = match wire.unpriced_fallback_cost_per_1k_tokens {
            ConfigField::Present(cost) => {
                merge_fields.unpriced_fallback_cost_per_1k_tokens = true;
                cost
            }
            ConfigField::Missing => None,
        };

        Ok(Self {
            source,
            allow_degraded,
            unpriced_model_policy,
            unpriced_fallback_cost_per_1k_tokens,
            merge_fields,
        })
    }
}

impl PartialEq for GatewayPricingConfig {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.allow_degraded == other.allow_degraded
            && self.unpriced_model_policy == other.unpriced_model_policy
            && self.unpriced_fallback_cost_per_1k_tokens
                == other.unpriced_fallback_cost_per_1k_tokens
    }
}

impl Default for GatewayPricingConfig {
    fn default() -> Self {
        Self {
            source: default_pricing_source(),
            allow_degraded: false,
            unpriced_model_policy: UnpricedModelPolicy::default(),
            unpriced_fallback_cost_per_1k_tokens: None,
            merge_fields: GatewayPricingMergeFields::default(),
        }
    }
}

impl GatewayPricingConfig {
    /// Merge pricing configurations, with explicit overlay values taking precedence.
    pub fn merge(mut self, other: Self) -> Self {
        let source_overridden =
            other.merge_fields.source || other.source != default_pricing_source();
        if source_overridden {
            self.source = other.source;
        }

        let allow_degraded_overridden = other.merge_fields.allow_degraded || other.allow_degraded;
        if allow_degraded_overridden {
            self.allow_degraded = other.allow_degraded;
        }

        let unpriced_model_policy_overridden = other.merge_fields.unpriced_model_policy
            || other.unpriced_model_policy != UnpricedModelPolicy::default();
        if unpriced_model_policy_overridden {
            self.unpriced_model_policy = other.unpriced_model_policy;
        }

        let unpriced_fallback_overridden = other.merge_fields.unpriced_fallback_cost_per_1k_tokens
            || other.unpriced_fallback_cost_per_1k_tokens.is_some();
        if unpriced_fallback_overridden {
            self.unpriced_fallback_cost_per_1k_tokens = other.unpriced_fallback_cost_per_1k_tokens;
        }

        self.merge_fields.source |= source_overridden;
        self.merge_fields.allow_degraded |= allow_degraded_overridden;
        self.merge_fields.unpriced_model_policy |= unpriced_model_policy_overridden;
        self.merge_fields.unpriced_fallback_cost_per_1k_tokens |= unpriced_fallback_overridden;
        self
    }

    fn mark_source_explicit_for_merge(&mut self) {
        self.merge_fields.source = true;
    }

    fn mark_unpriced_model_policy_explicit_for_merge(&mut self) {
        self.merge_fields.unpriced_model_policy = true;
    }

    fn mark_unpriced_fallback_explicit_for_merge(&mut self) {
        self.merge_fields.unpriced_fallback_cost_per_1k_tokens = true;
    }
}

fn default_pricing_source() -> Option<String> {
    // Keep the runtime default aligned with config/gateway.yaml.example.
    // User-provided relative paths are still resolved by the process working directory.
    Some(DEFAULT_PRICING_SOURCE.to_string())
}

/// Main gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Model aliases: request-facing name -> target model (or another alias).
    ///
    /// The router already resolves aliases at lookup time, including multi-hop
    /// chains and cycle detection (see `RoutingSnapshot::resolve_model_name`).
    /// This field is what makes that capability reachable from configuration:
    ///
    /// ```yaml
    /// model_aliases:
    ///   fast: "gpt-4o-mini"
    ///   smart: "anthropic/claude-opus-4.7"
    /// ```
    ///
    /// Aliases resolve to a *model group*, so a single alias can fan out over
    /// several weighted deployments that serve the same model, which is how
    /// load balancing across providers is expressed.
    ///
    /// Defaults to empty, so existing configurations are unaffected.
    #[serde(default)]
    pub model_aliases: HashMap<String, String>,
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
            model_aliases: HashMap::new(),
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
        if let Some(auto_migrate) = parse_env_bool(ENV_DATABASE_AUTO_MIGRATE)? {
            config.storage.database.auto_migrate = auto_migrate;
            config.storage.database.auto_migrate_configured = true;
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
            config.pricing.mark_source_explicit_for_merge();
        }
        if let Some(policy) = parse_env::<UnpricedModelPolicy>(ENV_UNPRICED_MODEL_POLICY)? {
            config.pricing.unpriced_model_policy = policy;
            config
                .pricing
                .mark_unpriced_model_policy_explicit_for_merge();
        }
        if let Some(cost) = parse_env::<f64>(ENV_UNPRICED_FALLBACK_COST_PER_1K_TOKENS)? {
            config.pricing.unpriced_fallback_cost_per_1k_tokens = Some(cost);
            config.pricing.mark_unpriced_fallback_explicit_for_merge();
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
        self.pricing = self.pricing.merge(other.pricing);

        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        crate::config::validation::Validate::validate(self)?;
        // Surface dead cache configuration at error level without blocking
        // startup: the response cache is not wired into the runtime yet.
        for warning in self.cache.not_yet_implemented_warnings() {
            tracing::error!("{}", warning);
        }
        Ok(())
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
mod endpoint_access_env_tests {
    use super::*;

    const ACCESS_KEY: &str = "LITELLM_PROVIDER_OPENAI_ENDPOINT_ACCESS";

    fn clear_env() {
        for key in [
            ENV_ENABLE_JWT,
            ENV_PROVIDERS,
            "LITELLM_PROVIDER_OPENAI_TYPE",
            "LITELLM_PROVIDER_OPENAI_API_KEY",
            ACCESS_KEY,
        ] {
            unsafe { env::remove_var(key) };
        }
    }

    #[test]
    fn endpoint_access_env_preserves_presence_and_rejects_invalid_values() {
        let _guard = GATEWAY_ENV_LOCK.blocking_lock();
        clear_env();
        unsafe {
            env::set_var(ENV_ENABLE_JWT, "false");
            env::set_var(ENV_PROVIDERS, "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_TYPE", "openai");
            env::set_var("LITELLM_PROVIDER_OPENAI_API_KEY", "sk-test");
            env::set_var(ACCESS_KEY, "private_network");
        }
        let private = GatewayConfig::from_env().expect("private access env should parse");
        assert_eq!(
            private.providers[0].endpoint_access,
            ProviderEndpointAccess::PrivateNetwork
        );
        for invalid in ["", "private"] {
            unsafe { env::set_var(ACCESS_KEY, invalid) };
            assert!(GatewayConfig::from_env().is_err());
        }
        clear_env();
    }
}

#[cfg(test)]
#[path = "gateway_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gateway_pricing_tests.rs"]
mod pricing_tests;
