use super::{
    super::{GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec},
    advanced_text_capabilities, promotional_flash_pricing_metadata,
};
use crate::core::types::model::ModelInfo;

pub(super) fn register(registry: &mut GeminiModelRegistry) {
    registry.register_model(
        "gemini-3.6-flash",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3.6-flash".to_string(),
                name: "Gemini 3.6 Flash".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_048_576,
                max_output_length: Some(65_536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: advanced_text_capabilities(),
                created_at: None,
                updated_at: None,
                metadata: promotional_flash_pricing_metadata(),
            },
            family: GeminiModelFamily::Gemini36Flash,
            features: vec![
                ModelFeature::MultimodalSupport,
                ModelFeature::ToolCalling,
                ModelFeature::FunctionCalling,
                ModelFeature::StreamingSupport,
                ModelFeature::ContextCaching,
                ModelFeature::SystemInstructions,
                ModelFeature::BatchProcessing,
                ModelFeature::JsonMode,
                ModelFeature::CodeExecution,
                ModelFeature::SearchGrounding,
                ModelFeature::VideoUnderstanding,
                ModelFeature::AudioUnderstanding,
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
}
