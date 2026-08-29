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

    assert_eq!(spec.model_info.input_cost_per_1k_tokens, None);
    assert_eq!(spec.model_info.output_cost_per_1k_tokens, None);
    assert_eq!(spec.pricing.input_cost_per_1k_tokens, 0.0);
    assert_eq!(spec.pricing.output_cost_per_1k_tokens, 0.0);
    assert_eq!(spec.pricing.cache_read_input_token_cost, None);
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
    for lookalike in [
        "GEMINI-3.7-FLASH",
        "gemini-3.7-flash-preview",
        "gemini-3.7-flash-20260813",
        "prefix-gemini-3.7-flash",
        "gemini-3.7-flash-suffix",
    ] {
        assert_eq!(gemini_context_window(lookalike), None, "{lookalike}");
    }
}

#[test]
fn gemini_36_static_catalog_defers_pricing_to_the_central_authority() {
    let spec = get_gemini_registry()
        .get_model_spec("gemini-3.6-flash")
        .expect("Gemini 3.6 Flash should remain in the static catalog");

    assert_eq!(spec.model_info.input_cost_per_1k_tokens, None);
    assert_eq!(spec.model_info.output_cost_per_1k_tokens, None);
    assert_eq!(spec.pricing.input_cost_per_1k_tokens, 0.0);
    assert_eq!(spec.pricing.output_cost_per_1k_tokens, 0.0);
    assert_eq!(spec.pricing.cache_read_input_token_cost, None);
    assert_eq!(spec.pricing.batch_discount, None);
}

#[test]
fn flash_static_pricing_switches_at_the_documented_2027_boundary() {
    let registry = get_gemini_registry();
    use chrono::TimeZone;

    let promotional_date = chrono::Utc
        .with_ymd_and_hms(2026, 12, 31, 23, 59, 59)
        .unwrap();
    let standard_date = chrono::Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();

    for model in ["gemini-3.7-flash", "gemini-3.6-flash"] {
        let promotional = registry
            .get_core_model_pricing_at(model, promotional_date)
            .expect("current Flash model should have promotional pricing");
        assert_eq!(promotional.input_cost_per_1k_tokens, 0.00075);
        assert_eq!(promotional.output_cost_per_1k_tokens, 0.00375);
        assert_eq!(promotional.cache_read_input_token_cost, Some(0.000075));
        assert_eq!(promotional.batch_discount, None);

        let standard = registry
            .get_core_model_pricing_at(model, standard_date)
            .expect("current Flash model should have standard pricing");
        assert_eq!(standard.input_cost_per_1k_tokens, 0.0015);
        assert_eq!(standard.output_cost_per_1k_tokens, 0.0075);
        assert_eq!(standard.cache_read_input_token_cost, Some(0.00015));
        assert_eq!(standard.batch_discount, None);
    }

    let promotional_cost = CostCalculator::calculate_multimodal_cost_at(
        "gemini-3.7-flash",
        1_000,
        1_000,
        Some(1_000),
        None,
        None,
        None,
        promotional_date,
    )
    .expect("promotional cost");
    let standard_cost = CostCalculator::calculate_multimodal_cost_at(
        "gemini-3.7-flash",
        1_000,
        1_000,
        Some(1_000),
        None,
        None,
        None,
        standard_date,
    )
    .expect("standard cost");
    assert!((promotional_cost - 0.003825).abs() < 1e-12);
    assert!((standard_cost - 0.00765).abs() < 1e-12);

    let spec = registry
        .get_model_spec("gemini-3.7-flash")
        .expect("Gemini 3.7 Flash spec");
    let promotional = registry
        .get_core_model_pricing_at("gemini-3.7-flash", promotional_date)
        .unwrap();
    let standard = registry
        .get_core_model_pricing_at("gemini-3.7-flash", standard_date)
        .unwrap();
    let promotional_info =
        GoogleGeminiApiSurface::DeveloperApi.overlay_model_info(spec, &promotional);
    let standard_info = GoogleGeminiApiSurface::DeveloperApi.overlay_model_info(spec, &standard);
    assert_eq!(promotional_info.input_cost_per_1k_tokens, Some(0.00075));
    assert_eq!(promotional_info.output_cost_per_1k_tokens, Some(0.00375));
    assert_eq!(standard_info.input_cost_per_1k_tokens, Some(0.0015));
    assert_eq!(standard_info.output_cost_per_1k_tokens, Some(0.0075));
}
