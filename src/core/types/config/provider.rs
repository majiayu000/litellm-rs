//! Provider configuration types

use super::defaults::*;
use super::rate_limit::RateLimitConfig;
use super::retry::RetryConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

/// Provider configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigEntry {
    /// Provider name (unique identifier)
    pub name: String,
    /// Provider type
    pub provider_type: String,
    /// Enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Routing weight (0.0-1.0)
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// Provider-specific configuration
    pub config: serde_json::Value,
    /// Labels (for routing and filtering)
    #[serde(default)]
    pub tags: HashMap<String, String>,
    /// Retry configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryConfig>,
    /// Rate limit configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
}

/// OpenAI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIProviderConfig {
    /// API key
    pub api_key: String,
    /// API base URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    /// Organization ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    /// Request timeout (seconds)
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    /// Maximum retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Supported models
    #[serde(default)]
    pub models: Vec<String>,
    /// Custom headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl crate::core::traits::provider::ProviderConfig for OpenAIProviderConfig {
    fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("API key is required".to_string());
        }

        if let Some(base_url) = &self.api_base
            && Url::parse(base_url).is_err()
        {
            return Err("Invalid API base URL".to_string());
        }

        if self.timeout_seconds == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }

        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        Some(&self.api_key)
    }

    fn api_base(&self) -> Option<&str> {
        self.api_base.as_deref()
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;
    use crate::core::types::config::rate_limit::RateLimitStrategy;

    // ==================== ProviderConfigEntry Tests ====================

    #[test]
    fn test_provider_config_entry_structure() {
        let entry = ProviderConfigEntry {
            name: "openai-prod".to_string(),
            provider_type: "openai".to_string(),
            enabled: true,
            weight: 0.8,
            config: serde_json::json!({"api_key": "test"}),
            tags: HashMap::from([("env".to_string(), "prod".to_string())]),
            retry: None,
            rate_limit: None,
        };
        assert_eq!(entry.name, "openai-prod");
        assert_eq!(entry.provider_type, "openai");
        assert!(entry.enabled);
        assert!((entry.weight - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_config_entry_with_all_options() {
        let entry = ProviderConfigEntry {
            name: "test-provider".to_string(),
            provider_type: "anthropic".to_string(),
            enabled: true,
            weight: 1.0,
            config: serde_json::json!({}),
            tags: HashMap::new(),
            retry: Some(RetryConfig::default()),
            rate_limit: Some(RateLimitConfig {
                strategy: RateLimitStrategy::TokenBucket,
                requests_per_second: Some(10),
                burst_size: Some(20),
                ..Default::default()
            }),
        };
        assert!(entry.retry.is_some());
        assert!(entry.rate_limit.is_some());
    }

    #[test]
    fn test_provider_config_entry_serialization() {
        let entry = ProviderConfigEntry {
            name: "test".to_string(),
            provider_type: "openai".to_string(),
            enabled: true,
            weight: 1.0,
            config: serde_json::json!({"key": "value"}),
            tags: HashMap::new(),

            retry: None,
            rate_limit: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["name"], "test");
        assert_eq!(json["provider_type"], "openai");
        assert_eq!(json["enabled"], true);
    }

    #[test]
    fn test_provider_config_entry_deserialization() {
        let json = r#"{
            "name": "my-provider",
            "provider_type": "azure",
            "enabled": false,
            "weight": 0.5,
            "config": {"deployment": "gpt-4"}
        }"#;
        let entry: ProviderConfigEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.name, "my-provider");
        assert_eq!(entry.provider_type, "azure");
        assert!(!entry.enabled);
        assert!((entry.weight - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_config_entry_deserialization_defaults() {
        let json = r#"{
            "name": "minimal",
            "provider_type": "openai",
            "config": {}
        }"#;
        let entry: ProviderConfigEntry = serde_json::from_str(json).unwrap();
        assert!(entry.enabled);
        assert!((entry.weight - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_config_entry_clone() {
        let entry = ProviderConfigEntry {
            name: "clone-test".to_string(),
            provider_type: "openai".to_string(),
            enabled: true,
            weight: 0.9,
            config: serde_json::json!({}),
            tags: HashMap::new(),

            retry: None,
            rate_limit: None,
        };
        let cloned = entry.clone();
        assert_eq!(entry.name, cloned.name);
        assert_eq!(entry.weight, cloned.weight);
    }

    // ==================== OpenAIProviderConfig Tests ====================

    #[test]
    fn test_openai_provider_config_structure() {
        let config = OpenAIProviderConfig {
            api_key: "sk-test123".to_string(),
            api_base: Some("https://api.openai.com/v1".to_string()),
            organization: Some("org-123".to_string()),
            timeout_seconds: 60,
            max_retries: 5,
            models: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
            headers: HashMap::from([("X-Custom".to_string(), "value".to_string())]),
        };
        assert_eq!(config.api_key, "sk-test123");
        assert!(config.api_base.is_some());
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.models.len(), 2);
    }

    #[test]
    fn test_openai_provider_config_validate_success() {
        let config = OpenAIProviderConfig {
            api_key: "sk-valid-key".to_string(),
            api_base: Some("https://api.openai.com/v1".to_string()),
            organization: None,
            timeout_seconds: 30,
            max_retries: 3,
            models: vec![],
            headers: HashMap::new(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_openai_provider_config_validate_empty_api_key() {
        let config = OpenAIProviderConfig {
            api_key: "".to_string(),
            api_base: None,
            organization: None,
            timeout_seconds: 30,
            max_retries: 3,
            models: vec![],
            headers: HashMap::new(),
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }

    #[test]
    fn test_openai_provider_config_validate_invalid_url() {
        let config = OpenAIProviderConfig {
            api_key: "sk-valid".to_string(),
            api_base: Some("not-a-valid-url".to_string()),
            organization: None,
            timeout_seconds: 30,
            max_retries: 3,
            models: vec![],
            headers: HashMap::new(),
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("URL"));
    }

    #[test]
    fn test_openai_provider_config_validate_zero_timeout() {
        let config = OpenAIProviderConfig {
            api_key: "sk-valid".to_string(),
            api_base: None,
            organization: None,
            timeout_seconds: 0,
            max_retries: 3,
            models: vec![],
            headers: HashMap::new(),
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Timeout"));
    }

    #[test]
    fn test_openai_provider_config_api_key_trait() {
        let config = OpenAIProviderConfig {
            api_key: "sk-my-api-key".to_string(),
            api_base: None,
            organization: None,
            timeout_seconds: 30,
            max_retries: 3,
            models: vec![],
            headers: HashMap::new(),
        };
        assert_eq!(config.api_key(), Some("sk-my-api-key"));
    }

    #[test]
    fn test_openai_provider_config_api_base_trait() {
        let config = OpenAIProviderConfig {
            api_key: "key".to_string(),
            api_base: Some("https://custom.api.com".to_string()),
            organization: None,
            timeout_seconds: 30,
            max_retries: 3,
            models: vec![],
            headers: HashMap::new(),
        };
        assert_eq!(config.api_base(), Some("https://custom.api.com"));

        let config_no_base = OpenAIProviderConfig {
            api_key: "key".to_string(),
            api_base: None,
            organization: None,
            timeout_seconds: 30,
            max_retries: 3,
            models: vec![],
            headers: HashMap::new(),
        };
        assert_eq!(config_no_base.api_base(), None);
    }

    #[test]
    fn test_openai_provider_config_timeout_trait() {
        let config = OpenAIProviderConfig {
            api_key: "key".to_string(),
            api_base: None,
            organization: None,
            timeout_seconds: 45,
            max_retries: 3,
            models: vec![],
            headers: HashMap::new(),
        };
        assert_eq!(config.timeout(), Duration::from_secs(45));
    }

    #[test]
    fn test_openai_provider_config_max_retries_trait() {
        let config = OpenAIProviderConfig {
            api_key: "key".to_string(),
            api_base: None,
            organization: None,
            timeout_seconds: 30,
            max_retries: 7,
            models: vec![],
            headers: HashMap::new(),
        };
        assert_eq!(config.max_retries(), 7);
    }

    #[test]
    fn test_openai_provider_config_serialization() {
        let config = OpenAIProviderConfig {
            api_key: "sk-test".to_string(),
            api_base: Some("https://api.example.com".to_string()),
            organization: Some("org-abc".to_string()),
            timeout_seconds: 60,
            max_retries: 5,
            models: vec!["model-1".to_string()],
            headers: HashMap::new(),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["api_key"], "sk-test");
        assert_eq!(json["timeout_seconds"], 60);
        assert_eq!(json["max_retries"], 5);
    }

    #[test]
    fn test_openai_provider_config_deserialization() {
        let json = r#"{
            "api_key": "sk-from-json",
            "api_base": "https://api.openai.com/v1",
            "timeout_seconds": 120,
            "max_retries": 10
        }"#;
        let config: OpenAIProviderConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, "sk-from-json");
        assert_eq!(config.timeout_seconds, 120);
        assert_eq!(config.max_retries, 10);
    }

    #[test]
    fn test_openai_provider_config_deserialization_defaults() {
        let json = r#"{"api_key": "sk-minimal"}"#;
        let config: OpenAIProviderConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, "sk-minimal");
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_openai_provider_config_clone() {
        let config = OpenAIProviderConfig {
            api_key: "key".to_string(),
            api_base: Some("https://api.com".to_string()),
            organization: None,
            timeout_seconds: 30,
            max_retries: 3,
            models: vec!["gpt-4".to_string()],
            headers: HashMap::new(),
        };
        let cloned = config.clone();
        assert_eq!(config.api_key, cloned.api_key);
        assert_eq!(config.models, cloned.models);
    }
}
