use super::*;

// Helper function to create basic usage
fn create_usage(prompt_tokens: u32, completion_tokens: u32) -> UsageTokens {
    UsageTokens::new(prompt_tokens, completion_tokens)
}

fn assert_cost_eq(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "expected {expected}, got {actual}"
    );
}

// Tests for generic_cost_per_token
#[test]
fn test_generic_cost_per_token_basic() {
    let usage = create_usage(1000, 500);
    let result = generic_cost_per_token("gpt-4o-mini", &usage, "openai");

    assert!(result.is_ok());
    let breakdown = result.unwrap();
    assert_eq!(breakdown.model, "gpt-4o-mini");
    assert_eq!(breakdown.provider, "openai");
    assert_eq!(breakdown.usage.prompt_tokens, 1000);
    assert_eq!(breakdown.usage.completion_tokens, 500);

    // Expected: 1000 tokens * 0.00015 / 1k = 0.00015
    // Expected: 500 tokens * 0.0006 / 1k = 0.0003
    assert!((breakdown.input_cost - 0.00015).abs() < 1e-6);
    assert!((breakdown.output_cost - 0.0003).abs() < 1e-6);
    assert!((breakdown.total_cost - 0.00045).abs() < 1e-6);
}

#[test]
fn test_generic_cost_per_token_with_cache() {
    let mut usage = create_usage(2000, 1000);
    usage.cached_tokens = Some(500);

    let result = generic_cost_per_token("gpt-4o", &usage, "openai");
    assert!(result.is_ok());
    let breakdown = result.unwrap();

    // Input cost should only be for non-cached tokens (2000 - 500 = 1500)
    let expected_input = (1500.0 / 1000.0) * 0.0025;
    assert!((breakdown.input_cost - expected_input).abs() < 1e-6);
    // Note: cache_cost may be 0 if pricing data doesn't include cache_read_input_token_cost
    // The important thing is that input cost is calculated correctly excluding cached tokens
}

#[test]
fn test_generic_cost_per_token_with_reasoning() {
    let mut usage = create_usage(1000, 500);
    usage.reasoning_tokens = Some(200);

    // Create custom pricing with reasoning cost
    let result = generic_cost_per_token("gpt-4o", &usage, "openai");
    assert!(result.is_ok());
    // Reasoning cost should be calculated if pricing supports it
}

#[test]
fn test_generic_cost_per_token_unsupported_model() {
    let usage = create_usage(1000, 500);
    let result = generic_cost_per_token("unknown-model", &usage, "openai");

    assert!(result.is_err());
    match result.unwrap_err() {
        CostError::ModelNotSupported { model, provider } => {
            assert_eq!(model, "unknown-model");
            assert_eq!(provider, "openai");
        }
        _ => panic!("Expected ModelNotSupported error"),
    }
}

#[test]
fn test_generic_cost_per_token_unsupported_provider() {
    let usage = create_usage(1000, 500);
    let result = generic_cost_per_token("gpt-4o", &usage, "unknown-provider");

    assert!(result.is_err());
    match result.unwrap_err() {
        CostError::ProviderNotSupported { provider } => {
            assert_eq!(provider, "unknown-provider");
        }
        _ => panic!("Expected ProviderNotSupported error"),
    }
}

// Tests for get_model_pricing
#[test]
fn test_get_openai_pricing_gpt4o_mini() {
    let pricing = get_model_pricing("gpt-4o-mini", "openai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.00015);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0006);
    assert_eq!(pricing.currency, "USD");
}

#[test]
fn test_get_openai_pricing_gpt4o() {
    let pricing = get_model_pricing("gpt-4o", "openai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0025);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.01);
}

#[test]
fn test_cost_pricing_prefers_shared_litellm_source() {
    let pricing = get_model_pricing("gpt-3.5-turbo", "openai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0015);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.002);
}

#[test]
fn test_get_openai_pricing_gpt4_turbo() {
    let pricing = get_model_pricing("gpt-4-turbo", "openai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.01);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.03);
}

#[test]
fn test_get_openai_pricing_gpt35_turbo() {
    let pricing = get_model_pricing("gpt-3.5-turbo", "openai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0015);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.002);
}

#[test]
fn test_get_anthropic_pricing_claude35_sonnet() {
    let pricing = get_model_pricing("claude-3-5-sonnet", "anthropic");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.003);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.015);
}

#[test]
fn test_get_anthropic_pricing_claude_opus_46() {
    let pricing = get_model_pricing("claude-opus-4-6", "anthropic");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.025);
}

#[test]
fn test_get_anthropic_pricing_claude_opus_47() {
    let pricing = get_model_pricing("claude-opus-4-7", "anthropic");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.025);
}

#[test]
fn test_get_anthropic_pricing_claude_sonnet_45() {
    let pricing = get_model_pricing("claude-sonnet-4-5", "anthropic");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.003);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.015);
}

#[test]
fn test_get_anthropic_pricing_claude35_haiku() {
    let pricing = get_model_pricing("claude-3-5-haiku", "anthropic");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.001);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.005);
}

#[test]
fn test_get_anthropic_pricing_claude3_haiku() {
    let pricing = get_model_pricing("claude-3-haiku", "anthropic");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.00025);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.00125);
}

#[test]
fn test_get_vertex_ai_pricing_gemini_pro() {
    let pricing = get_model_pricing("gemini-pro", "vertex_ai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_cost_eq(pricing.input_cost_per_1k_tokens, 0.00025);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.0005);
}

#[test]
fn test_get_vertex_ai_pricing_gemini_flash() {
    let pricing = get_model_pricing("gemini-flash", "vertexai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.000075);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0003);
}

#[test]
fn test_get_vertex_ai_pricing_gemini_35_flash() {
    let pricing = get_model_pricing("gemini-3.5-flash", "vertex_ai");
    let Ok(pricing) = pricing else {
        panic!("gemini-3.5-flash pricing should load from shared pricing data");
    };
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0015);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.009);
    assert_eq!(pricing.cache_read_input_token_cost, Some(0.00015));
}

#[test]
fn test_get_deepseek_pricing() {
    let Ok(flash) = get_model_pricing("deepseek-v4-flash", "deepseek") else {
        panic!("deepseek-v4-flash pricing should be available");
    };
    assert_cost_eq(flash.input_cost_per_1k_tokens, 0.00014);
    assert_cost_eq(flash.output_cost_per_1k_tokens, 0.00028);
    assert_eq!(flash.cache_read_input_token_cost, Some(0.0000028));

    let Ok(pro) = get_model_pricing("deepseek-v4-pro", "deepseek") else {
        panic!("deepseek-v4-pro pricing should be available");
    };
    assert_cost_eq(pro.input_cost_per_1k_tokens, 0.000435);
    assert_cost_eq(pro.output_cost_per_1k_tokens, 0.00087);
    assert_eq!(pro.cache_read_input_token_cost, Some(0.000003625));

    let Ok(chat_alias) = get_model_pricing("deepseek-chat", "deepseek") else {
        panic!("deepseek-chat alias pricing should be available");
    };
    assert_cost_eq(chat_alias.input_cost_per_1k_tokens, 0.00014);
    assert_cost_eq(chat_alias.output_cost_per_1k_tokens, 0.00028);

    let Ok(reasoner_alias) = get_model_pricing("deepseek-reasoner", "deepseek") else {
        panic!("deepseek-reasoner alias pricing should be available");
    };
    assert_cost_eq(reasoner_alias.input_cost_per_1k_tokens, 0.00014);
    assert_cost_eq(reasoner_alias.output_cost_per_1k_tokens, 0.00028);
}

#[test]
fn test_get_moonshot_pricing_8k() {
    let pricing = get_model_pricing("moonshot-v1-8k", "moonshot");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_cost_eq(pricing.input_cost_per_1k_tokens, 0.0002);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.002);
}

#[test]
fn test_get_moonshot_pricing_32k() {
    let pricing = get_model_pricing("moonshot-v1-32k", "moonshot");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.001);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.003);
}

#[test]
fn test_get_moonshot_pricing_128k() {
    let pricing = get_model_pricing("moonshot-v1-128k", "moonshot");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.002);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.005);
}

#[test]
fn test_get_moonshot_pricing_kimi_k2_5() {
    let pricing = get_model_pricing("kimi-k2.5", "moonshot");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0006);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.003);
}

#[test]
fn test_get_moonshot_pricing_kimi_k2_6() {
    let pricing = get_model_pricing("kimi-k2.6", "moonshot");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.00095);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.004);
}

#[test]
fn test_get_minimax_pricing_m2_5() {
    let pricing = get_model_pricing("MiniMax-M2.5", "minimax");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0003);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0012);
}

#[test]
fn test_get_zhipu_pricing_glm_5() {
    let pricing = get_model_pricing("glm-5", "zhipuai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.001);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.0032);
}

#[test]
fn test_get_zhipu_pricing_glm_5_1() {
    let pricing = get_model_pricing("glm-5.1", "zhipuai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0014);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0044);
}

#[test]
fn test_get_zhipu_pricing_glm_4_flash() {
    let pricing = get_model_pricing("glm-4-flash", "zhipuai");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_cost_eq(pricing.input_cost_per_1k_tokens, 0.00005);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.0001);
}

#[test]
fn test_get_azure_pricing() {
    let pricing = get_model_pricing("gpt-4o", "azure");
    assert!(pricing.is_ok());
    // Azure uses OpenAI pricing
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
}

// Tests for calculate_input_cost
#[test]
fn test_calculate_input_cost_no_cache() {
    let usage = create_usage(1000, 500);
    let cost = calculate_input_cost(&usage, 1.0);
    assert_eq!(cost, 1.0);
}

#[test]
fn test_calculate_input_cost_with_cache() {
    let mut usage = create_usage(2000, 500);
    usage.cached_tokens = Some(500);
    let cost = calculate_input_cost(&usage, 1.0);
    // Should only charge for 1500 non-cached tokens
    assert_eq!(cost, 1.5);
}

#[test]
fn test_calculate_input_cost_all_cached() {
    let mut usage = create_usage(1000, 500);
    usage.cached_tokens = Some(1000);
    let cost = calculate_input_cost(&usage, 1.0);
    // All tokens cached, should be 0
    assert_eq!(cost, 0.0);
}

#[test]
fn test_calculate_input_cost_zero_tokens() {
    let usage = create_usage(0, 500);
    let cost = calculate_input_cost(&usage, 1.0);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_output_cost
#[test]
fn test_calculate_output_cost_basic() {
    let usage = create_usage(1000, 500);
    let cost = calculate_output_cost(&usage, 2.0);
    assert_eq!(cost, 1.0); // 500 / 1000 * 2.0
}

#[test]
fn test_calculate_output_cost_zero() {
    let usage = create_usage(1000, 0);
    let cost = calculate_output_cost(&usage, 2.0);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_cache_cost
#[test]
fn test_calculate_cache_cost() {
    let cost = calculate_cache_cost(1000, 0.5, 0.1);
    // Using read cost: 1000 / 1000 * 0.1 = 0.1
    assert_eq!(cost, 0.1);
}

#[test]
fn test_calculate_cache_cost_zero_tokens() {
    let cost = calculate_cache_cost(0, 0.5, 0.1);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_audio_cost
#[test]
fn test_calculate_audio_cost_with_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        input_cost_per_audio_token: Some(0.001),
        ..Default::default()
    };

    let cost = calculate_audio_cost(&pricing, 1000);
    assert_eq!(cost, 1.0); // 1000 * 0.001
}

#[test]
fn test_calculate_audio_cost_no_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        ..Default::default()
    };

    let cost = calculate_audio_cost(&pricing, 1000);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_image_cost
#[test]
fn test_calculate_image_cost_with_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        image_cost_per_token: Some(0.002),
        ..Default::default()
    };

    let cost = calculate_image_cost(&pricing, 500);
    assert_eq!(cost, 1.0); // 500 * 0.002
}

#[test]
fn test_calculate_image_cost_no_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        ..Default::default()
    };

    let cost = calculate_image_cost(&pricing, 500);
    assert_eq!(cost, 0.0);
}

// Tests for calculate_reasoning_cost
#[test]
fn test_calculate_reasoning_cost_with_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        reasoning_cost_per_token: Some(0.003),
        ..Default::default()
    };

    let cost = calculate_reasoning_cost(&pricing, 300);
    assert_eq!(cost, 0.9); // 300 * 0.003
}

#[test]
fn test_calculate_reasoning_cost_no_pricing() {
    use chrono::Utc;
    let pricing = ModelPricing {
        model: "test".to_string(),
        input_cost_per_1k_tokens: 0.0,
        output_cost_per_1k_tokens: 0.0,
        currency: "USD".to_string(),
        updated_at: Utc::now(),
        ..Default::default()
    };

    let cost = calculate_reasoning_cost(&pricing, 300);
    assert_eq!(cost, 0.0);
}

// Tests for estimate_cost
#[test]
fn test_estimate_cost_basic() {
    let result = estimate_cost("gpt-4o-mini", "openai", 1000, Some(500));
    assert!(result.is_ok());
    let estimate = result.unwrap();

    let expected_input = (1000.0 / 1000.0) * 0.00015;
    let expected_output = (500.0 / 1000.0) * 0.0006;

    assert!((estimate.input_cost - expected_input).abs() < 1e-6);
    assert!((estimate.estimated_output_cost - expected_output).abs() < 1e-6);
    assert_eq!(estimate.min_cost, expected_input);
    assert!((estimate.max_cost - (expected_input + expected_output)).abs() < 1e-6);
    assert_eq!(estimate.currency, "USD");
}

#[test]
fn test_estimate_cost_no_max_output() {
    let result = estimate_cost("gpt-4o", "openai", 1000, None);
    assert!(result.is_ok());
    let estimate = result.unwrap();

    // Should use default 100 tokens
    let expected_output = (100.0 / 1000.0) * 0.01;
    assert!((estimate.estimated_output_cost - expected_output).abs() < 1e-6);
}

#[test]
fn test_estimate_cost_unsupported_model() {
    let result = estimate_cost("unknown-model", "openai", 1000, Some(500));
    assert!(result.is_err());
}

// Tests for compare_model_costs
#[test]
fn test_compare_model_costs_single_model() {
    let models = vec![("gpt-4o-mini".to_string(), "openai".to_string())];
    let comparisons = compare_model_costs(&models, 1000, 500);

    assert_eq!(comparisons.len(), 1);
    assert_eq!(comparisons[0].model, "gpt-4o-mini");
    assert_eq!(comparisons[0].provider, "openai");
    assert!(comparisons[0].total_cost > 0.0);
    assert!(comparisons[0].cost_per_token > 0.0);
    assert!(comparisons[0].efficiency_score > 0.0);
}

#[test]
fn test_compare_model_costs_multiple_models() {
    let models = vec![
        ("gpt-4o".to_string(), "openai".to_string()),
        ("gpt-4o-mini".to_string(), "openai".to_string()),
        ("claude-3-haiku".to_string(), "anthropic".to_string()),
    ];
    let comparisons = compare_model_costs(&models, 1000, 500);

    assert_eq!(comparisons.len(), 3);

    // Should be sorted by cost (lowest first)
    for i in 1..comparisons.len() {
        assert!(comparisons[i - 1].total_cost <= comparisons[i].total_cost);
    }

    // Verify efficiency score calculation
    for comparison in &comparisons {
        let expected_efficiency = 1500.0 / comparison.total_cost;
        assert!((comparison.efficiency_score - expected_efficiency).abs() < 1e-6);
    }
}

#[test]
fn test_compare_model_costs_with_invalid_model() {
    let models = vec![
        ("gpt-4o-mini".to_string(), "openai".to_string()),
        ("invalid-model".to_string(), "openai".to_string()),
        ("claude-3-haiku".to_string(), "anthropic".to_string()),
    ];
    let comparisons = compare_model_costs(&models, 1000, 500);

    // Should only include valid models
    assert_eq!(comparisons.len(), 2);
}

#[test]
fn test_compare_model_costs_empty_list() {
    let models: Vec<(String, String)> = vec![];
    let comparisons = compare_model_costs(&models, 1000, 500);
    assert_eq!(comparisons.len(), 0);
}

#[test]
fn test_compare_model_costs_zero_tokens() {
    let models = vec![("gpt-4o-mini".to_string(), "openai".to_string())];
    let comparisons = compare_model_costs(&models, 0, 0);

    // Should handle zero tokens gracefully
    assert_eq!(comparisons.len(), 1);
    assert_eq!(comparisons[0].total_cost, 0.0);
}

// Tests for cost breakdown calculation with all features
#[test]
fn test_generic_cost_per_token_all_features() {
    let mut usage = create_usage(5000, 2000);
    usage.cached_tokens = Some(1000);
    usage.audio_tokens = Some(500);
    usage.image_tokens = Some(300);
    usage.reasoning_tokens = Some(200);

    let result = generic_cost_per_token("gpt-4o", &usage, "openai");
    assert!(result.is_ok());
    let breakdown = result.unwrap();

    // Verify total is sum of all components
    let calculated_total = breakdown.input_cost
        + breakdown.output_cost
        + breakdown.cache_cost
        + breakdown.audio_cost
        + breakdown.image_cost
        + breakdown.reasoning_cost;

    assert!((breakdown.total_cost - calculated_total).abs() < 1e-10);
}

// Edge case tests
#[test]
fn test_large_token_counts() {
    let usage = create_usage(1_000_000, 500_000);
    let result = generic_cost_per_token("gpt-4o", &usage, "openai");
    assert!(result.is_ok());
    let breakdown = result.unwrap();
    assert!(breakdown.total_cost > 0.0);
    assert!(breakdown.total_cost < 1_000_000.0); // Sanity check
}

#[test]
fn test_case_insensitive_model_names() {
    let usage = create_usage(1000, 500);

    let result1 = generic_cost_per_token("GPT-4O-MINI", &usage, "openai");
    let result2 = generic_cost_per_token("gpt-4o-mini", &usage, "openai");
    let result3 = generic_cost_per_token("Gpt-4O-Mini", &usage, "openai");

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());

    let cost1 = result1.unwrap().total_cost;
    let cost2 = result2.unwrap().total_cost;
    let cost3 = result3.unwrap().total_cost;

    assert!((cost1 - cost2).abs() < 1e-10);
    assert!((cost2 - cost3).abs() < 1e-10);
}

#[test]
fn test_case_insensitive_provider_names() {
    let result1 = get_model_pricing("gpt-4o", "OpenAI");
    let result2 = get_model_pricing("gpt-4o", "OPENAI");
    let result3 = get_model_pricing("gpt-4o", "openai");

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());
}

#[test]
fn test_new_openai_models_have_cost_pricing() {
    let Ok(gpt55) = get_model_pricing("gpt-5.5", "openai") else {
        panic!("gpt-5.5 should have OpenAI pricing");
    };
    assert_eq!(gpt55.input_cost_per_1k_tokens, 0.005);
    assert_eq!(gpt55.output_cost_per_1k_tokens, 0.030);

    let Ok(gpt55_pro) = get_model_pricing("gpt-5.5-pro", "openai") else {
        panic!("gpt-5.5-pro should have OpenAI pricing");
    };
    assert_eq!(gpt55_pro.input_cost_per_1k_tokens, 0.030);
    assert_eq!(gpt55_pro.output_cost_per_1k_tokens, 0.180);

    let gpt54_pro = get_model_pricing("gpt-5.4-pro", "openai").unwrap();
    assert_eq!(gpt54_pro.input_cost_per_1k_tokens, 0.030);
    assert_eq!(gpt54_pro.output_cost_per_1k_tokens, 0.180);

    let realtime = get_model_pricing("gpt-realtime-1.5", "openai").unwrap();
    assert_eq!(realtime.input_cost_per_1k_tokens, 0.004);
    assert_eq!(realtime.output_cost_per_1k_tokens, 0.016);

    let deep_research = get_model_pricing("o3-deep-research", "openai").unwrap();
    assert_eq!(deep_research.input_cost_per_1k_tokens, 0.010);
    assert_eq!(deep_research.output_cost_per_1k_tokens, 0.040);
}

#[test]
fn test_new_anthropic_models_have_cost_pricing() {
    let opus47 = get_model_pricing("claude-opus-4-7", "anthropic").unwrap();
    assert_eq!(opus47.input_cost_per_1k_tokens, 0.005);
    assert_eq!(opus47.output_cost_per_1k_tokens, 0.025);

    let opus41 = get_model_pricing("claude-opus-4-1-20250805", "anthropic").unwrap();
    assert_eq!(opus41.input_cost_per_1k_tokens, 0.015);
    assert_eq!(opus41.output_cost_per_1k_tokens, 0.075);

    let opus4 = get_model_pricing("claude-opus-4-20250514", "anthropic").unwrap();
    assert_eq!(opus4.input_cost_per_1k_tokens, 0.015);
    assert_eq!(opus4.output_cost_per_1k_tokens, 0.075);

    let haiku45 = get_model_pricing("claude-haiku-4-5-20251001", "anthropic").unwrap();
    assert_eq!(haiku45.input_cost_per_1k_tokens, 0.001);
    assert_eq!(haiku45.output_cost_per_1k_tokens, 0.005);
}

#[test]
fn test_vertex_ai_provider_variants() {
    let usage = create_usage(1000, 500);

    let result1 = generic_cost_per_token("gemini-pro", &usage, "vertex_ai");
    let result2 = generic_cost_per_token("gemini-pro", &usage, "vertexai");

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    let cost1 = result1.unwrap().total_cost;
    let cost2 = result2.unwrap().total_cost;
    assert!((cost1 - cost2).abs() < 1e-10);
}

#[test]
fn test_cached_tokens_exceed_prompt_tokens() {
    // Edge case: cached tokens shouldn't exceed prompt tokens
    let mut usage = create_usage(1000, 500);
    usage.cached_tokens = Some(1500);

    let result = generic_cost_per_token("gpt-4o", &usage, "openai");
    assert!(result.is_ok());

    // Input cost should be 0 due to saturation
    let breakdown = result.unwrap();
    assert_eq!(breakdown.input_cost, 0.0);
}

// Integration tests
#[test]
fn test_cost_calculation_workflow() {
    // Simulate a complete workflow
    let usage = create_usage(2000, 1000);

    // 1. Get pricing
    let pricing = get_model_pricing("gpt-4o-mini", "openai");
    assert!(pricing.is_ok());

    // 2. Calculate cost
    let breakdown = generic_cost_per_token("gpt-4o-mini", &usage, "openai");
    assert!(breakdown.is_ok());
    let breakdown = breakdown.unwrap();

    // 3. Verify breakdown structure
    assert_eq!(breakdown.model, "gpt-4o-mini");
    assert_eq!(breakdown.provider, "openai");
    assert_eq!(breakdown.currency, "USD");
    assert!(breakdown.total_cost > 0.0);
    assert_eq!(breakdown.usage.total_tokens, 3000);
}

#[test]
fn test_estimate_and_actual_cost_consistency() {
    let input_tokens = 1000;
    let output_tokens = 500;

    // Estimate cost
    let estimate = estimate_cost("gpt-4o", "openai", input_tokens, Some(output_tokens));
    assert!(estimate.is_ok());
    let estimate = estimate.unwrap();

    // Calculate actual cost
    let usage = create_usage(input_tokens, output_tokens);
    let breakdown = generic_cost_per_token("gpt-4o", &usage, "openai");
    assert!(breakdown.is_ok());
    let breakdown = breakdown.unwrap();

    // Actual cost should match estimate max_cost
    assert!((breakdown.total_cost - estimate.max_cost).abs() < 1e-10);
    assert!((breakdown.input_cost - estimate.input_cost).abs() < 1e-10);
}

fn model_info_from_json(value: serde_json::Value) -> crate::core::pricing::LiteLLMModelInfo {
    serde_json::from_value(value).expect("valid LiteLLMModelInfo json")
}

#[test]
fn test_litellm_pricing_errors_when_both_token_costs_missing() {
    // A catalog entry with neither input nor output cost must not bill at $0.
    let info = model_info_from_json(serde_json::json!({
        "litellm_provider": "openai",
        "mode": "chat"
    }));
    let result = litellm_to_cost_pricing("mystery-model", &info);
    assert!(matches!(
        result,
        Err(CostError::MissingPricing { ref model }) if model == "mystery-model"
    ));
}

#[test]
fn test_litellm_pricing_allows_single_missing_side() {
    // Only one side missing: priced, the missing side billed at 0.
    let info = model_info_from_json(serde_json::json!({
        "litellm_provider": "openai",
        "mode": "chat",
        "input_cost_per_token": 0.000_01
    }));
    let pricing = litellm_to_cost_pricing("half-priced", &info).expect("should be priced");
    assert!(pricing.input_cost_per_1k_tokens > 0.0);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0);
}

#[test]
fn test_litellm_pricing_ok_when_both_present() {
    let info = model_info_from_json(serde_json::json!({
        "litellm_provider": "openai",
        "mode": "chat",
        "input_cost_per_token": 0.000_01,
        "output_cost_per_token": 0.000_03
    }));
    let pricing = litellm_to_cost_pricing("full", &info).expect("should be priced");
    assert!(pricing.input_cost_per_1k_tokens > 0.0);
    assert!(pricing.output_cost_per_1k_tokens > 0.0);
}
