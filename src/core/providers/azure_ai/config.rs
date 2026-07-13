//! Azure AI Configuration
//!
//! Configuration

// use serde::{Deserialize, Serialize};  // Not needed with the macro
use std::collections::HashMap;
use url::Url;

use crate::core::net::{ProviderEndpointAccess, ProviderEndpointPolicy};
use crate::core::traits::provider::ProviderConfig;
use crate::define_provider_config;

// Configuration
define_provider_config!(AzureAIConfig {});

impl AzureAIConfig {
    /// Create
    pub fn from_env() -> Self {
        let mut config = Self::new("azure_ai");

        // Default
        if config.base.api_base.is_none() {
            if let Ok(api_base) = std::env::var("AZURE_AI_API_BASE") {
                config.base.api_base = Some(api_base);
            } else if let Ok(endpoint) = std::env::var("AZURE_AI_ENDPOINT") {
                config.base.api_base = Some(endpoint);
            }
        }

        // Settings
        if config.base.api_key.is_none() {
            if let Ok(api_key) = std::env::var("AZURE_AI_API_KEY") {
                config.base.api_key = Some(api_key);
            } else if let Ok(api_key) = std::env::var("AZURE_API_KEY") {
                config.base.api_key = Some(api_key);
            }
        }

        config
    }

    /// Build
    pub fn build_endpoint_url(&self, path: &str) -> Result<String, String> {
        let base_url = self
            .base
            .api_base
            .as_ref()
            .ok_or("Azure AI API base URL not set")?;

        // Ensure base URL ends with '/' and path doesn't start with '/'
        let base = base_url.trim_end_matches('/');
        let endpoint_path = path.trim_start_matches('/');
        if endpoint_path == "models" && Self::has_models_path_segment(base) {
            return Ok(base.to_string());
        }
        let base = if Self::needs_models_prefix(base, endpoint_path) {
            format!("{}/models", base)
        } else {
            base.to_string()
        };

        Ok(format!("{}/{}", base, endpoint_path))
    }

    fn has_models_path_segment(base: &str) -> bool {
        let host_and_path = base.split_once("://").map(|(_, rest)| rest).unwrap_or(base);
        let path = host_and_path
            .split_once('/')
            .map(|(_, path)| path)
            .unwrap_or("");

        path.split('/').any(|segment| segment == "models")
    }

    fn needs_models_prefix(base: &str, endpoint_path: &str) -> bool {
        if endpoint_path == "models" || endpoint_path.starts_with("models/") {
            return false;
        }

        let host_and_path = base.split_once("://").map(|(_, rest)| rest).unwrap_or(base);
        let (host, _) = host_and_path.split_once('/').unwrap_or((host_and_path, ""));

        host.ends_with(".services.ai.azure.com") && !Self::has_models_path_segment(base)
    }

    /// Default
    pub fn create_default_headers(&self) -> Result<HashMap<String, String>, String> {
        let mut headers = HashMap::new();

        // Settings
        if let Some(api_key) = &self.base.api_key {
            headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        } else {
            return Err("Azure AI API key not set".to_string());
        }

        // Settings
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        // Settings
        headers.insert("User-Agent".to_string(), "litellm-rust/0.1.0".to_string());

        let api_version = self
            .base
            .api_version
            .as_deref()
            .unwrap_or("2024-05-01-preview");
        headers.insert("api-version".to_string(), api_version.to_string());

        for (key, value) in &self.base.headers {
            headers.insert(key.clone(), value.clone());
        }

        Ok(headers)
    }

    /// Configuration
    pub fn timeout(&self) -> std::time::Duration {
        self.base.timeout_duration()
    }

    /// Configuration
    pub fn validate(&self) -> Result<(), String> {
        self.validate_policy_client_settings()
    }

    pub(crate) fn validate_policy_client_settings(&self) -> Result<(), String> {
        self.base.validate("azure_ai")?;
        let endpoint = self
            .base
            .api_base
            .as_deref()
            .ok_or_else(|| "Azure AI API base URL not set".to_string())?;
        let endpoint_url =
            Url::parse(endpoint).map_err(|error| format!("Invalid Azure AI endpoint: {error}"))?;
        if !matches!(endpoint_url.scheme(), "http" | "https") {
            return Err("Azure AI endpoint must use http or https".to_string());
        }
        ProviderEndpointPolicy::for_base_url(self.base.endpoint_access, endpoint)
            .map(|_| ())
            .map_err(|error| format!("Invalid Azure AI endpoint policy: {error}"))
    }
}

// Implementation of ProviderConfig trait
impl ProviderConfig for AzureAIConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_policy_client_settings()
    }

    fn api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.base.api_base.as_deref()
    }

    fn timeout(&self) -> std::time::Duration {
        self.base.timeout_duration()
    }

    fn max_retries(&self) -> u32 {
        self.base.max_retries
    }

    fn endpoint_access(&self) -> ProviderEndpointAccess {
        self.base.endpoint_access
    }
}

/// Azure AI endpoint type enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AzureAIEndpointType {
    /// Chat completions endpoint
    ChatCompletions,
    /// Embeddings endpoint
    Embeddings,
    /// Image embeddings endpoint (multimodal)
    ImageEmbeddings,
    /// Image generation endpoint
    ImageGeneration,
    /// Rerank endpoint
    Rerank,
}

impl AzureAIEndpointType {
    /// Get
    pub fn as_path(&self) -> &'static str {
        match self {
            AzureAIEndpointType::ChatCompletions => "chat/completions",
            AzureAIEndpointType::Embeddings => "embeddings",
            AzureAIEndpointType::ImageEmbeddings => "images/embeddings",
            AzureAIEndpointType::ImageGeneration => "images/generations",
            AzureAIEndpointType::Rerank => "rerank",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_ai_config() {
        let config = AzureAIConfig::new("azure_ai");
        assert_eq!(config.base.max_retries, 3);
        assert_eq!(config.base.timeout, 60);
    }

    #[test]
    fn test_endpoint_types() {
        assert_eq!(
            AzureAIEndpointType::ChatCompletions.as_path(),
            "chat/completions"
        );
        assert_eq!(AzureAIEndpointType::Embeddings.as_path(), "embeddings");
        assert_eq!(
            AzureAIEndpointType::ImageGeneration.as_path(),
            "images/generations"
        );
        assert_eq!(AzureAIEndpointType::Rerank.as_path(), "rerank");
    }

    #[test]
    fn test_build_endpoint_url() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_base = Some("https://test.ai.azure.com".to_string());

        let url = config.build_endpoint_url("chat/completions").unwrap();
        assert_eq!(url, "https://test.ai.azure.com/chat/completions");

        // Test with trailing slash
        config.base.api_base = Some("https://test.ai.azure.com/".to_string());
        let url = config.build_endpoint_url("/chat/completions").unwrap();
        assert_eq!(url, "https://test.ai.azure.com/chat/completions");
    }

    #[test]
    fn test_build_endpoint_url_adds_models_for_resource_root() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_base = Some("https://resource.services.ai.azure.com".to_string());

        let url = config.build_endpoint_url("chat/completions").unwrap();
        assert_eq!(
            url,
            "https://resource.services.ai.azure.com/models/chat/completions"
        );
    }

    #[test]
    fn test_build_endpoint_url_does_not_duplicate_models() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_base = Some("https://resource.services.ai.azure.com/models".to_string());

        let url = config.build_endpoint_url("chat/completions").unwrap();
        assert_eq!(
            url,
            "https://resource.services.ai.azure.com/models/chat/completions"
        );
    }

    #[test]
    fn test_build_endpoint_url_does_not_duplicate_models_for_health_path() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_base = Some("https://resource.services.ai.azure.com/models".to_string());

        let url = match config.build_endpoint_url("models") {
            Ok(url) => url,
            Err(error) => panic!("models endpoint should build: {error}"),
        };
        assert_eq!(url, "https://resource.services.ai.azure.com/models");
    }

    #[test]
    fn test_build_endpoint_url_no_base() {
        // Use Default to get a config without default api_base
        let config = AzureAIConfig::default();
        let result = config.build_endpoint_url("chat/completions");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API base URL not set"));
    }

    #[test]
    fn test_create_default_headers_with_api_key() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test-api-key".to_string());
        config.base.api_version = Some("2024-08-01-preview".to_string());
        config
            .base
            .headers
            .insert("x-routing-tenant".to_string(), "tenant-a".to_string());

        let headers = config.create_default_headers().unwrap();
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer test-api-key")
        );
        assert_eq!(
            headers.get("Content-Type").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            headers.get("User-Agent").map(String::as_str),
            Some("litellm-rust/0.1.0")
        );
        assert_eq!(
            headers.get("api-version").map(String::as_str),
            Some("2024-08-01-preview")
        );
        assert_eq!(
            headers.get("x-routing-tenant").map(String::as_str),
            Some("tenant-a")
        );
    }

    #[test]
    fn test_create_default_headers_uses_default_api_version() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test-api-key".to_string());

        let headers = config.create_default_headers().unwrap();
        assert_eq!(
            headers.get("api-version").map(String::as_str),
            Some("2024-05-01-preview")
        );
    }

    #[test]
    fn test_create_default_headers_no_api_key() {
        let config = AzureAIConfig::new("azure_ai");
        let result = config.create_default_headers();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key not set"));
    }

    #[test]
    fn test_timeout() {
        let config = AzureAIConfig::new("azure_ai");
        let timeout = config.timeout();
        assert_eq!(timeout, std::time::Duration::from_secs(60));
    }

    #[test]
    fn test_validate_success() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test-key".to_string());
        config.base.api_base = Some("https://test.com".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_endpoint_policy_and_scheme() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test-key".to_string());

        for endpoint in ["http://example.com", "https://example.com"] {
            config.base.api_base = Some(endpoint.to_string());
            assert!(config.validate().is_ok(), "{endpoint} should be accepted");
        }

        for endpoint in ["ws://example.com", "wss://example.com"] {
            config.base.api_base = Some(endpoint.to_string());
            assert!(config.validate().is_err(), "{endpoint} should be rejected");
        }

        config.base.api_base = Some("http://127.0.0.1:18080".to_string());
        config.base.endpoint_access = ProviderEndpointAccess::PublicOnly;
        assert!(config.validate().is_err());
        config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        assert!(config.validate().is_ok());

        config.base.api_base = Some("http://169.254.169.254".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_missing_api_key() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_base = Some("https://test.com".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_image_embeddings_endpoint() {
        assert_eq!(
            AzureAIEndpointType::ImageEmbeddings.as_path(),
            "images/embeddings"
        );
    }

    #[test]
    fn test_endpoint_type_equality() {
        assert_eq!(
            AzureAIEndpointType::ChatCompletions,
            AzureAIEndpointType::ChatCompletions
        );
        assert_ne!(
            AzureAIEndpointType::ChatCompletions,
            AzureAIEndpointType::Embeddings
        );
    }

    #[test]
    fn test_provider_config_trait() {
        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test-key".to_string());
        config.base.api_base = Some("https://test.com".to_string());

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://test.com"));
        assert_eq!(
            ProviderConfig::timeout(&config),
            std::time::Duration::from_secs(60)
        );
        assert_eq!(config.max_retries(), 3);
        assert_eq!(config.endpoint_access(), ProviderEndpointAccess::PublicOnly);
    }
}
