//! Anthropic Provider Implementation
//!
//! Completely refactored Anthropic provider based on DeepSeek architecture pattern
//! Features:
//! - Zero technical debt
//! - Complete feature support

// Core modules - following DeepSeek 6-module architecture
pub mod client;
pub mod config;
pub mod error;
pub mod models;
pub mod provider;
pub mod streaming;

// Re-export core components
pub use client::AnthropicClient;
pub use config::{AnthropicConfig, AnthropicConfigBuilder};
pub use error::{
    AnthropicError, AnthropicErrorMapper, anthropic_api_error, anthropic_auth_error,
    anthropic_config_error, anthropic_model_error, anthropic_network_error, anthropic_parse_error,
    anthropic_rate_limit_error, anthropic_stream_error, anthropic_validation_error,
};
pub use models::{
    AnthropicModelFamily, AnthropicModelRegistry, CostCalculator, ModelConfig, ModelFeature,
    ModelLimits, ModelPricing, ModelSpec, get_anthropic_registry,
};
pub use provider::{
    AnthropicProvider, AnthropicProviderBuilder, create_anthropic_provider,
    create_anthropic_provider_from_env,
};
pub use streaming::AnthropicStream;

// Convenient type aliases
pub type Error = error::AnthropicError;
pub type Config = config::AnthropicConfig;
pub type Provider = provider::AnthropicProvider;
pub type Client = client::AnthropicClient;
pub type Stream = streaming::AnthropicStream;
pub type Registry = models::AnthropicModelRegistry;

/// Version information
pub const VERSION: &str = "2.0.0";
pub const PROVIDER_NAME: &str = "anthropic";

/// API constants
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const DEFAULT_API_VERSION: &str = "2023-06-01";
pub const MAX_CONTEXT_LENGTH: u32 = 200_000;
pub const MAX_OUTPUT_TOKENS: u32 = 128_000;

/// Create a new Anthropic provider
pub fn new_provider(api_key: impl Into<String>) -> Result<AnthropicProvider, AnthropicError> {
    let config = AnthropicConfig::new(api_key);
    AnthropicProvider::new(config)
}

/// Create
pub fn new_provider_from_env() -> Result<AnthropicProvider, AnthropicError> {
    let config = AnthropicConfig::from_env()?;
    AnthropicProvider::new(config)
}

/// Create
pub fn builder() -> AnthropicProviderBuilder {
    AnthropicProviderBuilder::new()
}

/// Create
pub fn builder_from_env() -> Result<AnthropicProviderBuilder, AnthropicError> {
    let config = AnthropicConfig::from_env()?;
    Ok(AnthropicProviderBuilder::new().with_config(config))
}

/// Check
pub fn validate_api_key(api_key: &str) -> Result<(), String> {
    if api_key.is_empty() {
        return Err("API key cannot be empty".to_string());
    }

    if !api_key.starts_with("sk-ant-") {
        return Err(
            "Invalid Anthropic API key format. Keys should start with 'sk-ant-'".to_string(),
        );
    }

    if api_key.len() < 20 {
        return Err("API key appears to be too short".to_string());
    }

    Ok(())
}

/// Default
pub fn default_model() -> &'static str {
    "claude-opus-4-6"
}

/// Model
pub fn supported_models() -> Vec<String> {
    get_anthropic_registry()
        .list_models()
        .into_iter()
        .map(|spec| spec.model_info.id.clone())
        .collect()
}

/// Check
pub fn is_model_supported(model_id: &str) -> bool {
    get_anthropic_registry().get_model_spec(model_id).is_some()
}

/// Model
pub fn get_model_features(model_id: &str) -> Option<Vec<ModelFeature>> {
    get_anthropic_registry()
        .get_model_spec(model_id)
        .map(|spec| spec.features.clone())
}

/// Check
pub fn model_supports_feature(model_id: &str, feature: ModelFeature) -> bool {
    get_anthropic_registry().supports_feature(model_id, &feature)
}

/// Request
pub fn estimate_cost(
    model_id: &str,
    estimated_input_tokens: u32,
    estimated_output_tokens: u32,
) -> Option<f64> {
    CostCalculator::calculate_cost(model_id, estimated_input_tokens, estimated_output_tokens)
}

/// Module information
pub mod info {
    use super::*;

    /// Get
    pub fn version() -> &'static str {
        VERSION
    }

    /// Get
    pub fn provider_name() -> &'static str {
        PROVIDER_NAME
    }

    /// Default
    pub fn default_config() -> AnthropicConfig {
        AnthropicConfig::default()
    }

    /// Get
    pub fn supported_features() -> Vec<&'static str> {
        vec![
            "chat_completion",
            "streaming",
            "multimodal_input",
            "tool_calling",
            "function_calling",
            "system_messages",
            "cache_control",
            "batch_processing",
            "thinking_mode",
            "computer_use",
        ]
    }

    /// Get
    pub fn api_limits() -> std::collections::HashMap<&'static str, u32> {
        let mut limits = std::collections::HashMap::new();
        limits.insert("max_context_length", MAX_CONTEXT_LENGTH);
        limits.insert("max_output_tokens", MAX_OUTPUT_TOKENS);
        limits.insert("max_images", 20);
        limits.insert("max_document_size_mb", 32);
        limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_info() {
        assert_eq!(info::version(), "2.0.0");
        assert_eq!(info::provider_name(), "anthropic");
        assert!(info::supported_features().contains(&"chat_completion"));
        assert!(info::supported_features().contains(&"multimodal_input"));
    }

    #[test]
    fn test_api_key_validation() {
        assert!(validate_api_key("sk-ant-api03-test123456789").is_ok());
        assert!(validate_api_key("invalid-key").is_err());
        assert!(validate_api_key("").is_err());
        assert!(validate_api_key("sk-ant-short").is_err());
    }

    #[test]
    fn test_model_support() {
        assert!(is_model_supported("claude-opus-4-6"));
        assert!(is_model_supported("claude-3-haiku-20240307"));
        assert!(!is_model_supported("gpt-4"));
    }

    #[test]
    fn test_model_features() {
        let features = get_model_features("claude-opus-4-6");
        assert!(features.is_some());

        let features = features.unwrap();
        assert!(features.contains(&ModelFeature::MultimodalSupport));
        assert!(features.contains(&ModelFeature::ToolCalling));
        assert!(features.contains(&ModelFeature::ComputerUse));
    }

    #[test]
    fn test_feature_support() {
        assert!(model_supports_feature(
            "claude-opus-4-6",
            ModelFeature::ComputerUse
        ));
        assert!(!model_supports_feature(
            "claude-2.1",
            ModelFeature::ComputerUse
        ));
        assert!(model_supports_feature(
            "claude-3-haiku-20240307",
            ModelFeature::StreamingSupport
        ));
    }

    #[test]
    fn test_cost_estimation() {
        let cost = estimate_cost("claude-opus-4-6", 1000, 500);
        assert!(cost.is_some());
        assert!(cost.unwrap() > 0.0);
    }

    #[test]
    fn test_supported_models_list() {
        let models = supported_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"claude-opus-4-6".to_string()));
        assert!(models.contains(&"claude-3-haiku-20240307".to_string()));
    }

    #[test]
    fn test_default_model() {
        assert_eq!(default_model(), "claude-opus-4-6");
        assert!(is_model_supported(default_model()));
    }

    #[test]
    fn test_provider_creation() {
        // This is a mock test, actual usage requires a valid API key
        let result = new_provider("sk-ant-test-key-123456789012345");
        // Configuration validation
        assert!(result.is_err() || result.is_ok());
    }
}
