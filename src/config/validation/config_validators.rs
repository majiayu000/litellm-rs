//! Core configuration validators
//!
//! This module provides validation implementations for the main gateway configuration
//! structures including GatewayConfig, ServerConfig, and ProviderConfig.

use super::trait_def::Validate;
use crate::config::models::gateway::{GatewayConfig, GatewayPricingConfig};
use crate::config::models::provider::{ProviderConfig, ProviderHealthCheckConfig, RetryConfig};
use crate::config::models::server::ServerConfig;
use crate::core::net::{
    ProviderEndpointAccess, ProviderEndpointPolicy, validate_provider_endpoint_url,
    validate_provider_endpoint_url_without_resolution,
};
use crate::core::providers::factory::{endpoint_keys_for_selector, invalid_endpoint};
use std::collections::{HashMap, HashSet};
use tracing::debug;

impl GatewayConfig {
    /// Validate alias shape and graph structure without consulting provider models.
    pub(crate) fn validate_model_alias_map(
        model_aliases: &HashMap<String, String>,
    ) -> Result<(), String> {
        let mut alias_names = model_aliases.keys().map(String::as_str).collect::<Vec<_>>();
        alias_names.sort_unstable();

        for alias in &alias_names {
            let target = &model_aliases[*alias];
            if alias.trim().is_empty() {
                return Err("Model alias name cannot be empty".to_string());
            }
            if *alias != alias.trim() {
                return Err(format!(
                    "Model alias name '{alias}' cannot contain leading or trailing whitespace"
                ));
            }
            if target.trim().is_empty() {
                return Err(format!("Model alias '{alias}' target cannot be empty"));
            }
            if alias.trim() == target.trim() {
                return Err(format!("Model alias '{alias}' cannot target itself"));
            }
        }

        for alias in alias_names {
            let mut current = alias;
            let mut visited = HashSet::new();
            let mut path = Vec::new();

            loop {
                if !visited.insert(current) {
                    path.push(current);
                    return Err(format!("Model alias cycle detected: {}", path.join(" -> ")));
                }
                path.push(current);
                let Some(next) = model_aliases.get(current) else {
                    break;
                };
                current = next;
            }
        }

        Ok(())
    }
}

impl Validate for GatewayConfig {
    fn validate(&self) -> Result<(), String> {
        debug!("Validating gateway configuration");

        // Validate schema version
        if self.schema_version.is_empty() {
            return Err("Schema version cannot be empty".to_string());
        }

        // Check schema version compatibility
        let supported_versions = ["1.0"];
        if !supported_versions.contains(&self.schema_version.as_str()) {
            return Err(format!(
                "Unsupported schema version '{}'. Supported versions: {}",
                self.schema_version,
                supported_versions.join(", ")
            ));
        }

        Validate::validate(&self.server)?;
        self.server.cors.validate()?;

        // Validate providers
        if self.providers.is_empty() {
            return Err("At least one provider must be configured".to_string());
        }

        // Check for duplicate provider names
        let mut provider_names = HashSet::new();
        for provider in &self.providers {
            if !provider_names.insert(&provider.name) {
                return Err(format!("Duplicate provider name: {}", provider.name));
            }
            Validate::validate(provider)?;
        }

        Self::validate_model_alias_map(&self.model_aliases)?;
        Validate::validate(&self.router)?;
        Validate::validate(&self.storage)?;
        Validate::validate(&self.auth)?;
        Validate::validate(&self.monitoring)?;
        Validate::validate(&self.cache)?;
        Validate::validate(&self.rate_limit)?;
        self.guardrails.validate()?;
        crate::config::models::validate_gateway_guardrails(&self.guardrails)?;
        self.ip_access.validate()?;
        Validate::validate(&self.enterprise)?;
        Validate::validate(&self.pricing)?;

        debug!("Gateway configuration validation completed");
        Ok(())
    }
}

impl Validate for GatewayPricingConfig {
    fn validate(&self) -> Result<(), String> {
        if let Some(source) = &self.source
            && source.trim().is_empty()
        {
            return Err("Pricing source cannot be empty when provided".to_string());
        }

        if let Some(cost) = self.unpriced_fallback_cost_per_1k_tokens
            && (!cost.is_finite() || cost < 0.0)
        {
            return Err(
                "pricing.unpriced_fallback_cost_per_1k_tokens must be finite and >= 0.0"
                    .to_string(),
            );
        }

        Ok(())
    }
}

impl Validate for ServerConfig {
    fn validate(&self) -> Result<(), String> {
        debug!("Validating server configuration");

        if self.host.is_empty() {
            return Err("Server host cannot be empty".to_string());
        }

        if self.port == 0 {
            return Err("Server port must be between 1 and 65535".to_string());
        }

        if self.port < 1024 && !cfg!(test) {
            return Err("Server port should be >= 1024 for non-root users".to_string());
        }

        if let Some(workers) = self.workers {
            if workers == 0 {
                return Err("Worker count must be greater than 0".to_string());
            }
            if workers > 1000 {
                return Err("Worker count seems too high (>1000)".to_string());
            }
        }

        if self.timeout == 0 {
            return Err("Server timeout must be greater than 0".to_string());
        }

        if self.timeout > 3600 {
            return Err("Server timeout should not exceed 1 hour".to_string());
        }

        if self.max_body_size == 0 {
            return Err("Max body size must be greater than 0".to_string());
        }

        if self.max_body_size > 1024 * 1024 * 100 {
            // 100MB
            return Err("Max body size should not exceed 100MB".to_string());
        }

        // Validate TLS configuration if present
        if let Some(tls) = &self.tls {
            if tls.cert_file.is_empty() {
                return Err("TLS cert file path cannot be empty".to_string());
            }
            if tls.key_file.is_empty() {
                return Err("TLS key file path cannot be empty".to_string());
            }
        }

        Ok(())
    }
}

impl Validate for ProviderConfig {
    fn validate(&self) -> Result<(), String> {
        debug!("Validating provider configuration: {}", self.name);

        if self.name.is_empty() {
            return Err("Provider name cannot be empty".to_string());
        }

        if self.provider_type.is_empty() {
            return Err(format!("Provider {} type cannot be empty", self.name));
        }

        let provider_selector = self.provider_type.as_str();
        if !crate::core::providers::is_provider_selector_supported(provider_selector) {
            return Err(format!(
                "Provider {} type '{}' is not supported by current runtime factory/catalog",
                self.name, self.provider_type
            ));
        }

        if self.settings.contains_key("endpoint_access") {
            return Err(format!(
                "Provider {} endpoint_access must be configured as a top-level field",
                self.name
            ));
        }
        let blank_base = self
            .base_url
            .as_ref()
            .is_some_and(|url| url.trim().is_empty());
        let endpoint_keys = endpoint_keys_for_selector(provider_selector);
        if blank_base
            || endpoint_keys
                .iter()
                .copied()
                .any(|key| invalid_endpoint(self.settings.get(key)))
        {
            return Err(format!("Provider {} endpoint must be a string", self.name));
        }
        let configured_endpoint = self.configured_endpoint();
        let has_configured_endpoint = configured_endpoint.is_some();
        if (self.endpoint_access == crate::core::net::ProviderEndpointAccess::PrivateNetwork
            || has_configured_endpoint)
            && !crate::core::providers::factory::selector_supports_endpoint_access(
                provider_selector,
            )
        {
            return Err(format!(
                "Provider {} configurable endpoint access is unavailable because provider type '{}' is not policy-wired",
                self.name, self.provider_type
            ));
        }
        if self.endpoint_access == crate::core::net::ProviderEndpointAccess::PrivateNetwork
            && !has_configured_endpoint
            && !crate::core::providers::factory::selector_allows_implicit_private(provider_selector)
        {
            return Err(format!(
                "Provider {} private_network endpoint access requires a base URL",
                self.name
            ));
        }
        crate::core::providers::factory::validate_private_official_openai_endpoint(
            self.endpoint_access,
            configured_endpoint,
        )
        .map_err(|message| format!("Provider {} {message}", self.name))?;

        let requires_api_key =
            !crate::core::providers::registry::selector_skips_api_key(provider_selector);

        if requires_api_key && self.api_key.is_empty() {
            return Err(format!("Provider {} API key cannot be empty", self.name));
        }

        if self.weight <= 0.0 {
            return Err(format!(
                "Provider {} weight must be greater than 0",
                self.name
            ));
        }

        if self.weight > 100.0 {
            return Err(format!(
                "Provider {} weight should not exceed 100",
                self.name
            ));
        }

        if self.timeout == 0 {
            return Err(format!(
                "Provider {} timeout must be greater than 0",
                self.name
            ));
        }

        if self.timeout > 300 {
            return Err(format!(
                "Provider {} timeout should not exceed 5 minutes",
                self.name
            ));
        }

        // Validate base URL if present (with SSRF protection)
        if let Some(base_url) = configured_endpoint {
            let endpoint = url::Url::parse(base_url)
                .map_err(|error| format!("Provider {} base URL is invalid: {error}", self.name))?;
            if !matches!(endpoint.scheme(), "http" | "https") {
                return Err(format!(
                    "Provider {} base URL must use http:// or https:// scheme",
                    self.name
                ));
            }
            validate_provider_endpoint_url_without_resolution(&endpoint, self.endpoint_access)
                .map_err(|error| format!("Provider {} base URL is invalid: {error}", self.name))?;
        }

        // Validate rate limits
        if self.rpm == 0 {
            return Err(format!("Provider {} RPM must be greater than 0", self.name));
        }

        if self.tpm == 0 {
            return Err(format!("Provider {} TPM must be greater than 0", self.name));
        }

        if self.max_concurrent_requests == 0 {
            return Err(format!(
                "Provider {} max concurrent requests must be greater than 0",
                self.name
            ));
        }

        // Validate retry configuration
        self.retry.validate()?;

        self.validate_health_check_runtime()?;

        Ok(())
    }
}

impl ProviderConfig {
    /// Validate health settings at every runtime construction boundary.
    pub(crate) fn validate_health_check_runtime(&self) -> Result<(), String> {
        self.health_check.validate()?;
        if self.health_check.has_runtime_overrides()
            && self.health_check.endpoint.is_none()
            && matches!(
                self.provider_type.trim().parse(),
                Ok(crate::core::providers::provider_type::ProviderType::FalAI)
            )
        {
            return Err(format!(
                "Provider {} FalAI active health checks require a custom health_check.endpoint",
                self.name
            ));
        }
        if let Some(endpoint) = self.resolved_health_check_endpoint()? {
            if !endpoint.username().is_empty() || endpoint.password().is_some() {
                return Err(format!(
                    "Provider {} health check endpoint cannot contain URL credentials",
                    self.name
                ));
            }
            if !matches!(endpoint.scheme(), "http" | "https") {
                return Err(format!(
                    "Provider {} health check endpoint must use http:// or https:// scheme, got: {}",
                    self.name,
                    endpoint.scheme()
                ));
            }
            if self.endpoint_access == ProviderEndpointAccess::PrivateNetwork {
                let base_url = self.configured_endpoint().ok_or_else(|| {
                    format!(
                        "Provider {} private health check requires a configured endpoint",
                        self.name
                    )
                })?;
                let policy = ProviderEndpointPolicy::for_base_url(self.endpoint_access, base_url)
                    .map_err(|error| {
                    format!("Provider {} endpoint policy is invalid: {error}", self.name)
                })?;
                policy
                    .validate_url_without_resolution(&endpoint)
                    .map_err(|error| {
                        format!(
                            "Provider {} health check endpoint is invalid: {error}",
                            self.name
                        )
                    })?;
            }
            validate_provider_endpoint_url(&endpoint, self.endpoint_access).map_err(|error| {
                format!(
                    "Provider {} health check endpoint is invalid: {error}",
                    self.name
                )
            })?;
        }

        Ok(())
    }
}

impl Validate for RetryConfig {
    fn validate(&self) -> Result<(), String> {
        if self.base_delay == 0 {
            return Err("Retry base delay must be greater than 0".to_string());
        }

        if self.max_delay == 0 {
            return Err("Retry max delay must be greater than 0".to_string());
        }

        if self.base_delay > self.max_delay {
            return Err("Retry base delay cannot be greater than max delay".to_string());
        }

        if !self.backoff_multiplier.is_finite() {
            return Err("Retry backoff multiplier must be finite".to_string());
        }

        if self.backoff_multiplier <= 0.0 {
            return Err("Retry backoff multiplier must be greater than 0".to_string());
        }

        if !self.jitter.is_finite() {
            return Err("Retry jitter must be finite".to_string());
        }

        if !(0.0..=1.0).contains(&self.jitter) {
            return Err("Retry jitter must be between 0.0 and 1.0".to_string());
        }

        Ok(())
    }
}

impl Validate for ProviderHealthCheckConfig {
    fn validate(&self) -> Result<(), String> {
        if self.interval == 0 {
            return Err("Health check interval must be greater than 0".to_string());
        }

        if self.failure_threshold == 0 {
            return Err("Health check failure threshold must be greater than 0".to_string());
        }

        if self.recovery_timeout == 0 {
            return Err("Health check recovery timeout must be greater than 0".to_string());
        }

        if self.expected_codes.is_empty() {
            return Err("Health check expected codes cannot be empty".to_string());
        }

        let mut unique_codes = HashSet::with_capacity(self.expected_codes.len());
        for code in &self.expected_codes {
            if !(100..=599).contains(code) {
                return Err(format!(
                    "Health check expected status code {code} must be between 100 and 599"
                ));
            }
            if !unique_codes.insert(*code) {
                return Err(format!(
                    "Health check expected status code {code} cannot be duplicated"
                ));
            }
        }

        if self.endpoint.is_none()
            && self.expected_codes != crate::config::models::default_health_expected_codes()
        {
            return Err(
                "Health check expected_codes requires a custom endpoint; provider-native checks do not expose HTTP status codes"
                    .to_string(),
            );
        }

        if self
            .endpoint
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("Health check endpoint cannot be empty".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod model_alias_validation_tests {
    use super::*;

    fn aliases(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(alias, target)| ((*alias).to_string(), (*target).to_string()))
            .collect()
    }

    #[test]
    fn phase_a_rejects_empty_self_and_cyclic_aliases() {
        for (entries, expected) in [
            (vec![("", "model")], "name cannot be empty"),
            (vec![("alias", " ")], "target cannot be empty"),
            (vec![("alias", "alias")], "cannot target itself"),
            (vec![("alias", " alias ")], "cannot target itself"),
            (vec![("a", "b"), ("b", "a")], "cycle detected"),
            (vec![("a", "b"), ("b", "c"), ("c", "a")], "cycle detected"),
        ] {
            let error = GatewayConfig::validate_model_alias_map(&aliases(&entries))
                .expect_err("invalid alias graph must fail");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn phase_a_rejects_alias_names_with_surrounding_whitespace() {
        for alias in [" public", "public ", " public "] {
            let error = GatewayConfig::validate_model_alias_map(&aliases(&[(alias, "gpt-4o")]))
                .expect_err("alias names must not contain surrounding whitespace");
            assert!(
                error.contains("cannot contain leading or trailing whitespace"),
                "{error}"
            );
        }

        assert!(GatewayConfig::validate_model_alias_map(&aliases(&[("public", "gpt-4o")])).is_ok());
    }

    #[test]
    fn phase_a_accepts_order_independent_long_chains_without_model_lookup() {
        let forward = aliases(&[
            ("public", "internal"),
            ("internal", "future-provider-model"),
        ]);
        let reverse = aliases(&[
            ("internal", "future-provider-model"),
            ("public", "internal"),
        ]);

        assert!(GatewayConfig::validate_model_alias_map(&forward).is_ok());
        assert!(GatewayConfig::validate_model_alias_map(&reverse).is_ok());

        let long_chain = (0..20)
            .map(|index| {
                (
                    format!("alias-{index}"),
                    if index == 19 {
                        "canonical".to_string()
                    } else {
                        format!("alias-{}", index + 1)
                    },
                )
            })
            .collect();
        assert!(GatewayConfig::validate_model_alias_map(&long_chain).is_ok());
    }
}

#[cfg(test)]
mod endpoint_access_tests {
    use super::*;
    use crate::core::net::ProviderEndpointAccess;

    fn public_provider() -> ProviderConfig {
        ProviderConfig {
            name: "staged".to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some("https://8.8.8.8/v1".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn endpoint_access_is_top_level_and_private_is_provider_gated() {
        let mut config = public_provider();
        assert!(Validate::validate(&config).is_ok());

        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        assert!(Validate::validate(&config).is_ok());

        config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
        assert!(Validate::validate(&config).is_ok());

        config.base_url = None;
        for (provider_type, allowed) in [("bedrock", true), ("vllm", true), ("openrouter", false)] {
            config.provider_type = provider_type.to_string();
            assert_eq!(Validate::validate(&config).is_ok(), allowed);
        }
        config.provider_type = "cloudflare".to_string();
        assert!(
            Validate::validate(&config)
                .unwrap_err()
                .contains("not policy-wired")
        );

        config.provider_type = "openai_compatible".to_string();
        config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
        config.endpoint_access = ProviderEndpointAccess::PublicOnly;
        assert!(Validate::validate(&config).is_err());
        config.base_url = Some("wss://8.8.8.8/v1".to_string());
        assert!(Validate::validate(&config).is_err());
        config.base_url = Some(" ".to_string());
        assert!(Validate::validate(&config).is_err());
        config.base_url = Some("https://8.8.8.8/v1".to_string());
        config.settings.insert(
            "endpoint_access".to_string(),
            serde_json::json!("private_network"),
        );
        assert!(
            Validate::validate(&config)
                .unwrap_err()
                .contains("top-level")
        );
    }

    #[cfg(feature = "providers-extended")]
    #[test]
    fn keyless_ollama_startup_validation_preserves_endpoint_policy() {
        let mut config = public_provider();
        config.name = "local-ollama".to_string();
        config.provider_type = "ollama".to_string();
        config.api_key.clear();
        config.base_url = None;
        assert!(Validate::validate(&config).is_ok());

        config.base_url = Some("http://127.0.0.1:11434".to_string());
        let error = Validate::validate(&config)
            .expect_err("explicit public-only loopback must remain fail-closed");
        assert!(error.contains("private or reserved"), "{error}");

        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        assert!(Validate::validate(&config).is_ok());
    }

    #[test]
    fn health_check_runtime_honors_endpoint_access() {
        let mut config = public_provider();
        config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
        config.health_check.endpoint = Some("health".to_string());
        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;

        assert!(config.validate_health_check_runtime().is_ok());

        config.endpoint_access = ProviderEndpointAccess::PublicOnly;
        assert!(config.validate_health_check_runtime().is_err());

        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        config.health_check.endpoint = Some("ws://127.0.0.1:18080/health".to_string());
        let error = config
            .validate_health_check_runtime()
            .expect_err("health checks must remain limited to HTTP transports");
        assert!(error.contains("http:// or https://"));
    }

    #[test]
    fn azure_endpoint_aliases_are_provider_specific_endpoint_sources() {
        for (provider_type, key, endpoint) in [
            (
                "azure",
                "endpoint",
                "http://127.0.0.1:18080/openai/deployments/test",
            ),
            (
                "azure",
                "azure_endpoint",
                "http://127.0.0.1:18080/openai/deployments/test",
            ),
            ("azure_ai", "endpoint", "http://127.0.0.1:18080/models"),
            (
                "azure_ai",
                "azure_ai_endpoint",
                "http://127.0.0.1:18080/models",
            ),
        ] {
            let mut config = public_provider();
            config.provider_type = provider_type.to_string();
            config.base_url = None;
            config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
            config.settings.insert(key.to_string(), endpoint.into());
            assert!(Validate::validate(&config).is_ok(), "{provider_type}.{key}");
        }

        let mut unrelated = public_provider();
        unrelated.base_url = None;
        unrelated.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        unrelated
            .settings
            .insert("endpoint".to_string(), "http://127.0.0.1:18080/v1".into());
        let error = Validate::validate(&unrelated).expect_err("OpenAI aliases must stay closed");
        assert!(error.contains("requires a base URL"), "{error}");

        let mut invalid = public_provider();
        invalid.provider_type = "azure".to_string();
        invalid.base_url = None;
        invalid
            .settings
            .insert("azure_endpoint".to_string(), serde_json::json!(42));
        let error = Validate::validate(&invalid).expect_err("invalid alias must fail");
        assert!(error.contains("must be a string"), "azure: {error}");
    }

    #[test]
    fn provider_selector_normalization_preserves_endpoint_alias_policy() {
        let mut config = public_provider();
        config.provider_type = " Azure ".to_string();
        config.base_url = None;
        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        config.settings.insert(
            "azure_endpoint".to_string(),
            "http://127.0.0.1:18080/openai/deployments/test".into(),
        );

        assert!(Validate::validate(&config).is_ok());
    }

    #[cfg(feature = "providers-extra")]
    #[test]
    fn vertex_endpoint_alias_is_a_provider_specific_endpoint_source() {
        let mut config = public_provider();
        config.provider_type = "vertex_ai".to_string();
        config.base_url = None;
        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        config
            .settings
            .insert("endpoint".to_string(), "http://127.0.0.1:18080/v1".into());
        assert!(Validate::validate(&config).is_ok());

        for value in [serde_json::json!(42), serde_json::json!(" ")] {
            config.settings.insert("endpoint".to_string(), value);
            let error = Validate::validate(&config).expect_err("invalid alias must fail");
            assert!(error.contains("must be a string"), "{error}");
        }
    }

    #[test]
    fn private_health_check_is_bound_to_the_provider_authority() {
        let mut config = public_provider();
        config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        for endpoint in ["health", "http://127.0.0.1:18080/health"] {
            config.health_check.endpoint = Some(endpoint.to_string());
            assert!(config.validate_health_check_runtime().is_ok(), "{endpoint}");
        }
        for endpoint in [
            "http://127.0.0.2:18080/health",
            "http://127.0.0.1:18081/health",
            "https://127.0.0.1:18080/health",
        ] {
            config.health_check.endpoint = Some(endpoint.to_string());
            let error = config
                .validate_health_check_runtime()
                .expect_err("private health authority must not expand");
            assert!(error.contains("authority"), "{endpoint}: {error}");
        }

        config.base_url = None;
        config.provider_type = "azure".to_string();
        config.settings.insert(
            "azure_endpoint".to_string(),
            "http://127.0.0.1:18080/openai/deployments/test".into(),
        );
        config.health_check.endpoint = Some("health".to_string());
        assert!(config.validate_health_check_runtime().is_ok());
    }

    #[test]
    fn official_openai_endpoint_cannot_receive_private_access() {
        for provider_type in ["openai", "openai_compatible"] {
            for endpoint_key in ["base_url", "api_base"] {
                let mut config = public_provider();
                config.provider_type = provider_type.to_string();
                config.base_url = None;
                config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
                if endpoint_key == "base_url" {
                    config.base_url = Some("https://api.openai.com/v1".to_string());
                } else {
                    config
                        .settings
                        .insert(endpoint_key.to_string(), "https://api.openai.com/v1".into());
                }
                let error =
                    Validate::validate(&config).expect_err("official OpenAI must stay public");
                assert!(
                    error.contains("official OpenAI"),
                    "{provider_type}.{endpoint_key}: {error}"
                );
            }
        }
    }
}
