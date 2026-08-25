//! Environment parsing for gateway configuration and presence-aware overlays.

use std::env;

use super::*;
use crate::core::net::ProviderEndpointAccess;

pub(crate) struct GatewayEnvOverlay {
    pub(crate) gateway: GatewayConfig,
    pub(crate) redis_cluster: Option<bool>,
    pub(crate) enable_jwt: Option<bool>,
    pub(crate) enable_api_key: Option<bool>,
}

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
            .collect()
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
    let Some(provider_names) = parse_env_list(ENV_PROVIDERS) else {
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
        let provider_type = required_env(&type_key)?;
        let selector = provider_type.to_lowercase();
        let skip_api_key = crate::core::providers::registry::selector_skips_api_key(&selector);
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
        if let Some(value) = env_var(&provider_env_name(&name, "API_VERSION")) {
            provider.api_version = Some(value);
        }
        if let Some(value) = env_var(&provider_env_name(&name, "ORGANIZATION")) {
            provider.organization = Some(value);
        }
        if let Some(value) = env_var(&provider_env_name(&name, "PROJECT")) {
            provider.project = Some(value);
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
        if let Some(value) = parse_env_list(&provider_env_name(&name, "MODELS")) {
            provider.models = value;
        }
        if let Some(value) = parse_env_list(&provider_env_name(&name, "TAGS")) {
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
        if let Some(value) = env_var(ENV_HOST) {
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

        if let Some(value) = env_var(ENV_DATABASE_URL) {
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

        if let Some(value) = env_var(ENV_REDIS_URL) {
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
        if let Some(value) = env_var(ENV_JWT_SECRET) {
            config.auth.jwt_secret = value;
        } else if standalone && config.auth.enable_jwt {
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
        if let Some(value) = env_var(ENV_API_KEY_HEADER) {
            config.auth.api_key_header = value;
        }

        config.providers = load_providers_from_env()?;
        if standalone && config.providers.is_empty() {
            return Err(crate::utils::error::gateway_error::GatewayError::Config(
                format!(
                    "{} must be set with at least one provider name",
                    ENV_PROVIDERS
                ),
            ));
        }

        if let Some(value) = env_var(ENV_PRICING_SOURCE) {
            config.pricing.source = Some(value);
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
        Ok(GatewayEnvOverlay {
            gateway: config,
            redis_cluster,
            enable_jwt,
            enable_api_key,
        })
    }
}
