//! Custom HTTPX Configuration

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::core::net::{ProviderEndpointAccess, validate_provider_endpoint_url_str};
use crate::core::providers::base::BaseConfig;
use crate::core::traits::provider::ProviderConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomHttpxConfig {
    #[serde(flatten)]
    pub base: BaseConfig,

    /// Custom endpoint URL
    pub endpoint_url: String,

    /// HTTP method (GET, POST, etc.)
    pub http_method: String,

    /// Request body template
    pub request_template: Option<String>,

    /// Response parser configuration
    pub response_parser: Option<String>,
}

impl Default for CustomHttpxConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig::default(),
            endpoint_url: String::new(),
            http_method: "POST".to_string(),
            request_template: None,
            response_parser: None,
        }
    }
}

impl CustomHttpxConfig {
    pub fn new(endpoint_url: impl Into<String>) -> Self {
        Self {
            endpoint_url: endpoint_url.into(),
            ..Self::default()
        }
    }

    pub fn from_env() -> Result<Self, String> {
        let endpoint_url = std::env::var("CUSTOM_HTTPX_ENDPOINT")
            .map_err(|_| "CUSTOM_HTTPX_ENDPOINT environment variable is required")?;

        let mut config = Self::new(endpoint_url);

        if let Ok(api_key) = std::env::var("CUSTOM_HTTPX_API_KEY") {
            config.base.api_key = Some(api_key);
        }

        if let Ok(method) = std::env::var("CUSTOM_HTTPX_METHOD") {
            config.http_method = method;
        }

        Ok(config)
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.base.api_key = Some(api_key.into());
        self
    }

    pub fn with_http_method(mut self, method: impl Into<String>) -> Self {
        self.http_method = method.into();
        self
    }

    pub fn with_request_template(mut self, template: impl Into<String>) -> Self {
        self.request_template = Some(template.into());
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.base.headers.insert(key.into(), value.into());
        self
    }
}

impl ProviderConfig for CustomHttpxConfig {
    fn validate(&self) -> Result<(), String> {
        if self.endpoint_url.is_empty() {
            return Err("Endpoint URL is required".to_string());
        }
        self.http_method
            .trim()
            .to_ascii_uppercase()
            .parse::<reqwest::Method>()
            .map_err(|error| format!("invalid HTTP method: {error}"))?;

        validate_provider_endpoint_url_str(&self.endpoint_url, self.base.endpoint_access)
            .map(|_| ())
            .map_err(|error| format!("invalid endpoint URL: {error}"))
    }

    fn api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        Some(&self.endpoint_url)
    }

    fn endpoint_access(&self) -> ProviderEndpointAccess {
        self.base.endpoint_access
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.base.timeout)
    }

    fn max_retries(&self) -> u32 {
        self.base.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_valid_public_url() {
        // Use a literal public IP — fictional subdomains may not resolve in all test environments
        let cfg = CustomHttpxConfig::new("https://8.8.8.8/v1/chat");
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_reject_localhost() {
        let cfg = CustomHttpxConfig::new("http://localhost:8080/endpoint");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_loopback_ip() {
        let cfg = CustomHttpxConfig::new("http://127.0.0.1:8080/endpoint");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_private_endpoint_access_is_validated_and_propagated() {
        let mut cfg = CustomHttpxConfig::new("http://127.0.0.1:8080/endpoint");
        cfg.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;

        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.api_base(), Some("http://127.0.0.1:8080/endpoint"));
        assert_eq!(
            cfg.endpoint_access(),
            ProviderEndpointAccess::PrivateNetwork
        );

        cfg.endpoint_url = "http://169.254.169.254/latest/meta-data".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_http_method_is_parsed_fallibly() {
        let mut cfg = CustomHttpxConfig::new("https://8.8.8.8/endpoint");
        cfg.http_method = "delete".to_string();
        assert!(cfg.validate().is_ok());

        cfg.http_method = "not a method".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_loopback_ip_range() {
        let cfg = CustomHttpxConfig::new("http://127.100.200.1/endpoint");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_private_10_block() {
        let cfg = CustomHttpxConfig::new("http://10.0.0.1/internal");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_private_172_block() {
        let cfg = CustomHttpxConfig::new("http://172.16.0.1/internal");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_private_192_168_block() {
        let cfg = CustomHttpxConfig::new("http://192.168.1.1/internal");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_link_local_metadata() {
        // AWS/GCP cloud metadata endpoint
        let cfg = CustomHttpxConfig::new("http://169.254.169.254/latest/meta-data/");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_ipv6_loopback() {
        let cfg = CustomHttpxConfig::new("http://[::1]/endpoint");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_empty_url() {
        let cfg = CustomHttpxConfig::new("");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_reject_non_http_scheme() {
        let cfg = CustomHttpxConfig::new("ftp://example.com/endpoint");
        assert!(cfg.validate().is_err());
    }
}
