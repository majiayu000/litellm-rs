//! Deepgram Provider Configuration
//!
//! Configuration for Deepgram API access including authentication and endpoint settings.

use crate::define_provider_config;

define_provider_config!(DeepgramConfig {
    debug: bool = false,
}, provider: "deepgram");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_deepgram_config_default() {
        let config = DeepgramConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(config.base.timeout, 60);
        assert_eq!(config.base.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_deepgram_config_from_env() {
        let config = DeepgramConfig::from_env();
        assert_eq!(config.get_api_base(), "https://api.deepgram.com/v1");
    }

    #[test]
    fn test_deepgram_config_get_api_base_custom() {
        let config = DeepgramConfig::from_env().with_base_url("https://custom.deepgram.com");
        assert_eq!(config.get_api_base(), "https://custom.deepgram.com");
    }

    #[test]
    fn test_deepgram_config_get_api_key() {
        let config = DeepgramConfig::from_env().with_api_key("test-key");
        assert_eq!(config.get_api_key(), Some("test-key".to_string()));
    }

    #[test]
    fn test_deepgram_config_provider_config_trait() {
        let config = DeepgramConfig::from_env()
            .with_api_key("test-key")
            .with_base_url("https://custom.deepgram.com")
            .with_timeout(180);

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://custom.deepgram.com"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(180));
    }

    #[test]
    fn test_deepgram_config_validation_with_key() {
        let config = DeepgramConfig::from_env().with_api_key("test-key");
        assert!(config.validate().is_ok());
    }
}
