//! Fal AI Configuration
//!
//! Configuration for Fal AI image generation provider

use crate::core::providers::base::config::BaseConfig;
use crate::core::traits::provider::ProviderConfig;
use serde::{Deserialize, Serialize};

/// Default Fal AI API base URL
pub const DEFAULT_FAL_AI_API_BASE: &str = "https://fal.run";

/// Fal AI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FalAIConfig {
    /// Base configuration
    #[serde(flatten)]
    pub base: BaseConfig,

    /// Default output format for images
    #[serde(default = "default_output_format")]
    pub output_format: String,

    /// Enable synchronous generation (vs queue-based)
    #[serde(default = "default_sync_mode")]
    pub sync_mode: bool,
}

fn default_output_format() -> String {
    "jpeg".to_string()
}

fn default_sync_mode() -> bool {
    true
}

impl Default for FalAIConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_base: Some(DEFAULT_FAL_AI_API_BASE.to_string()),
                ..Default::default()
            },
            output_format: default_output_format(),
            sync_mode: default_sync_mode(),
        }
    }
}

impl FalAIConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut base = BaseConfig::from_env("fal_ai");

        // Set default API base if not provided
        if base.api_base.is_none() {
            base.api_base = Some(DEFAULT_FAL_AI_API_BASE.to_string());
        }

        Self {
            base,
            output_format: std::env::var("FAL_AI_OUTPUT_FORMAT")
                .unwrap_or_else(|_| default_output_format()),
            sync_mode: std::env::var("FAL_AI_SYNC_MODE")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or_else(|_| default_sync_mode()),
        }
    }

    /// Create new configuration with API key
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.base.api_key = Some(api_key.into());
        config
    }

    /// Get effective API base URL
    pub fn get_api_base(&self) -> &str {
        self.base
            .api_base
            .as_deref()
            .unwrap_or(DEFAULT_FAL_AI_API_BASE)
    }

    /// Get effective API key
    pub fn get_api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }
}

impl ProviderConfig for FalAIConfig {
    fn validate(&self) -> Result<(), String> {
        if self.base.api_key.is_none() {
            return Err("Fal AI API key is required".to_string());
        }
        if self.base.timeout == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }
        if self.base.max_retries > 10 {
            return Err("Max retries must not exceed 10".to_string());
        }
        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FalAIConfig::default();
        assert_eq!(
            config.base.api_base,
            Some(DEFAULT_FAL_AI_API_BASE.to_string())
        );
        assert_eq!(config.output_format, "jpeg");
        assert!(config.sync_mode);
    }

    #[test]
    fn test_with_api_key() {
        let config = FalAIConfig::with_api_key("test-key-123");
        assert_eq!(config.base.api_key, Some("test-key-123".to_string()));
    }

    #[test]
    fn test_validate_missing_api_key() {
        let config = FalAIConfig::default();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }

    #[test]
    fn test_validate_success() {
        let config = FalAIConfig::with_api_key("test-key");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_rejects_excessive_retries() {
        let mut config = FalAIConfig::with_api_key("test-key");
        config.base.max_retries = 11;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Max retries"));
    }

    #[test]
    fn test_get_api_base() {
        let config = FalAIConfig::default();
        assert_eq!(config.get_api_base(), DEFAULT_FAL_AI_API_BASE);
    }

    #[test]
    fn test_provider_config_trait() {
        let config = FalAIConfig::with_api_key("my-key");
        assert_eq!(config.api_key(), Some("my-key"));
        assert_eq!(config.api_base(), Some(DEFAULT_FAL_AI_API_BASE));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
        assert_eq!(config.max_retries(), 3);
    }

    #[test]
    fn test_config_serialization() {
        let config = FalAIConfig::with_api_key("test-key");
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("test-key"));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{"api_key": "my-api-key", "output_format": "png"}"#;
        let config: FalAIConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.base.api_key, Some("my-api-key".to_string()));
        assert_eq!(config.output_format, "png");
    }
}
