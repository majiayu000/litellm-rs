use super::*;

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
fn test_shared_pricing_does_not_match_flash_lite_for_flash() {
    let pricing = get_model_pricing("gemini-3.1-flash", "vertex_ai");
    let Err(CostError::ModelNotSupported { model, provider }) = pricing else {
        panic!("gemini-3.1-flash must not borrow gemini-3.1-flash-lite shared pricing");
    };
    assert_eq!(model, "gemini-3.1-flash");
    assert_eq!(provider, "vertex_ai");
}

#[test]
fn test_runtime_pricing_reaches_groq_and_native_gemini_shared_rows() {
    let usage = create_usage(1000, 500);

    let gemini = generic_cost_per_token("gemini-3.1-flash-lite", &usage, "gemini")
        .expect("native Gemini provider should use shared Gemini pricing rows");
    assert_cost_eq(gemini.total_cost, 0.001);

    let groq = generic_cost_per_token("gpt-oss-120b", &usage, "groq")
        .expect("Groq provider should match prefixed shared pricing rows by model id");
    assert_cost_eq(groq.total_cost, 0.00045);
}

#[test]
fn test_runtime_pricing_uses_xiaomi_for_anthropic_compatible_mimo_models() {
    let usage = create_usage(1000, 500);

    let breakdown = generic_cost_per_token("mimo-v2.5", &usage, "anthropic")
        .expect("MiMo Anthropic-compatible routing should use Xiaomi pricing");

    assert_eq!(breakdown.provider, "anthropic");
    assert_cost_eq(breakdown.total_cost, 0.00028);
}

#[test]
fn test_runtime_pricing_keeps_unknown_anthropic_models_strict() {
    let usage = create_usage(1000, 500);
    let result = generic_cost_per_token("unknown-compatible-model", &usage, "anthropic");

    assert!(matches!(
        result,
        Err(CostError::ModelNotSupported { provider, .. }) if provider == "anthropic"
    ));
}

#[test]
fn test_runtime_pricing_normalizes_provider_prefixed_shared_models() {
    let usage = create_usage(1000, 500);

    let mimo = generic_cost_per_token("mimo/mimo-v2.5", &usage, "mimo")
        .expect("Mimo provider-prefixed model should resolve shared pricing");
    assert_cost_eq(mimo.total_cost, 0.00028);

    let groq = generic_cost_per_token("groq/llama-3.3-70b-versatile", &usage, "groq")
        .expect("Groq provider-prefixed model should resolve shared pricing");
    assert_cost_eq(groq.total_cost, 0.000985);
}

#[test]
fn test_runtime_pricing_supports_bedrock_provider_name() {
    let pricing = get_model_pricing("amazon.titan-text-express-v1", "bedrock")
        .expect("Bedrock provider should use embedded LiteLLM catalog pricing");

    assert_cost_eq(pricing.input_cost_per_1k_tokens, 0.0013);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.0017);
}

#[test]
fn test_runtime_pricing_supports_amazon_nova_provider_name() {
    let pricing = get_model_pricing("amazon.nova-2-lite-v1:0", "amazon_nova")
        .expect("Amazon Nova provider should use shared Bedrock catalog pricing");

    assert_cost_eq(pricing.input_cost_per_1k_tokens, 0.0003);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.0025);
}

#[cfg(feature = "providers-extended")]
#[test]
fn test_runtime_pricing_supports_amazon_nova_short_model_name() {
    let pricing = get_model_pricing("nova-2-lite", "amazon_nova")
        .expect("Amazon Nova provider should price short model aliases");

    assert_eq!(pricing.model, "amazon.nova-2-lite-v1:0");
    assert_cost_eq(pricing.input_cost_per_1k_tokens, 0.0003);
    assert_cost_eq(pricing.output_cost_per_1k_tokens, 0.0025);
}

#[test]
fn test_runtime_pricing_supports_prefixed_openai_like_models() {
    let usage = create_usage(1000, 500);

    let groq = generic_cost_per_token("groq/llama-3.3-70b-versatile", &usage, "openai_like")
        .expect("OpenAI-like provider should price explicitly prefixed Groq models");
    assert_cost_eq(groq.total_cost, 0.000985);

    let xai = generic_cost_per_token("xai/grok-4.3", &usage, "openai_like")
        .expect("OpenAI-like provider should price explicitly prefixed XAI models");
    assert_cost_eq(xai.total_cost, 0.0025);
}

#[test]
fn test_runtime_pricing_keeps_unprefixed_openai_like_models_strict() {
    let usage = create_usage(1000, 500);
    let result = generic_cost_per_token("grok-4.3", &usage, "openai_like");

    assert!(matches!(
        result,
        Err(CostError::ModelNotSupported { provider, .. }) if provider == "openai_like"
    ));
}

#[test]
fn test_get_deepseek_pricing() {
    let Ok(flash) = get_model_pricing("deepseek-v4-flash", "deepseek") else {
        panic!("deepseek-v4-flash pricing should be available");
    };
    assert_cost_eq(flash.input_cost_per_1k_tokens, 0.00022);
    assert_cost_eq(flash.output_cost_per_1k_tokens, 0.00066);
    assert_eq!(flash.cache_read_input_token_cost, Some(0.000007));

    let Ok(pro) = get_model_pricing("deepseek-v4-pro", "deepseek") else {
        panic!("deepseek-v4-pro pricing should be available");
    };
    assert_cost_eq(pro.input_cost_per_1k_tokens, 0.00066);
    assert_cost_eq(pro.output_cost_per_1k_tokens, 0.00198);
    assert_eq!(pro.cache_read_input_token_cost, Some(0.000022));

    let Ok(chat_alias) = get_model_pricing("deepseek-chat", "deepseek") else {
        panic!("deepseek-chat alias pricing should be available");
    };
    assert_cost_eq(chat_alias.input_cost_per_1k_tokens, 0.00022);
    assert_cost_eq(chat_alias.output_cost_per_1k_tokens, 0.00066);
    assert_eq!(chat_alias.cache_read_input_token_cost, Some(0.000007));

    let Ok(reasoner_alias) = get_model_pricing("deepseek-reasoner", "deepseek") else {
        panic!("deepseek-reasoner alias pricing should be available");
    };
    assert_cost_eq(reasoner_alias.input_cost_per_1k_tokens, 0.00022);
    assert_cost_eq(reasoner_alias.output_cost_per_1k_tokens, 0.00066);
    assert_eq!(reasoner_alias.cache_read_input_token_cost, Some(0.000007));

    for model in [
        "deepseek-v4-flash-vision-exp",
        "deepseek/deepseek-v4-flash-vision-exp",
    ] {
        let Ok(vision) = get_model_pricing(model, "deepseek") else {
            panic!("deepseek vision model '{model}' pricing should be available");
        };
        assert_cost_eq(vision.input_cost_per_1k_tokens, 0.00022);
        assert_cost_eq(vision.output_cost_per_1k_tokens, 0.00066);
        assert_eq!(vision.cache_read_input_token_cost, Some(0.000007));
    }
}

#[test]
fn test_deepseek_fallback_pricing() {
    let Ok(flash) = super::super::pricing::get_deepseek_pricing("deepseek-v4-flash") else {
        panic!("deepseek-v4-flash fallback pricing should be available");
    };
    assert_cost_eq(flash.input_cost_per_1k_tokens, 0.00022);
    assert_cost_eq(flash.output_cost_per_1k_tokens, 0.00066);
    assert_eq!(flash.cache_read_input_token_cost, Some(0.000007));

    let Ok(pro) = super::super::pricing::get_deepseek_pricing("deepseek-v4-pro") else {
        panic!("deepseek-v4-pro fallback pricing should be available");
    };
    assert_cost_eq(pro.input_cost_per_1k_tokens, 0.00066);
    assert_cost_eq(pro.output_cost_per_1k_tokens, 0.00198);
    assert_eq!(pro.cache_read_input_token_cost, Some(0.000022));

    let Ok(vision) = super::super::pricing::get_deepseek_pricing("deepseek-v4-flash-vision-exp")
    else {
        panic!("deepseek-v4-flash-vision-exp fallback pricing should be available");
    };
    assert_cost_eq(vision.input_cost_per_1k_tokens, 0.00022);
    assert_cost_eq(vision.output_cost_per_1k_tokens, 0.00066);
    assert_eq!(vision.cache_read_input_token_cost, Some(0.000007));

    let Ok(unlisted_vision_alias) =
        get_model_pricing("deepseek-v4-flash-vision-exp-unlisted", "deepseek")
    else {
        panic!("an unlisted DeepSeek V4 Flash alias should use fallback pricing");
    };
    assert_cost_eq(unlisted_vision_alias.input_cost_per_1k_tokens, 0.00022);
    assert_cost_eq(unlisted_vision_alias.output_cost_per_1k_tokens, 0.00066);
    assert_eq!(
        unlisted_vision_alias.cache_read_input_token_cost,
        Some(0.000007)
    );
}

#[test]
fn test_get_xiaomi_mimo_pricing() {
    let Ok(pro) = get_model_pricing("mimo-v2.5-pro", "xiaomi_mimo") else {
        panic!("mimo-v2.5-pro pricing should load from shared pricing data");
    };
    assert_cost_eq(pro.input_cost_per_1k_tokens, 0.000435);
    assert_cost_eq(pro.output_cost_per_1k_tokens, 0.00087);
    assert_eq!(pro.cache_read_input_token_cost, Some(0.0000036));

    let Ok(base) = get_model_pricing("mimo-v2.5", "mimo") else {
        panic!("mimo-v2.5 pricing should load through provider aliases");
    };
    assert_cost_eq(base.input_cost_per_1k_tokens, 0.00014);
    assert_cost_eq(base.output_cost_per_1k_tokens, 0.00028);
    assert_eq!(base.cache_read_input_token_cost, Some(0.0000028));
}

#[test]
fn test_get_cohere_pricing_from_shared_catalog() {
    let Ok(command_r) = get_model_pricing("command-r", "cohere") else {
        panic!("command-r pricing should load from shared pricing data");
    };
    assert_cost_eq(command_r.input_cost_per_1k_tokens, 0.0005);
    assert_cost_eq(command_r.output_cost_per_1k_tokens, 0.0015);

    let Ok(embed) = get_model_pricing("embed-english-v3.0", "cohere") else {
        panic!("embed-english-v3.0 pricing should load from shared pricing data");
    };
    assert_cost_eq(embed.input_cost_per_1k_tokens, 0.0001);
    assert_cost_eq(embed.output_cost_per_1k_tokens, 0.0);

    let Ok(command_a_plus) = get_model_pricing("command-a-plus-05-2026", "cohere") else {
        panic!("command-a-plus-05-2026 free pricing should load from shared pricing data");
    };
    assert_cost_eq(command_a_plus.input_cost_per_1k_tokens, 0.0);
    assert_cost_eq(command_a_plus.output_cost_per_1k_tokens, 0.0);

    let free_usage = UsageTokens::new(1000, 500);
    let Ok(command_a_plus_cost) =
        generic_cost_per_token("command-a-plus-05-2026", &free_usage, "cohere")
    else {
        panic!("command-a-plus-05-2026 free pricing should calculate");
    };
    assert_cost_eq(command_a_plus_cost.total_cost, 0.0);

    let unpriced = get_model_pricing("command-a-reasoning-08-2025", "cohere");
    assert!(matches!(unpriced, Err(CostError::MissingPricing { .. })));
}

#[test]
fn test_get_groq_time_based_pricing_from_shared_catalog() {
    let Ok(whisper) = get_model_pricing("whisper-large-v3-turbo", "groq") else {
        panic!("Groq Whisper Turbo pricing should expose cost_per_second");
    };

    assert_eq!(whisper.cost_per_second, Some(0.000011111111111111112));
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
    assert_eq!(pricing.cache_read_input_token_cost, Some(0.00003));
    assert_eq!(pricing.cache_creation_input_token_cost, Some(0.000375));
}

#[test]
fn test_get_minimax_pricing_m3_and_m2_7_highspeed() {
    let pricing = get_model_pricing("MiniMax-M3", "minimax");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0003);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0012);
    assert_eq!(pricing.cache_read_input_token_cost, Some(0.00006));
    assert_eq!(
        pricing
            .tiered_pricing
            .as_ref()
            .and_then(|tiered| tiered.get("input_cost_per_token_above_512k_tokens")),
        Some(&0.0006)
    );

    let pricing = get_model_pricing("MiniMax-M2.7-highspeed", "minimax");
    assert!(pricing.is_ok());
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0006);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0024);
    assert_eq!(pricing.cache_read_input_token_cost, Some(0.00006));
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
fn test_get_zhipu_pricing_glm_5_2() {
    let pricing = get_model_pricing("glm-5.2", "zhipuai");
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
    // Azure uses the embedded LiteLLM catalog when present.
    let pricing = pricing.unwrap();
    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0025);
}
