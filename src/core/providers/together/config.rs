//! Together AI Provider Configuration
//!
//! Configuration for Together AI API access including authentication and model settings.

use crate::define_provider_config;

define_provider_config!(TogetherConfig {
    debug: bool = false,
}, provider: "together");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_together_config_default() {
        let config = TogetherConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(config.base.timeout, 60);
        assert_eq!(config.base.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_together_config_get_api_base_default() {
        let config = TogetherConfig::from_env();
        assert_eq!(config.get_api_base(), "https://api.together.xyz/v1");
    }

    #[test]
    fn test_together_config_get_api_base_custom() {
        let config = TogetherConfig::from_env().with_base_url("https://custom.together.xyz");
        assert_eq!(config.get_api_base(), "https://custom.together.xyz");
    }

    #[test]
    fn test_together_config_get_api_key() {
        let config = TogetherConfig::from_env().with_api_key("test-key");
        assert_eq!(config.get_api_key(), Some("test-key".to_string()));
    }

    #[test]
    fn test_together_config_provider_config_trait() {
        let config = TogetherConfig::from_env()
            .with_api_key("test-key")
            .with_base_url("https://custom.together.xyz")
            .with_timeout(120);

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://custom.together.xyz"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(120));
    }

    #[test]
    fn test_together_config_validation_with_key() {
        let mut config = TogetherConfig::from_env();
        config.base.api_key = Some("test-key".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_together_config_validation_missing_key() {
        let config = TogetherConfig::default();
        assert!(config.validate().is_err());
    }
}
