//! Codestral Model Information

use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub model_id: &'static str,
    pub display_name: &'static str,
    pub max_context_length: u32,
    pub max_output_length: u32,
    pub supports_fim: bool,
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
}

static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelInfo>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    configs.insert(
        "codestral-latest",
        ModelInfo {
            model_id: "codestral-latest",
            display_name: "Codestral Latest",
            max_context_length: 256000,
            max_output_length: 256000,
            supports_fim: true,
            input_cost_per_million: 0.3,
            output_cost_per_million: 0.9,
        },
    );

    configs.insert(
        "codestral-2508",
        ModelInfo {
            model_id: "codestral-2508",
            display_name: "Codestral 2508",
            max_context_length: 256000,
            max_output_length: 256000,
            supports_fim: true,
            input_cost_per_million: 0.3,
            output_cost_per_million: 0.9,
        },
    );

    configs.insert(
        "codestral-2405",
        ModelInfo {
            model_id: "codestral-2405",
            display_name: "Codestral 2405 (legacy)",
            max_context_length: 32768,
            max_output_length: 32768,
            supports_fim: true,
            input_cost_per_million: 1.0,
            output_cost_per_million: 3.0,
        },
    );

    configs.insert(
        "codestral-mamba-latest",
        ModelInfo {
            model_id: "codestral-mamba-latest",
            display_name: "Codestral Mamba Latest",
            max_context_length: 256000,
            max_output_length: 256000,
            supports_fim: true,
            input_cost_per_million: 0.25,
            output_cost_per_million: 0.25,
        },
    );

    configs.insert(
        "codestral-mamba-2407",
        ModelInfo {
            model_id: "codestral-mamba-2407",
            display_name: "Codestral Mamba 2407",
            max_context_length: 256000,
            max_output_length: 256000,
            supports_fim: true,
            input_cost_per_million: 0.25,
            output_cost_per_million: 0.25,
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
        let info = get_model_info("codestral-latest");
        assert!(info.is_some());
        assert!(info.unwrap().supports_fim);
    }

    #[test]
    fn test_get_available_models() {
        let models = get_available_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"codestral-latest"));
    }
}
