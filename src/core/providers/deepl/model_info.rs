//! DeepL Model Information

use crate::core::types::{model::ModelInfo, model::ProviderCapability};

pub fn get_supported_models() -> Vec<ModelInfo> {
    vec![ModelInfo {
        id: "deepl-translate".to_string(),
        name: "DeepL Translate".to_string(),
        provider: "deepl".to_string(),
        max_context_length: 50000, // DeepL supports up to 50KB of text per request
        max_output_length: Some(50000),
        input_cost_per_1k_tokens: Some(0.00002), // Approximate cost based on DeepL pricing
        output_cost_per_1k_tokens: Some(0.00002),
        supports_streaming: false,
        supports_tools: false,
        supports_multimodal: false,
        capabilities: vec![ProviderCapability::ChatCompletion],
        currency: "USD".to_string(),
        ..Default::default()
    }]
}

pub fn is_model_supported(model_id: &str) -> bool {
    get_supported_models().iter().any(|m| m.id == model_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_models() {
        let models = get_supported_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "deepl-translate"));
    }

    #[test]
    fn test_is_model_supported() {
        assert!(is_model_supported("deepl-translate"));
        assert!(!is_model_supported("unknown-model"));
    }

    #[test]
    fn test_model_capabilities() {
        let models = get_supported_models();
        let translate_model = models.iter().find(|m| m.id == "deepl-translate").unwrap();
        assert!(
            translate_model
                .capabilities
                .contains(&ProviderCapability::ChatCompletion)
        );
        assert!(
            !translate_model
                .capabilities
                .contains(&ProviderCapability::AudioTranslation)
        );
    }
}
