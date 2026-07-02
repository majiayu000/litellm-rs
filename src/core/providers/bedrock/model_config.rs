//! Model Configuration for Bedrock Models
//!
//! Defines model families, capabilities, and routing configuration
//! for all supported Bedrock models.

use super::catalog::all_entries;
use crate::core::providers::unified_provider::ProviderError;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Bedrock model families
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BedrockModelFamily {
    Claude,
    TitanText,
    TitanEmbedding,
    TitanImage,
    Nova,
    Llama,
    Mistral,
    AI21,
    Cohere,
    DeepSeek,
    StabilityAI,
}

/// Bedrock API types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BedrockApiType {
    Invoke,
    Converse,
    InvokeStream,
    ConverseStream,
}

/// Model configuration for routing and capabilities
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub family: BedrockModelFamily,
    pub api_type: BedrockApiType,
    pub supports_streaming: bool,
    pub supports_function_calling: bool,
    pub supports_multimodal: bool,
    pub max_context_length: u32,
    pub max_output_length: Option<u32>,
    pub input_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
}

/// Model configuration database projected from the unified Bedrock catalog.
static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelConfig>> = LazyLock::new(|| {
    all_entries()
        .iter()
        .map(|entry| (entry.model_id, entry.to_model_config()))
        .collect()
});

/// Get model configuration for a specific model ID
pub fn get_model_config(model_id: &str) -> Result<&'static ModelConfig, ProviderError> {
    MODEL_CONFIGS.get(model_id).ok_or_else(|| {
        ProviderError::model_not_found("bedrock", format!("Model {} not supported", model_id))
    })
}

/// Check if a model supports a specific capability
pub fn model_supports_capability(model_id: &str, capability: &str) -> bool {
    if let Ok(config) = get_model_config(model_id) {
        match capability {
            "streaming" => config.supports_streaming,
            "function_calling" => config.supports_function_calling,
            "multimodal" => config.supports_multimodal,
            _ => false,
        }
    } else {
        false
    }
}

/// Get all supported model IDs
pub fn get_all_model_ids() -> Vec<&'static str> {
    MODEL_CONFIGS.keys().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_lookup() {
        let config = get_model_config("anthropic.claude-opus-4-6-v1:0").unwrap();
        assert_eq!(config.family, BedrockModelFamily::Claude);
        assert_eq!(config.api_type, BedrockApiType::Converse);

        let config = get_model_config("anthropic.claude-3-opus-20240229").unwrap();
        assert_eq!(config.family, BedrockModelFamily::Claude);
        assert_eq!(config.api_type, BedrockApiType::Converse);
        assert!(config.supports_streaming);
        assert!(config.supports_function_calling);
        assert!(config.supports_multimodal);

        let sonnet_v2 = get_model_config("anthropic.claude-3-5-sonnet-20241022-v2:0").unwrap();
        assert_eq!(sonnet_v2.family, BedrockModelFamily::Claude);
        assert_eq!(sonnet_v2.api_type, BedrockApiType::Converse);
    }

    #[test]
    fn test_model_capabilities() {
        assert!(model_supports_capability(
            "anthropic.claude-opus-4-6-v1:0",
            "streaming"
        ));
        assert!(model_supports_capability(
            "anthropic.claude-opus-4-6-v1:0",
            "function_calling"
        ));
        assert!(model_supports_capability(
            "anthropic.claude-opus-4-6-v1:0",
            "multimodal"
        ));

        assert!(model_supports_capability(
            "anthropic.claude-3-opus-20240229",
            "streaming"
        ));
        assert!(model_supports_capability(
            "anthropic.claude-3-opus-20240229",
            "function_calling"
        ));
        assert!(model_supports_capability(
            "anthropic.claude-3-opus-20240229",
            "multimodal"
        ));

        assert!(!model_supports_capability(
            "amazon.titan-text-express-v1",
            "function_calling"
        ));
        assert!(!model_supports_capability(
            "amazon.titan-text-express-v1",
            "multimodal"
        ));
    }

    #[test]
    fn test_unknown_model() {
        assert!(get_model_config("unknown-model").is_err());
        assert!(!model_supports_capability("unknown-model", "streaming"));
    }

    #[test]
    fn test_model_families() {
        let claude_config = get_model_config("anthropic.claude-3-opus-20240229").unwrap();
        assert_eq!(claude_config.family, BedrockModelFamily::Claude);

        let titan_config = get_model_config("amazon.titan-text-express-v1").unwrap();
        assert_eq!(titan_config.family, BedrockModelFamily::TitanText);

        let nova_config = get_model_config("amazon.nova-pro-v1:0").unwrap();
        assert_eq!(nova_config.family, BedrockModelFamily::Nova);
    }

    #[test]
    fn test_api_types() {
        let claude_config = get_model_config("anthropic.claude-3-opus-20240229").unwrap();
        assert_eq!(claude_config.api_type, BedrockApiType::Converse);

        let titan_config = get_model_config("amazon.titan-text-express-v1").unwrap();
        assert_eq!(titan_config.api_type, BedrockApiType::Invoke);
    }
}
