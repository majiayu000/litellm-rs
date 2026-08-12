use super::super::{
    GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec,
    pricing_per_million,
};
use crate::core::types::model::ModelInfo;

pub(super) fn register(registry: &mut GeminiModelRegistry) {
    registry.register_model(
        "gemini-3.5-flash-lite",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3.5-flash-lite".to_string(),
                name: "Gemini 3.5 Flash-Lite".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_048_576,
                max_output_length: Some(65_536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.0003),
                output_cost_per_1k_tokens: Some(0.0025),
                currency: "USD".to_string(),
                capabilities: super::advanced_text_capabilities(),
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini35FlashLite,
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
            pricing: pricing_per_million(0.3, 2.5, None, None, None, None),
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

    registry.register_model(
        "gemini-3.5-flash",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3.5-flash".to_string(),
                name: "Gemini 3.5 Flash".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_048_576,
                max_output_length: Some(65_536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.0015),
                output_cost_per_1k_tokens: Some(0.009),
                currency: "USD".to_string(),
                capabilities: vec![
                    crate::core::types::model::ProviderCapability::ChatCompletion,
                    crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    crate::core::types::model::ProviderCapability::ToolCalling,
                    crate::core::types::model::ProviderCapability::FunctionCalling,
                    crate::core::types::model::ProviderCapability::CodeExecution,
                    crate::core::types::model::ProviderCapability::BatchProcessing,
                ],
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini35Flash,
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
            // Official Gemini 3.5 Flash standard pricing is token-based and
            // does not publish separate image/video/audio unit rates.
            pricing: pricing_per_million(1.5, 9.0, Some(0.15), None, None, None),
            limits: ModelLimits {
                max_context_length: 1_048_576,
                max_output_tokens: 65_536,
                max_images: Some(3000),
                max_video_seconds: Some(3600),
                max_audio_seconds: Some(9600),
                // rpm_limit / tpm_limit deliberately None: official limits at
                // https://ai.google.dev/gemini-api/docs/rate-limits vary by billing tier
                // (Free: 10 RPM, Tier 1: 2000 RPM as of 2026-05-25). Add explicit limits
                // once the gateway routes by tier; until then, downstream rate-limiting
                // should rely on response-driven backoff.
                rpm_limit: None,
                tpm_limit: None,
            },
        },
    );
}
