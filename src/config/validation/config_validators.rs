//! Core configuration validators
//!
//! This module provides validation implementations for the main gateway configuration
//! structures including GatewayConfig, ServerConfig, and ProviderConfig.

use super::ssrf::validate_url_against_ssrf;
use super::trait_def::Validate;
use crate::config::models::gateway::{GatewayConfig, GatewayPricingConfig};
use crate::config::models::provider::{ProviderConfig, ProviderHealthCheckConfig, RetryConfig};
use crate::config::models::server::ServerConfig;
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
            return Err("Server port must be greater than 0".to_string());
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
            validate_url_against_ssrf(base_url, &format!("Provider {} base URL", self.name))?;
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

        // Validate health check configuration
        self.health_check.validate()?;

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

        if self.backoff_multiplier <= 0.0 {
            return Err("Retry backoff multiplier must be greater than 0".to_string());
        }

        // Validate jitter is between 0.0 and 1.0
        if self.jitter < 0.0 || self.jitter > 1.0 {
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

        Ok(())
    }
}
