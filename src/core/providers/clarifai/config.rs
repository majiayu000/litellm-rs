//! Clarifai Provider Configuration
//!
//! Configuration for Clarifai API access including authentication and model settings.

use crate::define_standalone_provider_config;

define_standalone_provider_config!(ClarifaiConfig,
    provider: "Clarifai",
    env_prefix: "CLARIFAI",
    default_base_url: "https://api.clarifai.com/v2/ext/openai/v1",
    default_timeout: 30,
    extra_fields: { debug: bool = false },
);

impl ClarifaiConfig {
    /// Parse Clarifai model string format and return the full model URL
    ///
    /// Clarifai model format: `user_id.app_id.model_id`
    /// Returns: `https://clarifai.com/{user_id}/{app_id}/models/{model_id}`
    pub fn get_model_url(model: &str) -> Option<String> {
        let parts: Vec<&str> = model.split('.').collect();
        if parts.len() == 3 {
            let user_id = parts[0];
            let app_id = parts[1];
            let model_id = parts[2];
            Some(format!(
                "https://clarifai.com/{}/{}/models/{}",
                user_id, app_id, model_id
            ))
        } else {
            None
        }
    }

    /// Check if a model string is in valid Clarifai format
    pub fn is_valid_model_format(model: &str) -> bool {
        let parts: Vec<&str> = model.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| !p.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::ProviderConfig;

    #[test]
    fn test_clarifai_config_default() {
        let config = ClarifaiConfig::default();
        assert!(config.api_key.is_none());
        assert!(config.api_base.is_none());
        assert_eq!(config.timeout, 30);
        assert_eq!(config.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_clarifai_config_get_api_base_default() {
        let config = ClarifaiConfig::default();
        assert_eq!(
            config.get_api_base(),
            "https://api.clarifai.com/v2/ext/openai/v1"
        );
    }

    #[test]
    fn test_get_model_url() {
        let url = ClarifaiConfig::get_model_url("user123.app456.model789");
        assert_eq!(
            url,
            Some("https://clarifai.com/user123/app456/models/model789".to_string())
        );
        assert!(ClarifaiConfig::get_model_url("user.app").is_none());
        assert!(ClarifaiConfig::get_model_url("singlepart").is_none());
    }

    #[test]
    fn test_is_valid_model_format() {
        assert!(ClarifaiConfig::is_valid_model_format("user.app.model"));
        assert!(!ClarifaiConfig::is_valid_model_format("user.app"));
        assert!(!ClarifaiConfig::is_valid_model_format("user..model"));
        assert!(!ClarifaiConfig::is_valid_model_format(".app.model"));
    }

    #[test]
    fn test_clarifai_config_validation_with_key() {
        let config = ClarifaiConfig::new("test-key");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_clarifai_config_validation_zero_timeout() {
        let config = ClarifaiConfig {
            api_key: Some("test-key".to_string()),
            timeout: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
