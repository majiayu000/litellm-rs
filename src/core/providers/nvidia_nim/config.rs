//! NVIDIA NIM Provider Configuration

use crate::define_provider_config;

define_provider_config!(NvidiaNimConfig {
    debug: bool = false,
}, provider: "nvidia_nim");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_nvidia_nim_config_default() {
        let config = NvidiaNimConfig::default();
        assert!(config.base.api_key.is_none());
        assert_eq!(config.base.timeout, 60);
        assert_eq!(config.base.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_nvidia_nim_config_get_api_base_default() {
        let config = NvidiaNimConfig::from_env();
        assert_eq!(config.get_api_base(), "https://integrate.api.nvidia.com/v1");
    }

    #[test]
    fn test_nvidia_nim_config_get_api_base_custom() {
        let config = NvidiaNimConfig::from_env().with_base_url("https://custom.nvidia.com");
        assert_eq!(config.get_api_base(), "https://custom.nvidia.com");
    }

    #[test]
    fn test_nvidia_nim_config_get_api_key() {
        let config = NvidiaNimConfig::from_env().with_api_key("nvapi-test-key");
        assert_eq!(config.get_api_key(), Some("nvapi-test-key".to_string()));
    }

    #[test]
    fn test_nvidia_nim_config_provider_config_trait() {
        let config = NvidiaNimConfig::from_env()
            .with_api_key("nvapi-test-key")
            .with_base_url("https://custom.nvidia.com")
            .with_timeout(120);

        assert_eq!(config.api_key(), Some("nvapi-test-key"));
        assert_eq!(config.api_base(), Some("https://custom.nvidia.com"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(120));
    }

    #[test]
    fn test_nvidia_nim_config_validation_with_key() {
        let mut config = NvidiaNimConfig::from_env();
        config.base.api_key = Some("nvapi-test-key".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_nvidia_nim_config_validation_missing_key() {
        let config = NvidiaNimConfig::default();
        assert!(config.validate().is_err());
    }
}
