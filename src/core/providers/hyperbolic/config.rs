//! Hyperbolic Provider Configuration
//!
//! Configuration for Hyperbolic API access including authentication and model settings.

use crate::define_provider_config;

define_provider_config!(HyperbolicConfig {
    debug: bool = false,
}, provider: "hyperbolic");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_hyperbolic_config_default() {
        let config = HyperbolicConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(config.base.timeout, 60);
        assert_eq!(config.base.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_hyperbolic_config_from_env() {
        let config = HyperbolicConfig::from_env();
        assert_eq!(config.get_api_base(), "https://api.hyperbolic.xyz/v1");
    }

    #[test]
    fn test_hyperbolic_config_get_api_base_custom() {
        let config = HyperbolicConfig::from_env().with_base_url("https://custom.hyperbolic.xyz");
        assert_eq!(config.get_api_base(), "https://custom.hyperbolic.xyz");
    }

    #[test]
    fn test_hyperbolic_config_get_api_key() {
        let config = HyperbolicConfig::from_env().with_api_key("test-key");
        assert_eq!(config.get_api_key(), Some("test-key".to_string()));
    }

    #[test]
    fn test_hyperbolic_config_provider_config_trait() {
        let config = HyperbolicConfig::from_env()
            .with_api_key("test-key")
            .with_base_url("https://custom.hyperbolic.xyz")
            .with_timeout(60);

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://custom.hyperbolic.xyz"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
    }

    #[test]
    fn test_hyperbolic_config_validation_with_key() {
        let config = HyperbolicConfig::from_env().with_api_key("test-key");
        assert!(config.validate().is_ok());
    }
}
