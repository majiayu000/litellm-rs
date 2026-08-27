use super::super::{
    AnthropicModelFamily, AnthropicModelRegistry, ModelConfig, ModelFeature, ModelLimits,
    ModelSpec, pricing_per_million,
};
use crate::core::types::model::{ModelInfo, ProviderCapability};

pub(super) fn register(registry: &mut AnthropicModelRegistry) {
    // Anthropic uses canonical dateless IDs for Claude 4.6 and later.
    // Sources:
    // https://platform.claude.com/docs/en/models/overview
    // https://platform.claude.com/docs/en/about-claude/pricing
    for (id, name, family, input, output, cache_write, cache_read) in [
        (
            "claude-fable-5",
            "Claude Fable 5",
            AnthropicModelFamily::ClaudeFable5,
            10.0,
            50.0,
            12.5,
            1.0,
        ),
        (
            "claude-opus-5",
            "Claude Opus 5",
            AnthropicModelFamily::ClaudeOpus5,
            5.0,
            25.0,
            6.25,
            0.5,
        ),
        (
            "claude-sonnet-5",
            "Claude Sonnet 5",
            AnthropicModelFamily::ClaudeSonnet5,
            2.0,
            10.0,
            2.5,
            0.2,
        ),
    ] {
        registry.register_model(
            id,
            claude_5_spec(id, name, family, input, output, cache_write, cache_read),
        );
    }
}

fn claude_5_spec(
    id: &str,
    name: &str,
    family: AnthropicModelFamily,
    input_price: f64,
    output_price: f64,
    cache_write_price: f64,
    cache_read_price: f64,
) -> ModelSpec {
    ModelSpec {
        model_info: ModelInfo {
            id: id.to_string(),
            name: name.to_string(),
            provider: "anthropic".to_string(),
            max_context_length: 1_000_000,
            max_output_length: Some(128_000),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            input_cost_per_1k_tokens: Some(input_price / 1000.0),
            output_cost_per_1k_tokens: Some(output_price / 1000.0),
            currency: "USD".to_string(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
                ProviderCapability::ToolCalling,
                ProviderCapability::FunctionCalling,
                ProviderCapability::BatchProcessing,
            ],
            created_at: None,
            updated_at: None,
            metadata: std::collections::HashMap::from([
                (
                    "supports_adaptive_thinking".to_string(),
                    serde_json::Value::Bool(true),
                ),
                (
                    "adaptive_thinking_always_on".to_string(),
                    serde_json::Value::Bool(id == "claude-fable-5"),
                ),
                (
                    "supports_manual_extended_thinking".to_string(),
                    serde_json::Value::Bool(false),
                ),
                (
                    "supports_sampling_params".to_string(),
                    serde_json::Value::Bool(false),
                ),
                (
                    "supports_assistant_prefill".to_string(),
                    serde_json::Value::Bool(false),
                ),
            ]),
        },
        family,
        features: vec![
            ModelFeature::MultimodalSupport,
            ModelFeature::ToolCalling,
            ModelFeature::FunctionCalling,
            ModelFeature::StreamingSupport,
            ModelFeature::CacheControl,
            ModelFeature::SystemMessages,
            ModelFeature::BatchProcessing,
            ModelFeature::ComputerUse,
        ],
        pricing: pricing_per_million(
            input_price,
            output_price,
            Some(cache_write_price),
            Some(cache_read_price),
            Some(0.5),
        ),
        limits: ModelLimits {
            max_context_length: 1_000_000,
            max_output_tokens: 128_000,
            max_images: Some(100),
            max_document_size_mb: Some(100),
        },
        config: ModelConfig::default(),
    }
}
