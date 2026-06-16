use super::*;

#[test]
fn test_resolve_dynamic_route_for_moonshot() {
    let options = CompletionOptions::default();
    let route = resolve_dynamic_provider_route("moonshot/kimi-k2.5", &options).unwrap();

    assert_eq!(route.provider_type, "moonshot");
    assert_eq!(route.provider_label, "Moonshot");
    assert_eq!(route.actual_model, "kimi-k2.5");
    assert_eq!(route.api_base, "https://api.moonshot.cn/v1");
}

#[test]
fn test_resolve_dynamic_route_for_minimax() {
    let options = CompletionOptions::default();
    let route = resolve_dynamic_provider_route("minimax/MiniMax-M2.5-lightning", &options).unwrap();

    assert_eq!(route.provider_type, "minimax");
    assert_eq!(route.provider_label, "MiniMax");
    assert_eq!(route.actual_model, "MiniMax-M2.5-lightning");
    assert_eq!(route.api_base, "https://api.minimax.chat/v1");
}

#[test]
fn test_resolve_dynamic_route_for_glm_alias() {
    let options = CompletionOptions::default();
    let route = resolve_dynamic_provider_route("glm/glm-5", &options).unwrap();

    assert_eq!(route.provider_type, "zhipu");
    assert_eq!(route.provider_label, "Zhipu");
    assert_eq!(route.actual_model, "glm-5");
    assert_eq!(route.api_base, "https://open.bigmodel.cn/api/paas/v4");
}

#[test]
fn test_resolve_dynamic_route_for_zai_alias() {
    let options = CompletionOptions::default();
    let route = resolve_dynamic_provider_route("zai/glm-5", &options).unwrap();

    assert_eq!(route.provider_type, "zhipu");
    assert_eq!(route.provider_label, "Zhipu");
    assert_eq!(route.actual_model, "glm-5");
    assert_eq!(route.api_base, "https://open.bigmodel.cn/api/paas/v4");
}

#[test]
fn test_resolve_dynamic_route_for_xai() {
    let options = CompletionOptions::default();
    let Some(route) = resolve_dynamic_provider_route("xai/grok-4.3", &options) else {
        panic!("xAI route should resolve");
    };

    assert_eq!(route.provider_type, "xai");
    assert_eq!(route.provider_label, "xAI");
    assert_eq!(route.actual_model, "grok-4.3");
    assert_eq!(route.api_base, "https://api.x.ai/v1");
}

#[test]
fn test_resolve_dynamic_route_for_groq() {
    let options = CompletionOptions::default();
    let Some(route) = resolve_dynamic_provider_route("groq/llama-3.3-70b-versatile", &options)
    else {
        panic!("Groq route should resolve");
    };

    assert_eq!(route.provider_type, "groq");
    assert_eq!(route.provider_label, "Groq");
    assert_eq!(route.actual_model, "llama-3.3-70b-versatile");
    assert_eq!(route.api_base, "https://api.groq.com/openai/v1");
}

#[test]
fn test_resolve_dynamic_route_for_xiaomi_mimo() {
    let options = CompletionOptions::default();
    let Some(route) = resolve_dynamic_provider_route("xiaomi_mimo/mimo-v2.5-pro", &options) else {
        panic!("Xiaomi MiMo route should resolve");
    };

    assert_eq!(route.provider_type, "xiaomi_mimo");
    assert_eq!(route.provider_label, "Xiaomi MiMo");
    assert_eq!(route.actual_model, "mimo-v2.5-pro");
    assert_eq!(route.api_base, "https://api.xiaomimimo.com/v1");

    let Some(alias_route) = resolve_dynamic_provider_route("mimo/mimo-v2.5", &options) else {
        panic!("MiMo alias route should resolve");
    };
    assert_eq!(alias_route.provider_type, "xiaomi_mimo");
    assert_eq!(alias_route.actual_model, "mimo-v2.5");
}

#[test]
fn test_resolve_dynamic_route_with_custom_api_base() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };

    let route = resolve_dynamic_provider_route("my-custom-model", &options).unwrap();
    assert_eq!(route.provider_type, "openai-compatible");
    assert_eq!(route.provider_label, "OpenAI-Compatible");
    assert_eq!(route.actual_model, "my-custom-model");
    assert_eq!(route.api_base, "http://localhost:5567/v1");
}

#[test]
fn test_custom_api_base_api_key_fallback_uses_env_key() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };
    let Some(route) = resolve_dynamic_provider_route("my-custom-model", &options) else {
        panic!("custom api_base route should resolve");
    };

    let api_key =
        custom_api_base_api_key_fallback(&options, &route, Some("sk-from-env".to_string()), None);

    assert_eq!(api_key.as_deref(), Some("sk-from-env"));
}

#[test]
fn test_custom_api_base_api_key_fallback_uses_dummy_without_env_key() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };
    let Some(route) = resolve_dynamic_provider_route("my-custom-model", &options) else {
        panic!("custom api_base route should resolve");
    };

    let api_key = custom_api_base_api_key_fallback(&options, &route, None, None);

    assert_eq!(api_key.as_deref(), Some("dummy-key-for-local"));
}

#[test]
fn test_dynamic_provider_api_key_env_var_maps_named_routes() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };
    let cases = [
        ("openai/gpt-4.1", "openai", "OPENAI_API_KEY"),
        (
            "openrouter/anthropic/claude-sonnet-4",
            "openrouter",
            "OPENROUTER_API_KEY",
        ),
        (
            "anthropic/claude-sonnet-4",
            "anthropic",
            "ANTHROPIC_API_KEY",
        ),
        ("deepseek/deepseek-chat", "deepseek", "DEEPSEEK_API_KEY"),
        ("moonshot/kimi-k2.5", "moonshot", "MOONSHOT_API_KEY"),
        (
            "minimax/MiniMax-M2.5-lightning",
            "minimax",
            "MINIMAX_API_KEY",
        ),
        ("zhipu/glm-5", "zhipu", "ZHIPU_API_KEY"),
        ("xai/grok-4.3", "xai", "XAI_API_KEY"),
        ("groq/llama-3.3-70b-versatile", "groq", "GROQ_API_KEY"),
        ("xiaomi_mimo/mimo-v2.5-pro", "xiaomi_mimo", "MIMO_API_KEY"),
        ("azure_ai/gpt-4.1", "azure_ai", "AZURE_AI_API_KEY"),
    ];

    for (model, provider_type, env_var) in cases {
        let Some(route) = resolve_dynamic_provider_route(model, &options) else {
            panic!("{model} route should resolve");
        };

        assert_eq!(route.provider_type, provider_type);
        assert_eq!(dynamic_provider_api_key_env_var(&route), Some(env_var));
    }
}

#[test]
fn test_custom_api_base_fallback_supports_openai_compatible_named_routes() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };
    let models = [
        "openai/gpt-4.1",
        "openrouter/anthropic/claude-sonnet-4",
        "deepseek/deepseek-chat",
        "moonshot/kimi-k2.5",
        "minimax/MiniMax-M2.5-lightning",
        "zhipu/glm-5",
        "xai/grok-4.3",
        "groq/llama-3.3-70b-versatile",
        "xiaomi_mimo/mimo-v2.5-pro",
    ];

    for model in models {
        let Some(route) = resolve_dynamic_provider_route(model, &options) else {
            panic!("{model} route should resolve");
        };

        let provider_key = custom_api_base_api_key_fallback(
            &options,
            &route,
            Some("sk-openai-env".to_string()),
            Some("sk-provider-env".to_string()),
        );
        assert_eq!(provider_key.as_deref(), Some("sk-provider-env"));

        let openai_key = custom_api_base_api_key_fallback(
            &options,
            &route,
            Some("sk-openai-env".to_string()),
            None,
        );
        assert_eq!(openai_key.as_deref(), Some("sk-openai-env"));

        let dummy_key = custom_api_base_api_key_fallback(&options, &route, None, None);
        assert_eq!(dummy_key.as_deref(), Some("dummy-key-for-local"));
    }
}

#[test]
fn test_custom_api_base_fallback_requires_provider_key_for_non_openai_compatible_routes() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };
    let models = ["anthropic/claude-sonnet-4", "azure_ai/gpt-4.1"];

    for model in models {
        let Some(route) = resolve_dynamic_provider_route(model, &options) else {
            panic!("{model} route should resolve");
        };

        let openai_key = custom_api_base_api_key_fallback(
            &options,
            &route,
            Some("sk-openai-env".to_string()),
            None,
        );
        assert!(openai_key.is_none());

        let provider_key = custom_api_base_api_key_fallback(
            &options,
            &route,
            Some("sk-openai-env".to_string()),
            Some("sk-provider-env".to_string()),
        );
        assert_eq!(provider_key.as_deref(), Some("sk-provider-env"));
    }
}

#[test]
fn test_dynamic_openai_compatible_config_preserves_headers() {
    let mut headers = std::collections::HashMap::new();
    headers.insert("x-proxy-route".to_string(), "tenant-a".to_string());
    headers.insert("x-request-source".to_string(), "stream-test".to_string());
    let options = CompletionOptions {
        headers: Some(headers),
        organization: Some("org-stream-test".to_string()),
        timeout: Some(17),
        ..CompletionOptions::default()
    };

    let config = dynamic_openai_compatible_config("sk-test", "http://localhost:5567/v1", &options);

    assert_eq!(
        config.base.headers.get("x-proxy-route").map(String::as_str),
        Some("tenant-a")
    );
    assert_eq!(
        config
            .base
            .headers
            .get("x-request-source")
            .map(String::as_str),
        Some("stream-test")
    );
    assert_eq!(config.organization.as_deref(), Some("org-stream-test"));
    assert_eq!(config.base.organization.as_deref(), Some("org-stream-test"));
    assert_eq!(config.base.timeout, 17);
}

#[test]
fn test_dynamic_xai_uses_openai_like_config() {
    let mut headers = std::collections::HashMap::new();
    headers.insert("x-proxy-route".to_string(), "tenant-a".to_string());
    let options = CompletionOptions {
        headers: Some(headers),
        organization: Some("org-xai-test".to_string()),
        timeout: Some(23),
        ..CompletionOptions::default()
    };
    let Some(route) = resolve_dynamic_provider_route("xai/grok-4.20-multi-agent", &options) else {
        panic!("xAI route should resolve");
    };

    let config = dynamic_openai_like_config("sk-test", &route.api_base, &route, &options);

    assert_eq!(config.provider_name, "xai");
    assert_eq!(config.base.api_key.as_deref(), Some("sk-test"));
    assert_eq!(config.base.timeout, 23);
    assert_eq!(
        config.base.headers.get("x-proxy-route").map(String::as_str),
        Some("tenant-a")
    );
    assert_eq!(config.base.organization.as_deref(), Some("org-xai-test"));
}

#[test]
fn test_dynamic_groq_uses_openai_like_config() {
    let mut headers = std::collections::HashMap::new();
    headers.insert("x-proxy-route".to_string(), "tenant-a".to_string());
    headers.insert("x-request-source".to_string(), "stream-test".to_string());
    let options = CompletionOptions {
        headers: Some(headers),
        organization: Some("org-groq-test".to_string()),
        timeout: Some(29),
        ..CompletionOptions::default()
    };
    let Some(route) = resolve_dynamic_provider_route("groq/llama-3.3-70b-versatile", &options)
    else {
        panic!("Groq route should resolve");
    };

    assert!(uses_dynamic_openai_like_provider(&route));

    let config = dynamic_openai_like_config("sk-test", &route.api_base, &route, &options);

    assert_eq!(config.provider_name, "groq");
    assert_eq!(config.base.api_key.as_deref(), Some("sk-test"));
    assert_eq!(config.base.timeout, 29);
    assert_eq!(
        config.base.headers.get("x-proxy-route").map(String::as_str),
        Some("tenant-a")
    );
    assert_eq!(
        config
            .base
            .headers
            .get("x-request-source")
            .map(String::as_str),
        Some("stream-test")
    );
    assert_eq!(config.base.organization.as_deref(), Some("org-groq-test"));
}

#[test]
fn test_dynamic_openrouter_uses_openai_like_config() {
    let mut headers = std::collections::HashMap::new();
    headers.insert("x-proxy-route".to_string(), "tenant-a".to_string());
    let options = CompletionOptions {
        headers: Some(headers),
        organization: Some("org-openrouter-test".to_string()),
        timeout: Some(31),
        ..CompletionOptions::default()
    };
    let Some(route) =
        resolve_dynamic_provider_route("openrouter/anthropic/claude-sonnet-4", &options)
    else {
        panic!("OpenRouter route should resolve");
    };

    assert!(uses_dynamic_openai_like_provider(&route));

    let config = dynamic_openai_like_config("sk-test", &route.api_base, &route, &options);

    assert_eq!(config.provider_name, "openrouter");
    assert_eq!(config.base.api_key.as_deref(), Some("sk-test"));
    assert_eq!(config.base.timeout, 31);
    assert_eq!(
        config.base.headers.get("x-proxy-route").map(String::as_str),
        Some("tenant-a")
    );
    assert_eq!(
        config.base.organization.as_deref(),
        Some("org-openrouter-test")
    );
}

#[test]
fn test_xai_api_base_override_stays_named_dynamic_route() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };

    assert!(is_named_dynamic_provider_route("xai/grok-4.3", &options));

    let Some(route) = resolve_dynamic_provider_route("xai/grok-4.3", &options) else {
        panic!("xAI route should resolve before generic custom api_base");
    };

    assert_eq!(route.provider_type, "xai");
    assert_eq!(route.actual_model, "grok-4.3");
    assert_eq!(route.api_base, "http://localhost:5567/v1");

    let api_key = custom_api_base_api_key_fallback(&options, &route, None, None);
    assert_eq!(api_key.as_deref(), Some("dummy-key-for-local"));
}

#[test]
fn test_xai_api_base_override_prefers_xai_env_key() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };
    let Some(route) = resolve_dynamic_provider_route("xai/grok-4.3", &options) else {
        panic!("xAI route should resolve before generic custom api_base");
    };

    let api_key = custom_api_base_api_key_fallback(
        &options,
        &route,
        Some("sk-openai-env".to_string()),
        Some("sk-xai-env".to_string()),
    );

    assert_eq!(api_key.as_deref(), Some("sk-xai-env"));
}

#[test]
fn test_api_key_fallback_does_not_apply_to_prefixed_routes() {
    let options = CompletionOptions::default();
    let Some(route) = resolve_dynamic_provider_route("xai/grok-4.3", &options) else {
        panic!("prefixed route should resolve");
    };

    let api_key =
        custom_api_base_api_key_fallback(&options, &route, Some("sk-from-env".to_string()), None);

    assert!(api_key.is_none());
}

#[test]
fn test_provider_env_key_does_not_activate_default_named_route() {
    let options = CompletionOptions::default();
    let Some(route) = resolve_dynamic_provider_route("xai/grok-4.3", &options) else {
        panic!("prefixed route should resolve");
    };

    let api_key = resolve_dynamic_provider_api_key_from_sources(
        &options,
        &route,
        Some("sk-xai-env".to_string()),
        Some("sk-openai-env".to_string()),
    );

    assert!(api_key.is_none());
}

#[test]
fn test_explicit_api_key_activates_default_named_route() {
    let options = CompletionOptions {
        api_key: Some("sk-request".to_string()),
        ..CompletionOptions::default()
    };
    let Some(route) = resolve_dynamic_provider_route("xai/grok-4.3", &options) else {
        panic!("prefixed route should resolve");
    };

    let api_key = resolve_dynamic_provider_api_key_from_sources(
        &options,
        &route,
        Some("sk-xai-env".to_string()),
        Some("sk-openai-env".to_string()),
    );

    assert_eq!(api_key.as_deref(), Some("sk-request"));
}

#[test]
fn test_provider_env_key_still_activates_api_base_override() {
    let options = CompletionOptions {
        api_base: Some("http://localhost:5567/v1".to_string()),
        ..CompletionOptions::default()
    };
    let Some(route) = resolve_dynamic_provider_route("xai/grok-4.3", &options) else {
        panic!("prefixed route should resolve");
    };

    let api_key = resolve_dynamic_provider_api_key_from_sources(
        &options,
        &route,
        Some("sk-xai-env".to_string()),
        Some("sk-openai-env".to_string()),
    );

    assert_eq!(api_key.as_deref(), Some("sk-xai-env"));
}

#[test]
fn test_resolve_dynamic_route_without_prefix_or_api_base() {
    let options = CompletionOptions::default();
    let route = resolve_dynamic_provider_route("my-custom-model", &options);
    assert!(route.is_none());
}
