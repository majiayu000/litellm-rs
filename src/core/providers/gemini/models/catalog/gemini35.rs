use super::super::{
    GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec,
    pricing_per_million,
};
use crate::core::types::model::ModelInfo;

pub(super) fn register(registry: &mut GeminiModelRegistry) {
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
            pricing: pricing_per_million(1.5, 9.0, Some(0.15), None, None, None),
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
