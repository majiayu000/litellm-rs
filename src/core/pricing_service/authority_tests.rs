use super::*;

fn test_model_info(provider: &str) -> LiteLLMModelInfo {
    LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: Some(4096),
        max_output_tokens: Some(4096),
        input_cost_per_token: Some(0.00001),
        output_cost_per_token: Some(0.00003),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: provider.to_string(),
        mode: "chat".to_string(),
        supports_function_calling: Some(true),
        supports_vision: Some(false),
        supports_streaming: Some(true),
        supports_parallel_function_calling: Some(true),
        supports_system_message: Some(true),
        extra: HashMap::new(),
    }
}

#[test]
fn provider_aware_authority_uses_loaded_custom_model() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "runtime-only-priced-model".to_string(),
        test_model_info("runtime_provider"),
    );

    let cost = match service.calculate_loaded_usage_cost_for_provider(
        "runtime_provider",
        "runtime-only-priced-model",
        &PricingUsage::new(1000, 500),
    ) {
        Ok(cost) => cost,
        Err(error) => panic!("runtime-loaded pricing should calculate cost: {error}"),
    };

    assert_eq!(cost.model, "runtime-only-priced-model");
    assert_eq!(cost.provider, "runtime_provider");
    assert_eq!(cost.input_cost, 0.01);
    assert!((cost.output_cost - 0.015).abs() < f64::EPSILON);
    assert!((cost.total_cost - 0.025).abs() < f64::EPSILON);
}

#[test]
fn provider_aware_authority_resolves_anthropic_mimo_alias() {
    let service = match PricingService::with_embedded_default() {
        Ok(service) => service,
        Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
    };

    let cost = match service.calculate_loaded_usage_cost_for_provider(
        "anthropic",
        "mimo-v2.5-pro",
        &PricingUsage::new(1000, 500),
    ) {
        Ok(cost) => cost,
        Err(error) => {
            panic!("Anthropic-compatible MiMo should resolve through Xiaomi pricing: {error}")
        }
    };

    assert_eq!(cost.model, "mimo-v2.5-pro");
    assert_eq!(cost.provider, "anthropic");
    assert!(cost.total_cost > 0.0);
}

#[test]
fn google_authority_uses_specialized_character_pricing() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));
    let cost = service
        .calculate_loaded_completion_cost_for_provider(
            "vertex_ai",
            "medlm-large",
            0,
            0,
            Some("é中"),
            Some("🙂"),
            None,
        )
        .unwrap_or_else(|error| panic!("specialized Vertex row should be priced: {error}"));

    assert_eq!(cost.cost_type, CostType::CharacterBased);
    assert!((cost.total_cost - 0.000025).abs() < 1e-12);
}

#[test]
fn provider_aware_completion_cost_uses_supplied_pricing_time() {
    use chrono::{TimeZone, Utc};

    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));
    let off_peak = Utc.with_ymd_and_hms(2026, 8, 24, 4, 0, 0).unwrap();
    let peak = Utc.with_ymd_and_hms(2026, 8, 24, 6, 0, 0).unwrap();

    let calculate_at = |pricing_time| {
        service
            .calculate_loaded_completion_cost_for_provider_at(
                "deepseek",
                "deepseek-v4-flash",
                1_000,
                1_000,
                None,
                None,
                None,
                pricing_time,
            )
            .unwrap_or_else(|error| panic!("loaded pricing should calculate cost: {error}"))
    };

    let off_peak_cost = calculate_at(off_peak);
    let peak_cost = calculate_at(peak);

    assert!((peak_cost.total_cost - off_peak_cost.total_cost * 2.0).abs() < 1e-12);
}

#[test]
fn google_authority_is_case_insensitive_but_not_suffix_fuzzy() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));

    assert!(
        service
            .get_model_info_for_provider("vertex_ai", "GEMINI-1.5-PRO")
            .is_some()
    );
    assert!(
        service
            .get_model_info_for_provider("vertex_ai", "GEMINI-1.5-PRO-9999")
            .is_none()
    );
}

#[test]
fn provider_aware_authority_resolves_loaded_openai_like_model_without_prefix() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "runtime-openai-like-model".to_string(),
        test_model_info("openai_like"),
    );

    let cost = match service.calculate_loaded_usage_cost_for_provider(
        "openai_like",
        "runtime-openai-like-model",
        &PricingUsage::new(1000, 500),
    ) {
        Ok(cost) => cost,
        Err(error) => panic!("loaded OpenAI-like pricing should calculate cost: {error}"),
    };

    assert_eq!(cost.model, "runtime-openai-like-model");
    assert_eq!(cost.provider, "openai_like");
    assert!((cost.total_cost - 0.025).abs() < f64::EPSILON);
}

#[test]
fn provider_aware_authority_resolves_xai_openai_like_prefix() {
    let service = match PricingService::with_embedded_default() {
        Ok(service) => service,
        Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
    };

    let cost = match service.calculate_loaded_usage_cost_for_provider(
        "openai_like",
        "xai/grok-4.3",
        &PricingUsage::new(1000, 500),
    ) {
        Ok(cost) => cost,
        Err(error) => panic!("xAI OpenAI-like prefixed model should resolve: {error}"),
    };

    assert_eq!(cost.model, "xai/grok-4.3");
    assert_eq!(cost.provider, "openai_like");
    assert!((cost.total_cost - 0.0025).abs() < f64::EPSILON);
}

#[cfg(feature = "providers-extended")]
#[test]
fn provider_aware_authority_resolves_amazon_nova_short_alias() {
    let service = match PricingService::with_embedded_default() {
        Ok(service) => service,
        Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
    };

    let cost = match service.calculate_loaded_usage_cost_for_provider(
        "amazon_nova",
        "nova-2-lite",
        &PricingUsage::new(1000, 500),
    ) {
        Ok(cost) => cost,
        Err(error) => panic!("Amazon Nova short alias should resolve: {error}"),
    };

    assert_eq!(cost.model, "amazon.nova-2-lite-v1:0");
    assert_eq!(cost.provider, "amazon_nova");
    assert!((cost.total_cost - 0.00155).abs() < f64::EPSILON);
}

#[test]
fn provider_aware_authority_resolves_mixed_case_provider_exact_key() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));

    let Some((resolved, _)) = service.get_model_info_for_provider("azure", "GPT-5.5") else {
        panic!("mixed-case Azure model should resolve");
    };

    assert_eq!(resolved, "azure/gpt-5.5");

    let Some((resolved, _)) = service.get_model_info_for_provider("azure_ai", "phi-4") else {
        panic!("lowercase Azure AI model should resolve to its mixed-case catalog key");
    };

    assert_eq!(resolved, "azure_ai/Phi-4");
}

#[test]
fn provider_aware_authority_preserves_core_pricing_tiers() {
    let service = match PricingService::with_embedded_default() {
        Ok(service) => service,
        Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
    };

    let cost = match service.calculate_loaded_usage_cost_for_provider(
        "azure",
        "gpt-5.5",
        &PricingUsage::new(300_000, 1_000),
    ) {
        Ok(cost) => cost,
        Err(error) => panic!("Azure tiered fallback pricing should resolve: {error}"),
    };

    assert_eq!(cost.model, "azure/gpt-5.5");
    assert_eq!(cost.provider, "azure");
    assert!((cost.input_cost - 3.0).abs() < 1e-12);
    assert!((cost.output_cost - 0.045).abs() < 1e-12);
    assert!((cost.total_cost - 3.045).abs() < 1e-12);
}

#[test]
fn provider_aware_authority_breaks_equal_length_fuzzy_ties_deterministically() {
    for candidates in [
        ["region-b/model-2026", "region-a/model-2026"],
        ["region-a/model-2026", "region-b/model-2026"],
    ] {
        let service = PricingService::new(None);
        for candidate in candidates {
            service.add_custom_model(candidate.to_string(), test_model_info("synthetic"));
        }
        let Some((resolved, _)) = service.get_model_info_for_provider("synthetic", "model") else {
            panic!("synthetic fuzzy model should resolve");
        };
        assert_eq!(resolved, "region-a/model-2026");
    }
}

#[test]
fn tier_threshold_ignores_named_price_variants() {
    assert_eq!(
        extract_tier_threshold("input_cost_per_token_above_272k_tokens"),
        Some(272_000)
    );
    assert_eq!(
        extract_tier_threshold("input_cost_per_token_above_272k_tokens_priority"),
        None
    );
    assert_eq!(
        extract_tier_threshold("input_cost_per_token_above_272k_tokens_flex"),
        None
    );
}

#[test]
fn provider_aware_authority_rejects_missing_token_pricing() {
    let service = PricingService::new(None);
    let mut model_info = test_model_info("runtime_provider");
    model_info.output_cost_per_token = None;
    service.add_custom_model("partial-priced-model".to_string(), model_info);

    let error = match service.calculate_loaded_usage_cost_for_provider(
        "runtime_provider",
        "partial-priced-model",
        &PricingUsage::new(1000, 500),
    ) {
        Ok(_) => panic!("incomplete pricing must fail closed"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("output_cost_per_token"));
}
