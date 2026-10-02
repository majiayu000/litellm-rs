//! Cloudflare Workers AI Model Information
//!
//! Model configurations for Cloudflare's Workers AI models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Cloudflare Workers AI model identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CloudflareModel {
    DeepSeekV4Flash,
    DeepSeekV4Pro,
    Gemma4,
    Glm53Flash,
    Glm53,
    GptOss120B,
    KimiK27Code,
    Qwen330BA3B,
}

/// Model configuration
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model ID as used in API
    pub model_id: &'static str,
    /// Display name
    pub display_name: &'static str,
    /// Maximum context length
    pub max_context_length: u32,
    /// Maximum output tokens
    pub max_output_length: Option<u32>,
    /// Whether the model supports tools/functions
    pub supports_tools: bool,
    /// Whether the model supports vision
    pub supports_multimodal: bool,
    /// Whether the model supports streaming
    pub supports_streaming: bool,
    /// Input cost per million tokens (in USD)
    pub input_cost_per_million: f64,
    /// Output cost per million tokens (in USD)
    pub output_cost_per_million: f64,
}

/// Static model configurations
static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelInfo>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    // First-party Workers AI model cards, checked 2026-10-03.
    // The OpenAI-compatible adapter preserves tools, image input and streaming.
    // Cards do not publish an independent output limit, so leave it unknown.
    for (id, name, context, input, output) in [
        (
            "@cf/deepseek-ai/deepseek-v4-flash-0731",
            "deepseek-v4-flash-0731",
            1048576,
            0.44,
            1.32,
        ),
        (
            "@cf/deepseek-ai/deepseek-v4-pro-0813",
            "deepseek-v4-pro-0813",
            1048576,
            1.32,
            3.96,
        ),
        (
            "@cf/google/gemma-4-26b-a4b-it",
            "gemma-4-26b-a4b-it",
            256000,
            0.1,
            0.3,
        ),
        (
            "@cf/zai-org/glm-5.3-flash",
            "glm-5.3-flash",
            1048576,
            0.15,
            0.5,
        ),
        ("@cf/zai-org/glm-5.3", "glm-5.3", 1048576, 1.4, 4.4),
        (
            "@cf/openai/gpt-oss-120b",
            "gpt-oss-120b",
            128000,
            0.35,
            0.75,
        ),
        (
            "@cf/moonshotai/kimi-k2.7-code",
            "kimi-k2.7-code",
            262144,
            0.95,
            4.0,
        ),
        (
            "@cf/qwen/qwen3-30b-a3b-fp8",
            "qwen3-30b-a3b-fp8",
            32768,
            0.0509,
            0.335,
        ),
    ] {
        configs.insert(
            id,
            ModelInfo {
                model_id: id,
                display_name: name,
                max_context_length: context,
                max_output_length: None,
                supports_tools: true,
                supports_multimodal: matches!(
                    id,
                    "@cf/google/gemma-4-26b-a4b-it"
                        | "@cf/zai-org/glm-5.3-flash"
                        | "@cf/moonshotai/kimi-k2.7-code"
                ),
                supports_streaming: true,
                input_cost_per_million: input,
                output_cost_per_million: output,
            },
        );
    }
    configs
});

/// Get model information by ID
pub fn get_model_info(model_id: &str) -> Option<&'static ModelInfo> {
    // Handle cloudflare/ prefix
    let model_id = model_id.strip_prefix("cloudflare/").unwrap_or(model_id);
    MODEL_CONFIGS.get(model_id)
}

/// Get all available model IDs
pub fn get_available_models() -> Vec<&'static str> {
    MODEL_CONFIGS.keys().copied().collect()
}

/// Estimate token cost using the model's published rates, before account allowances.
pub fn calculate_cost(model_id: &str, input_tokens: u32, output_tokens: u32) -> Option<f64> {
    get_model_info(model_id).map(|m| {
        (f64::from(input_tokens) * m.input_cost_per_million
            + f64::from(output_tokens) * m.output_cost_per_million)
            / 1_000_000.0
    })
}

impl CloudflareModel {
    /// Get the API model ID
    pub fn model_id(&self) -> &'static str {
        match self {
            Self::DeepSeekV4Flash => "@cf/deepseek-ai/deepseek-v4-flash-0731",
            Self::DeepSeekV4Pro => "@cf/deepseek-ai/deepseek-v4-pro-0813",
            Self::Gemma4 => "@cf/google/gemma-4-26b-a4b-it",
            Self::Glm53Flash => "@cf/zai-org/glm-5.3-flash",
            Self::Glm53 => "@cf/zai-org/glm-5.3",
            Self::GptOss120B => "@cf/openai/gpt-oss-120b",
            Self::KimiK27Code => "@cf/moonshotai/kimi-k2.7-code",
            Self::Qwen330BA3B => "@cf/qwen/qwen3-30b-a3b-fp8",
        }
    }

    /// Get model information
    pub fn info(&self) -> Option<&'static ModelInfo> {
        get_model_info(self.model_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_and_unverified_legacy_models_are_not_advertised() {
        // Retirement notice: https://developers.cloudflare.com/changelog/post/2026-05-08-planned-model-deprecations/
        for id in [
            "@cf/meta/llama-3-8b-instruct",
            "@cf/meta/llama-2-7b-chat-int8",
            "@cf/mistral/mistral-7b-instruct-v0.1",
            "@cf/microsoft/phi-2",
        ] {
            assert!(get_model_info(id).is_none());
            assert!(!get_available_models().contains(&id));
            assert!(calculate_cost(id, 1000, 500).is_none());
        }
        // These old zero-price entries have no card in the current public catalog.
        assert!(get_model_info("@cf/meta/llama-3-70b-instruct").is_none());
        assert!(get_model_info("@cf/google/gemma-7b-it").is_none());
    }

    #[test]
    fn current_model_enum_resolves_to_verified_cards_and_round_trips() {
        for model in [
            CloudflareModel::DeepSeekV4Flash,
            CloudflareModel::DeepSeekV4Pro,
            CloudflareModel::Gemma4,
            CloudflareModel::Glm53Flash,
            CloudflareModel::Glm53,
            CloudflareModel::GptOss120B,
            CloudflareModel::KimiK27Code,
            CloudflareModel::Qwen330BA3B,
        ] {
            let info = model.info().expect("current model has a card");
            assert_eq!(
                get_model_info(&format!("cloudflare/{}", model.model_id()))
                    .unwrap()
                    .model_id,
                info.model_id
            );
            assert!(info.supports_streaming && info.supports_tools);
            assert!(info.input_cost_per_million > 0.0);
            let json = serde_json::to_string(&model).unwrap();
            assert_eq!(
                serde_json::from_str::<CloudflareModel>(&json).unwrap(),
                model
            );
        }
    }

    #[test]
    fn prices_use_published_rates_before_account_allowances() {
        assert!(
            (calculate_cost(CloudflareModel::GptOss120B.model_id(), 1_000_000, 1_000_000).unwrap()
                - 1.1)
                .abs()
                < 1e-12
        );
        assert_eq!(
            calculate_cost(CloudflareModel::GptOss120B.model_id(), 0, 0),
            Some(0.0)
        );
        assert!(calculate_cost("unknown", 100, 50).is_none());
    }

    #[test]
    fn vision_and_limits_follow_model_cards() {
        assert!(CloudflareModel::Gemma4.info().unwrap().supports_multimodal);
        assert!(
            CloudflareModel::KimiK27Code
                .info()
                .unwrap()
                .supports_multimodal
        );
        assert!(
            !CloudflareModel::GptOss120B
                .info()
                .unwrap()
                .supports_multimodal
        );
        assert_eq!(
            CloudflareModel::Glm53.info().unwrap().max_context_length,
            1_048_576
        );
        assert_eq!(
            CloudflareModel::Glm53.info().unwrap().max_output_length,
            None
        );
    }
}
