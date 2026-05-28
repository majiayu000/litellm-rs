//! AWS Bedrock Provider
//!
//! AWS Bedrock provider for accessing foundation models from AWS.
//! This implementation uses the base infrastructure for HTTP operations
//! and includes AWS SigV4 authentication.

// Core modules
mod client;
mod config;
mod error;
mod model_config;
mod model_id;
mod parameter_policy;
mod provider;
mod sigv4;
mod transformation;
mod utils;

#[cfg(test)]
mod provider_tests;

// Feature modules
pub mod agents;
pub mod batch;
pub mod chat;
pub mod embeddings;
pub mod guardrails;
pub mod images;
pub mod knowledge_bases;
pub mod streaming;

// Re-export main types for external use
pub use client::BedrockClient;
pub use config::BedrockConfig;
pub use error::BedrockErrorMapper;
pub use model_config::{
    BedrockApiType, BedrockModelFamily, ModelConfig, get_all_model_ids, get_model_config,
    model_supports_capability,
};
pub use model_id::{
    BedrockModelIdKind, ParsedBedrockModelId, get_model_config_for_model_id, parse_bedrock_model_id,
};
pub use provider::BedrockProvider;
pub use sigv4::SigV4Signer;
pub use utils::{
    AWS_REGIONS, AwsAuth, AwsCredentials, CostCalculator, ModelPricing,
    is_model_available_in_region, normalize_bedrock_model_id, validate_region,
};

// Re-export feature modules
pub use chat::{route_chat_request, supports_converse, supports_streaming};
pub use embeddings::execute_embedding;
pub use guardrails::{GuardrailClient, GuardrailConfig, GuardrailSource};
pub use streaming::BedrockStream;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_routing() {
        // Test Claude model routing
        let claude_config = get_model_config("anthropic.claude-3-opus-20240229").unwrap();
        assert_eq!(claude_config.family, BedrockModelFamily::Claude);
        assert_eq!(claude_config.api_type, BedrockApiType::Converse);
        assert!(claude_config.supports_streaming);
        assert!(claude_config.supports_function_calling);

        // Test Titan model routing
        let titan_config = get_model_config("amazon.titan-text-express-v1").unwrap();
        assert_eq!(titan_config.family, BedrockModelFamily::TitanText);
        assert_eq!(titan_config.api_type, BedrockApiType::Invoke);
        assert!(titan_config.supports_streaming);
        assert!(!titan_config.supports_function_calling);

        // Test Nova model routing
        let nova_config = get_model_config("amazon.nova-pro-v1:0").unwrap();
        assert_eq!(nova_config.family, BedrockModelFamily::Nova);
        assert_eq!(nova_config.api_type, BedrockApiType::Converse);
        assert!(nova_config.supports_streaming);
        assert!(nova_config.supports_function_calling);

        // Test cost calculation
        assert_eq!(claude_config.input_cost_per_1k, 0.015);
        assert_eq!(claude_config.output_cost_per_1k, 0.075);
    }

    #[test]
    fn test_model_capabilities() {
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
}
