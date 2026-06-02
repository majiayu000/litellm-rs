use super::{
    super::{
        GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec,
        pricing_per_million,
    },
    advanced_text_capabilities,
};
use crate::core::types::model::ModelInfo;

pub(super) fn register(registry: &mut GeminiModelRegistry) {
    // ==================== Gemini 3.1 Series (2026 - Latest) ====================

    // Gemini 3.1 Pro Preview
    registry.register_model(
        "gemini-3.1-pro-preview",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3.1-pro-preview".to_string(),
                name: "Gemini 3.1 Pro Preview".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_048_576,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.002),
                output_cost_per_1k_tokens: Some(0.012),
                currency: "USD".to_string(),
                capabilities: advanced_text_capabilities(),
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini31ProPreview,
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
                2.0,
                12.0,
                Some(0.2),
                Some(0.005),
                Some(0.005),
                Some(0.0005),
            ),
            limits: ModelLimits {
                max_context_length: 1_048_576,
                max_output_tokens: 65536,
                max_images: Some(3000),
                max_video_seconds: Some(3600),
                max_audio_seconds: Some(9600),
                rpm_limit: Some(1000),
                tpm_limit: Some(4_000_000),
            },
        },
    );

    // Gemini 3.1 Flash
    registry.register_model(
        "gemini-3.1-flash",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3.1-flash".to_string(),
                name: "Gemini 3.1 Flash".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_048_576,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.000075),
                output_cost_per_1k_tokens: Some(0.0003),
                currency: "USD".to_string(),
                capabilities: advanced_text_capabilities(),
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini31Flash,
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
                0.075,
                0.30,
                Some(0.01875),
                Some(0.0002),
                Some(0.0002),
                Some(0.00002),
            ),
            limits: ModelLimits {
                max_context_length: 1_048_576,
                max_output_tokens: 65536,
                max_images: Some(3000),
                max_video_seconds: Some(3600),
                max_audio_seconds: Some(9600),
                rpm_limit: Some(2000),
                tpm_limit: Some(4_000_000),
            },
        },
    );

    // Gemini 3.1 Flash Lite
    registry.register_model(
        "gemini-3.1-flash-lite-preview",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3.1-flash-lite-preview".to_string(),
                name: "Gemini 3.1 Flash-Lite Preview".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_048_576,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.00025),
                output_cost_per_1k_tokens: Some(0.0015),
                currency: "USD".to_string(),
                capabilities: advanced_text_capabilities(),
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini31FlashLite,
            features: vec![
                ModelFeature::MultimodalSupport,
                ModelFeature::ToolCalling,
                ModelFeature::FunctionCalling,
                ModelFeature::StreamingSupport,
                ModelFeature::ContextCaching,
                ModelFeature::BatchProcessing,
                ModelFeature::SystemInstructions,
                ModelFeature::JsonMode,
                ModelFeature::CodeExecution,
                ModelFeature::SearchGrounding,
                ModelFeature::VideoUnderstanding,
                ModelFeature::AudioUnderstanding,
            ],
            pricing: pricing_per_million(0.25, 1.5, Some(0.025), None, None, None),
            limits: ModelLimits {
                max_context_length: 1_048_576,
                max_output_tokens: 65536,
                max_images: Some(3000),
                max_video_seconds: Some(3600),
                max_audio_seconds: Some(9600),
                rpm_limit: Some(4000),
                tpm_limit: Some(4_000_000),
            },
        },
    );
}
