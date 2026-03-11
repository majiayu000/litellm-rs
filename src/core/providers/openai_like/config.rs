//! OpenAI-Like Provider Configuration
//!
//! Configuration for connecting to any OpenAI-compatible API endpoint

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::core::providers::base::BaseConfig;
use crate::core::traits::provider::ProviderConfig;

/// OpenAI-like provider configuration
///
/// Supports any OpenAI-compatible API endpoint with custom configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAILikeConfig {
    /// Base configuration shared across all providers
    #[serde(flatten)]
    pub base: BaseConfig,

    /// Provider name for identification (optional, defaults to "openai_like")
    #[serde(default = "default_provider_name")]
    pub provider_name: String,

    /// Custom headers to include in requests
    #[serde(default)]
    pub custom_headers: HashMap<String, String>,

    /// Whether to skip API key requirement (for local endpoints)
    #[serde(default)]
    pub skip_api_key: bool,

    /// Model name prefix to strip from model IDs (optional)
    /// e.g., if set to "custom/", model "custom/gpt-4" becomes "gpt-4"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_prefix: Option<String>,

    /// Default model to use if not specified in request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,

    /// Whether to pass through all OpenAI parameters without filtering
    #[serde(default = "default_pass_through")]
    pub pass_through_params: bool,
}

fn default_provider_name() -> String {
    "openai_like".to_string()
}

fn default_pass_through() -> bool {
    true
}

impl Default for OpenAILikeConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_key: None,
                api_base: None, // Required - must be set by user
                timeout: 60,
                max_retries: 3,
                headers: HashMap::new(),
                organization: None,
                api_version: None,
            },
            provider_name: default_provider_name(),
            custom_headers: HashMap::new(),
            skip_api_key: false,
            model_prefix: None,
            default_model: None,
            pass_through_params: true,
        }
    }
}

impl OpenAILikeConfig {
    /// Create a new configuration with required api_base
    pub fn new(api_base: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.base.api_base = Some(api_base.into());
        config
    }

    /// Create a configuration with api_base and api_key
    pub fn with_api_key(api_base: impl Into<String>, api_key: impl Into<String>) -> Self {
        let mut config = Self::new(api_base);
        config.base.api_key = Some(api_key.into());
        config
    }

    /// Create configuration from environment variables
    ///
    /// Looks for:
    /// - OPENAI_LIKE_API_BASE or OPENAI_API_BASE
    /// - OPENAI_LIKE_API_KEY or OPENAI_API_KEY
    /// - OPENAI_LIKE_TIMEOUT
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // API Base (required)
        if let Ok(api_base) = std::env::var("OPENAI_LIKE_API_BASE") {
            config.base.api_base = Some(api_base);
        } else if let Ok(api_base) = std::env::var("OPENAI_API_BASE") {
            config.base.api_base = Some(api_base);
        }

        // API Key (optional)
        if let Ok(api_key) = std::env::var("OPENAI_LIKE_API_KEY") {
            config.base.api_key = Some(api_key);
        } else if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            config.base.api_key = Some(api_key);
        }

        // Timeout
        if let Ok(timeout_str) = std::env::var("OPENAI_LIKE_TIMEOUT")
            && let Ok(timeout) = timeout_str.parse::<u64>()
        {
            config.base.timeout = timeout;
        }

        // Skip API key check
        if let Ok(skip) = std::env::var("OPENAI_LIKE_SKIP_API_KEY") {
            config.skip_api_key = skip.to_lowercase() == "true" || skip == "1";
        }

        // Provider name
        if let Ok(name) = std::env::var("OPENAI_LIKE_PROVIDER_NAME") {
            config.provider_name = name;
        }

        config
    }

    /// Set the provider name
    pub fn with_provider_name(mut self, name: impl Into<String>) -> Self {
        self.provider_name = name.into();
        self
    }

    /// Add a custom header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }

    /// Add multiple custom headers
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.custom_headers.extend(headers);
        self
    }

    /// Set skip API key flag
    pub fn with_skip_api_key(mut self, skip: bool) -> Self {
        self.skip_api_key = skip;
        self
    }

    /// Set model prefix to strip
    pub fn with_model_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.model_prefix = Some(prefix.into());
        self
    }

    /// Set default model
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.base.timeout = timeout_secs;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // API base is required for openai_like
        if self.base.api_base.is_none() {
            return Err("api_base is required for openai_like provider".to_string());
        }

        // API key is required unless skip_api_key is set
        if !self.skip_api_key && self.base.api_key.is_none() {
            return Err(
                "api_key is required for openai_like provider (set skip_api_key=true to skip)"
                    .to_string(),
            );
        }

        // Validate timeout
        if self.base.timeout == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }

        // Validate max retries
        if self.base.max_retries > 10 {
            return Err("Max retries should not exceed 10".to_string());
        }

        Ok(())
    }

    /// Get the effective API base URL
    pub fn get_api_base(&self) -> String {
        self.base
            .api_base
            .clone()
            .unwrap_or_else(|| "http://localhost:8000/v1".to_string())
    }

    /// Get the effective model name (strip prefix if configured)
    pub fn get_effective_model(&self, model: &str) -> String {
        if let Some(prefix) = &self.model_prefix
            && model.starts_with(prefix)
        {
            return model[prefix.len()..].to_string();
        }
        model.to_string()
    }
}

impl ProviderConfig for OpenAILikeConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }

    fn api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.base.api_base.as_deref()
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

    #[test]
    fn test_default_config() {
        let config = OpenAILikeConfig::default();
        assert!(config.base.api_base.is_none());
        assert!(config.base.api_key.is_none());
        assert!(!config.skip_api_key);
        assert!(config.pass_through_params);
    }

    #[test]
    fn test_new_with_api_base() {
        let config = OpenAILikeConfig::new("http://localhost:8000/v1");
        assert_eq!(
            config.base.api_base,
            Some("http://localhost:8000/v1".to_string())
        );
    }

    #[test]
    fn test_with_api_key() {
        let config = OpenAILikeConfig::with_api_key("http://localhost:8000/v1", "sk-test123");
        assert_eq!(
            config.base.api_base,
            Some("http://localhost:8000/v1".to_string())
        );
        assert_eq!(config.base.api_key, Some("sk-test123".to_string()));
    }

    #[test]
    fn test_validation_missing_api_base() {
        let config = OpenAILikeConfig::default();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_missing_api_key() {
        let config = OpenAILikeConfig::new("http://localhost:8000/v1");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_skip_api_key() {
        let config = OpenAILikeConfig::new("http://localhost:8000/v1").with_skip_api_key(true);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_with_api_key() {
        let config = OpenAILikeConfig::with_api_key("http://localhost:8000/v1", "sk-test123");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_get_effective_model_no_prefix() {
        let config = OpenAILikeConfig::new("http://localhost:8000/v1");
        assert_eq!(config.get_effective_model("gpt-4"), "gpt-4");
    }

    #[test]
    fn test_get_effective_model_with_prefix() {
        let config = OpenAILikeConfig::new("http://localhost:8000/v1").with_model_prefix("custom/");
        assert_eq!(config.get_effective_model("custom/gpt-4"), "gpt-4");
        assert_eq!(config.get_effective_model("gpt-4"), "gpt-4");
    }

    #[test]
    fn test_custom_headers() {
        let config = OpenAILikeConfig::new("http://localhost:8000/v1")
            .with_header("X-Custom-Header", "value1")
            .with_header("X-Another-Header", "value2");

        assert_eq!(config.custom_headers.len(), 2);
        assert_eq!(
            config.custom_headers.get("X-Custom-Header"),
            Some(&"value1".to_string())
        );
    }

    #[test]
    fn test_builder_chain() {
        let config = OpenAILikeConfig::new("http://localhost:8000/v1")
            .with_provider_name("my-provider")
            .with_timeout(120)
            .with_default_model("llama-2-70b")
            .with_skip_api_key(true);

        assert_eq!(config.provider_name, "my-provider");
        assert_eq!(config.base.timeout, 120);
        assert_eq!(config.default_model, Some("llama-2-70b".to_string()));
        assert!(config.skip_api_key);
    }
}
