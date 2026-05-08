//! RAGFlow Model Information

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RagflowModel {
    Ragflow_Default,
}

pub use crate::core::providers::base::ProviderModelEntry as ModelInfo;
static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelInfo>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    configs.insert(
        "ragflow",
        ModelInfo {
            model_id: "ragflow",
            display_name: "RAGFlow Default",
            max_context_length: 8192,
            max_output_length: 4096,
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
