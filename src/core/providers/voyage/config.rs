//! Voyage AI Provider Configuration
//!
//! Configuration for Voyage AI API access including authentication and embedding settings.

use crate::define_provider_config;

define_provider_config!(VoyageConfig {
    debug: bool = false,
}, provider: "voyage");

impl VoyageConfig {
    /// Get embeddings endpoint URL
    pub fn get_embeddings_url(&self) -> String {
        let base = self.get_api_base();
        if base.ends_with("/embeddings") {
            base
        } else {
            format!("{}/embeddings", base)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_voyage_config_default() {
        let config = VoyageConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(config.base.timeout, 60);
        assert_eq!(config.base.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_voyage_config_get_api_base_default() {
        let config = VoyageConfig::from_env();
        assert_eq!(config.get_api_base(), "https://api.voyageai.com/v1");
    }

    #[test]
    fn test_voyage_config_get_api_base_custom() {
        let config = VoyageConfig::from_env().with_base_url("https://custom.voyageai.com");
        assert_eq!(config.get_api_base(), "https://custom.voyageai.com");
    }

    #[test]
    fn test_voyage_config_get_api_key() {
        let config = VoyageConfig::from_env().with_api_key("test-key");
        assert_eq!(config.get_api_key(), Some("test-key".to_string()));
    }

    #[test]
    fn test_voyage_config_get_embeddings_url() {
        let config = VoyageConfig::from_env();
        assert_eq!(
            config.get_embeddings_url(),
            "https://api.voyageai.com/v1/embeddings"
        );

        let config2 = VoyageConfig::from_env().with_base_url("https://custom.api.com/embeddings");
        assert_eq!(
            config2.get_embeddings_url(),
            "https://custom.api.com/embeddings"
        );
    }

    #[test]
    fn test_voyage_config_provider_config_trait() {
        let config = VoyageConfig::from_env()
            .with_api_key("test-key")
            .with_base_url("https://custom.voyageai.com")
            .with_timeout(90);

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("https://custom.voyageai.com"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(90));
    }

    #[test]
    fn test_voyage_config_validation_with_key() {
        let mut config = VoyageConfig::from_env();
        config.base.api_key = Some("test-key".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_voyage_config_validation_missing_key() {
        let config = VoyageConfig::default();
        assert!(config.validate().is_err());
    }
}
