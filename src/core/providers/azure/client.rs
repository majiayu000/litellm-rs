//! Azure OpenAI Client
//!
//! HTTP client wrapper for Azure OpenAI Service

use reqwest::{Method, header::HeaderMap};
use serde::{Deserialize, Serialize};

use super::config::AzureConfig;
use super::error::azure_config_error;
use super::utils::{AzureEndpointType, AzureUtils};
use crate::core::providers::base::{BaseConfig, BaseHttpClient, ProviderRequestBuilder};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::ProviderConfig as _;

/// Azure OpenAI client
#[derive(Debug, Clone)]
pub struct AzureClient {
    config: AzureConfig,
    http_client: BaseHttpClient,
    streaming_client: BaseHttpClient,
}

impl AzureClient {
    /// Create new Azure client
    pub fn new(mut config: AzureConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|error| azure_config_error(error.to_string()))?;
        let endpoint = config
            .get_effective_azure_endpoint()
            .ok_or_else(|| azure_config_error("Azure endpoint not configured"))?;
        config.azure_endpoint = Some(endpoint.clone());
        let base_config = BaseConfig {
            api_base: Some(endpoint),
            endpoint_access: config.endpoint_access,
            timeout: config.timeout,
            ..Default::default()
        };
        let http_client = BaseHttpClient::new_for_provider("azure", base_config.clone())?;
        let streaming_client = BaseHttpClient::new_for_provider_streaming("azure", base_config)?;

        Ok(Self {
            config,
            http_client,
            streaming_client,
        })
    }

    /// Get configuration
    pub fn get_config(&self) -> &AzureConfig {
        &self.config
    }

    /// Build request URL
    pub fn build_url(
        &self,
        deployment_name: &str,
        endpoint_type: AzureEndpointType,
    ) -> Result<String, ProviderError> {
        let endpoint = self
            .config
            .get_effective_azure_endpoint()
            .ok_or_else(|| azure_config_error("Azure endpoint not configured"))?;

        Ok(AzureUtils::build_azure_url(
            &endpoint,
            deployment_name,
            &self.config.api_version,
            endpoint_type,
        ))
    }

    pub(crate) fn request(
        &self,
        method: Method,
        url: &str,
    ) -> Result<ProviderRequestBuilder, ProviderError> {
        self.http_client.request(method, url)
    }

    pub(crate) fn streaming_request(
        &self,
        method: Method,
        url: &str,
    ) -> Result<ProviderRequestBuilder, ProviderError> {
        self.streaming_client.request(method, url)
    }

    pub(crate) fn validate_api_base_override(
        &self,
        api_base: Option<&str>,
    ) -> Result<(), ProviderError> {
        let Some(api_base) = api_base.filter(|value| !value.trim().is_empty()) else {
            return Ok(());
        };
        let configured = self.config.azure_endpoint.as_deref().ok_or_else(|| {
            ProviderError::configuration("azure", "Azure endpoint not configured")
        })?;
        if api_base.trim_end_matches('/') != configured.trim_end_matches('/') {
            return Err(ProviderError::configuration(
                "azure",
                "per-request api_base must match the policy-bound Azure endpoint",
            ));
        }
        Ok(())
    }
}

/// Default Azure configuration factory
pub struct AzureConfigFactory;

impl AzureConfigFactory {
    /// Create configuration from environment variables
    pub fn from_environment() -> AzureConfig {
        let mut config = AzureConfig::new();

        if let Ok(api_key) = std::env::var("AZURE_OPENAI_KEY") {
            config.api_key = Some(api_key);
        } else if let Ok(api_key) = std::env::var("AZURE_API_KEY") {
            config.api_key = Some(api_key);
        }

        if let Ok(endpoint) = std::env::var("AZURE_OPENAI_ENDPOINT") {
            config.azure_endpoint = Some(endpoint);
        } else if let Ok(endpoint) = std::env::var("AZURE_ENDPOINT") {
            config.azure_endpoint = Some(endpoint);
        }

        if let Ok(version) = std::env::var("AZURE_API_VERSION") {
            config.api_version = version;
        }

        if let Ok(deployment) = std::env::var("AZURE_DEPLOYMENT_NAME") {
            config.deployment_name = Some(deployment);
        }

        config
    }

    /// Create configuration for specific Azure service
    pub fn for_service(service: &str, _region: &str) -> AzureConfig {
        AzureConfig::new()
            .with_azure_endpoint(format!("https://{}.openai.azure.com", service))
            .with_api_version("2024-02-01".to_string())
    }
}

/// Azure rate limiting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureRateLimitInfo {
    pub requests_limit: Option<u32>,
    pub requests_remaining: Option<u32>,
    pub requests_reset: Option<u64>,
    pub tokens_limit: Option<u32>,
    pub tokens_remaining: Option<u32>,
    pub tokens_reset: Option<u64>,
}

impl AzureRateLimitInfo {
    /// Extract rate limit info from headers
    pub fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            requests_limit: headers
                .get("x-ratelimit-limit-requests")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            requests_remaining: headers
                .get("x-ratelimit-remaining-requests")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            requests_reset: headers
                .get("x-ratelimit-reset-requests")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            tokens_limit: headers
                .get("x-ratelimit-limit-tokens")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            tokens_remaining: headers
                .get("x-ratelimit-remaining-tokens")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            tokens_reset: headers
                .get("x-ratelimit-reset-tokens")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    // ==================== AzureClient Tests ====================

    #[test]
    fn test_azure_client_new_valid_config() {
        let config = AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint("https://test.openai.azure.com".to_string());

        let client = AzureClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_azure_client_new_missing_endpoint() {
        // Config without endpoint should fail validation
        let config = AzureConfig::new().with_api_key("test-key".to_string());

        let client = AzureClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_azure_client_new_without_key_but_with_endpoint() {
        // Config without key but with endpoint should succeed (key validated at request time)
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());

        let client = AzureClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_azure_client_get_config() {
        let config = AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint("https://test.openai.azure.com".to_string());

        let client = AzureClient::new(config).unwrap();
        assert_eq!(client.get_config().api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_azure_client_build_url() {
        let config = AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint("https://test.openai.azure.com".to_string())
            .with_api_version("2024-02-01".to_string());

        let client = AzureClient::new(config).unwrap();
        let url = client.build_url("gpt-4", AzureEndpointType::ChatCompletions);

        assert!(url.is_ok());
        let url = url.unwrap();
        assert!(url.contains("test.openai.azure.com"));
        assert!(url.contains("gpt-4"));
        assert!(url.contains("2024-02-01"));
    }

    #[test]
    fn test_azure_client_build_url_no_endpoint() {
        let config = AzureConfig::new().with_api_key("test-key".to_string());

        // This should fail validation during client creation
        let client = AzureClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_azure_client_honors_endpoint_access() {
        let config = AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint("http://127.0.0.1:18080".to_string());

        assert!(AzureClient::new(config.clone()).is_err());
        assert!(
            AzureClient::new(
                config
                    .with_endpoint_access(crate::core::net::ProviderEndpointAccess::PrivateNetwork)
            )
            .is_ok()
        );
    }

    #[test]
    fn test_azure_client_rejects_mismatched_api_base_override() {
        let config = AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint("https://test.openai.azure.com".to_string());

        let client = AzureClient::new(config).unwrap();
        assert!(
            client
                .validate_api_base_override(Some("https://other.openai.azure.com"))
                .is_err()
        );
    }

    // ==================== AzureConfigFactory Tests ====================

    #[test]
    fn test_config_factory_from_environment() {
        // Test that from_environment doesn't panic
        let config = AzureConfigFactory::from_environment();
        // Default values should be set
        assert!(!config.api_version.is_empty());
    }

    #[test]
    fn test_config_factory_for_service() {
        let config = AzureConfigFactory::for_service("myservice", "eastus");

        assert_eq!(
            config.azure_endpoint,
            Some("https://myservice.openai.azure.com".to_string())
        );
        assert_eq!(config.api_version, "2024-02-01");
    }

    #[test]
    fn test_config_factory_for_service_different_services() {
        let config1 = AzureConfigFactory::for_service("prod-service", "westus");
        let config2 = AzureConfigFactory::for_service("dev-service", "eastus");

        assert_ne!(config1.azure_endpoint, config2.azure_endpoint);
    }

    // ==================== AzureRateLimitInfo Tests ====================

    #[test]
    fn test_rate_limit_info_from_headers_empty() {
        let headers = HeaderMap::new();
        let info = AzureRateLimitInfo::from_headers(&headers);

        assert!(info.requests_limit.is_none());
        assert!(info.requests_remaining.is_none());
        assert!(info.requests_reset.is_none());
        assert!(info.tokens_limit.is_none());
        assert!(info.tokens_remaining.is_none());
        assert!(info.tokens_reset.is_none());
    }

    #[test]
    fn test_rate_limit_info_from_headers_requests() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("100"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("95"),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            HeaderValue::from_static("1234567890"),
        );

        let info = AzureRateLimitInfo::from_headers(&headers);

        assert_eq!(info.requests_limit, Some(100));
        assert_eq!(info.requests_remaining, Some(95));
        assert_eq!(info.requests_reset, Some(1234567890));
    }

    #[test]
    fn test_rate_limit_info_from_headers_tokens() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-tokens",
            HeaderValue::from_static("10000"),
        );
        headers.insert(
            "x-ratelimit-remaining-tokens",
            HeaderValue::from_static("9500"),
        );
        headers.insert(
            "x-ratelimit-reset-tokens",
            HeaderValue::from_static("1234567890"),
        );

        let info = AzureRateLimitInfo::from_headers(&headers);

        assert_eq!(info.tokens_limit, Some(10000));
        assert_eq!(info.tokens_remaining, Some(9500));
        assert_eq!(info.tokens_reset, Some(1234567890));
    }

    #[test]
    fn test_rate_limit_info_from_headers_full() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("100"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("95"),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            HeaderValue::from_static("1000"),
        );
        headers.insert(
            "x-ratelimit-limit-tokens",
            HeaderValue::from_static("50000"),
        );
        headers.insert(
            "x-ratelimit-remaining-tokens",
            HeaderValue::from_static("45000"),
        );
        headers.insert("x-ratelimit-reset-tokens", HeaderValue::from_static("2000"));

        let info = AzureRateLimitInfo::from_headers(&headers);

        assert_eq!(info.requests_limit, Some(100));
        assert_eq!(info.requests_remaining, Some(95));
        assert_eq!(info.requests_reset, Some(1000));
        assert_eq!(info.tokens_limit, Some(50000));
        assert_eq!(info.tokens_remaining, Some(45000));
        assert_eq!(info.tokens_reset, Some(2000));
    }

    #[test]
    fn test_rate_limit_info_from_headers_invalid_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("not-a-number"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("abc"),
        );

        let info = AzureRateLimitInfo::from_headers(&headers);

        // Invalid values should result in None
        assert!(info.requests_limit.is_none());
        assert!(info.requests_remaining.is_none());
    }

    #[test]
    fn test_rate_limit_info_serialization() {
        let info = AzureRateLimitInfo {
            requests_limit: Some(100),
            requests_remaining: Some(95),
            requests_reset: Some(1000),
            tokens_limit: Some(50000),
            tokens_remaining: Some(45000),
            tokens_reset: Some(2000),
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["requests_limit"], 100);
        assert_eq!(json["tokens_limit"], 50000);
    }

    #[test]
    fn test_rate_limit_info_debug() {
        let info = AzureRateLimitInfo {
            requests_limit: Some(100),
            requests_remaining: None,
            requests_reset: None,
            tokens_limit: None,
            tokens_remaining: None,
            tokens_reset: None,
        };

        let debug = format!("{:?}", info);
        assert!(debug.contains("AzureRateLimitInfo"));
        assert!(debug.contains("100"));
    }

    #[test]
    fn test_rate_limit_info_clone() {
        let info = AzureRateLimitInfo {
            requests_limit: Some(100),
            requests_remaining: Some(95),
            requests_reset: None,
            tokens_limit: None,
            tokens_remaining: None,
            tokens_reset: None,
        };

        let cloned = info.clone();
        assert_eq!(info.requests_limit, cloned.requests_limit);
        assert_eq!(info.requests_remaining, cloned.requests_remaining);
    }
}
