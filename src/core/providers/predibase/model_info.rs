//! Predibase Model Information

use std::collections::HashMap;
use std::sync::LazyLock;

pub use crate::core::providers::base::ProviderModelEntry as ModelInfo;
static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelInfo>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    configs.insert(
        "llama-3-8b-instruct",
        ModelInfo {
            model_id: "llama-3-8b-instruct",
            display_name: "Llama 3 8B Instruct",
            max_context_length: 8192,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_million: 0.20,
            output_cost_per_million: 0.20,
        },
    );

    configs.insert(
        "llama-3-70b-instruct",
        ModelInfo {
            model_id: "llama-3-70b-instruct",
            display_name: "Llama 3 70B Instruct",
            max_context_length: 8192,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_million: 1.20,
            output_cost_per_million: 1.20,
        },
    );

    configs.insert(
        "mistral-7b-instruct",
        ModelInfo {
            model_id: "mistral-7b-instruct",
            display_name: "Mistral 7B Instruct",
            max_context_length: 8192,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_million: 0.18,
            output_cost_per_million: 0.18,
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
        let info = get_model_info("llama-3-8b-instruct");
        assert!(info.is_some());
    }
}
