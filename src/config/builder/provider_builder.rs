//! Provider configuration builder implementation

use super::types::ProviderConfigBuilder;
use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess;
use crate::utils::data::type_utils::{NonEmptyString, PositiveF64};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::time::Duration;

impl ProviderConfigBuilder {
    /// Create a new provider configuration builder
    pub fn new() -> Self {
        Self {
            name: None,
            provider_type: None,
            api_key: None,
            base_url: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            models: Vec::new(),
            max_requests_per_minute: None,
            timeout: None,
            enabled: true,
            weight: None,
        }
    }

    /// Set the provider name
    pub fn name(mut self, name: impl TryInto<NonEmptyString>) -> Result<Self> {
        self.name = Some(
            name.try_into()
                .map_err(|_| GatewayError::Config("Provider name cannot be empty".to_string()))?,
        );
        Ok(self)
    }

    /// Set the provider type
    pub fn provider_type(mut self, provider_type: impl TryInto<NonEmptyString>) -> Result<Self> {
        self.provider_type = Some(
            provider_type
                .try_into()
                .map_err(|_| GatewayError::Config("Provider type cannot be empty".to_string()))?,
        );
        Ok(self)
    }

    /// Set the API key
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the base URL
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Set the requested network access for the provider endpoint.
    pub fn endpoint_access(mut self, endpoint_access: ProviderEndpointAccess) -> Self {
        self.endpoint_access = endpoint_access;
        self
    }

    /// Add a supported model
    pub fn add_model(mut self, model: impl Into<String>) -> Self {
        self.models.push(model.into());
        self
    }

    /// Set the rate limit
    pub fn rate_limit(mut self, requests_per_minute: u32) -> Self {
        self.max_requests_per_minute = Some(requests_per_minute);
        self
    }

    /// Set the timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Enable the provider
    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Disable the provider
    pub fn disable(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set the provider weight for load balancing
    pub fn weight(mut self, weight: f64) -> Result<Self> {
        self.weight =
            Some(PositiveF64::new(weight).map_err(|_| {
                GatewayError::Config("Provider weight must be positive".to_string())
            })?);
        Ok(self)
    }

    /// Build the provider configuration
    pub fn build(self) -> Result<ProviderConfig> {
        let name = self
            .name
            .ok_or_else(|| GatewayError::Config("Provider name is required".to_string()))?;

        let provider_type = self
            .provider_type
            .ok_or_else(|| GatewayError::Config("Provider type is required".to_string()))?;

        Ok(ProviderConfig {
            name: name.into_string(),
            provider_type: provider_type.into_string(),
            api_key: self.api_key.unwrap_or_default(),
            base_url: self.base_url,
            endpoint_access: self.endpoint_access,
            api_version: None,
            organization: None,
            project: None,
            weight: self.weight.map(|w| w.get() as f32).unwrap_or(1.0),
            priority: 0,
            rpm: self.max_requests_per_minute.unwrap_or(1000),
            tpm: 100000, // Default TPM
            max_concurrent_requests: 10,
            timeout: self.timeout.map(|d| d.as_secs()).unwrap_or(30),
            max_retries: 3,
            retry: crate::config::models::provider::RetryConfig::default(),
            health_check: crate::config::models::provider::ProviderHealthCheckConfig::default(),
            settings: std::collections::HashMap::new(),
            models: self.models,
            enabled: self.enabled,
            tags: Vec::new(),
        })
    }
}

impl Default for ProviderConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ProviderConfigBuilder Construction Tests ====================

    #[test]
    fn test_provider_config_builder_new() {
        let builder = ProviderConfigBuilder::new();
        assert!(builder.name.is_none());
        assert!(builder.provider_type.is_none());
        assert!(builder.api_key.is_none());
        assert!(builder.base_url.is_none());
        assert_eq!(builder.endpoint_access, ProviderEndpointAccess::PublicOnly);
        assert!(builder.models.is_empty());
        assert!(builder.max_requests_per_minute.is_none());
        assert!(builder.timeout.is_none());
        assert!(builder.enabled);
        assert!(builder.weight.is_none());
    }

    #[test]
    fn test_provider_config_builder_default() {
        let builder = ProviderConfigBuilder::default();
        assert!(builder.name.is_none());
        assert!(builder.enabled);
    }

    // ==================== Builder Method Tests ====================

    #[test]
    fn test_provider_config_builder_name() {
        let builder = ProviderConfigBuilder::new().name("my-provider").unwrap();
        assert!(builder.name.is_some());
    }

    #[test]
    fn test_provider_config_builder_name_empty() {
        let result = ProviderConfigBuilder::new().name("");
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_config_builder_provider_type() {
        let builder = ProviderConfigBuilder::new()
            .provider_type("openai")
            .unwrap();
        assert!(builder.provider_type.is_some());
    }

    #[test]
    fn test_provider_config_builder_provider_type_empty() {
        let result = ProviderConfigBuilder::new().provider_type("");
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_config_builder_api_key() {
        let builder = ProviderConfigBuilder::new().api_key("sk-test-key");
        assert_eq!(builder.api_key, Some("sk-test-key".to_string()));
    }

    #[test]
    fn test_provider_config_builder_api_key_string() {
        let builder = ProviderConfigBuilder::new().api_key(String::from("my-api-key"));
        assert_eq!(builder.api_key, Some("my-api-key".to_string()));
    }

    #[test]
    fn test_provider_config_builder_base_url() {
        let builder = ProviderConfigBuilder::new().base_url("https://api.example.com");
        assert_eq!(
            builder.base_url,
            Some("https://api.example.com".to_string())
        );
    }

    #[test]
    fn test_provider_config_builder_add_model() {
        let builder = ProviderConfigBuilder::new().add_model("gpt-4");
        assert_eq!(builder.models, vec!["gpt-4"]);
    }

    #[test]
    fn test_provider_config_builder_endpoint_access() {
        let config = ProviderConfigBuilder::new()
            .name("local")
            .unwrap()
            .provider_type("openai_compatible")
            .unwrap()
            .endpoint_access(ProviderEndpointAccess::PrivateNetwork)
            .build()
            .unwrap();
        assert_eq!(
            config.endpoint_access,
            ProviderEndpointAccess::PrivateNetwork
        );
    }

    #[test]
    fn test_provider_config_builder_add_multiple_models() {
        let builder = ProviderConfigBuilder::new()
            .add_model("gpt-4")
            .add_model("gpt-3.5-turbo")
            .add_model("claude-3-opus");
        assert_eq!(builder.models.len(), 3);
        assert!(builder.models.contains(&"gpt-4".to_string()));
        assert!(builder.models.contains(&"gpt-3.5-turbo".to_string()));
        assert!(builder.models.contains(&"claude-3-opus".to_string()));
    }

    #[test]
    fn test_provider_config_builder_rate_limit() {
        let builder = ProviderConfigBuilder::new().rate_limit(5000);
        assert_eq!(builder.max_requests_per_minute, Some(5000));
    }

    #[test]
    fn test_provider_config_builder_timeout() {
        let builder = ProviderConfigBuilder::new().timeout(Duration::from_secs(60));
        assert_eq!(builder.timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_provider_config_builder_enable() {
        let builder = ProviderConfigBuilder::new().disable().enable();
        assert!(builder.enabled);
    }

    #[test]
    fn test_provider_config_builder_disable() {
        let builder = ProviderConfigBuilder::new().disable();
        assert!(!builder.enabled);
    }

    #[test]
    fn test_provider_config_builder_weight() {
        let builder = ProviderConfigBuilder::new().weight(2.5).unwrap();
        assert!(builder.weight.is_some());
        assert!((builder.weight.unwrap().get() - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_provider_config_builder_weight_zero() {
        let result = ProviderConfigBuilder::new().weight(0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_config_builder_weight_negative() {
        let result = ProviderConfigBuilder::new().weight(-1.0);
        assert!(result.is_err());
    }

    // ==================== Builder Chain Tests ====================

    #[test]
    fn test_provider_config_builder_chain() {
        let builder = ProviderConfigBuilder::new()
            .name("openai-provider")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .api_key("sk-test")
            .base_url("https://api.openai.com/v1")
            .add_model("gpt-4")
            .add_model("gpt-3.5-turbo")
            .rate_limit(3000)
            .timeout(Duration::from_secs(30))
            .weight(1.5)
            .unwrap();

        assert!(builder.name.is_some());
        assert!(builder.provider_type.is_some());
        assert_eq!(builder.api_key, Some("sk-test".to_string()));
        assert_eq!(builder.models.len(), 2);
        assert_eq!(builder.max_requests_per_minute, Some(3000));
    }

    // ==================== Build Tests ====================

    #[test]
    fn test_provider_config_builder_build_success() {
        let config = ProviderConfigBuilder::new()
            .name("test-provider")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .api_key("test-key")
            .build()
            .unwrap();

        assert_eq!(config.name, "test-provider");
        assert_eq!(config.provider_type, "openai");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.priority, 0);
    }

    #[test]
    fn test_provider_config_builder_build_missing_name() {
        let result = ProviderConfigBuilder::new()
            .provider_type("openai")
            .unwrap()
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_config_builder_build_missing_provider_type() {
        let result = ProviderConfigBuilder::new().name("test").unwrap().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_config_builder_build_defaults() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(config.api_key, "");
        assert!(config.base_url.is_none());
        assert_eq!(config.endpoint_access, ProviderEndpointAccess::PublicOnly);
        assert_eq!(config.weight, 1.0);
        assert_eq!(config.rpm, 1000);
        assert_eq!(config.tpm, 100000);
        assert_eq!(config.max_concurrent_requests, 10);
        assert_eq!(config.timeout, 30);
        assert_eq!(config.max_retries, 3);
        assert!(config.enabled);
        assert!(config.models.is_empty());
        assert!(config.tags.is_empty());
    }

    #[test]
    fn test_provider_config_builder_build_with_weight() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .weight(2.0)
            .unwrap()
            .build()
            .unwrap();

        assert!((config.weight - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_provider_config_builder_build_with_rate_limit() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .rate_limit(5000)
            .build()
            .unwrap();

        assert_eq!(config.rpm, 5000);
    }

    #[test]
    fn test_provider_config_builder_build_with_timeout() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap();

        assert_eq!(config.timeout, 120);
    }

    #[test]
    fn test_provider_config_builder_build_disabled() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .disable()
            .build()
            .unwrap();

        assert!(!config.enabled);
    }

    #[test]
    fn test_provider_config_builder_build_with_models() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .add_model("gpt-4")
            .add_model("gpt-3.5-turbo")
            .build()
            .unwrap();

        assert_eq!(config.models.len(), 2);
        assert!(config.models.contains(&"gpt-4".to_string()));
    }

    #[test]
    fn test_provider_config_builder_build_with_base_url() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .base_url("https://custom.api.com")
            .build()
            .unwrap();

        assert_eq!(config.base_url, Some("https://custom.api.com".to_string()));
    }

    // ==================== Clone and Debug Tests ====================

    #[test]
    fn test_provider_config_builder_clone() {
        let builder = ProviderConfigBuilder::new()
            .api_key("test-key")
            .add_model("gpt-4");
        let cloned = builder.clone();

        assert_eq!(builder.api_key, cloned.api_key);
        assert_eq!(builder.models, cloned.models);
    }

    #[test]
    fn test_provider_config_builder_debug() {
        let builder = ProviderConfigBuilder::new().rate_limit(1000);
        let debug_str = format!("{:?}", builder);

        assert!(debug_str.contains("ProviderConfigBuilder"));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_provider_config_builder_empty_api_key() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .api_key("")
            .build()
            .unwrap();

        assert_eq!(config.api_key, "");
    }

    #[test]
    fn test_provider_config_builder_rate_limit_zero() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .rate_limit(0)
            .build()
            .unwrap();

        assert_eq!(config.rpm, 0);
    }

    #[test]
    fn test_provider_config_builder_timeout_zero() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .timeout(Duration::ZERO)
            .build()
            .unwrap();

        assert_eq!(config.timeout, 0);
    }

    #[test]
    fn test_provider_config_builder_very_small_weight() {
        let config = ProviderConfigBuilder::new()
            .name("test")
            .unwrap()
            .provider_type("openai")
            .unwrap()
            .weight(0.001)
            .unwrap()
            .build()
            .unwrap();

        assert!(config.weight > 0.0);
    }
}
