//! Runway ML Provider Configuration
//!
//! Configuration for the Runway ML API provider for video and image generation.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::core::providers::base::BaseConfig;
use crate::core::traits::provider::ProviderConfig;

/// Default polling delay in seconds when waiting for generation results
pub const DEFAULT_POLLING_DELAY_SECONDS: u64 = 2;

/// Default maximum number of polling retries (10 minutes with 2s delay)
pub const DEFAULT_POLLING_RETRIES: u32 = 300;

/// Default API base URL for Runway ML
pub const DEFAULT_API_BASE: &str = "https://api.dev.runwayml.com";

/// Runway ML provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunwayMLConfig {
    /// Base configuration shared across all providers
    #[serde(flatten)]
    pub base: BaseConfig,

    /// Polling delay in seconds when waiting for generation results
    #[serde(default = "default_polling_delay")]
    pub polling_delay_seconds: u64,

    /// Maximum number of polling retries
    #[serde(default = "default_polling_retries")]
    pub polling_retries: u32,

    /// Default video duration in seconds (5 or 10)
    #[serde(default = "default_video_duration")]
    pub default_video_duration: u32,

    /// Default video resolution
    #[serde(default = "default_video_resolution")]
    pub default_video_resolution: String,

    /// Whether to watermark generated content
    #[serde(default = "default_watermark")]
    pub watermark: bool,
}

fn default_polling_delay() -> u64 {
    DEFAULT_POLLING_DELAY_SECONDS
}

fn default_polling_retries() -> u32 {
    DEFAULT_POLLING_RETRIES
}

fn default_video_duration() -> u32 {
    5 // 5 seconds default
}

fn default_video_resolution() -> String {
    "720p".to_string()
}

fn default_watermark() -> bool {
    false
}

impl Default for RunwayMLConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_key: None,
                api_base: Some(DEFAULT_API_BASE.to_string()),
                timeout: 600, // Runway video generation can take a long time
                max_retries: 3,
                headers: std::collections::HashMap::new(),
                organization: None,
                api_version: Some("2024-11-06".to_string()),
            },
            polling_delay_seconds: DEFAULT_POLLING_DELAY_SECONDS,
            polling_retries: DEFAULT_POLLING_RETRIES,
            default_video_duration: 5,
            default_video_resolution: "720p".to_string(),
            watermark: false,
        }
    }
}

impl RunwayMLConfig {
    /// Create new configuration with API key
    pub fn new(api_key: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.base.api_key = Some(api_key.into());
        config
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(api_key) = std::env::var("RUNWAYML_API_KEY") {
            config.base.api_key = Some(api_key);
        }

        if let Ok(api_base) = std::env::var("RUNWAYML_API_BASE") {
            config.base.api_base = Some(api_base);
        }

        if let Ok(polling_delay) = std::env::var("RUNWAYML_POLLING_DELAY")
            && let Ok(delay) = polling_delay.parse()
        {
            config.polling_delay_seconds = delay;
        }

        if let Ok(polling_retries) = std::env::var("RUNWAYML_POLLING_RETRIES")
            && let Ok(retries) = polling_retries.parse()
        {
            config.polling_retries = retries;
        }

        if let Ok(duration) = std::env::var("RUNWAYML_VIDEO_DURATION")
            && let Ok(dur) = duration.parse()
        {
            config.default_video_duration = dur;
        }

        if let Ok(resolution) = std::env::var("RUNWAYML_VIDEO_RESOLUTION") {
            config.default_video_resolution = resolution;
        }

        if let Ok(watermark) = std::env::var("RUNWAYML_WATERMARK") {
            config.watermark = watermark.to_lowercase() == "true";
        }

        config
    }

    /// Get the effective API base URL
    pub fn get_api_base(&self) -> String {
        self.base
            .api_base
            .clone()
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string())
    }

    /// Get the generate endpoint URL
    pub fn get_generate_url(&self) -> String {
        format!("{}/v1/tasks", self.get_api_base())
    }

    /// Get the task status URL
    pub fn get_task_url(&self, task_id: &str) -> String {
        format!("{}/v1/tasks/{}", self.get_api_base(), task_id)
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

    /// With custom video duration
    pub fn with_video_duration(mut self, duration: u32) -> Self {
        self.default_video_duration = duration;
        self
    }

    /// With custom video resolution
    pub fn with_video_resolution(mut self, resolution: impl Into<String>) -> Self {
        self.default_video_resolution = resolution.into();
        self
    }

    /// With watermark setting
    pub fn with_watermark(mut self, watermark: bool) -> Self {
        self.watermark = watermark;
        self
    }
}

impl ProviderConfig for RunwayMLConfig {
    fn validate(&self) -> Result<(), String> {
        // API key is required
        if self.base.api_key.is_none() {
            return Err("Runway ML API key is required (RUNWAYML_API_KEY)".to_string());
        }

        // Validate polling settings
        if self.polling_delay_seconds == 0 {
            return Err("Polling delay must be greater than 0".to_string());
        }

        if self.polling_retries == 0 {
            return Err("Polling retries must be greater than 0".to_string());
        }

        // Validate video duration (Runway supports 5 or 10 seconds)
        if self.default_video_duration != 5 && self.default_video_duration != 10 {
            return Err("Video duration must be 5 or 10 seconds".to_string());
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
    fn test_runwayml_config_default() {
        let config = RunwayMLConfig::default();
        assert_eq!(config.get_api_base(), DEFAULT_API_BASE);
        assert_eq!(config.polling_delay_seconds, DEFAULT_POLLING_DELAY_SECONDS);
        assert_eq!(config.polling_retries, DEFAULT_POLLING_RETRIES);
        assert_eq!(config.default_video_duration, 5);
        assert_eq!(config.default_video_resolution, "720p");
        assert!(!config.watermark);
    }

    #[test]
    fn test_runwayml_config_new() {
        let config = RunwayMLConfig::new("test-api-key");
        assert_eq!(config.base.api_key, Some("test-api-key".to_string()));
    }

    #[test]
    fn test_runwayml_config_validate_missing_api_key() {
        let config = RunwayMLConfig::default();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }

    #[test]
    fn test_runwayml_config_validate_success() {
        let config = RunwayMLConfig::new("test-api-key");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_runwayml_config_validate_invalid_duration() {
        let mut config = RunwayMLConfig::new("test-api-key");
        config.default_video_duration = 7; // Invalid
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("5 or 10"));
    }

    #[test]
    fn test_get_generate_url() {
        let config = RunwayMLConfig::new("test-api-key");
        let url = config.get_generate_url();
        assert_eq!(url, format!("{}/v1/tasks", DEFAULT_API_BASE));
    }

    #[test]
    fn test_get_task_url() {
        let config = RunwayMLConfig::new("test-api-key");
        let url = config.get_task_url("task-123");
        assert_eq!(url, format!("{}/v1/tasks/task-123", DEFAULT_API_BASE));
    }

    #[test]
    fn test_provider_config_trait() {
        let config = RunwayMLConfig::new("test-api-key");
        assert_eq!(config.api_key(), Some("test-api-key"));
        assert_eq!(config.api_base(), Some(DEFAULT_API_BASE));
        assert_eq!(config.timeout(), Duration::from_secs(600));
        assert_eq!(config.max_retries(), 3);
    }

    #[test]
    fn test_config_builder_methods() {
        let config = RunwayMLConfig::new("api-key")
            .with_polling_delay(5)
            .with_polling_retries(100)
            .with_video_duration(10)
            .with_video_resolution("1080p")
            .with_watermark(true);

        assert_eq!(config.polling_delay_seconds, 5);
        assert_eq!(config.polling_retries, 100);
        assert_eq!(config.default_video_duration, 10);
        assert_eq!(config.default_video_resolution, "1080p");
        assert!(config.watermark);
    }

    #[test]
    fn test_validate_zero_polling_delay() {
        let mut config = RunwayMLConfig::new("api-key");
        config.polling_delay_seconds = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Polling delay"));
    }

    #[test]
    fn test_validate_zero_polling_retries() {
        let mut config = RunwayMLConfig::new("api-key");
        config.polling_retries = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Polling retries"));
    }
}
