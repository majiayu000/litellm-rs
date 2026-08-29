use super::{
    super::{GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec},
    advanced_text_capabilities,
};
use crate::core::types::model::ModelInfo;

pub(super) fn register(registry: &mut GeminiModelRegistry) {
    registry.register_model(
        "gemini-3.7-flash",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3.7-flash".to_string(),
                name: "Gemini 3.7 Flash".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_048_576,
                max_output_length: Some(65_536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: false,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: advanced_text_capabilities(),
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini37Flash,
            features: vec![
                ModelFeature::ToolCalling,
                ModelFeature::FunctionCalling,
                ModelFeature::StreamingSupport,
                ModelFeature::ContextCaching,
                ModelFeature::SystemInstructions,
                ModelFeature::BatchProcessing,
                ModelFeature::JsonMode,
                ModelFeature::CodeExecution,
                ModelFeature::SearchGrounding,
            ],
            pricing: Default::default(),
            limits: ModelLimits {
                max_context_length: 1_048_576,
                max_output_tokens: 65_536,
                max_images: None,
                max_video_seconds: None,
                max_audio_seconds: None,
                rpm_limit: None,
                tpm_limit: None,
            },
        },
    );

    #[cfg(test)]
    {
        let mut unpriced = registry.get_model_spec("gemini-3.7-flash").unwrap().clone();
        unpriced.model_info.id = "unpriced-static-fallback-test".to_string();
        unpriced.family = GeminiModelFamily::Gemini36Flash;
        registry.register_model("unpriced-static-fallback-test", unpriced);
    }
}
