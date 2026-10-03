use super::{
    super::{
        GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec,
        pricing_per_million,
    },
    advanced_text_capabilities,
};
use crate::core::types::model::ModelInfo;

pub(super) fn register(registry: &mut GeminiModelRegistry) {
    // ==================== Gemini 3.0 Series (2025 - Deprecated 2026-03-09) ====================

    // Gemini 3 Flash Preview
    registry.register_model(
        "gemini-3-flash-preview",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3-flash-preview".to_string(),
                name: "Gemini 3 Flash Preview".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_048_576,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.0005),
                output_cost_per_1k_tokens: Some(0.003),
                currency: "USD".to_string(),
                capabilities: advanced_text_capabilities(),
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini3Flash,
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
            pricing: pricing_per_million(
                0.5,
                3.0,
                Some(0.05),
                Some(0.002),
                Some(0.002),
                Some(0.0002),
            ),
            limits: ModelLimits {
                max_context_length: 1_048_576,
                max_output_tokens: 65536,
                max_images: Some(3000),
                max_video_seconds: Some(3600),
                max_audio_seconds: Some(9600),
                rpm_limit: Some(2000),
                tpm_limit: Some(8_000_000),
            },
        },
    );
}
