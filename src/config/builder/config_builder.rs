//! Main configuration builder implementation

use super::types::GatewayConfigBuilder;
use crate::config::{
    AuthConfig, Config, GatewayConfig, ProviderConfig, ServerConfig, StorageConfig,
};
use crate::utils::data::type_utils::Builder;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;

impl GatewayConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            server: None,
            auth: None,
            storage: None,
            providers: Vec::new(),
            features: HashMap::new(),
        }
    }

    /// Set the server configuration
    pub fn with_server(mut self, config: ServerConfig) -> Self {
        self.server = Some(config);
        self
    }

    /// Set the authentication configuration
    pub fn with_auth(mut self, config: AuthConfig) -> Self {
        self.auth = Some(config);
        self
    }

    /// Set the storage configuration
    pub fn with_storage(mut self, config: StorageConfig) -> Self {
        self.storage = Some(config);
        self
    }

    /// Add a provider configuration
    pub fn add_provider(mut self, config: ProviderConfig) -> Self {
        self.providers.push(config);
        self
    }

    /// Add multiple provider configurations
    pub fn add_providers(mut self, configs: Vec<ProviderConfig>) -> Self {
        self.providers.extend(configs);
        self
    }

    /// Enable a feature
    pub fn enable_feature(mut self, feature: impl Into<String>) -> Self {
        self.features.insert(feature.into(), true);
        self
    }

    /// Disable a feature
    pub fn disable_feature(mut self, feature: impl Into<String>) -> Self {
        self.features.insert(feature.into(), false);
        self
    }

    /// Build the configuration with validation
    pub fn build(self) -> Result<Config> {
        let gateway = GatewayConfig {
            schema_version: "1.0".to_string(),
            server: self.server.unwrap_or_default(),
            auth: self.auth.unwrap_or_default(),
            storage: self.storage.unwrap_or_default(),
            providers: self.providers,
            router: crate::config::models::router::GatewayRouterConfig::default(),
            monitoring: crate::config::models::monitoring::MonitoringConfig::default(),
            cache: crate::config::models::cache::CacheConfig::default(),
            rate_limit: crate::core::types::config::rate_limit::RateLimitConfig::default(),
            enterprise: crate::config::models::enterprise::EnterpriseConfig::default(),
            pricing: crate::config::models::gateway::GatewayPricingConfig::default(),
        };

        let config = Config { gateway };

        // Validate the configuration
        if let Err(e) = config.gateway.validate() {
            return Err(GatewayError::Config(e));
        }

        Ok(config)
    }

    /// Build the configuration or panic with a descriptive message
    ///
    /// # Panics
    /// This method will panic if the configuration validation fails.
    /// Use `build()` for fallible construction.
    pub fn build_or_panic(self) -> Config {
        self.build().unwrap_or_else(|e| {
            panic!("Failed to build configuration: {}", e);
        })
    }

    /// Build the configuration, returning defaults on validation failure
    ///
    /// This is useful when you need a guaranteed Config but want to avoid panics.
    /// Validation errors are logged as warnings.
    pub fn build_or_default(self) -> Config {
        self.build().unwrap_or_else(|e| {
            tracing::warn!("Configuration validation failed: {}, using defaults", e);
            Config::default()
        })
    }
}

impl Default for GatewayConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder<Config> for GatewayConfigBuilder {
    /// Build the configuration, returning defaults on validation failure
    ///
    /// Note: This trait requires a non-fallible return type.
    /// For fallible construction, use `GatewayConfigBuilder::build()` directly.
    fn build(self) -> Config {
        self.build_or_default()
    }
}
