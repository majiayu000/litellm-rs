//! Snowflake Provider Configuration
//!
//! Configuration for Snowflake Cortex AI API access including authentication and account settings.

use crate::core::traits::provider::ProviderConfig;
use serde::{Deserialize, Serialize};

/// Snowflake authentication type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AuthType {
    /// JWT key pair authentication
    #[default]
    KeypairJwt,
    /// Programmatic Access Token (PAT)
    ProgrammaticAccessToken,
}

/// Snowflake provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnowflakeConfig {
    /// JWT token or PAT for authentication
    /// If prefixed with "pat/", it will be treated as a Programmatic Access Token
    pub api_key: Option<String>,

    /// Snowflake account ID (e.g., "xy12345.us-east-1")
    pub account_id: Option<String>,

    /// API base URL (optional, defaults to <https://{account_id}.snowflakecomputing.com/api/v2>)
    pub api_base: Option<String>,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Maximum number of retries for failed requests
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Whether to enable debug logging
    #[serde(default)]
    pub debug: bool,
}

impl Default for SnowflakeConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            account_id: None,
            api_base: None,
            timeout: default_timeout(),
            max_retries: default_max_retries(),
            debug: false,
        }
    }
}

impl ProviderConfig for SnowflakeConfig {
    fn validate(&self) -> Result<(), String> {
        // Check for API key (JWT or PAT)
        if self.api_key.is_none() && std::env::var("SNOWFLAKE_JWT").is_err() {
            return Err("Snowflake JWT/PAT not provided. \
                Set SNOWFLAKE_JWT environment variable or pass api_key in configuration."
                .to_string());
        }

        // Check for account ID or API base
        let has_account =
            self.account_id.is_some() || std::env::var("SNOWFLAKE_ACCOUNT_ID").is_ok();
        let has_api_base = self.api_base.is_some();

        if !has_account && !has_api_base {
            return Err("Snowflake account_id or api_base not provided. \
                Set SNOWFLAKE_ACCOUNT_ID environment variable or pass in configuration."
                .to_string());
        }

        // Validate timeout
        if self.timeout == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }

        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.api_base.as_deref()
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

impl SnowflakeConfig {
    /// Get API key with environment variable fallback
    pub fn get_api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| std::env::var("SNOWFLAKE_JWT").ok())
    }

    /// Get account ID with environment variable fallback
    pub fn get_account_id(&self) -> Option<String> {
        self.account_id
            .clone()
            .or_else(|| std::env::var("SNOWFLAKE_ACCOUNT_ID").ok())
    }

    /// Get the authentication type based on the API key format
    pub fn get_auth_type(&self) -> AuthType {
        if let Some(ref key) = self.get_api_key()
            && key.starts_with("pat/")
        {
            return AuthType::ProgrammaticAccessToken;
        }
        AuthType::KeypairJwt
    }

    /// Get the raw API key (without pat/ prefix if present)
    pub fn get_raw_api_key(&self) -> Option<String> {
        self.get_api_key().map(|key| {
            if key.starts_with("pat/") {
                key.strip_prefix("pat/").unwrap_or(&key).to_string()
            } else {
                key
            }
        })
    }

    /// Get API base URL, constructing from account_id if not explicitly set
    pub fn get_api_base(&self) -> Result<String, String> {
        if let Some(ref base) = self.api_base {
            let mut base = base.trim_end_matches('/').to_string();
            if !base.ends_with("/api/v2") {
                base.push_str("/api/v2");
            }
            return Ok(base);
        }

        let account_id = self.get_account_id().ok_or_else(|| {
            "Snowflake account_id not set. Set SNOWFLAKE_ACCOUNT_ID environment variable \
            or pass account_id in configuration."
                .to_string()
        })?;

        Ok(format!(
            "https://{}.snowflakecomputing.com/api/v2",
            account_id
        ))
    }

    /// Build the complete URL for the Cortex inference endpoint
    pub fn build_inference_url(&self) -> Result<String, String> {
        let base = self.get_api_base()?;
        Ok(format!("{}/cortex/inference:complete", base))
    }
}

fn default_timeout() -> u64 {
    120 // Snowflake requests can take longer
}

fn default_max_retries() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snowflake_config_default() {
        let config = SnowflakeConfig::default();
        assert!(config.api_key.is_none());
        assert!(config.account_id.is_none());
        assert!(config.api_base.is_none());
        assert_eq!(config.timeout, 120);
        assert_eq!(config.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_snowflake_config_get_api_base() {
        let config = SnowflakeConfig {
            account_id: Some("xy12345.us-east-1".to_string()),
            ..Default::default()
        };
        let base = config.get_api_base().unwrap();
        assert_eq!(
            base,
            "https://xy12345.us-east-1.snowflakecomputing.com/api/v2"
        );
    }

    #[test]
    fn test_snowflake_config_get_api_base_explicit() {
        let config = SnowflakeConfig {
            api_base: Some("https://custom.snowflake.com".to_string()),
            ..Default::default()
        };
        let base = config.get_api_base().unwrap();
        assert_eq!(base, "https://custom.snowflake.com/api/v2");
    }

    #[test]
    fn test_snowflake_config_get_api_base_with_v2() {
        let config = SnowflakeConfig {
            api_base: Some("https://custom.snowflake.com/api/v2".to_string()),
            ..Default::default()
        };
        let base = config.get_api_base().unwrap();
        assert_eq!(base, "https://custom.snowflake.com/api/v2");
    }

    #[test]
    fn test_snowflake_config_auth_type_jwt() {
        let config = SnowflakeConfig {
            api_key: Some("jwt-token-here".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_auth_type(), AuthType::KeypairJwt);
    }

    #[test]
    fn test_snowflake_config_auth_type_pat() {
        let config = SnowflakeConfig {
            api_key: Some("pat/my-pat-token".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_auth_type(), AuthType::ProgrammaticAccessToken);
    }

    #[test]
    fn test_snowflake_config_get_raw_api_key_jwt() {
        let config = SnowflakeConfig {
            api_key: Some("jwt-token".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_raw_api_key(), Some("jwt-token".to_string()));
    }

    #[test]
    fn test_snowflake_config_get_raw_api_key_pat() {
        let config = SnowflakeConfig {
            api_key: Some("pat/my-pat-token".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_raw_api_key(), Some("my-pat-token".to_string()));
    }

    #[test]
    fn test_snowflake_config_build_inference_url() {
        let config = SnowflakeConfig {
            account_id: Some("xy12345.us-east-1".to_string()),
            ..Default::default()
        };
        let url = config.build_inference_url().unwrap();
        assert_eq!(
            url,
            "https://xy12345.us-east-1.snowflakecomputing.com/api/v2/cortex/inference:complete"
        );
    }

    #[test]
    fn test_snowflake_config_provider_config_trait() {
        let config = SnowflakeConfig {
            api_key: Some("test-key".to_string()),
            api_base: Some("https://custom.snowflake.com".to_string()),
            timeout: 180,
            max_retries: 5,
            ..Default::default()
        };

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://custom.snowflake.com"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(180));
        assert_eq!(config.max_retries(), 5);
    }

    #[test]
    fn test_snowflake_config_serialization() {
        let config = SnowflakeConfig {
            api_key: Some("test-key".to_string()),
            account_id: Some("xy12345".to_string()),
            timeout: 90,
            max_retries: 2,
            debug: true,
            ..Default::default()
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["api_key"], "test-key");
        assert_eq!(json["account_id"], "xy12345");
        assert_eq!(json["timeout"], 90);
        assert_eq!(json["max_retries"], 2);
        assert_eq!(json["debug"], true);
    }

    #[test]
    fn test_snowflake_config_deserialization() {
        let json = r#"{
            "api_key": "test-key",
            "account_id": "xy12345",
            "timeout": 150,
            "debug": true
        }"#;

        let config: SnowflakeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.account_id, Some("xy12345".to_string()));
        assert_eq!(config.timeout, 150);
        assert!(config.debug);
    }
}
