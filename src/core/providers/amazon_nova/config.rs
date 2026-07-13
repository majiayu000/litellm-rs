//! Amazon Nova Configuration
//!
//! Configuration for Amazon Nova multimodal provider

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::base::config::BaseConfig;
use crate::core::traits::provider::ProviderConfig;
use serde::{Deserialize, Serialize};

/// Default Amazon Nova API base URL
pub const DEFAULT_AMAZON_NOVA_API_BASE: &str = "https://api.nova.amazon.com/v1";

/// Amazon Nova provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmazonNovaConfig {
    /// Base configuration
    #[serde(flatten)]
    pub base: BaseConfig,

    /// AWS Region (optional, for regional endpoints)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    /// Enable reasoning mode for supported models
    #[serde(default)]
    pub enable_reasoning: bool,
}

impl Default for AmazonNovaConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_base: Some(DEFAULT_AMAZON_NOVA_API_BASE.to_string()),
                ..Default::default()
            },
            region: None,
            enable_reasoning: false,
        }
    }
}

impl AmazonNovaConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut base = BaseConfig::from_env("amazon_nova");

        // Set default API base if not provided
        if base.api_base.is_none() {
            base.api_base = Some(DEFAULT_AMAZON_NOVA_API_BASE.to_string());
        }

        Self {
            base,
            region: std::env::var("AMAZON_NOVA_REGION").ok(),
            enable_reasoning: std::env::var("AMAZON_NOVA_ENABLE_REASONING")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
        }
    }

    /// Create new configuration with API key
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.base.api_key = Some(api_key.into());
        config
    }

    /// Set AWS region
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Enable reasoning mode
    pub fn with_reasoning(mut self) -> Self {
        self.enable_reasoning = true;
        self
    }

    /// Get effective API base URL
    pub fn get_api_base(&self) -> &str {
        self.base
            .api_base
            .as_deref()
            .unwrap_or(DEFAULT_AMAZON_NOVA_API_BASE)
    }

    /// Get effective API key
    pub fn get_api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }

    /// Get chat completions endpoint
    pub fn get_chat_endpoint(&self) -> String {
        format!("{}/chat/completions", self.get_api_base())
    }
}

impl ProviderConfig for AmazonNovaConfig {
    fn validate(&self) -> Result<(), String> {
        if self.base.api_key.is_none() {
            return Err("Amazon Nova API key is required".to_string());
        }
        if self.base.timeout == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }
        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.base.api_base.as_deref()
    }

    fn endpoint_access(&self) -> ProviderEndpointAccess {
        self.base.endpoint_access
    }

    fn timeout(&self) -> std::time::Duration {
        self.base.timeout_duration()
    }

    fn max_retries(&self) -> u32 {
        self.base.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AmazonNovaConfig::default();
        assert_eq!(
            config.base.api_base,
            Some(DEFAULT_AMAZON_NOVA_API_BASE.to_string())
        );
        assert!(config.region.is_none());
        assert!(!config.enable_reasoning);
    }

    #[test]
    fn test_with_api_key() {
        let config = AmazonNovaConfig::with_api_key("test-key-123");
        assert_eq!(config.base.api_key, Some("test-key-123".to_string()));
    }

    #[test]
    fn test_with_region() {
        let config = AmazonNovaConfig::with_api_key("key").with_region("us-east-1");
        assert_eq!(config.region, Some("us-east-1".to_string()));
    }

    #[test]
    fn test_with_reasoning() {
        let config = AmazonNovaConfig::with_api_key("key").with_reasoning();
        assert!(config.enable_reasoning);
    }

    #[test]
    fn test_validate_missing_api_key() {
        let config = AmazonNovaConfig::default();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }

    #[test]
    fn test_validate_success() {
        let config = AmazonNovaConfig::with_api_key("test-key");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_get_api_base() {
        let config = AmazonNovaConfig::default();
        assert_eq!(config.get_api_base(), DEFAULT_AMAZON_NOVA_API_BASE);
    }

    #[test]
    fn test_get_chat_endpoint() {
        let config = AmazonNovaConfig::default();
        let endpoint = config.get_chat_endpoint();
        assert!(endpoint.ends_with("/chat/completions"));
    }

    #[test]
    fn test_provider_config_trait() {
        let config = AmazonNovaConfig::with_api_key("my-key");
        assert_eq!(config.api_key(), Some("my-key"));
        assert_eq!(config.api_base(), Some(DEFAULT_AMAZON_NOVA_API_BASE));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
        assert_eq!(config.max_retries(), 3);
        assert_eq!(config.endpoint_access(), ProviderEndpointAccess::PublicOnly);

        let mut private = config;
        private.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        assert_eq!(
            private.endpoint_access(),
            ProviderEndpointAccess::PrivateNetwork
        );
    }

    #[test]
    fn test_config_serialization() {
        let config = AmazonNovaConfig::with_api_key("test-key").with_region("us-west-2");
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("test-key"));
        assert!(json.contains("us-west-2"));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{"api_key": "my-api-key", "region": "eu-west-1"}"#;
        let config: AmazonNovaConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.base.api_key, Some("my-api-key".to_string()));
        assert_eq!(config.region, Some("eu-west-1".to_string()));
    }

    #[test]
    fn test_builder_chain() {
        let config = AmazonNovaConfig::with_api_key("key")
            .with_region("ap-northeast-1")
            .with_reasoning();

        assert_eq!(config.base.api_key, Some("key".to_string()));
        assert_eq!(config.region, Some("ap-northeast-1".to_string()));
        assert!(config.enable_reasoning);
    }
}
