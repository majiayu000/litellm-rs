use super::{
    super::{GeminiModelFamily, GeminiModelRegistry, ModelFeature, ModelLimits, ModelSpec},
    advanced_text_capabilities, promotional_flash_pricing_metadata,
};
use crate::core::types::model::ModelInfo;

fn model_metadata() -> std::collections::HashMap<String, serde_json::Value> {
    let mut metadata = promotional_flash_pricing_metadata();
    metadata.extend([
        (
            "google_input_modalities".to_string(),
            serde_json::json!(["text", "image", "video", "audio", "pdf"]),
        ),
        (
            "google_output_modalities".to_string(),
            serde_json::json!(["text"]),
        ),
        (
            "google_thinking_levels".to_string(),
            serde_json::json!(["low", "medium", "high"]),
        ),
        (
            "google_default_thinking_level".to_string(),
            serde_json::json!("medium"),
        ),
    ]);
    for capability in [
        "supports_computer_use_preview",
        "supports_file_search",
        "supports_maps_grounding",
        "supports_url_context",
        "supports_flex_inference",
        "supports_priority_inference",
    ] {
        metadata.insert(capability.to_string(), serde_json::json!(true));
    }
    for capability in [
        "supports_minimal_thinking",
        "supports_live_api",
        "supports_audio_generation",
        "supports_image_generation",
    ] {
        metadata.insert(capability.to_string(), serde_json::json!(false));
    }
    metadata
}

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
                supports_multimodal: true,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: advanced_text_capabilities(),
                created_at: None,
                updated_at: None,
                metadata: model_metadata(),
            },
            family: GeminiModelFamily::Gemini37Flash,
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
