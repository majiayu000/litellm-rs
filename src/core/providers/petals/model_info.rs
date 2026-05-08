//! Petals Model Information

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PetalsModel {
    Bloom_176B,
    Llama2_70B,
}

pub use crate::core::providers::base::ProviderModelEntry as ModelInfo;
static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelInfo>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    configs.insert(
        "bigscience/bloom-petals",
        ModelInfo {
            model_id: "bigscience/bloom-petals",
            display_name: "BLOOM 176B (Petals)",
            max_context_length: 2048,
            max_output_length: 1024,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "meta-llama/Llama-2-70b-hf",
        ModelInfo {
            model_id: "meta-llama/Llama-2-70b-hf",
            display_name: "Llama 2 70B (Petals)",
            max_context_length: 4096,
            max_output_length: 2048,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
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
