//! ElevenLabs Provider Configuration
//!
//! Configuration for ElevenLabs API access including authentication and endpoint settings.

use crate::define_provider_config;

define_provider_config!(ElevenLabsConfig {
    debug: bool = false,
}, provider: "elevenlabs");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_elevenlabs_config_default() {
        let config = ElevenLabsConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(config.base.timeout, 60);
        assert_eq!(config.base.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_elevenlabs_config_from_env() {
        let config = ElevenLabsConfig::from_env();
        assert_eq!(config.get_api_base(), "https://api.elevenlabs.io");
    }

    #[test]
    fn test_elevenlabs_config_get_api_base_custom() {
        let config = ElevenLabsConfig::from_env().with_base_url("https://custom.elevenlabs.io");
        assert_eq!(config.get_api_base(), "https://custom.elevenlabs.io");
    }

    #[test]
    fn test_elevenlabs_config_get_api_key() {
        let config = ElevenLabsConfig::from_env().with_api_key("test-key");
        assert_eq!(config.get_api_key(), Some("test-key".to_string()));
    }

    #[test]
    fn test_elevenlabs_config_provider_config_trait() {
        let config = ElevenLabsConfig::from_env()
            .with_api_key("test-key")
            .with_base_url("https://custom.elevenlabs.io")
            .with_timeout(90);

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://custom.elevenlabs.io"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(90));
    }

    #[test]
    fn test_elevenlabs_config_validation_with_key() {
        let config = ElevenLabsConfig::from_env().with_api_key("test-key");
        assert!(config.validate().is_ok());
    }
}
