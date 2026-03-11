//! Ollama Provider Configuration
//!
//! Configuration for Ollama API access including connection settings and model options.

use crate::core::traits::provider::ProviderConfig;
use serde::{Deserialize, Serialize};

/// Ollama provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// API key for Ollama authentication (optional, used with remote Ollama servers)
    pub api_key: Option<String>,

    /// API base URL (default: <http://localhost:11434>)
    pub api_base: Option<String>,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Maximum number of retries for failed requests
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Whether to enable debug logging
    #[serde(default)]
    pub debug: bool,

    // ==================== Ollama-specific options ====================
    /// Enable Mirostat sampling (0 = disabled, 1 = Mirostat, 2 = Mirostat 2.0)
    pub mirostat: Option<i32>,

    /// Mirostat eta (learning rate)
    pub mirostat_eta: Option<f32>,

    /// Mirostat tau (target entropy)
    pub mirostat_tau: Option<f32>,

    /// Context window size
    pub num_ctx: Option<u32>,

    /// Number of GQA groups
    pub num_gqa: Option<u32>,

    /// Number of GPU layers (-1 for all)
    pub num_gpu: Option<i32>,

    /// Number of threads for computation
    pub num_thread: Option<u32>,

    /// How far back to look to prevent repetition
    pub repeat_last_n: Option<i32>,

    /// Repetition penalty
    pub repeat_penalty: Option<f32>,

    /// Tail free sampling parameter
    pub tfs_z: Option<f32>,

    /// System prompt override
    pub system: Option<String>,

    /// Prompt template override
    pub template: Option<String>,

    /// Keep model loaded in memory (duration string like "5m" or -1 for forever)
    pub keep_alive: Option<String>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: None,
            timeout: default_timeout(),
            max_retries: default_max_retries(),
            debug: false,
            mirostat: None,
            mirostat_eta: None,
            mirostat_tau: None,
            num_ctx: None,
            num_gqa: None,
            num_gpu: None,
            num_thread: None,
            repeat_last_n: None,
            repeat_penalty: None,
            tfs_z: None,
            system: None,
            template: None,
            keep_alive: None,
        }
    }
}

impl ProviderConfig for OllamaConfig {
    fn validate(&self) -> Result<(), String> {
        // Ollama doesn't require API key for local usage
        // Validation can be relaxed compared to cloud providers

        // Validate timeout
        if self.timeout == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }

        // Validate mirostat value if set
        if let Some(mirostat) = self.mirostat
            && !(0..=2).contains(&mirostat)
        {
            return Err("Mirostat must be 0, 1, or 2".to_string());
        }

        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.api_base.as_deref()
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

impl OllamaConfig {
    /// Get API key with environment variable fallback
    pub fn get_api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| std::env::var("OLLAMA_API_KEY").ok())
    }

    /// Get API base with environment variable fallback
    pub fn get_api_base(&self) -> String {
        self.api_base
            .clone()
            .or_else(|| std::env::var("OLLAMA_API_BASE").ok())
            .unwrap_or_else(|| "http://localhost:11434".to_string())
    }

    /// Get chat completions endpoint
    pub fn get_chat_endpoint(&self) -> String {
        format!("{}/api/chat", self.get_api_base())
    }

    /// Get generate endpoint (for text completion)
    pub fn get_generate_endpoint(&self) -> String {
        format!("{}/api/generate", self.get_api_base())
    }

    /// Get embeddings endpoint
    pub fn get_embeddings_endpoint(&self) -> String {
        format!("{}/api/embed", self.get_api_base())
    }

    /// Get tags endpoint (list models)
    pub fn get_tags_endpoint(&self) -> String {
        format!("{}/api/tags", self.get_api_base())
    }

    /// Get show endpoint (model info)
    pub fn get_show_endpoint(&self) -> String {
        format!("{}/api/show", self.get_api_base())
    }

    /// Build Ollama options object from config
    pub fn build_options(&self) -> serde_json::Value {
        let mut options = serde_json::Map::new();

        if let Some(mirostat) = self.mirostat {
            options.insert("mirostat".to_string(), serde_json::json!(mirostat));
        }
        if let Some(mirostat_eta) = self.mirostat_eta {
            options.insert("mirostat_eta".to_string(), serde_json::json!(mirostat_eta));
        }
        if let Some(mirostat_tau) = self.mirostat_tau {
            options.insert("mirostat_tau".to_string(), serde_json::json!(mirostat_tau));
        }
        if let Some(num_ctx) = self.num_ctx {
            options.insert("num_ctx".to_string(), serde_json::json!(num_ctx));
        }
        if let Some(num_gqa) = self.num_gqa {
            options.insert("num_gqa".to_string(), serde_json::json!(num_gqa));
        }
        if let Some(num_gpu) = self.num_gpu {
            options.insert("num_gpu".to_string(), serde_json::json!(num_gpu));
        }
        if let Some(num_thread) = self.num_thread {
            options.insert("num_thread".to_string(), serde_json::json!(num_thread));
        }
        if let Some(repeat_last_n) = self.repeat_last_n {
            options.insert(
                "repeat_last_n".to_string(),
                serde_json::json!(repeat_last_n),
            );
        }
        if let Some(repeat_penalty) = self.repeat_penalty {
            options.insert(
                "repeat_penalty".to_string(),
                serde_json::json!(repeat_penalty),
            );
        }
        if let Some(tfs_z) = self.tfs_z {
            options.insert("tfs_z".to_string(), serde_json::json!(tfs_z));
        }

        serde_json::Value::Object(options)
    }
}

fn default_timeout() -> u64 {
    120 // Ollama can be slow for initial model loading
}

fn default_max_retries() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config_default() {
        let config = OllamaConfig::default();
        assert!(config.api_key.is_none());
        assert!(config.api_base.is_none());
        assert_eq!(config.timeout, 120);
        assert_eq!(config.max_retries, 3);
        assert!(!config.debug);
    }

    #[test]
    fn test_ollama_config_get_api_base_default() {
        let config = OllamaConfig::default();
        assert_eq!(config.get_api_base(), "http://localhost:11434");
    }

    #[test]
    fn test_ollama_config_get_api_base_custom() {
        let config = OllamaConfig {
            api_base: Some("http://192.168.1.100:11434".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_api_base(), "http://192.168.1.100:11434");
    }

    #[test]
    fn test_ollama_config_get_api_key() {
        let config = OllamaConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_api_key(), Some("test-key".to_string()));
    }

    #[test]
    fn test_ollama_config_endpoints() {
        let config = OllamaConfig::default();
        assert_eq!(
            config.get_chat_endpoint(),
            "http://localhost:11434/api/chat"
        );
        assert_eq!(
            config.get_generate_endpoint(),
            "http://localhost:11434/api/generate"
        );
        assert_eq!(
            config.get_embeddings_endpoint(),
            "http://localhost:11434/api/embed"
        );
        assert_eq!(
            config.get_tags_endpoint(),
            "http://localhost:11434/api/tags"
        );
        assert_eq!(
            config.get_show_endpoint(),
            "http://localhost:11434/api/show"
        );
    }

    #[test]
    fn test_ollama_config_provider_config_trait() {
        let config = OllamaConfig {
            api_key: Some("test-key".to_string()),
            api_base: Some("http://custom:11434".to_string()),
            timeout: 60,
            max_retries: 5,
            ..Default::default()
        };

        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.api_base(), Some("http://custom:11434"));
        assert_eq!(config.timeout(), std::time::Duration::from_secs(60));
        assert_eq!(config.max_retries(), 5);
    }

    #[test]
    fn test_ollama_config_validation_ok() {
        let config = OllamaConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_ollama_config_validation_zero_timeout() {
        let config = OllamaConfig {
            timeout: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ollama_config_validation_invalid_mirostat() {
        let config = OllamaConfig {
            mirostat: Some(5),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ollama_config_build_options() {
        let config = OllamaConfig {
            mirostat: Some(1),
            mirostat_eta: Some(0.1),
            num_ctx: Some(4096),
            ..Default::default()
        };

        let options = config.build_options();
        assert_eq!(options["mirostat"], 1);
        assert!((options["mirostat_eta"].as_f64().unwrap() - 0.1).abs() < 0.001);
        assert_eq!(options["num_ctx"], 4096);
    }

    #[test]
    fn test_ollama_config_serialization() {
        let config = OllamaConfig {
            api_key: Some("test-key".to_string()),
            api_base: Some("http://custom:11434".to_string()),
            timeout: 45,
            max_retries: 2,
            debug: true,
            mirostat: Some(1),
            num_ctx: Some(8192),
            ..Default::default()
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["api_key"], "test-key");
        assert_eq!(json["api_base"], "http://custom:11434");
        assert_eq!(json["timeout"], 45);
        assert_eq!(json["mirostat"], 1);
        assert_eq!(json["num_ctx"], 8192);
    }

    #[test]
    fn test_ollama_config_deserialization() {
        let json = r#"{
            "api_base": "http://192.168.1.100:11434",
            "timeout": 60,
            "debug": true,
            "num_ctx": 4096
        }"#;

        let config: OllamaConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.api_base,
            Some("http://192.168.1.100:11434".to_string())
        );
        assert_eq!(config.timeout, 60);
        assert!(config.debug);
        assert_eq!(config.num_ctx, Some(4096));
    }
}
