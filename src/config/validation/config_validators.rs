//! Core configuration validators
//!
//! This module provides validation implementations for the main gateway configuration
//! structures including GatewayConfig, ServerConfig, and ProviderConfig.

use super::trait_def::Validate;
use crate::config::models::gateway::{GatewayConfig, GatewayPricingConfig};
use crate::config::models::provider::{ProviderConfig, ProviderHealthCheckConfig, RetryConfig};
use crate::config::models::server::ServerConfig;
use crate::core::net::{
    validate_provider_endpoint_url, validate_provider_endpoint_url_without_resolution,
};
use std::collections::HashSet;
use tracing::debug;

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

        Validate::validate(&self.router)?;
        Validate::validate(&self.storage)?;
        Validate::validate(&self.auth)?;
        Validate::validate(&self.monitoring)?;
        Validate::validate(&self.cache)?;
        Validate::validate(&self.rate_limit)?;
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
        if (self.endpoint_access == crate::core::net::ProviderEndpointAccess::PrivateNetwork
            || self.base_url.is_some())
            && !crate::core::providers::factory::is_provider_endpoint_access_supported(
                provider_selector,
            )
        {
            return Err(format!(
                "Provider {} configurable endpoint access is unavailable because provider type '{}' is not policy-wired",
                self.name, self.provider_type
            ));
        }
        if self.endpoint_access == crate::core::net::ProviderEndpointAccess::PrivateNetwork
            && self.base_url.is_none()
        {
            return Err(format!(
                "Provider {} private_network endpoint access requires a base URL",
                self.name
            ));
        }

        let requires_api_key =
            crate::core::providers::registry::get_definition(&provider_selector.to_lowercase())
                .map(|def| !def.skip_api_key)
                .unwrap_or(true);

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
        if let Some(base_url) = &self.base_url {
            let endpoint = url::Url::parse(base_url)
                .map_err(|error| format!("Provider {} base URL is invalid: {error}", self.name))?;
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
mod endpoint_access_tests {
    use super::*;
    use crate::core::net::ProviderEndpointAccess;

    fn public_provider() -> ProviderConfig {
        ProviderConfig {
            name: "endpoint-policy".to_string(),
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

        config.provider_type = "cloudflare".to_string();
        assert!(
            Validate::validate(&config)
                .unwrap_err()
                .contains("not policy-wired")
        );

        config.provider_type = "openai_compatible".to_string();
        config.endpoint_access = ProviderEndpointAccess::PublicOnly;
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
}
