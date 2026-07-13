//! Azure OpenAI Configuration
//!
//! Configuration for Azure OpenAI Service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::net::{ProviderEndpointAccess, ProviderEndpointPolicy};

/// Azure OpenAI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureConfig {
    /// Azure API key
    pub api_key: Option<String>,
    /// Azure endpoint URL
    pub azure_endpoint: Option<String>,
    /// Network scope allowed for the Azure endpoint.
    #[serde(default)]
    pub endpoint_access: ProviderEndpointAccess,
    /// API version
    pub api_version: String,
    /// Azure AD token provider
    pub azure_ad_token_provider: Option<String>,
    /// Deployment name
    pub deployment_name: Option<String>,
    /// Resource group
    pub resource_group: Option<String>,
    /// Subscription ID
    pub subscription_id: Option<String>,
    /// Custom headers
    pub custom_headers: HashMap<String, String>,
    /// Request timeout in seconds
    #[serde(default = "default_timeout_seconds")]
    pub timeout: u64,
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_timeout_seconds() -> u64 {
    60
}

fn default_max_retries() -> u32 {
    3
}

impl Default for AzureConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            azure_endpoint: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            api_version: "2024-02-01".to_string(),
            azure_ad_token_provider: None,
            deployment_name: None,
            resource_group: None,
            subscription_id: None,
            custom_headers: HashMap::new(),
            timeout: 60,
            max_retries: 3,
        }
    }
}

impl AzureConfig {
    /// Create new Azure configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Set Azure endpoint
    pub fn with_azure_endpoint(mut self, endpoint: String) -> Self {
        self.azure_endpoint = Some(endpoint);
        self
    }

    /// Set the endpoint network scope.
    pub fn with_endpoint_access(mut self, endpoint_access: ProviderEndpointAccess) -> Self {
        self.endpoint_access = endpoint_access;
        self
    }

    /// Set API version
    pub fn with_api_version(mut self, version: String) -> Self {
        self.api_version = version;
        self
    }

    /// Set deployment name
    pub fn with_deployment_name(mut self, deployment: String) -> Self {
        self.deployment_name = Some(deployment);
        self
    }

    /// Get effective API key (from config, environment, or Azure AD)
    pub async fn get_effective_api_key(&self) -> Option<String> {
        // Priority: config -> environment -> Azure AD token
        if let Some(key) = &self.api_key {
            return Some(key.clone());
        }

        if let Ok(key) = std::env::var("AZURE_OPENAI_KEY") {
            return Some(key);
        }

        if let Ok(key) = std::env::var("AZURE_API_KEY") {
            return Some(key);
        }

        // Try Azure AD token (would need Azure AD integration)
        if self.azure_ad_token_provider.is_some() {
            // For now, return None - would implement Azure AD token acquisition
            return None;
        }

        None
    }

    /// Get effective Azure endpoint
    pub fn get_effective_azure_endpoint(&self) -> Option<String> {
        self.azure_endpoint
            .clone()
            .or_else(|| std::env::var("AZURE_OPENAI_ENDPOINT").ok())
            .or_else(|| std::env::var("AZURE_ENDPOINT").ok())
    }

    /// Get effective deployment name
    pub fn get_effective_deployment_name(&self, model: &str) -> String {
        self.deployment_name
            .clone()
            .or_else(|| std::env::var("AZURE_DEPLOYMENT_NAME").ok())
            .unwrap_or_else(|| model.to_string())
    }
}

/// Implement ProviderConfig trait for AzureConfig
impl crate::core::traits::provider::ProviderConfig for AzureConfig {
    fn validate(&self) -> Result<(), String> {
        let endpoint = self
            .get_effective_azure_endpoint()
            .ok_or_else(|| "Azure endpoint is required".to_string())?;

        if self.api_version.is_empty() {
            return Err("API version is required".to_string());
        }

        ProviderEndpointPolicy::for_base_url(self.endpoint_access, &endpoint)
            .map_err(|error| format!("Invalid Azure endpoint: {error}"))?;

        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.azure_endpoint.as_deref()
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }

    fn endpoint_access(&self) -> ProviderEndpointAccess {
        self.endpoint_access
    }
}

/// Azure model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureModelInfo {
    pub deployment_name: String,
    pub model_name: String,
    pub max_tokens: Option<u32>,
    pub supports_functions: bool,
    pub supports_streaming: bool,
    pub api_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_config_default() {
        let config = AzureConfig::default();
        assert!(config.api_key.is_none());
        assert!(config.azure_endpoint.is_none());
        assert_eq!(config.endpoint_access, ProviderEndpointAccess::PublicOnly);
        assert_eq!(config.api_version, "2024-02-01");
        assert!(config.deployment_name.is_none());
        assert_eq!(config.timeout, 60);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_azure_config_builder() {
        let config = AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint("https://test.openai.azure.com".to_string())
            .with_endpoint_access(ProviderEndpointAccess::PrivateNetwork)
            .with_deployment_name("gpt-4".to_string())
            .with_api_version("2024-03-01".to_string());

        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(
            config.azure_endpoint,
            Some("https://test.openai.azure.com".to_string())
        );
        assert_eq!(config.deployment_name, Some("gpt-4".to_string()));
        assert_eq!(
            config.endpoint_access,
            ProviderEndpointAccess::PrivateNetwork
        );
        assert_eq!(config.api_version, "2024-03-01");
    }

    #[test]
    fn test_azure_config_effective_deployment_name() {
        let config = AzureConfig::new().with_deployment_name("my-deployment".to_string());
        assert_eq!(
            config.get_effective_deployment_name("gpt-4"),
            "my-deployment"
        );

        let config_no_deployment = AzureConfig::new();
        assert_eq!(
            config_no_deployment.get_effective_deployment_name("gpt-4"),
            "gpt-4"
        );
    }

    #[test]
    fn test_azure_config_effective_endpoint() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        assert_eq!(
            config.get_effective_azure_endpoint(),
            Some("https://test.openai.azure.com".to_string())
        );
    }

    #[test]
    fn test_azure_config_validation() {
        use crate::core::traits::provider::ProviderConfig;

        let config = AzureConfig::new();
        assert!(config.validate().is_err()); // Missing endpoint

        let config_with_endpoint =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        assert!(config_with_endpoint.validate().is_ok());
    }

    #[test]
    fn test_azure_config_provider_config_trait() {
        use crate::core::traits::provider::ProviderConfig;

        let config = AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint("https://test.openai.azure.com".to_string());

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://test.openai.azure.com"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
        assert_eq!(config.max_retries(), 3);
        assert_eq!(config.endpoint_access(), ProviderEndpointAccess::PublicOnly);

        let mut custom_config = config;
        custom_config.timeout = 12;
        custom_config.max_retries = 5;
        assert_eq!(custom_config.timeout(), std::time::Duration::from_secs(12));
        assert_eq!(custom_config.max_retries(), 5);
    }

    #[test]
    fn test_azure_config_deserializes_defaults_for_timeout_and_retries() {
        let config: AzureConfig = serde_json::from_str(
            r#"{
                "api_key": "test-key",
                "azure_endpoint": "https://test.openai.azure.com",
                "api_version": "2024-02-01",
                "custom_headers": {}
            }"#,
        )
        .unwrap_or_else(|error| panic!("AzureConfig should deserialize with defaults: {error}"));

        assert_eq!(config.timeout, 60);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.endpoint_access, ProviderEndpointAccess::PublicOnly);
    }

    #[test]
    fn test_azure_config_endpoint_policy() {
        use crate::core::traits::provider::ProviderConfig;

        let public = AzureConfig::new().with_azure_endpoint("http://127.0.0.1:18080".to_string());
        assert!(public.validate().is_err());

        let private = public.with_endpoint_access(ProviderEndpointAccess::PrivateNetwork);
        assert!(private.validate().is_ok());
    }

    #[test]
    fn test_azure_model_info() {
        let model_info = AzureModelInfo {
            deployment_name: "gpt-4-deployment".to_string(),
            model_name: "gpt-4".to_string(),
            max_tokens: Some(8192),
            supports_functions: true,
            supports_streaming: true,
            api_version: "2024-02-01".to_string(),
        };

        assert_eq!(model_info.deployment_name, "gpt-4-deployment");
        assert_eq!(model_info.model_name, "gpt-4");
        assert!(model_info.supports_functions);
        assert!(model_info.supports_streaming);
    }
}
