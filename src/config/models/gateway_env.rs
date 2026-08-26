//! Environment parsing for gateway configuration and presence-aware overlays.

use std::{collections::BTreeSet, env};

use super::*;
use crate::core::net::ProviderEndpointAccess;

pub(crate) struct GatewayEnvOverlay {
    pub(crate) gateway: GatewayConfig,
    pub(crate) redis_cluster: Option<bool>,
    pub(crate) presence: GatewayEnvPresence,
}

#[derive(Debug, Clone)]
pub(crate) struct GatewayEnvPresence(BTreeSet<String>);

pub(super) const ENV_HOST: &str = "LITELLM_HOST";
pub(super) const ENV_PORT: &str = "LITELLM_PORT";
pub(super) const ENV_WORKERS: &str = "LITELLM_WORKERS";
pub(super) const ENV_TIMEOUT: &str = "LITELLM_TIMEOUT";
pub(super) const ENV_DATABASE_URL: &str = "LITELLM_DATABASE_URL";
pub(super) const ENV_DATABASE_MAX_CONNECTIONS: &str = "LITELLM_DATABASE_MAX_CONNECTIONS";
pub(super) const ENV_DATABASE_CONNECTION_TIMEOUT: &str = "LITELLM_DATABASE_CONNECTION_TIMEOUT";
pub(super) const ENV_DATABASE_SSL: &str = "LITELLM_DATABASE_SSL";
pub(super) const ENV_DATABASE_ENABLED: &str = "LITELLM_DATABASE_ENABLED";
pub(super) const ENV_DATABASE_AUTO_MIGRATE: &str = "LITELLM_DATABASE_AUTO_MIGRATE";
pub(super) const ENV_REDIS_URL: &str = "LITELLM_REDIS_URL";
pub(super) const ENV_REDIS_ENABLED: &str = "LITELLM_REDIS_ENABLED";
pub(super) const ENV_REDIS_MAX_CONNECTIONS: &str = "LITELLM_REDIS_MAX_CONNECTIONS";
pub(super) const ENV_REDIS_CONNECTION_TIMEOUT: &str = "LITELLM_REDIS_CONNECTION_TIMEOUT";
pub(super) const ENV_REDIS_CLUSTER: &str = "LITELLM_REDIS_CLUSTER";
pub(super) const ENV_ENABLE_JWT: &str = "LITELLM_ENABLE_JWT";
pub(super) const ENV_ENABLE_API_KEY: &str = "LITELLM_ENABLE_API_KEY";
pub(super) const ENV_JWT_SECRET: &str = "LITELLM_JWT_SECRET";
pub(super) const ENV_JWT_EXPIRATION: &str = "LITELLM_JWT_EXPIRATION";
pub(super) const ENV_API_KEY_HEADER: &str = "LITELLM_API_KEY_HEADER";
pub(super) const ENV_PROVIDERS: &str = "LITELLM_PROVIDERS";
pub(super) const ENV_PRICING_SOURCE: &str = "LITELLM_PRICING_SOURCE";
pub(super) const ENV_UNPRICED_MODEL_POLICY: &str = "LITELLM_UNPRICED_MODEL_POLICY";
pub(super) const ENV_UNPRICED_FALLBACK_COST_PER_1K_TOKENS: &str =
    "LITELLM_UNPRICED_FALLBACK_COST_PER_1K_TOKENS";
pub(super) const ENV_CACHE_ENABLED: &str = "LITELLM_CACHE_ENABLED";
pub(super) const ENV_RATE_LIMIT_ENABLED: &str = "LITELLM_RATE_LIMIT_ENABLED";
pub(super) const ENV_ENTERPRISE_ENABLED: &str = "LITELLM_ENTERPRISE_ENABLED";

const FIXED_ENV_KEYS: &[&str] = &[
    ENV_HOST,
    ENV_PORT,
    ENV_WORKERS,
    ENV_TIMEOUT,
    ENV_DATABASE_URL,
    ENV_DATABASE_MAX_CONNECTIONS,
    ENV_DATABASE_CONNECTION_TIMEOUT,
    ENV_DATABASE_SSL,
    ENV_DATABASE_ENABLED,
    ENV_DATABASE_AUTO_MIGRATE,
    ENV_REDIS_URL,
    ENV_REDIS_ENABLED,
    ENV_REDIS_MAX_CONNECTIONS,
    ENV_REDIS_CONNECTION_TIMEOUT,
    ENV_REDIS_CLUSTER,
    ENV_ENABLE_JWT,
    ENV_ENABLE_API_KEY,
    ENV_JWT_SECRET,
    ENV_JWT_EXPIRATION,
    ENV_API_KEY_HEADER,
    ENV_PROVIDERS,
    ENV_PRICING_SOURCE,
    ENV_UNPRICED_MODEL_POLICY,
    ENV_UNPRICED_FALLBACK_COST_PER_1K_TOKENS,
    ENV_CACHE_ENABLED,
    ENV_RATE_LIMIT_ENABLED,
    ENV_ENTERPRISE_ENABLED,
];

const PROVIDER_ENV_FIELDS: &[&str] = &[
    "TYPE",
    "API_KEY",
    "BASE_URL",
    "ENDPOINT_ACCESS",
    "API_VERSION",
    "ORGANIZATION",
    "PROJECT",
    "WEIGHT",
    "RPM",
    "TPM",
    "MAX_CONCURRENT_REQUESTS",
    "TIMEOUT",
    "MAX_RETRIES",
    "ENABLED",
    "MODELS",
    "TAGS",
];

fn env_var(key: &str) -> crate::utils::error::gateway_error::Result<Option<String>> {
    match env::var(key) {
        Ok(value) => Ok(Some(value.trim().to_string())),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(crate::utils::error::gateway_error::GatewayError::Config(
            format!("Invalid value for {}: {}", key, error),
        )),
    }
}

fn parse_env<T>(key: &str) -> crate::utils::error::gateway_error::Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let Some(raw) = env_var(key)? else {
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
    let Some(raw) = env_var(key)? else {
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

fn parse_env_list(key: &str) -> crate::utils::error::gateway_error::Result<Option<Vec<String>>> {
    env_var(key).map(|value| {
        value.map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToString::to_string)
                .collect()
        })
    })
}

fn required_env(key: &str) -> crate::utils::error::gateway_error::Result<String> {
    env_var(key)?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
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

impl GatewayEnvPresence {
    fn capture(providers: &[ProviderConfig]) -> Self {
        let mut keys = FIXED_ENV_KEYS
            .iter()
            .filter(|key| env::var_os(key).is_some())
            .map(|key| (*key).to_string())
            .collect::<BTreeSet<_>>();
        for provider in providers {
            for field in PROVIDER_ENV_FIELDS {
                let key = provider_env_name(&provider.name, field);
                if env::var_os(&key).is_some() {
                    keys.insert(key);
                }
            }
        }
        Self(keys)
    }

    pub(crate) fn contains(&self, key: &str) -> bool {
        self.0.contains(key)
    }

    pub(crate) fn contains_provider_field(&self, provider: &str, field: &str) -> bool {
        self.contains(&provider_env_name(provider, field))
    }
}

fn merge_database_env(
    base: &mut crate::config::models::storage::DatabaseConfig,
    other: crate::config::models::storage::DatabaseConfig,
    presence: &GatewayEnvPresence,
) {
    if presence.contains(ENV_DATABASE_URL) {
        base.url = other.url;
    }
    if presence.contains(ENV_DATABASE_MAX_CONNECTIONS) {
        base.max_connections = other.max_connections;
    }
    if presence.contains(ENV_DATABASE_CONNECTION_TIMEOUT) {
        base.connection_timeout = other.connection_timeout;
    }
    if presence.contains(ENV_DATABASE_SSL) {
        base.ssl = other.ssl;
    }
    if presence.contains(ENV_DATABASE_ENABLED) {
        base.enabled = other.enabled;
    }
    if presence.contains(ENV_DATABASE_AUTO_MIGRATE) {
        base.auto_migrate = other.auto_migrate;
        base.auto_migrate_configured = true;
    }
}

fn merge_redis_env(
    base: &mut crate::config::models::storage::RedisConfig,
    other: crate::config::models::storage::RedisConfig,
    redis_cluster: Option<bool>,
    presence: &GatewayEnvPresence,
) {
    if presence.contains(ENV_REDIS_URL) {
        base.url = other.url;
    }
    if presence.contains(ENV_REDIS_ENABLED) {
        base.enabled = other.enabled;
    }
    if presence.contains(ENV_REDIS_MAX_CONNECTIONS) {
        base.max_connections = other.max_connections;
    }
    if presence.contains(ENV_REDIS_CONNECTION_TIMEOUT) {
        base.connection_timeout = other.connection_timeout;
    }
    if let Some(cluster) = redis_cluster {
        base.cluster = cluster;
    }
}

fn merge_auth_env(base: &mut AuthConfig, other: AuthConfig, presence: &GatewayEnvPresence) {
    if presence.contains(ENV_ENABLE_JWT) {
        base.enable_jwt = other.enable_jwt;
    }
    if presence.contains(ENV_ENABLE_API_KEY) {
        base.enable_api_key = other.enable_api_key;
    }
    if presence.contains(ENV_JWT_SECRET) {
        base.jwt_secret = other.jwt_secret;
    }
    if presence.contains(ENV_JWT_EXPIRATION) {
        base.jwt_expiration = other.jwt_expiration;
    }
    if presence.contains(ENV_API_KEY_HEADER) {
        base.api_key_header = other.api_key_header;
    }
}

fn merge_provider_env(
    base: &mut Vec<ProviderConfig>,
    providers: Vec<ProviderConfig>,
    presence: &GatewayEnvPresence,
) {
    if !presence.contains(ENV_PROVIDERS) {
        return;
    }
    for provider in providers {
        match base.iter_mut().find(|item| item.name == provider.name) {
            Some(existing) => merge_existing_provider_env(existing, provider, presence),
            None => base.push(provider),
        }
    }
}

fn merge_existing_provider_env(
    base: &mut ProviderConfig,
    other: ProviderConfig,
    presence: &GatewayEnvPresence,
) {
    let name = &other.name;
    if presence.contains_provider_field(name, "TYPE") {
        base.provider_type = other.provider_type;
    }
    if presence.contains_provider_field(name, "API_KEY") {
        base.api_key = other.api_key;
    }
    if presence.contains_provider_field(name, "BASE_URL") {
        base.base_url = other.base_url;
    }
    if presence.contains_provider_field(name, "ENDPOINT_ACCESS") {
        base.endpoint_access = other.endpoint_access;
    }
    if presence.contains_provider_field(name, "API_VERSION") {
        base.api_version = other.api_version;
    }
    if presence.contains_provider_field(name, "ORGANIZATION") {
        base.organization = other.organization;
    }
    if presence.contains_provider_field(name, "PROJECT") {
        base.project = other.project;
    }
    if presence.contains_provider_field(name, "WEIGHT") {
        base.weight = other.weight;
    }
    if presence.contains_provider_field(name, "RPM") {
        base.rpm = other.rpm;
    }
    if presence.contains_provider_field(name, "TPM") {
        base.tpm = other.tpm;
    }
    if presence.contains_provider_field(name, "MAX_CONCURRENT_REQUESTS") {
        base.max_concurrent_requests = other.max_concurrent_requests;
    }
    if presence.contains_provider_field(name, "TIMEOUT") {
        base.timeout = other.timeout;
    }
    if presence.contains_provider_field(name, "MAX_RETRIES") {
        base.max_retries = other.max_retries;
    }
    if presence.contains_provider_field(name, "ENABLED") {
        base.enabled = other.enabled;
    }
    if presence.contains_provider_field(name, "MODELS") {
        base.models = other.models;
    }
    if presence.contains_provider_field(name, "TAGS") {
        base.tags = other.tags;
    }
}

impl GatewayConfig {
    pub(crate) fn merge_env_overlay(
        mut self,
        other: Self,
        redis_cluster: Option<bool>,
        presence: &GatewayEnvPresence,
    ) -> Self {
        if presence.contains(ENV_HOST) {
            self.server.host = other.server.host;
        }
        if presence.contains(ENV_PORT) {
            self.server.port = other.server.port;
        }
        if presence.contains(ENV_WORKERS) {
            self.server.workers = other.server.workers;
        }
        if presence.contains(ENV_TIMEOUT) {
            self.server.timeout = other.server.timeout;
        }
        merge_database_env(&mut self.storage.database, other.storage.database, presence);
        merge_redis_env(
            &mut self.storage.redis,
            other.storage.redis,
            redis_cluster,
            presence,
        );
        merge_auth_env(&mut self.auth, other.auth, presence);
        merge_provider_env(&mut self.providers, other.providers, presence);
        self.pricing = self.pricing.merge(other.pricing);
        if presence.contains(ENV_CACHE_ENABLED) {
            self.cache.enabled = other.cache.enabled;
        }
        if presence.contains(ENV_RATE_LIMIT_ENABLED) {
            self.rate_limit.enabled = other.rate_limit.enabled;
        }
        if presence.contains(ENV_ENTERPRISE_ENABLED) {
            self.enterprise.enabled = other.enterprise.enabled;
        }
        self
    }
}

fn load_providers_from_env(
    standalone: bool,
) -> crate::utils::error::gateway_error::Result<Vec<ProviderConfig>> {
    let Some(provider_names) = parse_env_list(ENV_PROVIDERS)? else {
        return Ok(Vec::new());
    };
    if provider_names.is_empty() {
        return Err(crate::utils::error::gateway_error::GatewayError::Config(
            format!("{} must contain at least one provider name", ENV_PROVIDERS),
        ));
    }

    let mut providers = Vec::with_capacity(provider_names.len());
    for name in provider_names {
        let type_key = provider_env_name(&name, "TYPE");
        let api_key_key = provider_env_name(&name, "API_KEY");
        let provider_type = if standalone {
            required_env(&type_key)?
        } else {
            env_var(&type_key)?.unwrap_or_default()
        };
        let selector = provider_type.to_lowercase();
        let skip_api_key = crate::core::providers::registry::selector_skips_api_key(&selector);
        let api_key = if skip_api_key || !standalone {
            env_var(&api_key_key)?.unwrap_or_default()
        } else {
            required_env(&api_key_key)?
        };
        let mut provider = ProviderConfig {
            name: name.clone(),
            provider_type,
            api_key,
            ..ProviderConfig::default()
        };

        if let Some(base_url) = env_var(&provider_env_name(&name, "BASE_URL"))? {
            provider.base_url = (!base_url.is_empty()).then_some(base_url);
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
        if let Some(value) = env_var(&provider_env_name(&name, "API_VERSION"))? {
            provider.api_version = (!value.is_empty()).then_some(value);
        }
        if let Some(value) = env_var(&provider_env_name(&name, "ORGANIZATION"))? {
            provider.organization = (!value.is_empty()).then_some(value);
        }
        if let Some(value) = env_var(&provider_env_name(&name, "PROJECT"))? {
            provider.project = (!value.is_empty()).then_some(value);
        }
        if let Some(value) = parse_env::<f32>(&provider_env_name(&name, "WEIGHT"))? {
            provider.weight = value;
        }
        if let Some(value) = parse_env::<u32>(&provider_env_name(&name, "RPM"))? {
            provider.rpm = value;
        }
        if let Some(value) = parse_env::<u32>(&provider_env_name(&name, "TPM"))? {
            provider.tpm = value;
        }
        if let Some(value) = parse_env::<u32>(&provider_env_name(&name, "MAX_CONCURRENT_REQUESTS"))?
        {
            provider.max_concurrent_requests = value;
        }
        if let Some(value) = parse_env::<u64>(&provider_env_name(&name, "TIMEOUT"))? {
            provider.timeout = value;
        }
        if let Some(value) = parse_env::<u32>(&provider_env_name(&name, "MAX_RETRIES"))? {
            provider.max_retries = value;
        }
        if let Some(value) = parse_env_bool(&provider_env_name(&name, "ENABLED"))? {
            provider.enabled = value;
        }
        if let Some(value) = parse_env_list(&provider_env_name(&name, "MODELS"))? {
            provider.models = value;
        }
        if let Some(value) = parse_env_list(&provider_env_name(&name, "TAGS"))? {
            provider.tags = value;
        }
        providers.push(provider);
    }
    Ok(providers)
}

impl GatewayConfig {
    pub fn from_env() -> crate::utils::error::gateway_error::Result<Self> {
        Self::from_env_inner(true).map(|overlay| overlay.gateway)
    }

    pub(crate) fn from_env_with_redis_cluster_presence()
    -> crate::utils::error::gateway_error::Result<GatewayEnvOverlay> {
        Self::from_env_inner(false)
    }

    fn from_env_inner(
        standalone: bool,
    ) -> crate::utils::error::gateway_error::Result<GatewayEnvOverlay> {
        let mut config = Self::default();
        if let Some(value) = env_var(ENV_HOST)? {
            config.server.host = value;
        }
        if let Some(value) = parse_env::<u16>(ENV_PORT)? {
            config.server.port = value;
        }
        if let Some(value) = parse_env::<usize>(ENV_WORKERS)? {
            config.server.workers = Some(value);
        }
        if let Some(value) = parse_env::<u64>(ENV_TIMEOUT)? {
            config.server.timeout = value;
        }

        if let Some(value) = env_var(ENV_DATABASE_URL)? {
            config.storage.database.url = value;
        }
        if let Some(value) = parse_env::<u32>(ENV_DATABASE_MAX_CONNECTIONS)? {
            config.storage.database.max_connections = value;
        }
        if let Some(value) = parse_env::<u64>(ENV_DATABASE_CONNECTION_TIMEOUT)? {
            config.storage.database.connection_timeout = value;
        }
        if let Some(value) = parse_env_bool(ENV_DATABASE_SSL)? {
            config.storage.database.ssl = value;
        }
        if let Some(value) = parse_env_bool(ENV_DATABASE_ENABLED)? {
            config.storage.database.enabled = value;
        }
        if let Some(value) = parse_env_bool(ENV_DATABASE_AUTO_MIGRATE)? {
            config.storage.database.auto_migrate = value;
            config.storage.database.auto_migrate_configured = true;
        }

        if let Some(value) = env_var(ENV_REDIS_URL)? {
            config.storage.redis.url = value;
        }
        if let Some(value) = parse_env_bool(ENV_REDIS_ENABLED)? {
            config.storage.redis.enabled = value;
        }
        if let Some(value) = parse_env::<u32>(ENV_REDIS_MAX_CONNECTIONS)? {
            config.storage.redis.max_connections = value;
        }
        if let Some(value) = parse_env::<u64>(ENV_REDIS_CONNECTION_TIMEOUT)? {
            config.storage.redis.connection_timeout = value;
        }
        let redis_cluster = parse_env_bool(ENV_REDIS_CLUSTER)?;
        if let Some(value) = redis_cluster {
            config.storage.redis.cluster = value;
        }

        let enable_jwt = parse_env_bool(ENV_ENABLE_JWT)?;
        if let Some(value) = enable_jwt {
            config.auth.enable_jwt = value;
        }
        let enable_api_key = parse_env_bool(ENV_ENABLE_API_KEY)?;
        if let Some(value) = enable_api_key {
            config.auth.enable_api_key = value;
        }
        if let Some(value) = env_var(ENV_JWT_SECRET)? {
            config.auth.jwt_secret = value;
        }
        if standalone && config.auth.enable_jwt && config.auth.jwt_secret.is_empty() {
            return Err(crate::utils::error::gateway_error::GatewayError::Config(
                format!(
                    "{} is required when {} is enabled",
                    ENV_JWT_SECRET, ENV_ENABLE_JWT
                ),
            ));
        }
        if let Some(value) = parse_env::<u64>(ENV_JWT_EXPIRATION)? {
            config.auth.jwt_expiration = value;
        }
        if let Some(value) = env_var(ENV_API_KEY_HEADER)? {
            config.auth.api_key_header = value;
        }

        config.providers = load_providers_from_env(standalone)?;
        if standalone && config.providers.is_empty() {
            return Err(crate::utils::error::gateway_error::GatewayError::Config(
                format!(
                    "{} must be set with at least one provider name",
                    ENV_PROVIDERS
                ),
            ));
        }

        if let Some(value) = env_var(ENV_PRICING_SOURCE)? {
            config.pricing.source = (!value.is_empty()).then_some(value);
            config.pricing.mark_source_explicit_for_merge();
        }
        if let Some(value) = parse_env::<UnpricedModelPolicy>(ENV_UNPRICED_MODEL_POLICY)? {
            config.pricing.unpriced_model_policy = value;
            config
                .pricing
                .mark_unpriced_model_policy_explicit_for_merge();
        }
        if let Some(value) = parse_env::<f64>(ENV_UNPRICED_FALLBACK_COST_PER_1K_TOKENS)? {
            config.pricing.unpriced_fallback_cost_per_1k_tokens = Some(value);
            config.pricing.mark_unpriced_fallback_explicit_for_merge();
        }
        if let Some(value) = parse_env_bool(ENV_CACHE_ENABLED)? {
            config.cache.enabled = value;
        }
        if let Some(value) = parse_env_bool(ENV_RATE_LIMIT_ENABLED)? {
            config.rate_limit.enabled = value;
        }
        if let Some(value) = parse_env_bool(ENV_ENTERPRISE_ENABLED)? {
            config.enterprise.enabled = value;
        }
        let presence = GatewayEnvPresence::capture(&config.providers);
        Ok(GatewayEnvOverlay {
            gateway: config,
            redis_cluster,
            presence,
        })
    }
}
