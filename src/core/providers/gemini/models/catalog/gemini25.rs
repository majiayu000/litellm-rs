use super::super::{
    GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec,
    pricing_per_million,
};
use crate::core::types::model::ModelInfo;

pub(super) fn register(registry: &mut GeminiModelRegistry) {
    // ==================== Gemini 2.5 Series (2025) ====================

    // Gemini 2.5 Pro
    registry.register_model(
        "gemini-2.5-pro",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-2.5-pro".to_string(),
                name: "Gemini 2.5 Pro".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_000_000,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.00125),
                output_cost_per_1k_tokens: Some(0.010),
                currency: "USD".to_string(),
                capabilities: vec![
                    crate::core::types::model::ProviderCapability::ChatCompletion,
                    crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    crate::core::types::model::ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini25Pro,
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
                1.25,
                10.0,
                Some(0.3125),
                Some(0.005),
                Some(0.005),
                Some(0.0005),
            ),
            limits: ModelLimits {
                max_context_length: 1_000_000,
                max_output_tokens: 65536,
                max_images: Some(3000),
                max_video_seconds: Some(3600),
                max_audio_seconds: Some(9600),
                rpm_limit: Some(1000),
                tpm_limit: Some(4_000_000),
            },
        },
    );

    // Gemini 2.5 Flash
    registry.register_model(
        "gemini-2.5-flash",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-2.5-flash".to_string(),
                name: "Gemini 2.5 Flash".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_000_000,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.0003),
                output_cost_per_1k_tokens: Some(0.0025),
                currency: "USD".to_string(),
                capabilities: vec![
                    crate::core::types::model::ProviderCapability::ChatCompletion,
                    crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    crate::core::types::model::ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini25Flash,
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
                0.30,
                2.50,
                Some(0.075),
                Some(0.0002),
                Some(0.0002),
                Some(0.0001),
            ),
            limits: ModelLimits {
                max_context_length: 1_000_000,
                max_output_tokens: 65536,
                max_images: Some(3000),
                max_video_seconds: Some(3600),
                max_audio_seconds: Some(9600),
                rpm_limit: Some(2000),
                tpm_limit: Some(4_000_000),
            },
        },
    );

    // Gemini 2.5 Flash-Lite
    registry.register_model(
        "gemini-2.5-flash-lite",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-2.5-flash-lite".to_string(),
                name: "Gemini 2.5 Flash-Lite".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_000_000,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.0001),
                output_cost_per_1k_tokens: Some(0.0004),
                currency: "USD".to_string(),
                capabilities: vec![
                    crate::core::types::model::ProviderCapability::ChatCompletion,
                    crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    crate::core::types::model::ProviderCapability::ToolCalling,
                ],
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini25FlashLite,
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
            ],
            pricing: pricing_per_million(0.10, 0.40, Some(0.025), Some(0.0001), None, None),
            limits: ModelLimits {
                max_context_length: 1_000_000,
                max_output_tokens: 65536,
                max_images: Some(3000),
                max_video_seconds: None,
                max_audio_seconds: None,
                rpm_limit: Some(4000),
                tpm_limit: Some(4_000_000),
            },
        },
    );
}
