//! Replicate Provider Configuration
//!
//! Configuration for the Replicate API provider

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::core::providers::base::BaseConfig;
use crate::core::traits::provider::ProviderConfig;

/// Default polling delay in seconds when waiting for prediction results
pub const DEFAULT_POLLING_DELAY_SECONDS: u64 = 1;

/// Default maximum number of polling retries
pub const DEFAULT_POLLING_RETRIES: u32 = 60;

/// Length of version ID in model strings (e.g., the 64-character hash)
pub const MODEL_VERSION_ID_LENGTH: usize = 64;

/// Replicate provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicateConfig {
    /// Base configuration shared across all providers
    #[serde(flatten)]
    pub base: BaseConfig,

    /// Polling delay in seconds when waiting for prediction results
    #[serde(default = "default_polling_delay")]
    pub polling_delay_seconds: u64,

    /// Maximum number of polling retries
    #[serde(default = "default_polling_retries")]
    pub polling_retries: u32,

    /// Whether to use the streaming API for predictions (when available)
    #[serde(default = "default_use_streaming")]
    pub use_streaming: bool,
}

fn default_polling_delay() -> u64 {
    DEFAULT_POLLING_DELAY_SECONDS
}

fn default_polling_retries() -> u32 {
    DEFAULT_POLLING_RETRIES
}

fn default_use_streaming() -> bool {
    false
}

impl Default for ReplicateConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_key: None,
                api_base: Some("https://api.replicate.com/v1".to_string()),
                timeout: 600, // Replicate can take a long time for predictions
                max_retries: 3,
                headers: std::collections::HashMap::new(),
                organization: None,
                api_version: None,
            },
            polling_delay_seconds: DEFAULT_POLLING_DELAY_SECONDS,
            polling_retries: DEFAULT_POLLING_RETRIES,
            use_streaming: false,
        }
    }
}

impl ReplicateConfig {
    /// Create new configuration with API key
    pub fn new(api_key: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.base.api_key = Some(api_key.into());
        config
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(api_key) = std::env::var("REPLICATE_API_TOKEN") {
            config.base.api_key = Some(api_key);
        }

        if let Ok(api_base) = std::env::var("REPLICATE_API_BASE") {
            config.base.api_base = Some(api_base);
        }

        if let Ok(polling_delay) = std::env::var("REPLICATE_POLLING_DELAY")
            && let Ok(delay) = polling_delay.parse()
        {
            config.polling_delay_seconds = delay;
        }

        if let Ok(polling_retries) = std::env::var("REPLICATE_POLLING_RETRIES")
            && let Ok(retries) = polling_retries.parse()
        {
            config.polling_retries = retries;
        }

        config
    }

    /// Get the effective API base URL
    pub fn get_api_base(&self) -> String {
        self.base
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.replicate.com/v1".to_string())
    }

    /// Get the prediction URL for a model
    ///
    /// Handles both regular models and deployments
    pub fn get_prediction_url(&self, model: &str) -> String {
        let base = self.get_api_base();
        let version_id = Self::extract_version_id(model);

        if version_id.contains("deployments/") {
            let deployment = version_id.replace("deployments/", "");
            format!("{}/deployments/{}/predictions", base, deployment)
        } else {
            format!("{}/models/{}/predictions", base, version_id)
        }
    }

    /// Extract version ID from model string
    ///
    /// Model strings can be:
    /// - "owner/model" - uses the latest version
    /// - "owner/model:version" - uses a specific version
    /// - "deployments/owner/deployment" - uses a deployment
    pub fn extract_version_id(model: &str) -> String {
        if model.contains(':') {
            let parts: Vec<&str> = model.split(':').collect();
            if parts.len() > 1 {
                return parts[0].to_string();
            }
        }
        model.to_string()
    }

    /// Extract the version hash from a model string if present
    pub fn extract_version_hash(model: &str) -> Option<String> {
        if model.contains(':') {
            let parts: Vec<&str> = model.split(':').collect();
            if parts.len() > 1 && parts[1].len() == MODEL_VERSION_ID_LENGTH {
                return Some(parts[1].to_string());
            }
        }
        None
    }

    /// With custom polling delay
    pub fn with_polling_delay(mut self, delay_seconds: u64) -> Self {
        self.polling_delay_seconds = delay_seconds;
        self
    }

    /// With custom polling retries
    pub fn with_polling_retries(mut self, retries: u32) -> Self {
        self.polling_retries = retries;
        self
    }

    /// Enable or disable streaming
    pub fn with_streaming(mut self, use_streaming: bool) -> Self {
        self.use_streaming = use_streaming;
        self
    }
}

impl ProviderConfig for ReplicateConfig {
    fn validate(&self) -> Result<(), String> {
        // API key is required
        if self.base.api_key.is_none() {
            return Err("Replicate API token is required".to_string());
        }

        // Validate polling settings
        if self.polling_delay_seconds == 0 {
            return Err("Polling delay must be greater than 0".to_string());
        }

        if self.polling_retries == 0 {
            return Err("Polling retries must be greater than 0".to_string());
        }

        Ok(())
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
    fn test_replicate_config_default() {
        let config = ReplicateConfig::default();
        assert_eq!(config.get_api_base(), "https://api.replicate.com/v1");
        assert_eq!(config.polling_delay_seconds, DEFAULT_POLLING_DELAY_SECONDS);
        assert_eq!(config.polling_retries, DEFAULT_POLLING_RETRIES);
        assert!(!config.use_streaming);
    }

    #[test]
    fn test_replicate_config_new() {
        let config = ReplicateConfig::new("test-token");
        assert_eq!(config.base.api_key, Some("test-token".to_string()));
    }

    #[test]
    fn test_replicate_config_validate_missing_api_key() {
        let config = ReplicateConfig::default();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API token"));
    }

    #[test]
    fn test_replicate_config_validate_success() {
        let config = ReplicateConfig::new("test-token");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_extract_version_id_simple() {
        assert_eq!(
            ReplicateConfig::extract_version_id("meta/llama-2-70b-chat"),
            "meta/llama-2-70b-chat"
        );
    }

    #[test]
    fn test_extract_version_id_with_version() {
        assert_eq!(
            ReplicateConfig::extract_version_id("meta/llama-2-70b-chat:abc123"),
            "meta/llama-2-70b-chat"
        );
    }

    #[test]
    fn test_extract_version_hash() {
        let hash = "a".repeat(64);
        let model = format!("meta/llama-2-70b-chat:{}", hash);
        assert_eq!(ReplicateConfig::extract_version_hash(&model), Some(hash));
    }

    #[test]
    fn test_extract_version_hash_no_version() {
        assert_eq!(
            ReplicateConfig::extract_version_hash("meta/llama-2-70b-chat"),
            None
        );
    }

    #[test]
    fn test_get_prediction_url_model() {
        let config = ReplicateConfig::new("test-token");
        let url = config.get_prediction_url("meta/llama-2-70b-chat");
        assert_eq!(
            url,
            "https://api.replicate.com/v1/models/meta/llama-2-70b-chat/predictions"
        );
    }

    #[test]
    fn test_get_prediction_url_deployment() {
        let config = ReplicateConfig::new("test-token");
        let url = config.get_prediction_url("deployments/owner/my-deployment");
        assert_eq!(
            url,
            "https://api.replicate.com/v1/deployments/owner/my-deployment/predictions"
        );
    }

    #[test]
    fn test_provider_config_trait() {
        let config = ReplicateConfig::new("test-token");
        assert_eq!(config.api_key(), Some("test-token"));
        assert_eq!(config.api_base(), Some("https://api.replicate.com/v1"));
        assert_eq!(config.timeout(), Duration::from_secs(600));
        assert_eq!(config.max_retries(), 3);
    }

    #[test]
    fn test_config_builder_methods() {
        let config = ReplicateConfig::new("token")
            .with_polling_delay(5)
            .with_polling_retries(30)
            .with_streaming(true);

        assert_eq!(config.polling_delay_seconds, 5);
        assert_eq!(config.polling_retries, 30);
        assert!(config.use_streaming);
    }

    #[test]
    fn test_validate_zero_polling_delay() {
        let mut config = ReplicateConfig::new("token");
        config.polling_delay_seconds = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Polling delay"));
    }

    #[test]
    fn test_validate_zero_polling_retries() {
        let mut config = ReplicateConfig::new("token");
        config.polling_retries = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Polling retries"));
    }
}
