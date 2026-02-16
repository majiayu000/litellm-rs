//! Novita Provider Configuration

use crate::define_provider_config;

define_provider_config!(NovitaConfig {
    debug: bool = false,
}, provider: "novita");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_novita_config_default() {
        let config = NovitaConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(config.base.timeout, 60);
        assert_eq!(config.base.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_novita_config_get_api_base_default() {
        let config = NovitaConfig::from_env();
        assert_eq!(config.get_api_base(), "https://api.novita.ai/v3/openai");
    }

    #[test]
    fn test_novita_config_get_api_base_custom() {
        let config = NovitaConfig::from_env().with_base_url("https://custom.novita.ai");
        assert_eq!(config.get_api_base(), "https://custom.novita.ai");
    }

    #[test]
    fn test_novita_config_get_api_key() {
        let config = NovitaConfig::from_env().with_api_key("test-key");
        assert_eq!(config.get_api_key(), Some("test-key".to_string()));
    }

    #[test]
    fn test_novita_config_provider_config_trait() {
        let config = NovitaConfig::from_env()
            .with_api_key("test-key")
            .with_base_url("https://custom.novita.ai")
            .with_timeout(60);

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://custom.novita.ai"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
    }

    #[test]
    fn test_novita_config_validation_with_key() {
        let mut config = NovitaConfig::from_env();
        config.base.api_key = Some("test-key".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_novita_config_validation_missing_key() {
        let config = NovitaConfig::default();
        assert!(config.validate().is_err());
    }
}
