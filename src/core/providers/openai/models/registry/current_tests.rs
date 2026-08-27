use super::{OpenAIModelFamily, OpenAIModelFeature, get_openai_registry};

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
        assert!(
            model
                .features
                .contains(&OpenAIModelFeature::StreamingSupport)
        );
        assert!(model.features.contains(&OpenAIModelFeature::ReasoningMode));
        assert!(
            model
                .features
                .contains(&OpenAIModelFeature::FunctionCalling)
        );
        assert!(model.features.contains(&OpenAIModelFeature::VisionSupport));
        assert!(model.features.contains(&OpenAIModelFeature::AudioInput));
        assert!(model.features.contains(&OpenAIModelFeature::AudioOutput));
        assert!(model.features.contains(&OpenAIModelFeature::RealtimeAudio));
    }
}
