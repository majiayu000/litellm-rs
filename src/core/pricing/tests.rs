use super::*;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn parse_litellm_pricing_json_filters_metadata_entries() {
    let content = r#"{
            "sample_spec": {"this": "is not a model"},
            "_metadata": {"source": "upstream"},
            "fallback_generalizations": {"gpt-test": "gpt"},
            "_comment": {
                "input_cost_per_token": 0.000003,
                "output_cost_per_token": 0.000004,
                "litellm_provider": "test"
            },
            "provider_example_model": {
                "input_cost_per_token": 0.000005,
                "output_cost_per_token": 0.000006,
                "litellm_provider": "test"
            },
            "gpt-test": {
                "max_tokens": 4096,
                "input_cost_per_token": 0.000001,
                "output_cost_per_token": 0.000002,
                "litellm_provider": "openai",
                "mode": "chat"
            }
        }"#;

    let parsed = parse_litellm_pricing_json(content).unwrap();

    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed["gpt-test"].litellm_provider, "openai");
    assert_eq!(parsed["gpt-test"].input_cost_per_token, Some(0.000001));
    assert!(parsed.contains_key("_comment"));
    assert!(parsed.contains_key("provider_example_model"));
}

#[test]
fn parse_litellm_pricing_json_rejects_malformed_exact_control_blocks() {
    for key in ["_metadata", "fallback_generalizations", "sample_spec"] {
        let content = format!(r#"{{"{key}": []}}"#);
        assert!(
            parse_litellm_pricing_json(&content).is_err(),
            "{key} must be an object"
        );
    }
}

#[test]
fn embedded_default_pricing_catalog_tracks_litellm_scale() {
    let models = match embedded_default_pricing_models() {
        Ok(models) => models,
        Err(error) => panic!("embedded LiteLLM pricing catalog should parse: {error}"),
    };

    assert!(
        models.len() >= 2500,
        "embedded LiteLLM pricing catalog has only {} model entries",
        models.len()
    );

    for model in [
        "1024-x-1024/dall-e-2",
        "1024-x-1024/50-steps/bedrock/amazon.nova-canvas-v1:0",
        "ai21.j2-mid-v1",
        "aiml/flux-pro",
        "azure_ai/gpt-5.5",
        "openrouter/deepseek/deepseek-v3.2",
        "xai/grok-4",
    ] {
        assert!(
            models.contains_key(model),
            "embedded LiteLLM pricing catalog is missing {model}"
        );
    }

    for model in ["mimo-v2.5-pro", "command-a-plus-05-2026"] {
        assert!(
            models.contains_key(model),
            "embedded LiteLLM pricing catalog dropped local compatibility row {model}"
        );
    }
}

#[test]
fn parse_litellm_pricing_json_rejects_malformed_model_entries() {
    let content = r#"{
            "bad-model": {
                "input_cost_per_token": 0.000001,
                "output_cost_per_token": 0.000002,
                "mode": "chat"
            }
        }"#;

    assert!(parse_litellm_pricing_json(content).is_err());
}

#[test]
fn parse_litellm_pricing_json_rejects_malformed_time_of_use_pricing() {
    let cases = [
        (
            "timezone",
            serde_json::json!({
                "timezone": "Asia/Shanghai",
                "peak_windows": [{"weekdays": [1], "start_hour": 1, "end_hour": 4}],
                "peak_rates": {"input_cost_per_token": 1.0, "output_cost_per_token": 2.0, "cache_read_input_token_cost": 0.5}
            }),
        ),
        (
            "weekday",
            serde_json::json!({
                "timezone": "UTC",
                "peak_windows": [{"weekdays": [0], "start_hour": 1, "end_hour": 4}],
                "peak_rates": {"input_cost_per_token": 1.0, "output_cost_per_token": 2.0, "cache_read_input_token_cost": 0.5}
            }),
        ),
        (
            "window",
            serde_json::json!({
                "timezone": "UTC",
                "peak_windows": [{"weekdays": [1], "start_hour": 4, "end_hour": 4}],
                "peak_rates": {"input_cost_per_token": 1.0, "output_cost_per_token": 2.0, "cache_read_input_token_cost": 0.5}
            }),
        ),
        (
            "rate",
            serde_json::json!({
                "timezone": "UTC",
                "peak_windows": [{"weekdays": [1], "start_hour": 1, "end_hour": 4}],
                "peak_rates": {"input_cost_per_token": -1.0, "output_cost_per_token": 2.0, "cache_read_input_token_cost": 0.5}
            }),
        ),
    ];

    for (case, schedule) in cases {
        let content = serde_json::json!({
            "bad-time-priced-model": {
                "input_cost_per_token": 0.000001,
                "output_cost_per_token": 0.000002,
                "litellm_provider": "test",
                "mode": "chat",
                "time_of_use_pricing": schedule
            }
        })
        .to_string();
        let error = parse_litellm_pricing_json(&content).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("bad-time-priced-model"),
            "{case}: {message}"
        );
        assert!(message.contains("time_of_use_pricing"), "{case}: {message}");
    }
}

#[test]
fn parse_litellm_pricing_json_accepts_integral_float_token_limits_and_missing_mode() {
    let content = r#"{
            "float-token-model": {
                "max_tokens": 2000000.0,
                "max_input_tokens": 2000000.0,
                "max_output_tokens": 8192.0,
                "input_cost_per_token": 0.000001,
                "output_cost_per_token": 0.000002,
                "litellm_provider": "openai"
            }
        }"#;

    let parsed = match parse_litellm_pricing_json(content) {
        Ok(parsed) => parsed,
        Err(error) => panic!("integral float token limits should parse: {error}"),
    };
    let Some(model) = parsed.get("float-token-model") else {
        panic!("float-token-model should be present");
    };

    assert_eq!(model.max_tokens, Some(2_000_000));
    assert_eq!(model.max_input_tokens, Some(2_000_000));
    assert_eq!(model.max_output_tokens, Some(8_192));
    assert_eq!(model.mode, "");
}

#[test]
fn parse_litellm_pricing_json_rejects_lossy_token_limits() {
    for (field, value) in [
        ("max_tokens", "2000000.5"),
        ("max_input_tokens", "-1"),
        ("max_output_tokens", "4294967296"),
    ] {
        let content = format!(
            r#"{{
                "bad-token-model": {{
                    "{field}": {value},
                    "input_cost_per_token": 0.000001,
                    "output_cost_per_token": 0.000002,
                    "litellm_provider": "openai",
                    "mode": "chat"
                }}
            }}"#
        );

        assert!(
            parse_litellm_pricing_json(&content).is_err(),
            "{field}={value} should be rejected"
        );
    }
}

#[test]
fn pricing_database_skips_blank_mode_one_sided_token_prices() {
    let content = r#"{
            "half-priced-missing-mode": {
                "input_cost_per_token": 0.000001,
                "litellm_provider": "openai"
            }
        }"#;
    let models = parse_litellm_pricing_json(content).unwrap();
    let db = PricingDatabase { models };
    let usage = Usage::new(1000, 500);

    assert_eq!(db.calculate("half-priced-missing-mode", &usage), 0.0);
}

#[test]
fn test_default_pricing() {
    let db = PricingDatabase::default();

    let usage = Usage::new(1000, 500);

    let cost = db.calculate("gpt-4", &usage);
    assert!(cost > 0.0);
    assert_eq!(cost, 1000.0 * 0.00003 + 500.0 * 0.00006);

    let cost = db.calculate("claude-3-opus", &usage);
    assert!(cost > 0.0);
}

#[test]
fn calculate_for_provider_uses_matching_provider_rates() {
    let db = PricingDatabase::default();
    let usage = Usage::new(1000, 500);

    assert_eq!(
        db.calculate_for_provider("openai", "gpt-4", &usage),
        1000.0 * 0.00003 + 500.0 * 0.00006
    );
    assert_eq!(db.calculate_for_provider("anthropic", "gpt-4", &usage), 0.0);
    assert_eq!(
        db.calculate_for_provider("openai", "claude-3-opus", &usage),
        0.0
    );
}

#[test]
fn extended_pricing_uses_exact_mistral_alias_rates() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("extended pricing source should load");
    };
    let usage = Usage::new(1000, 500);

    let large = db.calculate_for_provider("mistral", "mistral-large", &usage);
    let small = db.calculate_for_provider("mistral", "mistral-small", &usage);
    let small_4 = db.calculate_for_provider("mistral", "mistral-small-4", &usage);
    let small_2506 = db.calculate_for_provider("mistral", "mistral-small-2506", &usage);

    assert!((large - 0.00125).abs() < 1e-12);
    assert!((small - 0.00025).abs() < 1e-12);
    assert!((small_4 - 0.00045).abs() < 1e-12);
    assert!((small_2506 - 0.00025).abs() < 1e-12);
}

#[test]
fn extended_pricing_has_nova_2_lite_rates_and_magistral_capabilities() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("extended pricing source should load");
    };
    let usage = Usage::new(1000, 500);

    let nova = db.calculate("amazon.nova-2-lite-v1:0", &usage);
    assert!((nova - 0.00155).abs() < 1e-12);

    let Some(nova_info) = db.get_model_info("amazon.nova-2-lite-v1:0") else {
        panic!("nova 2 lite pricing entry should exist");
    };
    assert_eq!(
        nova_info
            .extra
            .get("supports_reasoning")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );

    assert!(db.supports_feature("magistral-small-latest", "function_calling"));
    assert!(db.supports_feature("magistral-small-latest", "vision"));
    assert!(db.supports_feature("magistral-medium-latest", "function_calling"));
    assert!(db.supports_feature("magistral-medium-latest", "vision"));
}

#[test]
fn extended_pricing_handles_cohere_command_a_rates() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("extended pricing source should load");
    };
    let usage = Usage::new(1000, 500);

    let Some(command_a_plus) = db.get_model_info("command-a-plus-05-2026") else {
        panic!("command-a-plus-05-2026 pricing entry should exist");
    };
    assert_eq!(command_a_plus.input_cost_per_token, Some(0.0));
    assert_eq!(command_a_plus.output_cost_per_token, Some(0.0));
    assert_eq!(
        command_a_plus
            .extra
            .get("pricing_status")
            .and_then(|v| v.as_str()),
        Some("official_free_until_rate_limits")
    );
    assert_eq!(
        db.calculate_for_provider("cohere", "command-a-plus-05-2026", &usage),
        0.0
    );

    let Some(command_a) = db.get_model_info("command-a-03-2025") else {
        panic!("command-a-03-2025 pricing entry should exist");
    };
    assert_eq!(command_a.input_cost_per_token, Some(2.5e-06));
    assert_eq!(command_a.output_cost_per_token, Some(1e-05));
    assert!(
        (db.calculate_for_provider("cohere", "command-a-03-2025", &usage) - 0.0075).abs() < 1e-12
    );
}

#[test]
fn deepseek_v4_pricing_surfaces_use_the_off_peak_card() {
    const PRICING_STATUS: &str = "official_off_peak_rate_checked_2026_08_24";
    const FLASH_RATES: (f64, f64, f64) = (2.2e-7, 6.6e-7, 7e-9);
    const PRO_RATES: (f64, f64, f64) = (6.6e-7, 1.98e-6, 2.2e-8);

    let builtin = PricingDatabase::default();
    for (model, expected) in [
        ("deepseek-v4-flash", FLASH_RATES),
        ("deepseek-v4-flash-vision-exp", FLASH_RATES),
        ("deepseek-chat", FLASH_RATES),
        ("deepseek-reasoner", FLASH_RATES),
        ("deepseek-v4-pro", PRO_RATES),
    ] {
        let Some(pricing) = builtin.get_model_info(model) else {
            panic!("built-in pricing is missing {model}");
        };
        assert_eq!(pricing.input_cost_per_token, Some(expected.0));
        assert_eq!(pricing.output_cost_per_token, Some(expected.1));
        assert_eq!(
            pricing
                .extra
                .get("cache_read_input_token_cost")
                .and_then(serde_json::Value::as_f64),
            Some(expected.2)
        );
        assert_eq!(
            pricing
                .extra
                .get("pricing_status")
                .and_then(serde_json::Value::as_str),
            Some(PRICING_STATUS)
        );
    }

    let embedded = match embedded_default_pricing_models() {
        Ok(models) => models,
        Err(error) => panic!("embedded pricing catalog should parse: {error}"),
    };
    for (model, expected) in [
        ("deepseek-v4-flash", FLASH_RATES),
        ("deepseek/deepseek-v4-flash", FLASH_RATES),
        ("deepseek-v4-flash-vision-exp", FLASH_RATES),
        ("deepseek/deepseek-v4-flash-vision-exp", FLASH_RATES),
        ("deepseek-chat", FLASH_RATES),
        ("deepseek/deepseek-chat", FLASH_RATES),
        ("deepseek-reasoner", FLASH_RATES),
        ("deepseek/deepseek-reasoner", FLASH_RATES),
        ("deepseek-v4-pro", PRO_RATES),
        ("deepseek/deepseek-v4-pro", PRO_RATES),
    ] {
        let Some(pricing) = embedded.get(model) else {
            panic!("embedded pricing is missing {model}");
        };
        assert_eq!(pricing.input_cost_per_token, Some(expected.0));
        assert_eq!(pricing.output_cost_per_token, Some(expected.1));
        assert_eq!(
            pricing
                .extra
                .get("cache_read_input_token_cost")
                .and_then(serde_json::Value::as_f64),
            Some(expected.2)
        );
        assert_eq!(
            pricing
                .extra
                .get("pricing_status")
                .and_then(serde_json::Value::as_str),
            Some(PRICING_STATUS)
        );
    }

    let assert_vision_limits = |pricing: &LiteLLMModelInfo| {
        assert_eq!(pricing.supports_vision, Some(true));
        assert_eq!(pricing.max_tokens, Some(1_048_576));
        assert_eq!(pricing.max_input_tokens, Some(1_048_576));
        assert_eq!(pricing.max_output_tokens, Some(393_216));
    };

    for model in [
        "deepseek-v4-flash-vision-exp",
        "deepseek/deepseek-v4-flash-vision-exp",
    ] {
        let Some(pricing) = embedded.get(model) else {
            panic!("embedded pricing is missing {model}");
        };
        assert_vision_limits(pricing);
    }

    let Some(builtin_vision) = builtin.get_model_info("deepseek-v4-flash-vision-exp") else {
        panic!("built-in pricing is missing deepseek-v4-flash-vision-exp");
    };
    assert_vision_limits(builtin_vision);

    let Some(canonical_vision) = embedded.get("deepseek-v4-flash-vision-exp") else {
        panic!("embedded pricing is missing canonical vision metadata");
    };
    let Some(prefixed_vision) = embedded.get("deepseek/deepseek-v4-flash-vision-exp") else {
        panic!("embedded pricing is missing prefixed vision metadata");
    };
    assert_eq!(
        canonical_vision.supports_streaming,
        prefixed_vision.supports_streaming
    );
    for key in [
        "supported_endpoints",
        "supports_assistant_prefill",
        "supports_native_streaming",
        "supports_reasoning",
        "supports_system_messages",
        "thinking_mode_default",
    ] {
        assert_eq!(
            canonical_vision.extra.get(key),
            prefixed_vision.extra.get(key),
            "vision catalog aliases disagree on {key}"
        );
    }
}

#[test]
fn xiaomi_mimo_provider_aliases_share_pricing_rows() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };
    let usage = Usage::new(1000, 500);
    let expected = 1000.0 * 0.00000014 + 500.0 * 0.00000028;

    for provider in ["xiaomi_mimo", "mimo", "xiaomi"] {
        assert!(
            (db.calculate_for_provider(provider, "mimo-v2.5", &usage) - expected).abs() < 1e-12,
            "{provider} should resolve Xiaomi MiMo pricing"
        );
        assert!(
            db.get_provider_models(provider)
                .contains(&"mimo-v2.5".to_string()),
            "{provider} should list Xiaomi MiMo models"
        );
    }
}

#[test]
fn extended_pricing_has_current_anthropic_gemini_and_groq_rates() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };
    let usage = Usage::new(1000, 500);

    assert!(
        (db.calculate_for_provider("anthropic", "claude-opus-4-8", &usage) - 0.0175).abs() < 1e-12
    );
    assert!(
        (db.calculate_for_provider("anthropic", "claude-sonnet-4-6", &usage) - 0.0105).abs()
            < 1e-12
    );
    assert!(
        (db.calculate_for_provider("anthropic", "claude-haiku-4-5-20251001", &usage) - 0.0035)
            .abs()
            < 1e-12
    );

    assert!(
        (db.calculate_for_provider("vertex_ai", "gemini-3.1-flash-lite", &usage) - 0.001).abs()
            < 1e-12
    );
    assert!(
        (db.calculate_for_provider("gemini", "gemini-3.1-flash-lite", &usage) - 0.001).abs()
            < 1e-12
    );
    assert!(
        db.get_provider_models("gemini")
            .contains(&"gemini/gemini-2.5-flash".to_string()),
        "gemini provider should list prefixed Gemini 2.5 rows"
    );
    assert_eq!(
        db.calculate_for_provider("vertex_ai", "gemini-3.1-flash", &usage),
        0.0,
        "Gemini Flash must not borrow Flash-Lite pricing"
    );

    let long_usage = Usage::new(300_000, 1_000);
    let long_context_cost = 300_000.0 * 0.000004 + 1_000.0 * 0.000018;
    assert!(
        (db.calculate_for_provider("vertex_ai", "gemini-3.1-pro-preview", &long_usage)
            - long_context_cost)
            .abs()
            < 1e-12
    );

    assert!(
        (db.calculate_for_provider("groq", "llama-3.3-70b-versatile", &usage) - 0.000985).abs()
            < 1e-12
    );
    assert!(
        (db.calculate_for_provider("groq", "openai/gpt-oss-120b", &usage) - 0.00045).abs() < 1e-12
    );
    assert!((db.calculate_for_provider("groq", "gpt-oss-120b", &usage) - 0.00045).abs() < 1e-12);

    let Some(whisper) = db.get_model_info("whisper-large-v3-turbo") else {
        panic!("Groq Whisper Turbo ASR pricing entry should exist");
    };
    assert_eq!(whisper.cost_per_second, Some(0.000011111111111111112));
    assert_eq!(whisper.mode, "audio_transcription");
}

#[test]
fn provider_model_listing_does_not_cross_advertise_foreign_prefixed_rows() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };
    let usage = Usage::new(1000, 500);

    assert!(
        !db.get_provider_models("openai")
            .contains(&"openai/gpt-oss-120b".to_string()),
        "Groq-hosted OpenAI OSS rows must not be advertised as OpenAI models"
    );
    assert!(
        db.get_provider_models("groq")
            .contains(&"openai/gpt-oss-120b".to_string()),
        "Groq provider should still list its OpenAI OSS compatibility rows"
    );
    assert_eq!(
        db.calculate_for_provider("openai", "openai/gpt-oss-120b", &usage),
        0.0
    );
    assert!((db.calculate_for_provider("groq", "gpt-oss-120b", &usage) - 0.00045).abs() < 1e-12);
}

#[test]
fn pricing_alias_fallback_preserves_longer_model_aliases() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };
    let usage = Usage::new(1000, 500);

    assert_eq!(
        db.calculate_for_provider("openai", "gpt-4-0613", &usage),
        db.calculate_for_provider("openai", "gpt-4", &usage)
    );
    assert_eq!(
        db.calculate_for_provider("anthropic", "claude-opus-4-7-latest", &usage),
        db.calculate_for_provider("anthropic", "claude-opus-4-7", &usage)
    );
}

#[test]
fn provider_aliases_share_normalized_pricing_rows() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };
    let usage = Usage::new(1000, 500);

    assert_eq!(
        db.calculate_for_provider("google", "gemini-1.5-flash", &usage),
        db.calculate_for_provider("vertex_ai", "gemini-1.5-flash", &usage)
    );
    assert!(
        db.get_provider_models("google")
            .contains(&"gemini-1.5-flash".to_string()),
        "google should list Vertex AI pricing rows"
    );

    assert_eq!(
        db.calculate_for_provider("zhipuai", "glm-5", &usage),
        db.calculate_for_provider("zhipu", "glm-5", &usage)
    );
    assert!(
        db.get_provider_models("zhipuai")
            .contains(&"glm-5".to_string()),
        "zhipuai should list Zhipu pricing rows"
    );

    assert_eq!(
        db.calculate_for_provider("together", "together-ai-4.1b-8b", &usage),
        db.calculate_for_provider("together_ai", "together-ai-4.1b-8b", &usage)
    );
    assert_eq!(
        db.calculate_for_provider("fireworks", "fireworks_ai/deepseek-v4-flash", &usage),
        db.calculate_for_provider("fireworks_ai", "fireworks_ai/deepseek-v4-flash", &usage)
    );
    assert!(
        db.get_provider_models("aiml_api")
            .contains(&"aiml/flux-pro".to_string()),
        "aiml_api should list AIML pricing rows"
    );
    assert_eq!(normalize_pricing_provider("aiml_api"), "aiml");
    assert_eq!(normalize_pricing_provider("zai"), "zai");
    assert_ne!(
        normalize_pricing_provider("zai"),
        normalize_pricing_provider("zhipu")
    );
}

#[test]
fn test_model_info() {
    let db = PricingDatabase::default();

    assert!(db.get_model_info("gpt-4").is_some());
    assert!(db.get_model_info("non-existent-model").is_none());

    assert_eq!(db.get_max_tokens("gpt-4"), Some(8192));
    assert!(db.supports_feature("gpt-4", "function_calling"));
    assert!(!db.supports_feature("gpt-4", "vision"));
    assert!(db.supports_feature("gpt-4-turbo", "vision"));
}

#[test]
fn test_quick_calculate() {
    let cost = calculate_cost("gpt-3.5-turbo", 1000, 500);
    assert!(cost > 0.0);
}

#[test]
fn gpt55_shared_pricing_charges_long_context_tiers() {
    let db = PricingDatabase::default();
    let usage = Usage::new(300_000, 2_000);

    assert!((db.calculate("gpt-5.5", &usage) - 3.09).abs() < 1e-12);
    assert!((db.calculate_for_provider("openai", "gpt-5.5", &usage) - 3.09).abs() < 1e-12);

    let Ok(shared_db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };
    assert!((shared_db.calculate("gpt-5.5", &usage) - 3.09).abs() < 1e-12);
    assert!((calculate_cost("gpt-5.5", 300_000, 2_000) - 3.09).abs() < 1e-12);
}

#[test]
fn mistral_medium_2508_shared_pricing_uses_exact_row() {
    let usage = Usage::new(1_000, 500);
    let expected_cost = 0.00525;

    let Ok(shared_db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };

    assert!(
        (shared_db.calculate_for_provider("mistral", "mistral-medium-2508", &usage)
            - expected_cost)
            .abs()
            < 1e-12
    );
}

#[test]
fn devstral_2_2512_shared_pricing_uses_exact_row() {
    let usage = Usage::new(1_000, 500);
    let expected_cost = 0.0014;

    let Ok(shared_db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };

    assert!(
        (shared_db.calculate_for_provider("mistral", "devstral-2-2512", &usage) - expected_cost)
            .abs()
            < 1e-12
    );
}

#[test]
fn gpt55_provider_prefixed_pro_pricing_uses_exact_model() {
    let usage = Usage::new(1_000, 500);
    let expected_pro_cost = 1_000.0 * 0.00003 + 500.0 * 0.00018;

    let db = PricingDatabase::default();
    assert!((db.calculate("openai/gpt-5.5-pro", &usage) - expected_pro_cost).abs() < 1e-12);
    assert!(
        (db.calculate_for_provider("openai", "openai/gpt-5.5-pro", &usage) - expected_pro_cost)
            .abs()
            < 1e-12
    );

    let Ok(shared_db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };
    assert!((shared_db.calculate("openai/gpt-5.5-pro", &usage) - expected_pro_cost).abs() < 1e-12);
    assert!(
        (shared_db.calculate_for_provider("openai", "openai/gpt-5.5-pro", &usage)
            - expected_pro_cost)
            .abs()
            < 1e-12
    );

    let long_usage = Usage::new(300_000, 1_000);
    let expected_long_cost = 300_000.0 * 0.00003 + 1_000.0 * 0.00018;
    assert!(
        (shared_db.calculate_for_provider("openai", "gpt-5.5-pro", &long_usage)
            - expected_long_cost)
            .abs()
            < 1e-12
    );
}

#[test]
fn xai_long_context_pricing_is_inclusive_at_200k() {
    let Ok(db) = PricingDatabase::from_default_source() else {
        panic!("shared pricing source should load");
    };

    for (prompt_tokens, expected_cost) in [
        (199_999, 199_999.0 * 0.000002),
        (200_000, 200_000.0 * 0.000004),
        (200_001, 200_001.0 * 0.000004),
    ] {
        let usage = Usage::new(prompt_tokens, 0);
        for model in ["grok-4.5", "grok-4.6"] {
            let cost = db.calculate_for_provider("xai", model, &usage);
            assert!(
                (cost - expected_cost).abs() < 1e-12,
                "{model} at {prompt_tokens}"
            );
        }
    }
}

#[test]
fn provider_prefixed_exact_prices_are_preserved_before_normalization() {
    let usage = Usage::new(1_000, 1_000);
    let mut db = PricingDatabase::default();
    let azure_override = builtin_model("azure", 0.000001, 0.000002, 8_192, 4_096, true, false);
    db.models.insert("azure/gpt-4".to_string(), azure_override);

    let expected_azure_cost = 1_000.0 * 0.000001 + 1_000.0 * 0.000002;

    assert!((db.calculate("azure/gpt-4", &usage) - expected_azure_cost).abs() < 1e-12);
    assert!(
        (db.calculate_for_provider("azure", "azure/gpt-4", &usage) - expected_azure_cost).abs()
            < 1e-12
    );
}

#[test]
fn gpt55_builtin_pro_model_info_is_non_streaming() {
    let db = PricingDatabase::default();
    let Some(info) = db.to_model_info("gpt-5.5-pro", "openai") else {
        panic!("built-in GPT-5.5 Pro pricing should be present");
    };

    assert!(!info.supports_streaming);
}

#[test]
fn to_model_info_uses_supported_modalities_for_visual_multimodal() {
    let mut db = PricingDatabase {
        models: HashMap::new(),
    };
    let mut visual = builtin_model("openai", 0.000001, 0.000002, 8_192, 4_096, true, false);
    visual.extra.insert(
        "supported_modalities".to_string(),
        serde_json::json!(["text", "image"]),
    );
    let mut audio_only = builtin_model("openai", 0.000001, 0.000002, 8_192, 4_096, true, false);
    audio_only.extra.insert(
        "supported_modalities".to_string(),
        serde_json::json!(["text", "audio"]),
    );
    db.models.insert("visual-model".to_string(), visual);
    db.models.insert("audio-model".to_string(), audio_only);

    let Some(visual_info) = db.to_model_info("visual-model", "openai") else {
        panic!("visual model should convert to ModelInfo");
    };
    let Some(audio_info) = db.to_model_info("audio-model", "openai") else {
        panic!("audio model should convert to ModelInfo");
    };

    assert!(visual_info.supports_multimodal);
    assert!(!audio_info.supports_multimodal);
}

#[test]
fn to_model_info_uses_supports_tool_choice_for_tools() {
    let mut db = PricingDatabase {
        models: HashMap::new(),
    };
    let mut model = builtin_model("ai21", 0.000001, 0.000002, 8_192, 4_096, true, false);
    model.supports_function_calling = None;
    model.extra.insert(
        "supports_tool_choice".to_string(),
        serde_json::Value::from(true),
    );
    db.models.insert("tool-choice-model".to_string(), model);

    let Some(info) = db.to_model_info("tool-choice-model", "ai21") else {
        panic!("tool-choice model should convert to ModelInfo");
    };

    assert!(info.supports_tools);
}

#[test]
fn test_default_source_loads_shared_pricing_file() {
    let db = PricingDatabase::from_default_source().unwrap();

    assert!(db.get_model_info("gpt-4o").is_some());
    assert!(db.calculate("gpt-4o", &Usage::new(1000, 500)) > 0.0);
}

#[test]
fn deepseek_v4_catalog_rows_select_peak_rates_at_utc_boundaries() {
    use chrono::{TimeZone, Utc};

    let db = PricingDatabase::from_default_source().unwrap();
    let usage = Usage::new(1_000, 1_000);
    let off_peak = Utc.with_ymd_and_hms(2026, 8, 24, 4, 0, 0).unwrap();
    let peak = Utc.with_ymd_and_hms(2026, 8, 24, 6, 0, 0).unwrap();
    let models = [
        "deepseek-chat",
        "deepseek-reasoner",
        "deepseek-v4-flash",
        "deepseek-v4-flash-vision-exp",
        "deepseek-v4-pro",
        "deepseek/deepseek-chat",
        "deepseek/deepseek-reasoner",
        "deepseek/deepseek-v4-flash",
        "deepseek/deepseek-v4-flash-vision-exp",
        "deepseek/deepseek-v4-pro",
    ];

    for model in models {
        let info = db.get_model_info(model).unwrap();
        assert!(
            info.extra
                .contains_key(time_of_use::TIME_OF_USE_PRICING_KEY)
        );
        let off_peak_cost = db.calculate_for_provider_at("deepseek", model, &usage, off_peak);
        let peak_cost = db.calculate_for_provider_at("deepseek", model, &usage, peak);
        assert!(off_peak_cost > 0.0, "{model} should have an off-peak cost");
        assert!(
            (peak_cost - off_peak_cost * 2.0).abs() < 1e-12,
            "{model} peak cost {peak_cost} should be twice off-peak {off_peak_cost}"
        );
    }
}

#[test]
fn provider_code_uses_core_pricing_directly() {
    let providers_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/providers");
    let mut rust_files = Vec::new();
    collect_rust_files(&providers_dir, &mut rust_files);

    for path in rust_files {
        if is_pricing_compatibility_module(&path) {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("failed to read provider source {}: {}", path.display(), err)
        });

        for forbidden in [
            "providers::base::pricing",
            "providers::base::get_pricing_db",
            "providers::base::PricingDatabase",
            "providers::base::{get_pricing_db",
            "providers::base::{PricingDatabase",
        ] {
            assert!(
                !content.contains(forbidden),
                "{} should import pricing database APIs from core::pricing, not {}",
                path.display(),
                forbidden
            );
        }
    }
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("provider source directory should be readable") {
        let entry = entry.expect("provider source entry should be readable");
        let path = entry.path();
        let file_type = entry
            .file_type()
            .expect("provider source file type should be readable");

        if file_type.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn is_pricing_compatibility_module(path: &Path) -> bool {
    path.ends_with("src/core/providers/base/pricing.rs")
        || path.ends_with("src/core/providers/base/mod.rs")
}
