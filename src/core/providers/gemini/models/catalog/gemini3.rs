use super::super::{
    GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec,
    pricing_per_million,
};
use crate::core::types::model::ModelInfo;

pub(super) fn register(registry: &mut GeminiModelRegistry) {
    // ==================== Gemini 3.0 Series (2025 - Deprecated 2026-03-09) ====================

    // Gemini 3 Pro
    registry.register_model(
        "gemini-3-pro",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3-pro".to_string(),
                name: "Gemini 3 Pro".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_000_000,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.002),
                output_cost_per_1k_tokens: Some(0.012),
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
            family: GeminiModelFamily::Gemini3Pro,
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
                Some(0.5),
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

    // Gemini 3 Pro Deep Think
    registry.register_model(
        "gemini-3-pro-deep-think",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3-pro-deep-think".to_string(),
                name: "Gemini 3 Pro Deep Think".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 1_000_000,
                max_output_length: Some(65536),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.004),
                output_cost_per_1k_tokens: Some(0.024),
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
            family: GeminiModelFamily::Gemini3ProDeepThink,
            features: vec![
                ModelFeature::MultimodalSupport,
                ModelFeature::ToolCalling,
                ModelFeature::FunctionCalling,
                ModelFeature::StreamingSupport,
                ModelFeature::ContextCaching,
                ModelFeature::SystemInstructions,
                ModelFeature::JsonMode,
                ModelFeature::CodeExecution,
                ModelFeature::SearchGrounding,
                ModelFeature::VideoUnderstanding,
                ModelFeature::AudioUnderstanding,
            ],
            pricing: pricing_per_million(4.0, 24.0, Some(1.0), Some(0.01), Some(0.01), Some(0.001)),
            limits: ModelLimits {
                max_context_length: 1_000_000,
                max_output_tokens: 65536,
                max_images: Some(3000),
                max_video_seconds: Some(3600),
                max_audio_seconds: Some(9600),
                rpm_limit: Some(500),
                tpm_limit: Some(2_000_000),
            },
        },
    );

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
                capabilities: vec![
                    crate::core::types::model::ProviderCapability::ChatCompletion,
                    crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    crate::core::types::model::ProviderCapability::ToolCalling,
                ],
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

    // Gemini 3 Pro Image Preview
    registry.register_model(
        "gemini-3-pro-image-preview",
        ModelSpec {
            model_info: ModelInfo {
                id: "gemini-3-pro-image-preview".to_string(),
                name: "Gemini 3 Pro Image Preview".to_string(),
                provider: "gemini".to_string(),
                max_context_length: 65536,
                max_output_length: Some(8192),
                supports_streaming: true,
                supports_tools: true,
                supports_multimodal: true,
                input_cost_per_1k_tokens: Some(0.002),
                output_cost_per_1k_tokens: Some(0.012),
                currency: "USD".to_string(),
                capabilities: vec![
                    crate::core::types::model::ProviderCapability::ChatCompletion,
                    crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    crate::core::types::model::ProviderCapability::ImageGeneration,
                ],
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            },
            family: GeminiModelFamily::Gemini3ProImage,
            features: vec![
                ModelFeature::MultimodalSupport,
                ModelFeature::StreamingSupport,
                ModelFeature::SystemInstructions,
                ModelFeature::JsonMode,
            ],
            pricing: pricing_per_million(2.0, 12.0, Some(0.5), Some(0.04), None, None),
            limits: ModelLimits {
                max_context_length: 65536,
                max_output_tokens: 8192,
                max_images: Some(16),
                max_video_seconds: None,
                max_audio_seconds: None,
                rpm_limit: Some(500),
                tpm_limit: Some(1_000_000),
            },
        },
    );
}
