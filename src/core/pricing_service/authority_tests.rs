use super::*;
use crate::core::pricing_service::PricingBillingMode;

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
fn gemini_flash_runtime_pricing_switches_at_the_exact_utc_boundary() {
    use chrono::TimeZone;

    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));
    let promotional_time = Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 59).unwrap();
    let standard_time = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();
    let usage = PricingUsage {
        prompt_tokens: 1_000,
        completion_tokens: 1_000,
        total_tokens: 2_000,
        cached_tokens: Some(1_000),
        ..PricingUsage::default()
    };

    for (provider, model) in [
        ("gemini", "gemini-3.6-flash"),
        ("gemini", "gemini/gemini-3.6-flash"),
        ("gemini", "gemini-3.7-flash"),
        ("gemini", "gemini/gemini-3.7-flash"),
        ("vertex_ai", "gemini-3.7-flash"),
        ("vertex_ai", "vertex_ai/gemini-3.7-flash"),
    ] {
        let promotional = service
            .calculate_loaded_usage_cost_for_provider_at(provider, model, &usage, promotional_time)
            .unwrap_or_else(|error| panic!("promotional {model} pricing: {error}"));
        let standard = service
            .calculate_loaded_usage_cost_for_provider_at(provider, model, &usage, standard_time)
            .unwrap_or_else(|error| panic!("standard {model} pricing: {error}"));

        assert!(
            (promotional.total_cost - 0.003_825).abs() < 1e-12,
            "promotional {provider}/{model}: {promotional:?}"
        );
        assert!(
            (standard.total_cost - 0.007_65).abs() < 1e-12,
            "standard {provider}/{model}: {standard:?}"
        );
    }
}

#[test]
fn gemini_flash_schedule_preserves_explicit_custom_pricing() {
    use chrono::TimeZone;

    let service = PricingService::new(None);
    let mut custom = test_model_info("gemini");
    custom.input_cost_per_token = Some(0.123);
    custom.output_cost_per_token = Some(0.456);
    custom.extra.insert(
        "cache_read_input_token_cost".to_string(),
        serde_json::json!(0.078),
    );
    custom.extra.insert(
        "source".to_string(),
        serde_json::json!("https://ai.google.dev/gemini-api/docs/pricing"),
    );
    service.add_custom_model("gemini-3.7-flash".to_string(), custom);

    let after_cutoff = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 1).unwrap();
    let (_, pricing) = service
        .get_model_info_for_provider_at("gemini", "gemini-3.7-flash", after_cutoff)
        .expect("custom Gemini pricing should resolve");

    assert_eq!(pricing.input_cost_per_token, Some(0.123));
    assert_eq!(pricing.output_cost_per_token, Some(0.456));
    assert_eq!(
        pricing.extra["cache_read_input_token_cost"],
        serde_json::json!(0.078)
    );
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
fn provider_aware_authority_strips_qualified_anthropic_mimo_prefix() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));

    let Some((resolved, info)) =
        service.get_model_info_for_provider("anthropic", "anthropic/mimo-v2.5-pro")
    else {
        panic!("qualified Anthropic-compatible MiMo should resolve through Xiaomi pricing");
    };

    assert_eq!(resolved, "mimo-v2.5-pro");
    assert_eq!(info.litellm_provider, "xiaomi_mimo");
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
fn provider_scoped_casefold_exact_precedes_google_explicit_alias() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));
    let mut custom = test_model_info("vertex_ai");
    custom.input_cost_per_token = Some(0.123);
    service.add_custom_model("Gemini-1.5-Pro-001".to_string(), custom);

    let (resolved, info) = service
        .get_model_info_for_provider("vertex_ai", "GEMINI-1.5-PRO-001")
        .expect("provider-scoped casefold exact row should resolve");

    assert_eq!(resolved, "Gemini-1.5-Pro-001");
    assert_eq!(info.input_cost_per_token, Some(0.123));
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

#[test]
fn openai_like_raw_exact_native_slash_precedes_selector_routing() {
    let service = PricingService::new(None);
    let mut custom = test_model_info("openai_like");
    custom.input_cost_per_token = Some(0.456);
    service.add_custom_model("xai/review-future".to_string(), custom);

    let (resolved, info) = service
        .get_model_info_for_provider("openai_like", "xai/review-future")
        .expect("raw exact OpenAI-like slash row should resolve before xAI routing");

    assert_eq!(resolved, "xai/review-future");
    assert_eq!(info.input_cost_per_token, Some(0.456));
}

#[test]
fn openai_like_selector_aliases_route_after_raw_exact_miss() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));

    assert!(
        service
            .get_model_info_for_provider("openai_like", "google_vertex/gemini-1.5-pro")
            .is_some()
    );
    assert!(
        service
            .get_model_info_for_provider(
                "openai_like",
                "aws_bedrock/anthropic.claude-3-sonnet-20240229-v1:0",
            )
            .is_some()
    );
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

#[test]
fn batch_usage_uses_exact_rates_for_reservation_and_settlement() {
    let service = PricingService::new(None);
    let mut model_info = test_model_info("runtime_provider");
    model_info.extra.extend([
        (
            "input_cost_per_token_batches".to_string(),
            serde_json::json!(0.000005),
        ),
        (
            "output_cost_per_token_batches".to_string(),
            serde_json::json!(0.000010),
        ),
        (
            "cache_read_input_token_cost_batches".to_string(),
            serde_json::json!(0.000001),
        ),
    ]);
    service.add_custom_model("batch-priced-model".to_string(), model_info);

    let mut usage = PricingUsage::new(1_000, 500);
    usage.cached_tokens = Some(200);
    usage.billing_mode = PricingBillingMode::Batch;

    let reservation = service
        .dry_run_loaded_usage_cost_for_provider("runtime_provider", "batch-priced-model", &usage)
        .expect("batch reservation pricing");
    let settlement = service
        .calculate_loaded_settlement_cost_for_provider(
            "runtime_provider",
            "batch-priced-model",
            &usage,
        )
        .expect("batch settlement pricing");

    assert!((reservation.input_cost - 0.004).abs() < 1e-12);
    assert!((reservation.output_cost - 0.005).abs() < 1e-12);
    assert!((reservation.cache_cost - 0.0002).abs() < 1e-12);
    assert!((reservation.total_cost - 0.0092).abs() < 1e-12);
    assert!((settlement.total_cost - reservation.total_cost).abs() < 1e-12);
    assert!(
        service
            .calculate_loaded_usage_cost_for_provider(
                "runtime_provider",
                "batch-priced-model-lookalike",
                &usage,
            )
            .is_err(),
        "batch pricing must retain exact model authority"
    );
}

#[test]
fn batch_usage_fails_closed_for_missing_rates_and_cache_storage() {
    let service = PricingService::new(None);
    let mut model_info = test_model_info("runtime_provider");
    model_info.extra.extend([
        (
            "input_cost_per_token_batches".to_string(),
            serde_json::json!(0.000005),
        ),
        (
            "output_cost_per_token_batches".to_string(),
            serde_json::json!(0.000010),
        ),
    ]);
    service.add_custom_model("partial-batch-model".to_string(), model_info);

    let mut cache_read = PricingUsage::new(1_000, 500);
    cache_read.cached_tokens = Some(200);
    cache_read.billing_mode = PricingBillingMode::Batch;
    let error = service
        .calculate_loaded_usage_cost_for_provider(
            "runtime_provider",
            "partial-batch-model",
            &cache_read,
        )
        .expect_err("missing batch cache-read pricing must fail");
    assert!(
        error
            .to_string()
            .contains("cache_read_input_token_cost_batches")
    );

    let mut cache_storage = PricingUsage::new(1_000, 500);
    cache_storage.cache_creation_tokens = Some(200);
    cache_storage.billing_mode = PricingBillingMode::Batch;
    let error = service
        .calculate_loaded_settlement_cost_for_provider(
            "runtime_provider",
            "partial-batch-model",
            &cache_storage,
        )
        .expect_err("token-hour cache storage must not use a token rate");
    assert!(error.to_string().contains("cache creation/storage"));

    for (missing_key, prompt_tokens, completion_tokens) in [
        ("input_cost_per_token_batches", 1_000, 0),
        ("output_cost_per_token_batches", 0, 500),
    ] {
        let mut model_info = test_model_info("runtime_provider");
        for (key, rate) in [
            ("input_cost_per_token_batches", 0.000005),
            ("output_cost_per_token_batches", 0.000010),
            ("cache_read_input_token_cost_batches", 0.000001),
        ] {
            if key != missing_key {
                model_info
                    .extra
                    .insert(key.to_string(), serde_json::json!(rate));
            }
        }
        let model = format!("missing-{missing_key}");
        service.add_custom_model(model.clone(), model_info);
        let mut usage = PricingUsage::new(prompt_tokens, completion_tokens);
        usage.billing_mode = PricingBillingMode::Batch;
        let error = service
            .calculate_loaded_usage_cost_for_provider("runtime_provider", &model, &usage)
            .expect_err("a required batch token rate must not fall back to standard pricing");
        assert!(error.to_string().contains(missing_key));
    }
}

#[test]
fn embedded_gemini_flash_batch_rates_cover_input_output_and_cache_read() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));
    let mut usage = PricingUsage::new(1_000, 500);
    usage.cached_tokens = Some(200);
    usage.billing_mode = PricingBillingMode::Batch;

    for model in ["gemini-3.6-flash", "gemini-3.7-flash"] {
        let cost = service
            .calculate_loaded_usage_cost_for_provider("gemini", model, &usage)
            .unwrap_or_else(|error| panic!("{model} batch pricing should resolve: {error}"));
        assert!((cost.input_cost - 0.0003).abs() < 1e-12, "{model}");
        assert!((cost.output_cost - 0.0009375).abs() < 1e-12, "{model}");
        assert!((cost.cache_cost - 0.0000075).abs() < 1e-12, "{model}");
    }
}

#[test]
fn standard_usage_ignores_batch_rates() {
    let service = PricingService::new(None);
    let mut model_info = test_model_info("runtime_provider");
    model_info.extra.extend([
        (
            "input_cost_per_token_batches".to_string(),
            serde_json::json!(0.5),
        ),
        (
            "output_cost_per_token_batches".to_string(),
            serde_json::json!(0.5),
        ),
    ]);
    service.add_custom_model("standard-priced-model".to_string(), model_info);

    let cost = service
        .calculate_loaded_usage_cost_for_provider(
            "runtime_provider",
            "standard-priced-model",
            &PricingUsage::new(1_000, 500),
        )
        .expect("standard pricing");
    assert!((cost.total_cost - 0.025).abs() < 1e-12);
}

#[test]
fn provider_scoped_authority_preserves_native_slash_namespaces() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));

    for (provider, model, expected) in [
        (
            "together_ai",
            "BAAI/bge-base-en-v1.5",
            "together_ai/BAAI/bge-base-en-v1.5",
        ),
        (
            "together_ai",
            "openai/gpt-oss-120b",
            "together_ai/openai/gpt-oss-120b",
        ),
        (
            "anyscale",
            "google/gemma-7b-it",
            "anyscale/google/gemma-7b-it",
        ),
    ] {
        let Some((resolved, info)) = service.get_model_info_for_provider(provider, model) else {
            panic!("{provider}/{model} should retain its provider-native namespace");
        };
        assert_eq!(resolved, expected);
        assert_eq!(
            crate::core::pricing::normalize_pricing_provider(&info.litellm_provider),
            provider
        );
    }
}

#[test]
fn provider_scoped_authority_preserves_region_and_deployment_qualifiers() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));

    for (model, expected) in [
        ("us/gpt-5.1", "azure/us/gpt-5.1"),
        ("AZURE/EU/GPT-5.1", "azure/eu/gpt-5.1"),
        ("gpt-5.1", "azure/gpt-5.1"),
    ] {
        let Some((resolved, _)) = service.get_model_info_for_provider("azure", model) else {
            panic!("Azure pricing should resolve {model}");
        };
        assert_eq!(resolved, expected);
    }

    service.add_custom_model(
        "azure/deployment-blue/gpt-5.1".to_string(),
        test_model_info("azure"),
    );
    let Some((resolved, _)) =
        service.get_model_info_for_provider("azure", "deployment-blue/gpt-5.1")
    else {
        panic!("deployment-qualified model should resolve without losing its qualifier");
    };
    assert_eq!(resolved, "azure/deployment-blue/gpt-5.1");

    let Some((plain, _)) = service.get_model_info_for_provider("azure", "gpt-5.1") else {
        panic!("plain Azure model should retain its generic exact row");
    };
    assert_eq!(plain, "azure/gpt-5.1");
}

#[test]
fn provider_scoped_authority_resolves_vertex_publishers_but_not_unknown_suffixes() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));

    let Some((resolved, info)) =
        service.get_model_info_for_provider("vertex_ai", "meta/llama-4-scout-17b-16e-instruct")
    else {
        panic!("Vertex publisher alias should resolve through its bounded explicit alias");
    };
    assert_eq!(
        resolved,
        "vertex_ai/meta/llama-4-scout-17b-16e-instruct-maas"
    );
    assert_eq!(info.litellm_provider, "vertex_ai-llama_models");
    for (wire_model, expected_pricing_model, expected_provider) in [
        (
            "ai21/jamba-1.5-large",
            "vertex_ai/jamba-1.5-large",
            "vertex_ai-ai21_models",
        ),
        (
            "mistral/mistral-large-2411",
            "vertex_ai/mistral-large-2411",
            "vertex_ai-mistral_models",
        ),
    ] {
        let (resolved, info) = service
            .get_model_info_for_provider("vertex_ai", wire_model)
            .unwrap_or_else(|| panic!("{wire_model} should use an exact Vertex pricing alias"));
        assert_eq!(resolved, expected_pricing_model);
        assert_eq!(info.litellm_provider, expected_provider);
        assert!(
            service
                .get_model_info_for_provider("vertex_ai", &format!("{wire_model}-unknown"))
                .is_none(),
            "{wire_model} lookalike must fail closed"
        );
    }
    assert!(
        service
            .get_model_info_for_provider("vertex_ai", "gemini-1.5-pro-9999")
            .is_none()
    );
}

#[test]
fn provider_scoped_casefold_keeps_cross_provider_collisions() {
    let service = PricingService::new(None);
    let mut gemini = test_model_info("gemini");
    gemini.input_cost_per_token = Some(0.000_001);
    let mut vertex = test_model_info("vertex_ai");
    vertex.input_cost_per_token = Some(0.000_002);
    service.add_custom_model("Model-X".to_string(), gemini);
    service.add_custom_model("model-x".to_string(), vertex);

    let (gemini_key, gemini_info) = service
        .get_model_info_for_provider("gemini", "MODEL-X")
        .expect("Gemini collision entry should remain scoped");
    let (vertex_key, vertex_info) = service
        .get_model_info_for_provider("vertex_ai", "MODEL-X")
        .expect("Vertex collision entry should remain scoped");
    assert_eq!(gemini_key, "Model-X");
    assert_eq!(gemini_info.input_cost_per_token, Some(0.000_001));
    assert_eq!(vertex_key, "model-x");
    assert_eq!(vertex_info.input_cost_per_token, Some(0.000_002));
}

#[test]
fn provider_scoped_casefold_is_lexical_regardless_of_insertion_order() {
    fn build(reverse: bool) -> PricingService {
        let service = PricingService::new(None);
        let lower = ("model-x".to_string(), test_model_info("runtime_provider"));
        let upper = ("Model-X".to_string(), test_model_info("runtime_provider"));
        let entries = if reverse {
            vec![upper, lower]
        } else {
            vec![lower, upper]
        };
        for (model, info) in entries {
            service.add_custom_model(model, info);
        }
        service
    }

    for service in [build(false), build(true)] {
        let (mixed, _) = service
            .get_model_info_for_provider("runtime_provider", "MODEL-X")
            .expect("mixed-case lookup should use deterministic canonical key");
        assert_eq!(mixed, "Model-X");
        assert_eq!(
            service
                .get_model_info_for_provider("runtime_provider", "model-x")
                .expect("exact lowercase spelling should win")
                .0,
            "model-x"
        );
    }
}

#[test]
fn provider_scoped_exact_rows_precede_old_fuzzy_aliases() {
    let service = PricingService::with_embedded_default()
        .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));

    assert_eq!(
        service
            .get_model_info_for_provider("openai_like", "xai/grok-4.3")
            .expect("xAI exact row should resolve")
            .0,
        "xai/grok-4.3"
    );
    assert_eq!(
        service
            .get_model_info_for_provider("azure", "gpt-5.5")
            .expect("Azure exact row should resolve")
            .0,
        "azure/gpt-5.5"
    );

    assert!(
        service
            .get_model_info_for_provider("anthropic", "mimo-v2.5-pro")
            .is_some()
    );
    assert_eq!(
        service
            .get_model_info_for_provider("amazon_nova", "nova-2-lite")
            .expect("Amazon Nova explicit catalog alias should remain supported")
            .0,
        "amazon.nova-2-lite-v1:0"
    );
}
