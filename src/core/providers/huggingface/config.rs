//! HuggingFace Provider Configuration
//!
//! Configuration for HuggingFace Hub Inference API and Inference Endpoints.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::core::traits::provider::ProviderConfig;

/// Default HuggingFace router base URL
pub const HF_ROUTER_BASE: &str = "https://router.huggingface.co";

/// Default HuggingFace Hub API base URL
pub const HF_HUB_URL: &str = "https://huggingface.co";

/// HuggingFace provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceConfig {
    /// HuggingFace API token (HF_TOKEN)
    pub api_key: String,

    /// API base URL (optional, for custom endpoints)
    #[serde(default)]
    pub api_base: Option<String>,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Maximum retries for failed requests
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Whether to use the inference router (for provider routing)
    #[serde(default = "default_use_router")]
    pub use_router: bool,
}

fn default_timeout() -> u64 {
    60
}

fn default_max_retries() -> u32 {
    3
}

fn default_use_router() -> bool {
    true
}

impl Default for HuggingFaceConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: None,
            timeout_seconds: default_timeout(),
            max_retries: default_max_retries(),
            use_router: default_use_router(),
        }
    }
}

impl HuggingFaceConfig {
    /// Create a new configuration with API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            ..Default::default()
        }
    }

    /// Create a new configuration with API key and custom base URL
    pub fn with_api_base(api_key: impl Into<String>, api_base: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_base: Some(api_base.into()),
            ..Default::default()
        }
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let api_key = std::env::var("HF_TOKEN")
            .or_else(|_| std::env::var("HUGGINGFACE_API_KEY"))
            .unwrap_or_default();

        let api_base = std::env::var("HF_API_BASE")
            .or_else(|_| std::env::var("HUGGINGFACE_API_BASE"))
            .ok();

        let timeout_seconds = std::env::var("HF_TIMEOUT")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(default_timeout());

        Self {
            api_key,
            api_base,
            timeout_seconds,
            max_retries: default_max_retries(),
            use_router: default_use_router(),
        }
    }

    /// Get the effective API base URL
    pub fn get_api_base(&self) -> String {
        self.api_base
            .clone()
            .unwrap_or_else(|| HF_ROUTER_BASE.to_string())
    }

    /// Get chat completions endpoint URL for a provider/model combination
    pub fn get_chat_url(&self, provider: Option<&str>, model: &str) -> String {
        if let Some(api_base) = &self.api_base {
            // Custom endpoint (Inference Endpoint)
            let base = api_base.trim_end_matches('/');
            if base.ends_with("/v1") {
                format!("{}/chat/completions", base)
            } else if base.ends_with("/chat/completions") {
                base.to_string()
            } else {
                format!("{}/v1/chat/completions", base)
            }
        } else if let Some(provider) = provider {
            // Route through HuggingFace router with specific provider
            match provider {
                "hf-inference" => {
                    format!(
                        "{}/hf-inference/models/{}/v1/chat/completions",
                        HF_ROUTER_BASE, model
                    )
                }
                "novita" => {
                    format!("{}/novita/v3/openai/chat/completions", HF_ROUTER_BASE)
                }
                "fireworks-ai" => {
                    format!(
                        "{}/fireworks-ai/inference/v1/chat/completions",
                        HF_ROUTER_BASE
                    )
                }
                _ => {
                    format!("{}/{}/v1/chat/completions", HF_ROUTER_BASE, provider)
                }
            }
        } else {
            // Default HuggingFace Inference API route
            format!("{}/v1/chat/completions", HF_ROUTER_BASE)
        }
    }

    /// Get embeddings endpoint URL
    pub fn get_embeddings_url(&self, task: &str, model: &str) -> String {
        if let Some(api_base) = &self.api_base {
            // Custom endpoint
            let base = api_base.trim_end_matches('/');
            format!("{}/embeddings", base)
        } else {
            // HuggingFace Inference API for embeddings
            format!(
                "{}/hf-inference/pipeline/{}/{}",
                HF_ROUTER_BASE, task, model
            )
        }
    }
}

impl ProviderConfig for HuggingFaceConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("HuggingFace")
    }

    fn api_key(&self) -> Option<&str> {
        Some(&self.api_key)
    }

    fn api_base(&self) -> Option<&str> {
        self.api_base.as_deref()
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HuggingFaceConfig::default();
        assert!(config.api_key.is_empty());
        assert!(config.api_base.is_none());
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.max_retries, 3);
        assert!(config.use_router);
    }

    #[test]
    fn test_config_new() {
        let config = HuggingFaceConfig::new("hf_test_token");
        assert_eq!(config.api_key, "hf_test_token");
        assert!(config.api_base.is_none());
    }

    #[test]
    fn test_config_with_api_base() {
        let config = HuggingFaceConfig::with_api_base(
            "hf_test_token",
            "https://my-endpoint.endpoints.huggingface.cloud",
        );
        assert_eq!(config.api_key, "hf_test_token");
        assert_eq!(
            config.api_base,
            Some("https://my-endpoint.endpoints.huggingface.cloud".to_string())
        );
    }

    #[test]
    fn test_validation_empty_api_key() {
        let config = HuggingFaceConfig::default();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_valid_config() {
        let config = HuggingFaceConfig::new("hf_test_token");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_zero_timeout() {
        let mut config = HuggingFaceConfig::new("hf_test_token");
        config.timeout_seconds = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_too_many_retries() {
        let mut config = HuggingFaceConfig::new("hf_test_token");
        config.max_retries = 11;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_get_chat_url_default_router() {
        let config = HuggingFaceConfig::new("hf_token");
        let url = config.get_chat_url(None, "meta-llama/Llama-3.3-70B-Instruct");
        assert!(url.contains("router.huggingface.co"));
        assert!(url.contains("/v1/chat/completions"));
    }

    #[test]
    fn test_get_chat_url_with_provider() {
        let config = HuggingFaceConfig::new("hf_token");

        // Together AI
        let url = config.get_chat_url(Some("together"), "deepseek-ai/DeepSeek-R1");
        assert!(url.contains("router.huggingface.co"));
        assert!(url.contains("/together/v1/chat/completions"));

        // Fireworks AI
        let url = config.get_chat_url(Some("fireworks-ai"), "deepseek-ai/DeepSeek-R1");
        assert!(url.contains("router.huggingface.co"));
        assert!(url.contains("/fireworks-ai/inference/v1/chat/completions"));
    }

    #[test]
    fn test_get_chat_url_custom_endpoint() {
        let config = HuggingFaceConfig::with_api_base(
            "hf_token",
            "https://my-endpoint.endpoints.huggingface.cloud/v1",
        );
        let url = config.get_chat_url(None, "any-model");
        assert_eq!(
            url,
            "https://my-endpoint.endpoints.huggingface.cloud/v1/chat/completions"
        );
    }

    #[test]
    fn test_get_embeddings_url() {
        let config = HuggingFaceConfig::new("hf_token");
        let url = config.get_embeddings_url("feature-extraction", "microsoft/codebert-base");
        assert!(url.contains("hf-inference/pipeline"));
        assert!(url.contains("feature-extraction"));
        assert!(url.contains("microsoft/codebert-base"));
    }

    #[test]
    fn test_provider_config_trait() {
        let config = HuggingFaceConfig::new("hf_test_token");
        assert_eq!(config.api_key(), Some("hf_test_token"));
        assert!(config.api_base().is_none());
        assert_eq!(config.timeout(), Duration::from_secs(60));
        assert_eq!(config.max_retries(), 3);
    }
}
