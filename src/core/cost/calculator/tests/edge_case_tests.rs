use super::*;

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
    let opus48 = get_model_pricing("claude-opus-4-8", "anthropic").unwrap();
    assert_eq!(opus48.input_cost_per_1k_tokens, 0.005);
    assert_eq!(opus48.output_cost_per_1k_tokens, 0.025);

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
    let result3 = generic_cost_per_token("gemini-pro", &usage, "google");

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());

    let cost1 = result1.unwrap().total_cost;
    let cost2 = result2.unwrap().total_cost;
    let cost3 = result3.unwrap().total_cost;
    assert!((cost1 - cost2).abs() < 1e-10);
    assert!((cost1 - cost3).abs() < 1e-10);
}

#[test]
fn test_cost_calculator_uses_shared_provider_normalization() {
    let google = get_model_pricing("gemini-1.5-flash", "google").unwrap();
    let vertex_ai = get_model_pricing("gemini-1.5-flash", "vertex_ai").unwrap();
    assert_eq!(
        google.input_cost_per_1k_tokens,
        vertex_ai.input_cost_per_1k_tokens
    );
    assert_eq!(
        google.output_cost_per_1k_tokens,
        vertex_ai.output_cost_per_1k_tokens
    );

    let glm = get_model_pricing("glm-5", "glm").unwrap();
    let zhipuai = get_model_pricing("glm-5", "zhipuai").unwrap();
    assert_eq!(
        glm.input_cost_per_1k_tokens,
        zhipuai.input_cost_per_1k_tokens
    );
    assert_eq!(
        glm.output_cost_per_1k_tokens,
        zhipuai.output_cost_per_1k_tokens
    );
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
