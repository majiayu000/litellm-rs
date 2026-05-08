//! NLP Cloud Model Information

use std::collections::HashMap;
use std::sync::LazyLock;

pub use crate::core::providers::base::ProviderModelEntry as ModelInfo;
static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelInfo>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    configs.insert(
        "finetuned-llama-3-70b",
        ModelInfo {
            model_id: "finetuned-llama-3-70b",
            display_name: "Fine-tuned Llama 3 70B",
            max_context_length: 8192,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_million: 1.00,
            output_cost_per_million: 1.50,
        },
    );

    configs.insert(
        "dolphin-mixtral-8x7b",
        ModelInfo {
            model_id: "dolphin-mixtral-8x7b",
            display_name: "Dolphin Mixtral 8x7B",
            max_context_length: 32768,
            max_output_length: 8192,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_million: 0.80,
            output_cost_per_million: 1.00,
        },
    );

    configs.insert(
        "chatdolphin",
        ModelInfo {
            model_id: "chatdolphin",
            display_name: "ChatDolphin",
            max_context_length: 16384,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_million: 0.50,
            output_cost_per_million: 0.70,
        },
    );

    configs
});

pub fn get_model_info(model_id: &str) -> Option<&'static ModelInfo> {
    MODEL_CONFIGS.get(model_id)
}

pub fn get_available_models() -> Vec<&'static str> {
    MODEL_CONFIGS.keys().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model_info() {
        let info = get_model_info("finetuned-llama-3-70b");
        assert!(info.is_some());
    }
}
