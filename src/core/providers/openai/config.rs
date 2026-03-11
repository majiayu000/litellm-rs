//! OpenAI Provider Configuration
//!
//! Unified configuration system following the base provider pattern

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::core::providers::base::BaseConfig;
use crate::core::traits::provider::ProviderConfig;

/// OpenAI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// Base configuration shared across all providers
    #[serde(flatten)]
    pub base: BaseConfig,

    /// OpenAI-specific configuration
    /// Organization ID (optional)
    pub organization: Option<String>,

    /// Project ID (optional)  
    pub project: Option<String>,

    /// Custom model mappings
    pub model_mappings: HashMap<String, String>,

    /// Feature flags
    pub features: OpenAIFeatures,
}

/// OpenAI feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFeatures {
    /// Enable O-series model optimizations
    pub o_series_optimizations: bool,

    /// Enable GPT-5 specific features
    pub gpt5_features: bool,

    /// Enable audio model support
    pub audio_models: bool,

    /// Enable DALL-E image generation
    pub image_generation: bool,

    /// Enable Whisper transcription
    pub audio_transcription: bool,

    /// Enable fine-tuning capabilities
    pub fine_tuning: bool,

    /// Enable vector store integration
    pub vector_stores: bool,

    /// Enable real-time audio (beta)
    pub realtime_audio: bool,
}

impl Default for OpenAIFeatures {
    fn default() -> Self {
        Self {
            o_series_optimizations: true,
            gpt5_features: false, // Beta feature
            audio_models: true,
            image_generation: true,
            audio_transcription: true,
            fine_tuning: false,    // Enterprise feature
            vector_stores: false,  // Enterprise feature
            realtime_audio: false, // Beta feature
        }
    }
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_key: None,
                api_base: Some("https://api.openai.com/v1".to_string()),
                timeout: 60, // OpenAI can be slow for complex requests
                max_retries: 3,
                headers: HashMap::new(),
                organization: None,
                api_version: None,
            },
            organization: None,
            project: None,
            model_mappings: HashMap::new(),
            features: OpenAIFeatures::default(),
        }
    }
}

impl OpenAIConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // API Key
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            config.base.api_key = Some(api_key);
        }

        // Organization
        if let Ok(org) = std::env::var("OPENAI_ORG_ID") {
            config.organization = Some(org);
        }

        // Project
        if let Ok(project) = std::env::var("OPENAI_PROJECT_ID") {
            config.project = Some(project);
        }

        // Base URL
        if let Ok(base_url) = std::env::var("OPENAI_API_BASE") {
            config.base.api_base = Some(base_url);
        }

        // Timeout
        if let Ok(timeout_str) = std::env::var("OPENAI_TIMEOUT")
            && let Ok(timeout) = timeout_str.parse::<u64>()
        {
            config.base.timeout = timeout;
        }

        config
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate base config
        self.base.validate("openai")?;

        // OpenAI specific validations
        if let Some(ref api_key) = self.base.api_key
            && !api_key.starts_with("sk-")
            && !api_key.starts_with("sk-proj-")
        {
            return Err("OpenAI API key must start with 'sk-' or 'sk-proj-'".to_string());
        }

        if let Some(ref org) = self.organization
            && org.is_empty()
        {
            return Err("Organization ID cannot be empty".to_string());
        }

        if let Some(ref project) = self.project
            && project.is_empty()
        {
            return Err("Project ID cannot be empty".to_string());
        }

        Ok(())
    }

    /// Get the effective API base URL
    pub fn get_api_base(&self) -> String {
        self.base
            .api_base
            .as_ref()
            .unwrap_or(&"https://api.openai.com/v1".to_string())
            .clone()
    }

    /// Check if a feature is enabled
    pub fn is_feature_enabled(&self, feature: OpenAIFeature) -> bool {
        match feature {
            OpenAIFeature::OSeriesOptimizations => self.features.o_series_optimizations,
            OpenAIFeature::GPT5Features => self.features.gpt5_features,
            OpenAIFeature::AudioModels => self.features.audio_models,
            OpenAIFeature::ImageGeneration => self.features.image_generation,
            OpenAIFeature::AudioTranscription => self.features.audio_transcription,
            OpenAIFeature::FineTuning => self.features.fine_tuning,
            OpenAIFeature::VectorStores => self.features.vector_stores,
            OpenAIFeature::RealtimeAudio => self.features.realtime_audio,
        }
    }

    /// Get model mapping for a given model name
    pub fn get_model_mapping(&self, model: &str) -> String {
        self.model_mappings
            .get(model)
            .unwrap_or(&model.to_string())
            .clone()
    }
}

/// OpenAI feature enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum OpenAIFeature {
    OSeriesOptimizations,
    GPT5Features,
    AudioModels,
    ImageGeneration,
    AudioTranscription,
    FineTuning,
    VectorStores,
    RealtimeAudio,
}

impl ProviderConfig for OpenAIConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate()
    }

    fn api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.base.api_base.as_deref()
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.base.timeout)
    }

    fn max_retries(&self) -> u32 {
        self.base.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OpenAIConfig::default();
        // Remove provider_name check as it doesn't exist in BaseConfig
        assert_eq!(config.get_api_base(), "https://api.openai.com/v1");
        assert!(config.features.image_generation);
    }

    #[test]
    fn test_config_validation() {
        let mut config = OpenAIConfig::default();

        // Should fail without API key
        assert!(config.validate().is_err());

        // Should pass with valid API key
        config.base.api_key = Some("sk-test123".to_string());
        assert!(config.validate().is_ok());

        // Should fail with invalid API key
        config.base.api_key = Some("invalid-key".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_feature_flags() {
        let config = OpenAIConfig::default();

        assert!(config.is_feature_enabled(OpenAIFeature::ImageGeneration));
        assert!(!config.is_feature_enabled(OpenAIFeature::GPT5Features));
        assert!(!config.is_feature_enabled(OpenAIFeature::RealtimeAudio));
    }

    #[test]
    fn test_model_mapping() {
        let mut config = OpenAIConfig::default();
        config
            .model_mappings
            .insert("gpt-4".to_string(), "gpt-4-0613".to_string());

        assert_eq!(config.get_model_mapping("gpt-4"), "gpt-4-0613");
        assert_eq!(config.get_model_mapping("gpt-3.5-turbo"), "gpt-3.5-turbo");
    }
}
