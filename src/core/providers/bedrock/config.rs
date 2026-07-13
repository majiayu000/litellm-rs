//! Bedrock Provider Configuration
//!
//! Configuration management for AWS Bedrock provider including
//! AWS credentials, regions, and model-specific settings.

use crate::core::net::ProviderEndpointAccess;
use crate::core::traits::provider::ProviderConfig;
use serde::{Deserialize, Serialize};

/// AWS Bedrock provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConfig {
    /// AWS access key ID
    pub aws_access_key_id: String,
    /// AWS secret access key
    pub aws_secret_access_key: String,
    /// AWS session token (optional, for temporary credentials)
    pub aws_session_token: Option<String>,
    /// AWS region
    pub aws_region: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Network scope allowed for Bedrock endpoints
    #[serde(default)]
    pub endpoint_access: ProviderEndpointAccess,
}

impl Default for BedrockConfig {
    fn default() -> Self {
        Self {
            aws_access_key_id: String::new(),
            aws_secret_access_key: String::new(),
            aws_session_token: None,
            aws_region: "us-east-1".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
        }
    }
}

impl ProviderConfig for BedrockConfig {
    fn validate(&self) -> Result<(), String> {
        if self.aws_access_key_id.is_empty() {
            return Err("AWS access key ID is required".to_string());
        }
        if self.aws_secret_access_key.is_empty() {
            return Err("AWS secret access key is required".to_string());
        }
        if self.aws_region.is_empty() {
            return Err("AWS region is required".to_string());
        }
        if self.timeout_seconds == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }
        if self.max_retries > 10 {
            return Err("Max retries should not exceed 10".to_string());
        }
        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        // Bedrock uses AWS credentials instead of API key
        None
    }

    fn api_base(&self) -> Option<&str> {
        // Bedrock URL is constructed dynamically based on region and model
        None
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_seconds)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }

    fn endpoint_access(&self) -> ProviderEndpointAccess {
        self.endpoint_access
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = BedrockConfig::default();
        assert!(config.validate().is_err()); // No credentials

        config.aws_access_key_id = "test_key".to_string();
        assert!(config.validate().is_err()); // No secret

        config.aws_secret_access_key = "test_secret".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_default_config() {
        let config = BedrockConfig::default();
        assert_eq!(config.aws_region, "us-east-1");
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.endpoint_access, ProviderEndpointAccess::PublicOnly);
    }
}
