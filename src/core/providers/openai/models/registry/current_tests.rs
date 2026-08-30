use super::{OpenAIModelFamily, OpenAIModelFeature, OpenAIModelRegistry, get_openai_registry};
use crate::core::types::model::{ModelInfo, ProviderCapability};

#[test]
fn gpt56_catalog_entries_match_official_model_cards() {
    let registry = get_openai_registry();
    let cases = [
        ("gpt-5.6", OpenAIModelFamily::GPT56Sol, 0.004, 0.020),
        ("gpt-5.6-sol", OpenAIModelFamily::GPT56Sol, 0.004, 0.020),
        ("gpt-5.6-terra", OpenAIModelFamily::GPT56Terra, 0.002, 0.012),
        ("gpt-5.6-luna", OpenAIModelFamily::GPT56Luna, 0.0002, 0.0012),
    ];

    for (id, family, input_cost, output_cost) in cases {
        let model = registry
            .get_model_spec(id)
            .unwrap_or_else(|| panic!("{id} should be in the static OpenAI catalog"));

        assert_eq!(model.family, family);
        assert_eq!(model.model_info.max_context_length, 1_050_000);
        assert_eq!(model.model_info.max_output_length, Some(128_000));
        assert_eq!(model.model_info.input_cost_per_1k_tokens, Some(input_cost));
        assert_eq!(
            model.model_info.output_cost_per_1k_tokens,
            Some(output_cost)
        );
        assert!(model.model_info.supports_tools);
        assert!(model.model_info.supports_streaming);
        assert!(model.model_info.supports_multimodal);
        assert!(
            model
                .features
                .contains(&OpenAIModelFeature::StreamingSupport)
        );
        assert!(
            model
                .features
                .contains(&OpenAIModelFeature::FunctionCalling)
        );
        assert!(model.features.contains(&OpenAIModelFeature::VisionSupport));
        assert!(model.features.contains(&OpenAIModelFeature::ReasoningMode));
        assert!(model.features.contains(&OpenAIModelFeature::JsonMode));
        assert!(!model.features.contains(&OpenAIModelFeature::AudioInput));
        assert!(!model.features.contains(&OpenAIModelFeature::AudioOutput));
        assert!(model.config.supports_batch);
    }
}

#[test]
fn gpt56_cyber_catalog_entry_matches_official_model_card() {
    let registry = get_openai_registry();
    let model = registry
        .get_model_spec("gpt-5.6-cyber")
        .expect("gpt-5.6-cyber should be in the static OpenAI catalog");

    assert_eq!(model.family, OpenAIModelFamily::GPT56Cyber);
    assert_eq!(model.model_info.max_context_length, 400_000);
    assert_eq!(model.model_info.max_output_length, Some(128_000));
    assert_eq!(model.model_info.input_cost_per_1k_tokens, Some(0.0125));
    assert_eq!(model.model_info.output_cost_per_1k_tokens, Some(0.075));
    assert!(
        model
            .features
            .contains(&OpenAIModelFeature::StreamingSupport)
    );
    assert!(
        model
            .features
            .contains(&OpenAIModelFeature::FunctionCalling)
    );
    assert!(model.features.contains(&OpenAIModelFeature::VisionSupport));
    assert!(model.features.contains(&OpenAIModelFeature::ReasoningMode));
    assert!(model.features.contains(&OpenAIModelFeature::JsonMode));
    assert!(!model.features.contains(&OpenAIModelFeature::AudioInput));
    assert!(model.config.supports_batch);
}

#[test]
fn realtime_2x_catalog_entries_match_official_model_cards() {
    let registry = get_openai_registry();
    let cases = [
        ("gpt-realtime-2", 0.004, 0.024),
        ("gpt-realtime-2.1", 0.004, 0.024),
        ("gpt-realtime-2.1-mini", 0.0006, 0.0024),
    ];

    for (id, input_cost, output_cost) in cases {
        let model = registry
            .get_model_spec(id)
            .unwrap_or_else(|| panic!("{id} should be in the static OpenAI catalog"));

        assert_eq!(model.family, OpenAIModelFamily::Realtime);
        assert_eq!(model.model_info.max_context_length, 128_000);
        assert_eq!(model.model_info.max_output_length, Some(32_000));
        assert_eq!(model.model_info.input_cost_per_1k_tokens, Some(input_cost));
        assert_eq!(
            model.model_info.output_cost_per_1k_tokens,
            Some(output_cost)
        );
        assert!(!model.model_info.supports_streaming);
        assert!(!model.config.supports_streaming);
        assert!(
            !model
                .features
                .contains(&OpenAIModelFeature::StreamingSupport)
        );
        assert!(!model.features.contains(&OpenAIModelFeature::JsonMode));
        assert!(!model.features.contains(&OpenAIModelFeature::ChatCompletion));
        assert!(
            !model
                .model_info
                .capabilities
                .contains(&ProviderCapability::ChatCompletion)
        );
        assert!(
            !model
                .model_info
                .capabilities
                .contains(&ProviderCapability::ChatCompletionStream)
        );
        assert!(model.features.contains(&OpenAIModelFeature::ReasoningMode));
        assert!(
            model
                .features
                .contains(&OpenAIModelFeature::FunctionCalling)
        );
        assert!(model.features.contains(&OpenAIModelFeature::VisionSupport));
        assert!(model.features.contains(&OpenAIModelFeature::AudioInput));
        assert!(
            model
                .features
                .contains(&OpenAIModelFeature::RealtimeAudioOutput)
        );
        assert!(!model.features.contains(&OpenAIModelFeature::AudioOutput));
        assert!(
            !model
                .model_info
                .capabilities
                .contains(&ProviderCapability::TextToSpeech)
        );
        assert!(model.features.contains(&OpenAIModelFeature::RealtimeAudio));
    }
}

#[test]
fn realtime_2_reasoning_detection_is_boundary_safe() {
    let registry = get_openai_registry();

    for model_id in [
        "gpt-realtime-2",
        "gpt-realtime-2.1",
        "gpt-realtime-2.1-mini",
    ] {
        assert!(
            registry.supports_feature(model_id, &OpenAIModelFeature::ReasoningMode),
            "{model_id} should expose documented Realtime reasoning"
        );
    }

    assert!(
        registry.get_model_spec("gpt-realtime-2025-08-28").is_some(),
        "legacy Realtime snapshot should be present in the embedded catalog"
    );
    assert!(
        !registry.supports_feature(
            "gpt-realtime-2025-08-28",
            &OpenAIModelFeature::ReasoningMode
        ),
        "legacy Realtime snapshot must not be classified as Realtime 2"
    );
}

#[test]
fn gpt56_family_detection_accepts_only_documented_exact_ids() {
    let registry = OpenAIModelRegistry::new();
    let cases = [
        ("gpt-5.6", OpenAIModelFamily::GPT56Sol),
        ("gpt-5.6-sol", OpenAIModelFamily::GPT56Sol),
        ("gpt-5.6-terra", OpenAIModelFamily::GPT56Terra),
        ("gpt-5.6-luna", OpenAIModelFamily::GPT56Luna),
        ("gpt-5.6-cyber", OpenAIModelFamily::GPT56Cyber),
        ("gpt-5.6-2026-08-01", OpenAIModelFamily::GPT5),
        ("gpt-5.6-sol-2026-08-01", OpenAIModelFamily::GPT5),
        ("gpt-5.60", OpenAIModelFamily::GPT5),
        ("gpt-5.6-solstice", OpenAIModelFamily::GPT5),
        ("gpt-5.6-cybernetic", OpenAIModelFamily::GPT5),
        ("gpt-5.6-foo", OpenAIModelFamily::GPT5),
    ];

    for (id, expected) in cases {
        assert_eq!(registry.determine_family(&model_info(id)), expected, "{id}");
    }
}

fn model_info(id: &str) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: id.to_string(),
        provider: "openai".to_string(),
        max_context_length: 1_050_000,
        max_output_length: Some(128_000),
        supports_streaming: true,
        supports_tools: true,
        supports_multimodal: true,
        input_cost_per_1k_tokens: None,
        output_cost_per_1k_tokens: None,
        currency: "USD".to_string(),
        capabilities: vec![],
        created_at: None,
        updated_at: None,
        metadata: std::collections::HashMap::new(),
    }
}
