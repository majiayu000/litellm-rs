//! Module
//!
//! SDK client-side configuration models.
//! Canonical gateway/server runtime config models are under `crate::config::models::*`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientConfig {
    /// Default
    pub default_provider: Option<String>,
    /// Configuration
    pub providers: Vec<SdkProviderConfig>,
    /// Settings
    pub settings: ClientSettings,
}

/// Settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSettings {
    /// Request
    pub timeout: u64,
    /// Number of retries
    pub max_retries: u32,
    /// Request
    pub max_concurrent_requests: u32,
    /// Request
    pub enable_logging: bool,
    /// Enable metrics collection
    pub enable_metrics: bool,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            timeout: 30,
            max_retries: 3,
            max_concurrent_requests: 100,
            enable_logging: true,
            enable_metrics: true,
        }
    }
}

/// SDK client-side provider configuration.
///
/// This is the client-facing provider config used by the SDK.
/// For the gateway/server-side YAML config model, see
/// [`crate::config::models::provider::ProviderConfig`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProviderConfig {
    /// Provider unique ID
    pub id: String,
    /// Provider type
    pub provider_type: ProviderType,
    /// Display name
    pub name: String,
    /// API key
    pub api_key: String,
    /// Base URL (optional)
    pub base_url: Option<String>,
    /// Model
    pub models: Vec<String>,
    /// Enabled status
    pub enabled: bool,
    /// Weight (for load balancing)
    pub weight: f32,
    /// Request
    pub rate_limit_rpm: Option<u32>,
    /// Token limit per minute
    pub rate_limit_tpm: Option<u32>,
    /// Settings
    pub settings: HashMap<String, serde_json::Value>,
}

/// Canonical alias for SDK client configuration.
pub type ClientRuntimeConfig = ClientConfig;
/// Canonical alias for SDK provider configuration.
pub type ClientProviderConfig = SdkProviderConfig;
/// Backward-compatible alias.
pub type ProviderConfig = SdkProviderConfig;

/// Provider type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// OpenAI provider (GPT models)
    OpenAI,
    /// Anthropic provider (Claude models)
    Anthropic,
    /// Azure OpenAI provider
    Azure,
    /// Google provider (PaLM, Gemini models)
    Google,
    /// Cohere provider
    Cohere,
    /// Hugging Face provider
    HuggingFace,
    /// Ollama provider (local models)
    Ollama,
    /// AWS Bedrock provider
    AwsBedrock,
    /// Google Vertex AI provider
    GoogleVertex,
    /// Mistral provider
    Mistral,
    /// Custom provider with specified name
    Custom(String),
}

impl From<&str> for ProviderType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => ProviderType::OpenAI,
            "anthropic" => ProviderType::Anthropic,
            "azure" => ProviderType::Azure,
            "google" => ProviderType::Google,
            "cohere" => ProviderType::Cohere,
            "huggingface" => ProviderType::HuggingFace,
            "ollama" => ProviderType::Ollama,
            "aws_bedrock" => ProviderType::AwsBedrock,
            "google_vertex" => ProviderType::GoogleVertex,
            "mistral" => ProviderType::Mistral,
            _ => ProviderType::Custom(s.to_string()),
        }
    }
}

/// Configuration
pub struct SdkConfigBuilder {
    config: ClientConfig,
}

impl SdkConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }

    /// Default
    pub fn default_provider(mut self, provider_id: &str) -> Self {
        self.config.default_provider = Some(provider_id.to_string());
        self
    }

    /// Add provider
    pub fn add_provider(mut self, provider: SdkProviderConfig) -> Self {
        self.config.providers.push(provider);
        self
    }

    /// Add OpenAI provider
    pub fn add_openai(self, id: &str, api_key: &str) -> Self {
        self.add_provider(SdkProviderConfig {
            id: id.to_string(),
            provider_type: ProviderType::OpenAI,
            name: "OpenAI".to_string(),
            api_key: api_key.to_string(),
            base_url: None,
            models: vec![
                "gpt-5.4".to_string(),
                "gpt-5.5".to_string(),
                "gpt-5.5-pro".to_string(),
                "gpt-5.4-mini".to_string(),
                "gpt-5.4-nano".to_string(),
            ],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(3000),
            rate_limit_tpm: Some(250000),
            settings: HashMap::new(),
        })
    }

    /// Add Anthropic provider
    pub fn add_anthropic(self, id: &str, api_key: &str) -> Self {
        self.add_provider(SdkProviderConfig {
            id: id.to_string(),
            provider_type: ProviderType::Anthropic,
            name: "Anthropic".to_string(),
            api_key: api_key.to_string(),
            base_url: None,
            models: vec![
                "claude-opus-4-7".to_string(),
                "claude-sonnet-4-6".to_string(),
                "claude-haiku-4-5-20251001".to_string(),
                "claude-3-5-haiku-20241022".to_string(),
                "claude-haiku-4-5".to_string(),
                "claude-haiku-4-5-20251001".to_string(),
            ],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(100000),
            settings: HashMap::new(),
        })
    }

    /// Settings
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.config.settings.timeout = timeout;
        self
    }

    /// Settings
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config.settings.max_retries = retries;
        self
    }

    /// Configuration
    pub fn build(self) -> ClientConfig {
        self.config
    }
}

impl Default for SdkConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Backward-compatible alias.
pub type ConfigBuilder = SdkConfigBuilder;

/// Configuration
impl ClientConfig {
    /// Configuration
    pub fn from_env() -> crate::sdk::errors::Result<Self> {
        let mut builder = SdkConfigBuilder::new();

        // Configuration
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            builder = builder.add_openai("openai", &api_key);
        }

        // Configuration
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            builder = builder.add_anthropic("anthropic", &api_key);
        }

        let config = builder.build();

        if config.providers.is_empty() {
            return Err(crate::sdk::errors::SDKError::ConfigError(
                "No providers configured. Please set OPENAI_API_KEY or ANTHROPIC_API_KEY environment variables.".to_string()
            ));
        }

        Ok(config)
    }

    /// Configuration
    pub fn from_file(path: &str) -> crate::sdk::errors::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::sdk::errors::SDKError::ConfigError(format!(
                "Failed to read config file {}: {}",
                path, e
            ))
        })?;

        serde_yml::from_str(&content).map_err(|e| {
            crate::sdk::errors::SDKError::ConfigError(format!(
                "Failed to parse config file {}: {}",
                path, e
            ))
        })
    }
}

// ==================== Unit Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ClientConfig Tests ====================

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert!(config.default_provider.is_none());
        assert!(config.providers.is_empty());
        assert_eq!(config.settings.timeout, 30);
    }

    #[test]
    fn test_client_config_clone() {
        let config = ClientConfig {
            default_provider: Some("openai".to_string()),
            providers: vec![],
            settings: ClientSettings::default(),
        };
        let cloned = config.clone();
        assert_eq!(config.default_provider, cloned.default_provider);
    }

    #[test]
    fn test_client_config_serialization() {
        let config = ClientConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"settings\""));
        assert!(json.contains("\"providers\""));
    }

    #[test]
    fn test_client_config_deserialization() {
        let json = r#"{
            "default_provider": "openai",
            "providers": [],
            "settings": {
                "timeout": 60,
                "max_retries": 5,
                "max_concurrent_requests": 50,
                "enable_logging": false,
                "enable_metrics": true
            }
        }"#;
        let config: ClientConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.default_provider, Some("openai".to_string()));
        assert_eq!(config.settings.timeout, 60);
        assert_eq!(config.settings.max_retries, 5);
        assert!(!config.settings.enable_logging);
    }

    // ==================== ClientSettings Tests ====================

    #[test]
    fn test_client_settings_default() {
        let settings = ClientSettings::default();
        assert_eq!(settings.timeout, 30);
        assert_eq!(settings.max_retries, 3);
        assert_eq!(settings.max_concurrent_requests, 100);
        assert!(settings.enable_logging);
        assert!(settings.enable_metrics);
    }

    #[test]
    fn test_client_settings_clone() {
        let settings = ClientSettings {
            timeout: 60,
            max_retries: 5,
            max_concurrent_requests: 200,
            enable_logging: false,
            enable_metrics: false,
        };
        let cloned = settings.clone();
        assert_eq!(settings.timeout, cloned.timeout);
        assert_eq!(settings.max_retries, cloned.max_retries);
        assert_eq!(settings.enable_logging, cloned.enable_logging);
    }

    #[test]
    fn test_client_settings_serialization() {
        let settings = ClientSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"timeout\":30"));
        assert!(json.contains("\"max_retries\":3"));
    }

    // ==================== ProviderConfig Tests ====================

    #[test]
    fn test_provider_config_creation() {
        let config = ProviderConfig {
            id: "openai-1".to_string(),
            provider_type: ProviderType::OpenAI,
            name: "OpenAI Production".to_string(),
            api_key: "sk-test".to_string(),
            base_url: None,
            models: vec!["gpt-4".to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(3000),
            rate_limit_tpm: Some(250000),
            settings: HashMap::new(),
        };
        assert_eq!(config.id, "openai-1");
        assert!(config.enabled);
        assert_eq!(config.weight, 1.0);
    }

    #[test]
    fn test_provider_config_with_base_url() {
        let config = ProviderConfig {
            id: "custom".to_string(),
            provider_type: ProviderType::Custom("local".to_string()),
            name: "Local LLM".to_string(),
            api_key: "".to_string(),
            base_url: Some("http://localhost:8000".to_string()),
            models: vec!["llama-2".to_string()],
            enabled: true,
            weight: 0.5,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
            settings: HashMap::new(),
        };
        assert_eq!(config.base_url, Some("http://localhost:8000".to_string()));
    }

    #[test]
    fn test_provider_config_with_settings() {
        let mut settings = HashMap::new();
        settings.insert("temperature".to_string(), serde_json::json!(0.7));
        settings.insert("max_tokens".to_string(), serde_json::json!(1000));

        let config = ProviderConfig {
            id: "openai".to_string(),
            provider_type: ProviderType::OpenAI,
            name: "OpenAI".to_string(),
            api_key: "sk-test".to_string(),
            base_url: None,
            models: vec![],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
            settings,
        };
        assert_eq!(config.settings.len(), 2);
        assert_eq!(
            config.settings.get("temperature").unwrap(),
            &serde_json::json!(0.7)
        );
    }

    #[test]
    fn test_provider_config_serialization() {
        let config = ProviderConfig {
            id: "test".to_string(),
            provider_type: ProviderType::OpenAI,
            name: "Test".to_string(),
            api_key: "key".to_string(),
            base_url: None,
            models: vec!["gpt-4".to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: None,
            settings: HashMap::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("\"enabled\":true"));
    }

    // ==================== ProviderType Tests ====================

    #[test]
    fn test_provider_type_from_str_known() {
        assert!(matches!(ProviderType::from("openai"), ProviderType::OpenAI));
        assert!(matches!(
            ProviderType::from("anthropic"),
            ProviderType::Anthropic
        ));
        assert!(matches!(ProviderType::from("azure"), ProviderType::Azure));
        assert!(matches!(ProviderType::from("google"), ProviderType::Google));
        assert!(matches!(ProviderType::from("cohere"), ProviderType::Cohere));
        assert!(matches!(
            ProviderType::from("huggingface"),
            ProviderType::HuggingFace
        ));
        assert!(matches!(ProviderType::from("ollama"), ProviderType::Ollama));
        assert!(matches!(
            ProviderType::from("aws_bedrock"),
            ProviderType::AwsBedrock
        ));
        assert!(matches!(
            ProviderType::from("google_vertex"),
            ProviderType::GoogleVertex
        ));
        assert!(matches!(
            ProviderType::from("mistral"),
            ProviderType::Mistral
        ));
    }

    #[test]
    fn test_provider_type_from_str_case_insensitive() {
        assert!(matches!(ProviderType::from("OpenAI"), ProviderType::OpenAI));
        assert!(matches!(ProviderType::from("OPENAI"), ProviderType::OpenAI));
        assert!(matches!(
            ProviderType::from("Anthropic"),
            ProviderType::Anthropic
        ));
        assert!(matches!(ProviderType::from("AZURE"), ProviderType::Azure));
    }

    #[test]
    fn test_provider_type_from_str_custom() {
        let custom = ProviderType::from("my-custom-provider");
        assert!(matches!(custom, ProviderType::Custom(s) if s == "my-custom-provider"));
    }

    #[test]
    fn test_provider_type_clone() {
        let provider = ProviderType::OpenAI;
        let cloned = provider.clone();
        assert!(matches!(cloned, ProviderType::OpenAI));

        let custom = ProviderType::Custom("test".to_string());
        let custom_cloned = custom.clone();
        assert!(matches!(custom_cloned, ProviderType::Custom(s) if s == "test"));
    }

    #[test]
    fn test_provider_type_serialization() {
        let openai = ProviderType::OpenAI;
        let json = serde_json::to_string(&openai).unwrap();
        assert_eq!(json, "\"open_a_i\"");

        let anthropic = ProviderType::Anthropic;
        let json = serde_json::to_string(&anthropic).unwrap();
        assert_eq!(json, "\"anthropic\"");
    }

    #[test]
    fn test_provider_type_deserialization() {
        let openai: ProviderType = serde_json::from_str("\"open_a_i\"").unwrap();
        assert!(matches!(openai, ProviderType::OpenAI));

        let anthropic: ProviderType = serde_json::from_str("\"anthropic\"").unwrap();
        assert!(matches!(anthropic, ProviderType::Anthropic));
    }

    // ==================== ConfigBuilder Tests ====================

    #[test]
    fn test_config_builder_new() {
        let builder = ConfigBuilder::new();
        let config = builder.build();
        assert!(config.default_provider.is_none());
        assert!(config.providers.is_empty());
    }

    #[test]
    fn test_config_builder_default() {
        let builder = ConfigBuilder::default();
        let config = builder.build();
        assert!(config.providers.is_empty());
    }

    #[test]
    fn test_config_builder_default_provider() {
        let config = ConfigBuilder::new().default_provider("openai").build();
        assert_eq!(config.default_provider, Some("openai".to_string()));
    }

    #[test]
    fn test_config_builder_add_provider() {
        let provider = ProviderConfig {
            id: "test".to_string(),
            provider_type: ProviderType::OpenAI,
            name: "Test".to_string(),
            api_key: "key".to_string(),
            base_url: None,
            models: vec![],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
            settings: HashMap::new(),
        };
        let config = ConfigBuilder::new().add_provider(provider).build();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].id, "test");
    }

    #[test]
    fn test_config_builder_add_openai() {
        let config = ConfigBuilder::new()
            .add_openai("openai-prod", "sk-test-key")
            .build();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].id, "openai-prod");
        assert_eq!(config.providers[0].api_key, "sk-test-key");
        assert!(matches!(
            config.providers[0].provider_type,
            ProviderType::OpenAI
        ));
        assert!(!config.providers[0].models.is_empty());
        assert!(
            config.providers[0]
                .models
                .iter()
                .any(|model| model.starts_with("gpt-"))
        );
        assert_eq!(
            config.providers[0].models.first(),
            Some(&"gpt-5.4".to_string())
        );
        assert!(config.providers[0].models.contains(&"gpt-5.4".to_string()));
    }

    #[test]
    fn test_config_builder_add_anthropic() {
        let config = ConfigBuilder::new()
            .add_anthropic("anthropic-prod", "sk-ant-test")
            .build();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].id, "anthropic-prod");
        assert!(matches!(
            config.providers[0].provider_type,
            ProviderType::Anthropic
        ));
        assert!(
            config.providers[0]
                .models
                .iter()
                .any(|m| m == "claude-3-5-haiku-20241022")
        );
        assert!(
            config.providers[0]
                .models
                .iter()
                .any(|m| m == "claude-haiku-4-5")
        );
        assert!(
            config.providers[0]
                .models
                .iter()
                .any(|m| m == "claude-haiku-4-5-20251001")
        );
    }

    #[test]
    fn test_config_builder_timeout() {
        let config = ConfigBuilder::new().timeout(120).build();
        assert_eq!(config.settings.timeout, 120);
    }

    #[test]
    fn test_config_builder_max_retries() {
        let config = ConfigBuilder::new().max_retries(5).build();
        assert_eq!(config.settings.max_retries, 5);
    }

    #[test]
    fn test_config_builder_chaining() {
        let config = ConfigBuilder::new()
            .default_provider("openai")
            .add_openai("openai", "sk-key1")
            .add_anthropic("anthropic", "sk-ant-key")
            .timeout(60)
            .max_retries(5)
            .build();

        assert_eq!(config.default_provider, Some("openai".to_string()));
        assert_eq!(config.providers.len(), 2);
        assert_eq!(config.settings.timeout, 60);
        assert_eq!(config.settings.max_retries, 5);
    }

    #[test]
    fn test_config_builder_multiple_providers() {
        let config = ConfigBuilder::new()
            .add_openai("openai-1", "key1")
            .add_openai("openai-2", "key2")
            .add_anthropic("anthropic-1", "ant-key")
            .build();

        assert_eq!(config.providers.len(), 3);
    }

    // ==================== ClientConfig Methods Tests ====================

    #[test]
    fn test_client_config_from_file_not_found() {
        let result = ClientConfig::from_file("/nonexistent/path/config.yaml");
        assert!(result.is_err());
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_full_config_roundtrip() {
        let config = ConfigBuilder::new()
            .default_provider("openai")
            .add_openai("openai", "sk-test")
            .timeout(45)
            .build();

        // Serialize to JSON
        let json = serde_json::to_string(&config).unwrap();

        // Deserialize back
        let restored: ClientConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.default_provider, restored.default_provider);
        assert_eq!(config.providers.len(), restored.providers.len());
        assert_eq!(config.settings.timeout, restored.settings.timeout);
    }

    #[test]
    fn test_yaml_serialization() {
        let config = ConfigBuilder::new().add_openai("openai", "sk-test").build();

        let yaml = serde_yml::to_string(&config).unwrap();
        assert!(yaml.contains("providers"));
        assert!(yaml.contains("settings"));

        let restored: ClientConfig = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(config.providers.len(), restored.providers.len());
    }
}
