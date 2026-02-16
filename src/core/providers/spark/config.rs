//! iFlytek Spark Configuration
//!
//! Configuration for Spark provider with WebSocket-based API support

use std::collections::HashMap;
use std::env;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::ProviderConfig;

/// iFlytek Spark configuration
#[derive(Debug, Clone)]
pub struct SparkConfig {
    /// Application ID
    pub app_id: Option<String>,
    /// API key
    pub api_key: Option<String>,
    /// API secret for HMAC authentication
    pub api_secret: Option<String>,
    /// Base URL for HTTP API (fallback)
    pub api_base: String,
    /// Request timeout in seconds
    pub request_timeout: u64,
    /// Connection timeout in seconds
    pub connect_timeout: u64,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Retry delay base (milliseconds)
    pub retry_delay_base: u64,
    /// Custom headers
    pub custom_headers: HashMap<String, String>,
    /// Enable WebSocket streaming (default: true)
    pub enable_websocket: bool,
    /// WebSocket timeout in seconds
    pub websocket_timeout: u64,
}

impl Default for SparkConfig {
    fn default() -> Self {
        Self {
            app_id: None,
            api_key: None,
            api_secret: None,
            api_base: "https://spark-api.xf-yun.com".to_string(),
            request_timeout: 120,
            connect_timeout: 10,
            max_retries: 3,
            retry_delay_base: 1000,
            custom_headers: HashMap::new(),
            enable_websocket: true,
            websocket_timeout: 120,
        }
    }
}

impl SparkConfig {
    /// Create new configuration
    pub fn new(
        app_id: impl Into<String>,
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
    ) -> Self {
        Self {
            app_id: Some(app_id.into()),
            api_key: Some(api_key.into()),
            api_secret: Some(api_secret.into()),
            ..Default::default()
        }
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let app_id = env::var("SPARK_APP_ID")
            .ok()
            .or_else(|| env::var("IFLYTEK_APP_ID").ok());

        let api_key = env::var("SPARK_API_KEY")
            .ok()
            .or_else(|| env::var("IFLYTEK_API_KEY").ok());

        let api_secret = env::var("SPARK_API_SECRET")
            .ok()
            .or_else(|| env::var("IFLYTEK_API_SECRET").ok());

        if app_id.is_none() || api_key.is_none() || api_secret.is_none() {
            return Err(ProviderError::configuration(
                "spark",
                "SPARK_APP_ID, SPARK_API_KEY, and SPARK_API_SECRET environment variables are required",
            ));
        }

        let mut config = Self {
            app_id,
            api_key,
            api_secret,
            ..Default::default()
        };

        // Optional environment overrides
        if let Ok(api_base) = env::var("SPARK_API_BASE") {
            config.api_base = api_base;
        }

        if let Ok(timeout) = env::var("SPARK_TIMEOUT") {
            config.request_timeout = timeout.parse().unwrap_or(120);
        }

        if let Ok(enable_ws) = env::var("SPARK_ENABLE_WEBSOCKET") {
            config.enable_websocket = enable_ws.parse().unwrap_or(true);
        }

        if let Ok(ws_timeout) = env::var("SPARK_WEBSOCKET_TIMEOUT") {
            config.websocket_timeout = ws_timeout.parse().unwrap_or(120);
        }

        Ok(config)
    }

    /// Set app ID
    pub fn with_app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set API secret
    pub fn with_api_secret(mut self, api_secret: impl Into<String>) -> Self {
        self.api_secret = Some(api_secret.into());
        self
    }

    /// Set API base URL
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    /// Set request timeout
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set WebSocket support
    pub fn with_websocket(mut self, enabled: bool) -> Self {
        self.enable_websocket = enabled;
        self
    }

    /// Set WebSocket timeout
    pub fn with_websocket_timeout(mut self, timeout: u64) -> Self {
        self.websocket_timeout = timeout;
        self
    }

    /// Add custom header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }

    /// Get WebSocket URL for model
    pub fn get_websocket_url(&self, model: &str) -> &'static str {
        if model.contains("v3.5") || model.contains("v4") {
            super::WS_V3_5_URL
        } else if model.contains("v3") {
            super::WS_V3_URL
        } else if model.contains("v2") {
            super::WS_V2_URL
        } else {
            super::WS_V1_5_URL
        }
    }

    /// Get effective API base
    pub fn get_effective_api_base(&self) -> &str {
        &self.api_base
    }
}

impl ProviderConfig for SparkConfig {
    fn validate(&self) -> Result<(), String> {
        let app_id = self.app_id.as_ref().ok_or("Spark APP_ID is required")?;
        if app_id.is_empty() {
            return Err("Spark APP_ID cannot be empty".to_string());
        }

        let api_key = self.api_key.as_ref().ok_or("Spark API key is required")?;
        if api_key.is_empty() {
            return Err("Spark API key cannot be empty".to_string());
        }

        let api_secret = self
            .api_secret
            .as_ref()
            .ok_or("Spark API secret is required")?;
        if api_secret.is_empty() {
            return Err("Spark API secret cannot be empty".to_string());
        }

        // Validate base URL
        if self.api_base.is_empty() {
            return Err("API base URL cannot be empty".to_string());
        }

        if !self.api_base.starts_with("http://") && !self.api_base.starts_with("https://") {
            return Err("API base URL must start with http:// or https://".to_string());
        }

        // Validate timeout settings
        if self.request_timeout == 0 {
            return Err("Request timeout must be greater than 0".to_string());
        }

        if self.connect_timeout == 0 {
            return Err("Connect timeout must be greater than 0".to_string());
        }

        if self.connect_timeout > self.request_timeout {
            return Err("Connect timeout cannot be greater than request timeout".to_string());
        }

        if self.websocket_timeout == 0 {
            return Err("WebSocket timeout must be greater than 0".to_string());
        }

        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        Some(&self.api_base)
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.request_timeout)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// Configuration builder
pub struct SparkConfigBuilder {
    config: SparkConfig,
}

impl SparkConfigBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: SparkConfig::default(),
        }
    }

    /// Build from environment
    pub fn from_env() -> Result<Self, ProviderError> {
        Ok(Self {
            config: SparkConfig::from_env()?,
        })
    }

    /// Set app ID
    pub fn app_id(mut self, app_id: impl Into<String>) -> Self {
        self.config.app_id = Some(app_id.into());
        self
    }

    /// Set API key
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.config.api_key = Some(api_key.into());
        self
    }

    /// Set API secret
    pub fn api_secret(mut self, api_secret: impl Into<String>) -> Self {
        self.config.api_secret = Some(api_secret.into());
        self
    }

    /// Set API base URL
    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.config.api_base = api_base.into();
        self
    }

    /// Set timeout
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.config.request_timeout = timeout;
        self
    }

    /// Enable/disable WebSocket
    pub fn websocket(mut self, enabled: bool) -> Self {
        self.config.enable_websocket = enabled;
        self
    }

    /// Build configuration
    pub fn build(self) -> Result<SparkConfig, ProviderError> {
        self.config
            .validate()
            .map_err(|e| ProviderError::configuration("spark", e))?;
        Ok(self.config)
    }
}

impl Default for SparkConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SparkConfig::default();
        assert_eq!(config.api_base, "https://spark-api.xf-yun.com");
        assert_eq!(config.request_timeout, 120);
        assert!(config.enable_websocket);
    }

    #[test]
    fn test_config_validation() {
        let mut config = SparkConfig::default();

        // Should fail without credentials
        assert!(config.validate().is_err());

        // Should pass with valid credentials
        config.app_id = Some("test-app-id".to_string());
        config.api_key = Some("test-api-key".to_string());
        config.api_secret = Some("test-api-secret".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_builder() {
        let config = SparkConfigBuilder::new()
            .app_id("test-app-id")
            .api_key("test-api-key")
            .api_secret("test-api-secret")
            .timeout(60)
            .websocket(false)
            .build();

        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.app_id, Some("test-app-id".to_string()));
        assert_eq!(config.api_key, Some("test-api-key".to_string()));
        assert_eq!(config.api_secret, Some("test-api-secret".to_string()));
        assert_eq!(config.request_timeout, 60);
        assert!(!config.enable_websocket);
    }

    #[test]
    fn test_websocket_url_selection() {
        let config = SparkConfig::default();

        assert_eq!(
            config.get_websocket_url("spark-desk-v3.5"),
            super::super::WS_V3_5_URL
        );
        assert_eq!(
            config.get_websocket_url("spark-desk-v3"),
            super::super::WS_V3_URL
        );
        assert_eq!(
            config.get_websocket_url("spark-desk-v2"),
            super::super::WS_V2_URL
        );
        assert_eq!(
            config.get_websocket_url("spark-desk-v1.5"),
            super::super::WS_V1_5_URL
        );
    }
}
