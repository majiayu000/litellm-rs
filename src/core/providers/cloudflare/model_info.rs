//! Cloudflare Workers AI Model Information
//!
//! Model configurations for Cloudflare's Workers AI models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Cloudflare Workers AI model identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CloudflareModel {
    // Llama models
    Llama3_8B,
    Llama3_8BInstruct,
    Llama3_70B,
    Llama3_70BInstruct,
    Llama2_7B,
    Llama2_13B,

    // Mistral models
    Mistral7BInstruct,
    Mixtral8x7BInstruct,

    // Other open models
    Qwen15_7BChat,
    Deepseek1_5B,
    Phi2,
    Gemma7BIT,

    // Code models
    CodeLlama7B,
    DeepseekCoder6_7B,
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

    // Llama 3 models
    configs.insert(
        "@cf/meta/llama-3-8b-instruct",
        ModelInfo {
            model_id: "@cf/meta/llama-3-8b-instruct",
            display_name: "Llama 3 8B Instruct",
            max_context_length: 8192,
            max_output_length: Some(2048),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0, // Free on Cloudflare Workers
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "@cf/meta/llama-3-70b-instruct",
        ModelInfo {
            model_id: "@cf/meta/llama-3-70b-instruct",
            display_name: "Llama 3 70B Instruct",
            max_context_length: 8192,
            max_output_length: Some(2048),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "@cf/meta/llama-2-7b-chat-int8",
        ModelInfo {
            model_id: "@cf/meta/llama-2-7b-chat-int8",
            display_name: "Llama 2 7B Chat",
            max_context_length: 4096,
            max_output_length: Some(2048),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    // Mistral models
    configs.insert(
        "@cf/mistral/mistral-7b-instruct-v0.1",
        ModelInfo {
            model_id: "@cf/mistral/mistral-7b-instruct-v0.1",
            display_name: "Mistral 7B Instruct",
            max_context_length: 8192,
            max_output_length: Some(2048),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "@hf/thebloke/mixtral-8x7b-instruct-v0.1-awq",
        ModelInfo {
            model_id: "@hf/thebloke/mixtral-8x7b-instruct-v0.1-awq",
            display_name: "Mixtral 8x7B Instruct",
            max_context_length: 32768,
            max_output_length: Some(4096),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    // Qwen model
    configs.insert(
        "@cf/qwen/qwen1.5-7b-chat-awq",
        ModelInfo {
            model_id: "@cf/qwen/qwen1.5-7b-chat-awq",
            display_name: "Qwen 1.5 7B Chat",
            max_context_length: 32768,
            max_output_length: Some(4096),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    // Code models
    configs.insert(
        "@cf/meta/codellama-7b-instruct",
        ModelInfo {
            model_id: "@cf/meta/codellama-7b-instruct",
            display_name: "Code Llama 7B",
            max_context_length: 16384,
            max_output_length: Some(4096),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq",
        ModelInfo {
            model_id: "@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq",
            display_name: "DeepSeek Coder 6.7B",
            max_context_length: 16384,
            max_output_length: Some(4096),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    // Smaller models
    configs.insert(
        "@cf/microsoft/phi-2",
        ModelInfo {
            model_id: "@cf/microsoft/phi-2",
            display_name: "Phi-2",
            max_context_length: 2048,
            max_output_length: Some(1024),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "@cf/google/gemma-7b-it",
        ModelInfo {
            model_id: "@cf/google/gemma-7b-it",
            display_name: "Gemma 7B IT",
            max_context_length: 8192,
            max_output_length: Some(2048),
            supports_tools: false,
            supports_multimodal: false,
            supports_streaming: true,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    // First-party Workers AI model cards, checked 2026-10-01. The native
    // adapter currently forwards text chat only; do not advertise tools/vision.
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
                supports_tools: false,
                supports_multimodal: false,
                supports_streaming: false,
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
            CloudflareModel::Llama3_8B => "@cf/meta/llama-3-8b",
            CloudflareModel::Llama3_8BInstruct => "@cf/meta/llama-3-8b-instruct",
            CloudflareModel::Llama3_70B => "@cf/meta/llama-3-70b",
            CloudflareModel::Llama3_70BInstruct => "@cf/meta/llama-3-70b-instruct",
            CloudflareModel::Llama2_7B => "@cf/meta/llama-2-7b-chat-int8",
            CloudflareModel::Llama2_13B => "@cf/meta/llama-2-13b-chat",
            CloudflareModel::Mistral7BInstruct => "@cf/mistral/mistral-7b-instruct-v0.1",
            CloudflareModel::Mixtral8x7BInstruct => "@hf/thebloke/mixtral-8x7b-instruct-v0.1-awq",
            CloudflareModel::Qwen15_7BChat => "@cf/qwen/qwen1.5-7b-chat-awq",
            CloudflareModel::Deepseek1_5B => "@cf/deepseek-ai/deepseek-1.5b",
            CloudflareModel::Phi2 => "@cf/microsoft/phi-2",
            CloudflareModel::Gemma7BIT => "@cf/google/gemma-7b-it",
            CloudflareModel::CodeLlama7B => "@cf/meta/codellama-7b-instruct",
            CloudflareModel::DeepseekCoder6_7B => {
                "@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq"
            }
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

    // ==================== get_model_info Tests ====================

    #[test]
    fn test_model_info() {
        let info = get_model_info("@cf/meta/llama-3-8b-instruct").unwrap();
        assert_eq!(info.model_id, "@cf/meta/llama-3-8b-instruct");
        assert_eq!(info.max_context_length, 8192);
        assert!(info.supports_streaming);

        // Test with cloudflare/ prefix
        let info = get_model_info("cloudflare/@cf/meta/llama-3-8b-instruct").unwrap();
        assert_eq!(info.model_id, "@cf/meta/llama-3-8b-instruct");
    }

    #[test]
    fn test_model_info_llama3_70b() {
        let info = get_model_info("@cf/meta/llama-3-70b-instruct").unwrap();
        assert_eq!(info.display_name, "Llama 3 70B Instruct");
        assert_eq!(info.max_context_length, 8192);
        assert_eq!(info.max_output_length, Some(2048));
        assert!(!info.supports_tools);
        assert!(!info.supports_multimodal);
    }

    #[test]
    fn test_model_info_mistral() {
        let info = get_model_info("@cf/mistral/mistral-7b-instruct-v0.1").unwrap();
        assert_eq!(info.display_name, "Mistral 7B Instruct");
        assert_eq!(info.max_context_length, 8192);
        assert!(info.supports_streaming);
    }

    #[test]
    fn test_model_info_mixtral() {
        let info = get_model_info("@hf/thebloke/mixtral-8x7b-instruct-v0.1-awq").unwrap();
        assert_eq!(info.display_name, "Mixtral 8x7B Instruct");
        assert_eq!(info.max_context_length, 32768);
        assert_eq!(info.max_output_length, Some(4096));
    }

    #[test]
    fn test_model_info_qwen() {
        let info = get_model_info("@cf/qwen/qwen1.5-7b-chat-awq").unwrap();
        assert_eq!(info.display_name, "Qwen 1.5 7B Chat");
        assert_eq!(info.max_context_length, 32768);
    }

    #[test]
    fn test_model_info_codellama() {
        let info = get_model_info("@cf/meta/codellama-7b-instruct").unwrap();
        assert_eq!(info.display_name, "Code Llama 7B");
        assert_eq!(info.max_context_length, 16384);
        assert_eq!(info.max_output_length, Some(4096));
    }

    #[test]
    fn test_model_info_deepseek_coder() {
        let info = get_model_info("@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq").unwrap();
        assert_eq!(info.display_name, "DeepSeek Coder 6.7B");
        assert_eq!(info.max_context_length, 16384);
    }

    #[test]
    fn test_model_info_phi2() {
        let info = get_model_info("@cf/microsoft/phi-2").unwrap();
        assert_eq!(info.display_name, "Phi-2");
        assert_eq!(info.max_context_length, 2048);
        assert_eq!(info.max_output_length, Some(1024));
    }

    #[test]
    fn test_model_info_gemma() {
        let info = get_model_info("@cf/google/gemma-7b-it").unwrap();
        assert_eq!(info.display_name, "Gemma 7B IT");
        assert_eq!(info.max_context_length, 8192);
    }

    #[test]
    fn test_model_info_llama2() {
        let info = get_model_info("@cf/meta/llama-2-7b-chat-int8").unwrap();
        assert_eq!(info.display_name, "Llama 2 7B Chat");
        assert_eq!(info.max_context_length, 4096);
    }

    #[test]
    fn test_model_info_unknown() {
        assert!(get_model_info("unknown-model").is_none());
        assert!(get_model_info("").is_none());
    }

    #[test]
    fn test_model_info_with_prefix_stripped() {
        // Prefix should be stripped
        let with_prefix = get_model_info("cloudflare/@cf/meta/llama-3-8b-instruct");
        let without_prefix = get_model_info("@cf/meta/llama-3-8b-instruct");

        assert!(with_prefix.is_some());
        assert!(without_prefix.is_some());
        assert_eq!(
            with_prefix.unwrap().model_id,
            without_prefix.unwrap().model_id
        );
    }

    // ==================== get_available_models Tests ====================

    #[test]
    fn test_available_models() {
        let models = get_available_models();
        assert!(models.contains(&"@cf/meta/llama-3-8b-instruct"));
        assert!(models.contains(&"@cf/mistral/mistral-7b-instruct-v0.1"));
    }

    #[test]
    fn test_available_models_count() {
        let models = get_available_models();
        assert!(
            models.len() >= 10,
            "Expected at least 10 models, got {}",
            models.len()
        );
    }

    #[test]
    fn test_available_models_contains_code_models() {
        let models = get_available_models();
        assert!(models.contains(&"@cf/meta/codellama-7b-instruct"));
        assert!(models.contains(&"@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq"));
    }

    #[test]
    fn test_available_models_contains_llama_models() {
        let models = get_available_models();
        let llama_count = models.iter().filter(|m| m.contains("llama")).count();
        assert!(
            llama_count >= 3,
            "Expected at least 3 llama models, got {}",
            llama_count
        );
    }

    // ==================== calculate_cost Tests ====================

    #[test]
    fn test_cost_calculation() {
        // Cloudflare Workers AI is free
        let cost = calculate_cost("@cf/meta/llama-3-8b-instruct", 1000, 500).unwrap();
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_cost_calculation_large_tokens() {
        let cost = calculate_cost("@cf/meta/llama-3-70b-instruct", 100000, 50000).unwrap();
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_cost_calculation_zero_tokens() {
        let cost = calculate_cost("@cf/meta/llama-3-8b-instruct", 0, 0).unwrap();
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_cost_calculation_unknown_model() {
        let cost = calculate_cost("unknown-model", 1000, 500);
        assert!(cost.is_none());
    }

    #[test]
    fn test_cost_calculation_with_prefix() {
        let cost = calculate_cost("cloudflare/@cf/meta/llama-3-8b-instruct", 1000, 500).unwrap();
        assert_eq!(cost, 0.0);
    }

    // ==================== CloudflareModel Enum Tests ====================

    #[test]
    fn test_cloudflare_model_llama3_8b_instruct() {
        assert_eq!(
            CloudflareModel::Llama3_8BInstruct.model_id(),
            "@cf/meta/llama-3-8b-instruct"
        );
    }

    #[test]
    fn test_cloudflare_model_llama3_70b_instruct() {
        assert_eq!(
            CloudflareModel::Llama3_70BInstruct.model_id(),
            "@cf/meta/llama-3-70b-instruct"
        );
    }

    #[test]
    fn test_cloudflare_model_mistral() {
        assert_eq!(
            CloudflareModel::Mistral7BInstruct.model_id(),
            "@cf/mistral/mistral-7b-instruct-v0.1"
        );
    }

    #[test]
    fn test_cloudflare_model_mixtral() {
        assert_eq!(
            CloudflareModel::Mixtral8x7BInstruct.model_id(),
            "@hf/thebloke/mixtral-8x7b-instruct-v0.1-awq"
        );
    }

    #[test]
    fn test_cloudflare_model_codellama() {
        assert_eq!(
            CloudflareModel::CodeLlama7B.model_id(),
            "@cf/meta/codellama-7b-instruct"
        );
    }

    #[test]
    fn test_cloudflare_model_deepseek_coder() {
        assert_eq!(
            CloudflareModel::DeepseekCoder6_7B.model_id(),
            "@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq"
        );
    }

    #[test]
    fn test_cloudflare_model_phi2() {
        assert_eq!(CloudflareModel::Phi2.model_id(), "@cf/microsoft/phi-2");
    }

    #[test]
    fn test_cloudflare_model_gemma() {
        assert_eq!(
            CloudflareModel::Gemma7BIT.model_id(),
            "@cf/google/gemma-7b-it"
        );
    }

    #[test]
    fn test_cloudflare_model_qwen() {
        assert_eq!(
            CloudflareModel::Qwen15_7BChat.model_id(),
            "@cf/qwen/qwen1.5-7b-chat-awq"
        );
    }

    #[test]
    fn test_cloudflare_model_llama2() {
        assert_eq!(
            CloudflareModel::Llama2_7B.model_id(),
            "@cf/meta/llama-2-7b-chat-int8"
        );
    }

    // ==================== CloudflareModel::info() Tests ====================

    #[test]
    fn test_cloudflare_model_info_exists() {
        // Models with configs in MODEL_CONFIGS
        assert!(CloudflareModel::Llama3_8BInstruct.info().is_some());
        assert!(CloudflareModel::Llama3_70BInstruct.info().is_some());
        assert!(CloudflareModel::Mistral7BInstruct.info().is_some());
        assert!(CloudflareModel::Phi2.info().is_some());
    }

    #[test]
    fn test_cloudflare_model_info_not_in_configs() {
        // Llama3_8B (without Instruct) is not in MODEL_CONFIGS
        assert!(CloudflareModel::Llama3_8B.info().is_none());
        assert!(CloudflareModel::Llama3_70B.info().is_none());
    }

    #[test]
    fn test_cloudflare_model_info_content() {
        let info = CloudflareModel::Llama3_8BInstruct.info().unwrap();
        assert_eq!(info.model_id, "@cf/meta/llama-3-8b-instruct");
        assert_eq!(info.display_name, "Llama 3 8B Instruct");
        assert_eq!(info.max_context_length, 8192);
    }

    // ==================== ModelInfo Struct Tests ====================

    #[test]
    fn current_model_cost_uses_published_rates() {
        let id = "@cf/zai-org/glm-5.3";
        let cost = calculate_cost(id, 1_000_000, 1_000_000).unwrap();
        assert!((cost - 5.8).abs() < 1e-12);
        assert_eq!(get_model_info(id).unwrap().max_output_length, None);
    }

    #[test]
    fn current_catalog_does_not_advertise_unimplemented_streaming() {
        for id in [
            "@cf/zai-org/glm-5.3",
            "@cf/zai-org/glm-5.3-flash",
            "@cf/openai/gpt-oss-120b",
        ] {
            assert!(!get_model_info(id).unwrap().supports_streaming);
        }
    }

    #[test]
    fn test_model_info_no_tools_support() {
        // Currently no Cloudflare models support tools
        let models = get_available_models();
        for model_id in models {
            if let Some(info) = get_model_info(model_id) {
                assert!(
                    !info.supports_tools,
                    "Model {} shouldn't support tools yet",
                    model_id
                );
            }
        }
    }

    #[test]
    fn test_model_info_no_vision_support() {
        // Currently no Cloudflare models support vision
        let models = get_available_models();
        for model_id in models {
            if let Some(info) = get_model_info(model_id) {
                assert!(
                    !info.supports_multimodal,
                    "Model {} shouldn't support vision yet",
                    model_id
                );
            }
        }
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_cloudflare_model_serialize() {
        let model = CloudflareModel::Llama3_8BInstruct;
        let serialized = serde_json::to_string(&model).unwrap();
        assert!(serialized.contains("Llama3_8BInstruct"));
    }

    #[test]
    fn test_cloudflare_model_deserialize() {
        let model: CloudflareModel = serde_json::from_str("\"Llama3_8BInstruct\"").unwrap();
        assert_eq!(model, CloudflareModel::Llama3_8BInstruct);
    }

    #[test]
    fn test_cloudflare_model_roundtrip() {
        let original = CloudflareModel::Mixtral8x7BInstruct;
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: CloudflareModel = serde_json::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }

    // ==================== Clone/Debug/PartialEq Tests ====================

    #[test]
    fn test_cloudflare_model_clone() {
        let model = CloudflareModel::Mistral7BInstruct;
        let cloned = model;
        assert_eq!(model, cloned);
    }

    #[test]
    fn test_cloudflare_model_debug() {
        let model = CloudflareModel::Phi2;
        let debug_str = format!("{:?}", model);
        assert_eq!(debug_str, "Phi2");
    }

    #[test]
    fn test_cloudflare_model_eq() {
        assert_eq!(CloudflareModel::Gemma7BIT, CloudflareModel::Gemma7BIT);
        assert_ne!(CloudflareModel::Gemma7BIT, CloudflareModel::Phi2);
    }

    #[test]
    fn test_cloudflare_model_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(CloudflareModel::Llama3_8BInstruct);
        set.insert(CloudflareModel::Llama3_70BInstruct);

        assert!(set.contains(&CloudflareModel::Llama3_8BInstruct));
        assert!(!set.contains(&CloudflareModel::Phi2));
    }
}
