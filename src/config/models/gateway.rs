//! Main gateway configuration

#![allow(missing_docs)]

use super::auth::AuthConfig;
use super::cache::CacheConfig;
use super::enterprise::EnterpriseConfig;
use super::guardrails::{default_gateway_guardrails, deserialize_gateway_guardrails};
use super::monitoring::MonitoringConfig;
use super::provider::ProviderConfig;
use super::rate_limit::RateLimitConfig;
use super::router::GatewayRouterConfig;
use super::server::ServerConfig;
use super::storage::StorageConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(test)]
use std::env;
use std::str::FromStr;

use crate::core::{guardrails::GuardrailConfig, ip_access::IpAccessConfig};

const DEFAULT_PRICING_SOURCE: &str = crate::core::pricing_service::DEFAULT_PRICING_SOURCE;

#[path = "gateway_env.rs"]
pub(crate) mod env_config;
#[cfg(test)]
use env_config::*;

#[cfg(test)]
pub(crate) static GATEWAY_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
    /// Public model aliases resolved by the runtime router
    #[serde(default)]
    pub model_aliases: HashMap<String, String>,
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
    /// Content guardrails. Prompt-injection protection is enabled by default
    /// and can be explicitly disabled with `guardrails.enabled: false`.
    #[serde(
        default = "default_gateway_guardrails",
        deserialize_with = "deserialize_gateway_guardrails"
    )]
    pub guardrails: GuardrailConfig,
    /// IP access policy. Empty/default rules preserve allow-all behavior.
    #[serde(default)]
    pub ip_access: IpAccessConfig,
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
            model_aliases: HashMap::new(),
            router: GatewayRouterConfig::default(),
            storage: StorageConfig::default(),
            auth: AuthConfig::default(),
            monitoring: MonitoringConfig::default(),
            cache: CacheConfig::default(),
            rate_limit: RateLimitConfig::default(),
            guardrails: default_gateway_guardrails(),
            ip_access: IpAccessConfig::default(),
            enterprise: EnterpriseConfig::default(),
            pricing: GatewayPricingConfig::default(),
        }
    }
}

impl GatewayConfig {
    /// Merge two configurations, with other taking precedence
    pub fn merge(self, other: Self) -> Self {
        self.merge_with_redis_cluster_override(other, None)
    }

    pub(crate) fn merge_with_redis_cluster_override(
        mut self,
        other: Self,
        redis_cluster: Option<bool>,
    ) -> Self {
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
        self.model_aliases.extend(other.model_aliases);
        self.router = self.router.merge(other.router);
        self.storage = self
            .storage
            .merge_with_redis_cluster_override(other.storage, redis_cluster);
        self.auth = self.auth.merge(other.auth);
        self.monitoring = self.monitoring.merge(other.monitoring);
        self.cache = self.cache.merge(other.cache);
        self.rate_limit = self.rate_limit.merge(other.rate_limit);
        self.guardrails = other.guardrails;
        self.ip_access = other.ip_access;
        self.enterprise = self.enterprise.merge(other.enterprise);
        self.pricing = self.pricing.merge(other.pricing);

        self
    }

    pub(crate) fn merge_env_overlay(
        self,
        other: Self,
        redis_cluster: Option<bool>,
        enable_jwt: Option<bool>,
        enable_api_key: Option<bool>,
    ) -> Self {
        let jwt = enable_jwt.unwrap_or(self.auth.enable_jwt);
        let api_key = enable_api_key.unwrap_or(self.auth.enable_api_key);
        let guardrails = self.guardrails.clone();
        let ip_access = self.ip_access.clone();
        let mut merged = self.merge_with_redis_cluster_override(other, redis_cluster);
        merged.auth.enable_jwt = jwt;
        merged.auth.enable_api_key = api_key;
        merged.guardrails = guardrails;
        merged.ip_access = ip_access;
        merged
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        crate::config::validation::Validate::validate(self)?;
        // Surface dead cache configuration at error level without blocking
        // startup. `cache.enabled` itself is wired (the response cache is
        // built in AppState and used by the chat and embedding routes), so
        // only settings with no runtime effect, such as `semantic_cache`,
        // produce a warning here.
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
#[path = "gateway_env_tests.rs"]
mod env_tests;

#[cfg(test)]
#[path = "gateway_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gateway_pricing_tests.rs"]
mod pricing_tests;

#[cfg(test)]
#[path = "gateway_alias_tests.rs"]
mod alias_tests;
