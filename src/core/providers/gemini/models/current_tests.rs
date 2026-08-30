use super::*;
use crate::core::types::model::ProviderCapability;

#[test]
fn gemini_37_flash_is_registered_with_official_limits() {
    let spec = get_gemini_registry()
        .get_model_spec("gemini-3.7-flash")
        .expect("Gemini 3.7 Flash should be in the static catalog");

    assert_eq!(spec.model_info.id, "gemini-3.7-flash");
    assert_eq!(spec.limits.max_context_length, 1_048_576);
    assert_eq!(spec.limits.max_output_tokens, 65_536);
}

#[test]
fn gemini_37_flash_matches_official_capabilities_and_promotional_pricing() {
    let spec = get_gemini_registry()
        .get_model_spec("gemini-3.7-flash")
        .expect("Gemini 3.7 Flash should be in the static catalog");

    assert_eq!(spec.family, GeminiModelFamily::Gemini37Flash);
    for capability in [
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
        ProviderCapability::ToolCalling,
        ProviderCapability::FunctionCalling,
        ProviderCapability::CodeExecution,
        ProviderCapability::BatchProcessing,
    ] {
        assert!(spec.model_info.capabilities.contains(&capability));
    }
    for feature in [
        ModelFeature::MultimodalSupport,
        ModelFeature::ToolCalling,
        ModelFeature::FunctionCalling,
        ModelFeature::StreamingSupport,
        ModelFeature::ContextCaching,
        ModelFeature::BatchProcessing,
        ModelFeature::JsonMode,
        ModelFeature::CodeExecution,
        ModelFeature::SearchGrounding,
        ModelFeature::VideoUnderstanding,
        ModelFeature::AudioUnderstanding,
    ] {
        assert!(spec.features.contains(&feature), "missing {feature:?}");
    }
    assert!(!spec.features.contains(&ModelFeature::RealtimeStreaming));

    assert_eq!(
        spec.model_info.metadata["google_input_modalities"],
        serde_json::json!(["text", "image", "video", "audio", "pdf"])
    );
    assert_eq!(
        spec.model_info.metadata["google_output_modalities"],
        serde_json::json!(["text"])
    );
    assert_eq!(
        spec.model_info.metadata["google_thinking_levels"],
        serde_json::json!(["low", "medium", "high"])
    );
    assert_eq!(
        spec.model_info.metadata["google_default_thinking_level"],
        serde_json::json!("medium")
    );
    for key in [
        "supports_computer_use_preview",
        "supports_file_search",
        "supports_maps_grounding",
        "supports_url_context",
        "supports_flex_inference",
        "supports_priority_inference",
    ] {
        assert_eq!(spec.model_info.metadata[key], serde_json::json!(true));
    }
    for key in [
        "supports_minimal_thinking",
        "supports_live_api",
        "supports_audio_generation",
        "supports_image_generation",
    ] {
        assert_eq!(spec.model_info.metadata[key], serde_json::json!(false));
    }

    assert_eq!(spec.model_info.input_cost_per_1k_tokens, Some(0.00075));
    assert_eq!(spec.model_info.output_cost_per_1k_tokens, Some(0.00375));
    assert_eq!(spec.pricing.input_cost_per_1k_tokens, 0.00075);
    assert_eq!(spec.pricing.output_cost_per_1k_tokens, 0.00375);
    assert_eq!(spec.pricing.cache_read_input_token_cost, Some(0.000075));
    assert_eq!(spec.pricing.batch_discount, None);
    assert_eq!(
        spec.model_info.metadata["google_promotional_pricing_through"],
        serde_json::json!("2026-12-31")
    );
    assert_eq!(
        spec.model_info.metadata["google_standard_pricing_from"],
        serde_json::json!("2027-01-01")
    );
}

#[test]
fn gemini_37_family_recognition_is_exact_only() {
    assert_eq!(
        GeminiModelRegistry::from_model_name("gemini-3.7-flash"),
        Some(GeminiModelFamily::Gemini37Flash)
    );
    for lookalike in [
        "GEMINI-3.7-FLASH",
        "gemini-3.7-flash-preview",
        "gemini-3.7-flash-20260813",
        "prefix-gemini-3.7-flash",
        "gemini-3.7-flash-suffix",
    ] {
        assert_eq!(GeminiModelRegistry::from_model_name(lookalike), None);
        assert!(get_gemini_registry().get_model_spec(lookalike).is_none());
    }
}

#[test]
fn gemini_37_shared_context_window_is_exact_only() {
    use crate::core::providers::shared::{GEMINI_31_CONTEXT_WINDOW, gemini_context_window};

    assert_eq!(
        gemini_context_window("gemini-3.7-flash"),
        Some(GEMINI_31_CONTEXT_WINDOW)
    );
    for qualified in [
        "gemini/gemini-3.7-flash",
        "google/gemini-3.7-flash",
        "vertex_ai/gemini-3.7-flash",
    ] {
        assert_eq!(
            gemini_context_window(qualified),
            Some(GEMINI_31_CONTEXT_WINDOW),
            "{qualified}"
        );
    }
    for lookalike in [
        "GEMINI-3.7-FLASH",
        "other/gemini-3.7-flash",
        "gemini/gemini/gemini-3.7-flash",
        "gemini-3.7-flash-preview",
        "gemini-3.7-flash-20260813",
        "prefix-gemini-3.7-flash",
        "gemini-3.7-flash-suffix",
    ] {
        assert_eq!(gemini_context_window(lookalike), None, "{lookalike}");
    }
}

#[test]
fn gemini_36_static_catalog_uses_current_promotional_fallback() {
    let spec = get_gemini_registry()
        .get_model_spec("gemini-3.6-flash")
        .expect("Gemini 3.6 Flash should remain in the static catalog");

    assert_eq!(spec.model_info.input_cost_per_1k_tokens, Some(0.00075));
    assert_eq!(spec.model_info.output_cost_per_1k_tokens, Some(0.00375));
    assert_eq!(spec.pricing.input_cost_per_1k_tokens, 0.00075);
    assert_eq!(spec.pricing.output_cost_per_1k_tokens, 0.00375);
    assert_eq!(spec.pricing.cache_read_input_token_cost, Some(0.000075));
    assert_eq!(spec.pricing.batch_discount, None);
}
