use super::*;
#[cfg(feature = "providers-extended")]
use crate::core::providers::gemini::get_gemini_registry;
use crate::core::providers::openai::get_openai_registry;
use crate::core::providers::shared::{
    GEMINI_15_PRO_CONTEXT_WINDOW, GEMINI_20_FLASH_CONTEXT_WINDOW, GEMINI_31_CONTEXT_WINDOW,
};

// ==================== get_model_capabilities Tests ====================

#[test]
fn test_get_model_capabilities_gpt4() {
    let caps = ModelUtils::get_model_capabilities("gpt-4");
    assert!(caps.supports_function_calling);
    assert!(caps.supports_parallel_function_calling);
    assert!(caps.supports_tool_choice);
    assert!(caps.supports_response_schema);
    assert_eq!(caps.max_tokens, Some(8192));
}

#[test]
fn test_get_model_capabilities_gpt4_32k() {
    let caps = ModelUtils::get_model_capabilities("gpt-4-32k");
    assert_eq!(caps.max_tokens, Some(32768));
    assert_eq!(caps.context_window, Some(32768));
}

#[test]
fn test_get_model_capabilities_gpt4_turbo_vision() {
    let caps = ModelUtils::get_model_capabilities("gpt-4-turbo-preview");
    assert!(caps.supports_vision);
}

#[test]
fn test_get_model_capabilities_gpt54_pro_uses_long_context() {
    let caps = ModelUtils::get_model_capabilities("gpt-5.4-pro");
    assert_eq!(caps.context_window, Some(1_048_576));
}

#[test]
fn test_get_model_capabilities_gpt55_matches_catalog_shape() {
    let base_caps = ModelUtils::get_model_capabilities("gpt-5.5");
    assert_eq!(base_caps.context_window, Some(1_048_576));
    assert_eq!(base_caps.max_tokens, Some(128000));
    assert!(base_caps.supports_streaming);

    let pro_caps = ModelUtils::get_model_capabilities("gpt-5.5-pro");
    assert_eq!(pro_caps.context_window, Some(1_048_576));
    assert_eq!(pro_caps.max_tokens, Some(128000));
    assert!(!pro_caps.supports_streaming);

    let prefixed_base_caps = ModelUtils::get_model_capabilities("openai/gpt-5.5");
    assert_eq!(prefixed_base_caps.context_window, Some(1_048_576));
    assert_eq!(prefixed_base_caps.max_tokens, Some(128000));
    assert!(prefixed_base_caps.supports_streaming);

    let prefixed_pro_caps = ModelUtils::get_model_capabilities("openai/gpt-5.5-pro");
    assert_eq!(prefixed_pro_caps.context_window, Some(1_048_576));
    assert_eq!(prefixed_pro_caps.max_tokens, Some(128000));
    assert!(!prefixed_pro_caps.supports_streaming);
}

#[test]
fn test_get_model_capabilities_gpt56_matches_registry_shape() {
    let registry = get_openai_registry();
    for (model, catalog_id) in [
        ("gpt-5.6", "gpt-5.6"),
        ("gpt-5.6-sol", "gpt-5.6-sol"),
        ("gpt-5.6-terra", "gpt-5.6-terra"),
        ("gpt-5.6-luna", "gpt-5.6-luna"),
        ("gpt-5.6-cyber", "gpt-5.6-cyber"),
        ("openai/gpt-5.6-terra", "gpt-5.6-terra"),
    ] {
        let spec = registry
            .get_model_spec(catalog_id)
            .expect("GPT-5.6 catalog entry should exist");
        let caps = ModelUtils::get_model_capabilities(model);

        assert_eq!(
            caps.context_window,
            Some(spec.model_info.max_context_length as usize),
            "{model} context window should match the OpenAI registry"
        );
        assert_eq!(
            caps.max_tokens,
            spec.model_info
                .max_output_length
                .map(|limit| limit as usize),
            "{model} max output should match the OpenAI registry"
        );
        assert!(caps.supports_function_calling, "{model}");
        assert!(caps.supports_response_schema, "{model}");
        assert!(caps.supports_web_search, "{model}");
        assert!(caps.supports_vision, "{model}");
        assert!(caps.supports_streaming, "{model}");
    }
}

#[test]
fn test_get_model_capabilities_realtime2_matches_registry_shape() {
    for model in [
        "gpt-realtime-2",
        "gpt-realtime-2.1",
        "gpt-realtime-2.1-mini",
        "openai/gpt-realtime-2",
    ] {
        let caps = ModelUtils::get_model_capabilities(model);
        assert!(caps.supports_function_calling, "{model}");
        assert!(caps.supports_parallel_function_calling, "{model}");
        assert!(caps.supports_tool_choice, "{model}");
        assert!(caps.supports_system_messages, "{model}");
        assert!(caps.supports_vision, "{model}");
        assert!(!caps.supports_streaming, "{model}");
        assert_eq!(caps.context_window, Some(128_000), "{model}");
        assert_eq!(caps.max_tokens, Some(32_000), "{model}");
    }
}

#[test]
fn test_get_model_capabilities_gpt35() {
    let caps = ModelUtils::get_model_capabilities("gpt-3.5-turbo");
    assert!(caps.supports_function_calling);
    assert!(!caps.supports_parallel_function_calling);
    assert!(!caps.supports_response_schema);
    assert_eq!(caps.max_tokens, Some(4096));
}

#[test]
fn test_get_model_capabilities_gpt35_16k() {
    let caps = ModelUtils::get_model_capabilities("gpt-3.5-turbo-16k");
    assert_eq!(caps.max_tokens, Some(16384));
    assert_eq!(caps.context_window, Some(16384));
}

#[test]
fn test_get_model_capabilities_claude3() {
    let caps = ModelUtils::get_model_capabilities("claude-3-opus");
    assert!(caps.supports_function_calling);
    assert!(caps.supports_vision);
    assert!(caps.supports_url_context);
    assert_eq!(caps.max_tokens, Some(200000));
}

#[test]
fn test_get_model_capabilities_claude_opus_47() {
    let caps = ModelUtils::get_model_capabilities("claude-opus-4-7");
    assert!(caps.supports_function_calling);
    assert!(caps.supports_vision);
    assert_eq!(caps.max_tokens, Some(1_000_000));
    assert_eq!(caps.context_window, Some(1_000_000));
}

#[test]
fn test_get_model_capabilities_claude_haiku_45() {
    let caps = ModelUtils::get_model_capabilities("claude-haiku-4-5");
    assert!(caps.supports_function_calling);
    assert!(caps.supports_tool_choice);
    assert!(caps.supports_vision);
    assert!(caps.supports_streaming);
    assert_eq!(caps.max_tokens, Some(200000));
    assert_eq!(caps.context_window, Some(200000));

    let dotted_caps = ModelUtils::get_model_capabilities("claude-haiku-4.5");
    assert!(dotted_caps.supports_function_calling);
    assert!(dotted_caps.supports_tool_choice);
    assert!(dotted_caps.supports_vision);
    assert!(dotted_caps.supports_streaming);
    assert_eq!(dotted_caps.max_tokens, Some(200000));
    assert_eq!(dotted_caps.context_window, Some(200000));
}

#[test]
fn test_get_model_capabilities_claude2() {
    let caps = ModelUtils::get_model_capabilities("claude-2.1");
    assert!(!caps.supports_function_calling);
    assert!(!caps.supports_vision);
    assert_eq!(caps.max_tokens, Some(100000));
}

#[test]
fn test_get_model_capabilities_claude_instant() {
    let caps = ModelUtils::get_model_capabilities("claude-instant-1.2");
    assert!(!caps.supports_function_calling);
    assert_eq!(caps.max_tokens, Some(100000));
}

#[test]
fn test_get_model_capabilities_gemini() {
    let caps = ModelUtils::get_model_capabilities("gemini-3.1-pro-preview");
    assert!(caps.supports_function_calling);
    assert!(caps.supports_web_search);
    assert!(caps.supports_vision);
    assert_eq!(caps.max_tokens, Some(65536));
}

#[test]
fn test_get_model_capabilities_gemini_15_pro() {
    let caps = ModelUtils::get_model_capabilities("gemini-1.5-pro");
    assert_eq!(caps.max_tokens, Some(8192));
    assert_eq!(
        caps.context_window,
        Some(GEMINI_15_PRO_CONTEXT_WINDOW as usize)
    );
}

#[test]
fn test_get_model_capabilities_gemini_20_flash() {
    let caps = ModelUtils::get_model_capabilities("gemini-2.0-flash");
    assert_eq!(caps.max_tokens, Some(8192));
    assert_eq!(
        caps.context_window,
        Some(GEMINI_20_FLASH_CONTEXT_WINDOW as usize)
    );
}

#[test]
fn qualified_gemini_37_uses_the_registry_context_window() {
    let capabilities = ModelUtils::get_model_capabilities("gemini/gemini-3.7-flash");
    assert_eq!(
        capabilities.context_window,
        Some(GEMINI_31_CONTEXT_WINDOW as usize)
    );
}

#[cfg(feature = "providers-extended")]
#[test]
fn test_get_model_capabilities_gemini_context_matches_registry() {
    for spec in get_gemini_registry().list_models() {
        let caps = ModelUtils::get_model_capabilities(&spec.model_info.id);

        assert_eq!(
            caps.context_window,
            Some(spec.limits.max_context_length as usize),
            "{} utility context window drifted from registry",
            spec.model_info.id
        );
    }
}

#[test]
fn test_get_model_capabilities_unknown() {
    let caps = ModelUtils::get_model_capabilities("unknown-model");
    assert!(!caps.supports_function_calling);
    assert!(caps.max_tokens.is_none());
}

// ==================== supports_* convenience function Tests ====================

#[test]
fn test_supports_function_calling() {
    assert!(ModelUtils::supports_function_calling("gpt-4"));
    assert!(ModelUtils::supports_function_calling("claude-haiku-4-5"));
    assert!(ModelUtils::supports_function_calling("claude-haiku-4.5"));
    assert!(!ModelUtils::supports_function_calling("claude-2"));
}

#[test]
fn test_supports_parallel_function_calling() {
    assert!(ModelUtils::supports_parallel_function_calling("gpt-4"));
    assert!(!ModelUtils::supports_parallel_function_calling(
        "gpt-3.5-turbo"
    ));
}

#[test]
fn test_supports_tool_choice() {
    assert!(ModelUtils::supports_tool_choice("gpt-4"));
    assert!(ModelUtils::supports_tool_choice("claude-3-sonnet"));
}

#[test]
fn test_supports_response_schema() {
    assert!(ModelUtils::supports_response_schema("gpt-4"));
    assert!(!ModelUtils::supports_response_schema("gpt-3.5-turbo"));
}

#[test]
fn test_supports_system_messages() {
    assert!(ModelUtils::supports_system_messages("gpt-4"));
    assert!(ModelUtils::supports_system_messages("claude-3-opus"));
}

#[test]
fn test_supports_web_search() {
    assert!(ModelUtils::supports_web_search("gemini-3.1-pro-preview"));
    assert!(!ModelUtils::supports_web_search("gpt-4"));
}

#[test]
fn test_supports_url_context() {
    assert!(ModelUtils::supports_url_context("gpt-4"));
    assert!(ModelUtils::supports_url_context("claude-3-opus"));
    assert!(!ModelUtils::supports_url_context("gpt-3.5-turbo"));
}

#[test]
fn test_supports_vision() {
    assert!(ModelUtils::supports_vision("gpt-4-turbo"));
    assert!(ModelUtils::supports_vision("claude-3-opus"));
    assert!(ModelUtils::supports_vision("claude-haiku-4-5"));
    assert!(ModelUtils::supports_vision("claude-haiku-4.5"));
    assert!(!ModelUtils::supports_vision("gpt-3.5-turbo"));
    // o3 and o4-mini support vision
    assert!(ModelUtils::supports_vision("o3"));
    assert!(ModelUtils::supports_vision("o3-mini"));
    assert!(ModelUtils::supports_vision("o4-mini"));
    // GPT-5.4 family supports vision (covered by gpt-5 prefix)
    assert!(ModelUtils::supports_vision("gpt-5.4"));
    assert!(ModelUtils::supports_vision("gpt-5.4-mini"));
    assert!(ModelUtils::supports_vision("gpt-5.4-pro"));
}

#[test]
fn test_supports_streaming() {
    assert!(ModelUtils::supports_streaming("gpt-4"));
    assert!(ModelUtils::supports_streaming("claude-3-opus"));
}

// ==================== get_provider_from_model Tests ====================

#[test]
fn test_get_provider_from_model_openai() {
    assert_eq!(
        ModelUtils::get_provider_from_model("gpt-4"),
        Some("openai".to_string())
    );
    assert_eq!(
        ModelUtils::get_provider_from_model("gpt-3.5-turbo"),
        Some("openai".to_string())
    );
}

#[test]
fn test_get_provider_from_model_anthropic() {
    assert_eq!(
        ModelUtils::get_provider_from_model("claude-3-opus"),
        Some("anthropic".to_string())
    );
    assert_eq!(
        ModelUtils::get_provider_from_model("claude-2"),
        Some("anthropic".to_string())
    );
}

#[test]
fn test_get_provider_from_model_google() {
    assert_eq!(
        ModelUtils::get_provider_from_model("gemini-3.1-pro-preview"),
        Some("google".to_string())
    );
}

#[test]
fn test_get_provider_from_model_cohere() {
    assert_eq!(
        ModelUtils::get_provider_from_model("command-r-plus"),
        Some("cohere".to_string())
    );
}

#[test]
fn test_get_provider_from_model_mistral() {
    assert_eq!(
        ModelUtils::get_provider_from_model("mistral-large"),
        Some("mistral".to_string())
    );
}

#[test]
fn test_get_provider_from_model_meta() {
    assert_eq!(
        ModelUtils::get_provider_from_model("llama-2-70b"),
        Some("meta".to_string())
    );
}

#[test]
fn test_get_provider_from_model_unknown() {
    assert_eq!(ModelUtils::get_provider_from_model("unknown-model"), None);
}

// ==================== get_base_model Tests ====================

#[test]
fn test_get_base_model_gpt4() {
    assert_eq!(ModelUtils::get_base_model("gpt-4-0613"), "gpt-4");
    assert_eq!(ModelUtils::get_base_model("gpt-4-32k-0613"), "gpt-4-32k");
    assert_eq!(
        ModelUtils::get_base_model("gpt-4-turbo-preview"),
        "gpt-4-turbo"
    );
}

#[test]
fn test_get_base_model_gpt55() {
    assert_eq!(ModelUtils::get_base_model("gpt-5.5-2026-04-23"), "gpt-5.5");
    assert_eq!(
        ModelUtils::get_base_model("gpt-5.5-pro-2026-04-23"),
        "gpt-5.5-pro"
    );
}

#[test]
fn test_get_base_model_gpt56_preserves_exact_family() {
    for model in [
        "gpt-5.6",
        "gpt-5.6-sol",
        "gpt-5.6-terra",
        "gpt-5.6-luna",
        "gpt-5.6-cyber",
    ] {
        assert_eq!(ModelUtils::get_base_model(model), model);
    }
    assert_eq!(
        ModelUtils::get_base_model("openai/gpt-5.6-terra"),
        "gpt-5.6-terra"
    );
}

#[test]
fn test_get_base_model_gpt35() {
    assert_eq!(
        ModelUtils::get_base_model("gpt-3.5-turbo-0613"),
        "gpt-3.5-turbo"
    );
    assert_eq!(
        ModelUtils::get_base_model("gpt-3.5-turbo-16k-0613"),
        "gpt-3.5-turbo-16k"
    );
}

#[test]
fn test_get_base_model_claude3() {
    assert_eq!(
        ModelUtils::get_base_model("claude-3-opus-20240229"),
        "claude-3-opus"
    );
    assert_eq!(
        ModelUtils::get_base_model("claude-3-sonnet-20240229"),
        "claude-3-sonnet"
    );
    assert_eq!(
        ModelUtils::get_base_model("claude-3-haiku-20240307"),
        "claude-3-haiku"
    );
}

#[test]
fn test_get_base_model_claude4() {
    assert_eq!(
        ModelUtils::get_base_model("claude-opus-4-8"),
        "claude-opus-4-8"
    );
    assert_eq!(
        ModelUtils::get_base_model("claude-opus-4-7"),
        "claude-opus-4-7"
    );
    assert_eq!(
        ModelUtils::get_base_model("claude-sonnet-4-6"),
        "claude-sonnet-4-6"
    );
    assert_eq!(
        ModelUtils::get_base_model("gemini-2.0-flash-exp"),
        "gemini-2.0-flash"
    );
    assert_eq!(
        ModelUtils::get_base_model("gemini-2.0-flash-thinking-exp"),
        "gemini-2.0-flash-thinking-exp"
    );
}

#[test]
fn test_get_base_model_unknown() {
    assert_eq!(ModelUtils::get_base_model("unknown-model"), "unknown-model");
}

// ==================== is_valid_model Tests ====================

#[test]
fn test_is_valid_model_known() {
    assert!(ModelUtils::is_valid_model("gpt-5.5"));
    assert!(ModelUtils::is_valid_model("gpt-5.5-pro"));
    assert!(ModelUtils::is_valid_model("gpt-4"));
    assert!(ModelUtils::is_valid_model("gpt-3.5-turbo"));
    assert!(ModelUtils::is_valid_model("claude-3-opus"));
    assert!(ModelUtils::is_valid_model("claude-opus-4-8"));
    assert!(ModelUtils::is_valid_model("claude-opus-4-6"));
    assert!(ModelUtils::is_valid_model("claude-sonnet-4-5"));
    assert!(ModelUtils::is_valid_model("gemini-pro"));
    assert!(ModelUtils::is_valid_model("gemini-3.5-flash"));
    assert!(ModelUtils::is_valid_model("gemini-3.1-flash-lite"));
    assert!(ModelUtils::is_valid_model("gemini-2.5-pro"));
    assert!(ModelUtils::is_valid_model("gemini-3.1-pro-preview"));
    assert!(ModelUtils::is_valid_model("command-r"));
    assert!(ModelUtils::is_valid_model("mistral-large"));
}

#[test]
fn test_is_valid_model_with_provider() {
    assert!(ModelUtils::is_valid_model("openai/gpt-4"));
    assert!(ModelUtils::is_valid_model("anthropic/claude-3"));
}

#[test]
fn test_is_valid_model_unknown() {
    assert!(!ModelUtils::is_valid_model("unknown-xyz-123"));
}

#[test]
fn test_gpt56_validation_is_registry_aligned_and_boundary_safe() {
    for model in [
        "gpt-5.6",
        "gpt-5.6-sol",
        "gpt-5.6-terra",
        "gpt-5.6-luna",
        "gpt-5.6-cyber",
        "openai/gpt-5.6-luna",
    ] {
        assert!(ModelUtils::is_valid_model(model), "{model}");
        assert!(
            ModelUtils::validate_model_with_provider(model, "openai").is_ok(),
            "{model}"
        );
    }

    for model in [
        "gpt-5.60",
        "gpt-5.6-foo",
        "gpt-5.6-solstice",
        "gpt-5.6-cybernetic",
        "openai/gpt-5.6-lunatic",
        "gpt-5.6-2026-08-01",
        "OPENAI/gpt-5.6",
        "openai/GPT-5.6",
        "azure/gpt-5.6",
    ] {
        assert!(!ModelUtils::is_valid_model(model), "{model}");
        assert!(
            ModelUtils::validate_model_with_provider(model, "openai").is_err(),
            "{model}"
        );
    }
}

#[test]
fn test_realtime2_validation_is_exact_and_provider_safe() {
    for model in [
        "gpt-realtime-2",
        "gpt-realtime-2.1",
        "gpt-realtime-2.1-mini",
        "openai/gpt-realtime-2.1",
    ] {
        assert!(ModelUtils::is_valid_model(model), "{model}");
        assert!(
            ModelUtils::validate_model_with_provider(model, "openai").is_ok(),
            "{model}"
        );
    }

    for model in [
        "gpt-realtime-2.2",
        "gpt-realtime-2.1-mini-preview",
        "gpt-realtime-2-2026-08-01",
        "OPENAI/gpt-realtime-2",
        "openai/GPT-REALTIME-2",
        "azure/gpt-realtime-2",
    ] {
        assert!(!ModelUtils::is_valid_model(model), "{model}");
        assert!(
            ModelUtils::validate_model_with_provider(model, "openai").is_err(),
            "{model}"
        );
    }
}

// ==================== get_model_family Tests ====================

#[test]
fn test_get_model_family_gpt() {
    assert_eq!(ModelUtils::get_model_family("gpt-4"), "gpt");
    assert_eq!(ModelUtils::get_model_family("gpt-3.5-turbo"), "gpt");
}

#[test]
fn test_get_model_family_claude() {
    assert_eq!(ModelUtils::get_model_family("claude-3-opus"), "claude");
    assert_eq!(ModelUtils::get_model_family("claude-2"), "claude");
}

#[test]
fn test_get_model_family_gemini() {
    assert_eq!(
        ModelUtils::get_model_family("gemini-3.1-pro-preview"),
        "gemini"
    );
}

#[test]
fn test_get_model_family_command() {
    assert_eq!(ModelUtils::get_model_family("command-r-plus"), "command");
}

#[test]
fn test_get_model_family_llama() {
    assert_eq!(ModelUtils::get_model_family("llama-2-70b"), "llama");
}

#[test]
fn test_get_model_family_mistral() {
    assert_eq!(ModelUtils::get_model_family("mistral-large"), "mistral");
}

#[test]
fn test_get_model_family_unknown() {
    assert_eq!(ModelUtils::get_model_family("unknown-model"), "unknown");
}

// ==================== validate_model_with_provider Tests ====================

#[test]
fn test_validate_model_with_provider_valid() {
    assert!(ModelUtils::validate_model_with_provider("gpt-5.5", "openai").is_ok());
    assert!(ModelUtils::validate_model_with_provider("gpt-5.5-pro", "openai").is_ok());
    assert!(ModelUtils::validate_model_with_provider("openai/gpt-5.5", "openai").is_ok());
    assert!(ModelUtils::validate_model_with_provider("openai/gpt-5.5-pro", "openai").is_ok());
    assert!(ModelUtils::validate_model_with_provider("gpt-4", "openai").is_ok());
    assert!(ModelUtils::validate_model_with_provider("claude-3-opus", "anthropic").is_ok());
    assert!(ModelUtils::validate_model_with_provider("gemini-3.1-pro-preview", "google").is_ok());
    assert!(ModelUtils::validate_model_with_provider("gemini-3.7-flash", "google").is_ok());
    assert!(ModelUtils::validate_model_with_provider("google/gemini-3.7-flash", "google").is_ok());
}

#[test]
fn test_validate_model_with_provider_invalid() {
    assert!(ModelUtils::validate_model_with_provider("gpt-4", "anthropic").is_err());
    assert!(ModelUtils::validate_model_with_provider("claude-3-opus", "openai").is_err());
    for near_match in [
        "GEMINI-3.7-FLASH",
        "google/GEMINI-3.7-FLASH",
        "gemini-3.7-flash-preview",
        "gemini-3.7-flash-20260813",
        "gemini-3.7-flash-suffix",
    ] {
        assert!(
            ModelUtils::validate_model_with_provider(near_match, "google").is_err(),
            "stable Gemini 3.7 validation must reject {near_match}"
        );
    }
}

#[test]
fn test_validate_model_with_provider_unknown_provider() {
    assert!(ModelUtils::validate_model_with_provider("any-model", "unknown-provider").is_ok());
}

// ==================== get_compatible_models_for_provider Tests ====================

#[test]
fn test_get_compatible_models_openai() {
    let models = ModelUtils::get_compatible_models_for_provider("openai");
    assert!(models.contains(&"gpt-5.6".to_string()));
    assert!(models.contains(&"gpt-5.6-sol".to_string()));
    assert!(models.contains(&"gpt-5.6-terra".to_string()));
    assert!(models.contains(&"gpt-5.6-luna".to_string()));
    assert!(models.contains(&"gpt-5.6-cyber".to_string()));
    assert!(models.contains(&"gpt-5.5".to_string()));
    assert!(models.contains(&"gpt-5.5-pro".to_string()));
    assert!(models.contains(&"gpt-4".to_string()));
    assert!(models.contains(&"gpt-3.5-turbo".to_string()));
}

#[test]
fn test_get_compatible_models_anthropic() {
    let models = ModelUtils::get_compatible_models_for_provider("anthropic");
    assert!(models.contains(&"claude-opus-4-8".to_string()));
    assert!(models.contains(&"claude-3-opus".to_string()));
    assert!(models.contains(&"claude-2".to_string()));
}

#[test]
fn test_get_compatible_models_google() {
    let models = ModelUtils::get_compatible_models_for_provider("google");
    assert!(models.contains(&"gemini-3.7-flash".to_string()));
    assert!(models.contains(&"gemini-3.5-flash".to_string()));
    assert!(models.contains(&"gemini-3.1-flash-lite".to_string()));
    assert!(models.contains(&"gemini-pro".to_string()));
    assert!(models.contains(&"gemini-1.5-pro".to_string()));
    assert!(models.contains(&"gemini-2.0-flash".to_string()));
    assert!(models.contains(&"gemini-3.1-flash".to_string()));
    assert!(models.contains(&"gemini-3.1-pro-preview".to_string()));
    assert!(models.contains(&"gemini-3-flash-preview".to_string()));
}

#[test]
fn test_get_compatible_models_cohere() {
    let models = ModelUtils::get_compatible_models_for_provider("cohere");
    assert!(models.contains(&"command".to_string()));
    assert!(models.contains(&"command-r-plus".to_string()));
}

#[test]
fn test_get_compatible_models_mistral() {
    let models = ModelUtils::get_compatible_models_for_provider("mistral");
    assert!(models.contains(&"mistral-large".to_string()));
}

#[test]
fn test_get_compatible_models_unknown() {
    let models = ModelUtils::get_compatible_models_for_provider("unknown");
    assert!(models.is_empty());
}

#[test]
fn test_get_compatible_models_case_insensitive() {
    let models = ModelUtils::get_compatible_models_for_provider("OPENAI");
    assert!(!models.is_empty());
}
