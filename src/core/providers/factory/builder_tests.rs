//! Tests for provider-specific config builders

use super::builder::*;

#[test]
fn test_build_openai_config_from_factory_maps_optional_fields() {
    let config = serde_json::json!({
        "api_key": "sk-test123",
        "base_url": "https://example-openai.test/v1",
        "timeout": 42,
        "max_retries": 7,
        "organization": "org-test",
        "project": "proj-test",
        "headers": {
            "x-team-id": "team-1"
        },
        "custom_headers": {
            "x-request-source": "gateway"
        },
        "model_mappings": {
            "gpt-4": "gpt-4o",
            "ignored": 123
        }
    });

    let openai_config = build_openai_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("openai config should parse: {err}"));
    assert_eq!(openai_config.base.api_key.as_deref(), Some("sk-test123"));
    assert_eq!(
        openai_config.base.api_base.as_deref(),
        Some("https://example-openai.test/v1")
    );
    assert_eq!(openai_config.base.timeout, 42);
    assert_eq!(openai_config.base.max_retries, 7);
    assert_eq!(openai_config.organization.as_deref(), Some("org-test"));
    assert_eq!(openai_config.project.as_deref(), Some("proj-test"));
    assert_eq!(
        openai_config
            .base
            .headers
            .get("x-team-id")
            .map(String::as_str),
        Some("team-1")
    );
    assert_eq!(
        openai_config
            .base
            .headers
            .get("x-request-source")
            .map(String::as_str),
        Some("gateway")
    );
    assert_eq!(
        openai_config
            .model_mappings
            .get("gpt-4")
            .map(String::as_str),
        Some("gpt-4o")
    );
    assert!(!openai_config.model_mappings.contains_key("ignored"));
}

#[test]
fn test_build_anthropic_config_from_factory_maps_optional_fields() {
    let config = serde_json::json!({
        "api_key": "sk-ant-test",
        "api_base": "https://example-anthropic.test",
        "api_version": "2024-01-01",
        "timeout": 99,
        "connect_timeout": 12,
        "max_retries": 6,
        "retry_delay_base": 250,
        "proxy": "http://localhost:8080",
        "headers": {
            "x-anthropic-a": "a"
        },
        "custom_headers": {
            "x-anthropic-b": "b"
        },
        "enable_multimodal": false,
        "enable_cache_control": false,
        "enable_computer_use": true,
        "enable_experimental": true
    });

    let anthropic_config = build_anthropic_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("anthropic config should parse: {err}"));
    assert_eq!(anthropic_config.api_key.as_deref(), Some("sk-ant-test"));
    assert_eq!(anthropic_config.base_url, "https://example-anthropic.test");
    assert_eq!(anthropic_config.api_version, "2024-01-01");
    assert_eq!(anthropic_config.request_timeout, 99);
    assert_eq!(anthropic_config.connect_timeout, 12);
    assert_eq!(anthropic_config.max_retries, 6);
    assert_eq!(anthropic_config.retry_delay_base, 250);
    assert_eq!(
        anthropic_config.proxy_url.as_deref(),
        Some("http://localhost:8080")
    );
    assert_eq!(
        anthropic_config
            .custom_headers
            .get("x-anthropic-a")
            .map(String::as_str),
        Some("a")
    );
    assert_eq!(
        anthropic_config
            .custom_headers
            .get("x-anthropic-b")
            .map(String::as_str),
        Some("b")
    );
    assert!(!anthropic_config.enable_multimodal);
    assert!(!anthropic_config.enable_cache_control);
    assert!(anthropic_config.enable_computer_use);
    assert!(anthropic_config.enable_experimental);
}

#[test]
fn test_build_mistral_config_from_factory_maps_optional_fields() {
    let config = serde_json::json!({
        "api_key": "mistral-key",
        "api_base": "https://example-mistral.test/v1",
        "timeout": 88,
        "max_retries": 4
    });

    let mistral_config = build_mistral_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("mistral config should parse: {err}"));
    assert_eq!(mistral_config.api_key, "mistral-key");
    assert_eq!(mistral_config.api_base, "https://example-mistral.test/v1");
    assert_eq!(mistral_config.timeout_seconds, 88);
    assert_eq!(mistral_config.max_retries, 4);
}

#[test]
fn test_build_cloudflare_config_from_factory_maps_alias_and_optional_fields() {
    let config = serde_json::json!({
        "organization": "acct-xyz",
        "api_key": "token-xyz",
        "base_url": "https://cf.example.test",
        "timeout": 77,
        "max_retries": 5,
        "debug": true
    });

    let cf_config = build_cloudflare_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("cloudflare config should parse: {err}"));
    assert_eq!(cf_config.account_id.as_deref(), Some("acct-xyz"));
    assert_eq!(cf_config.api_token.as_deref(), Some("token-xyz"));
    assert_eq!(
        cf_config.api_base.as_deref(),
        Some("https://cf.example.test")
    );
    assert_eq!(cf_config.timeout, 77);
    assert_eq!(cf_config.max_retries, 5);
    assert!(cf_config.debug);
}

#[test]
fn test_build_openai_like_config_from_factory_maps_optional_fields() {
    let config = serde_json::json!({
        "base_url": "https://openai-like.example.test/v1",
        "api_key": "sk-openai-like",
        "provider_name": "custom-like",
        "timeout": 55,
        "max_retries": 4,
        "model_prefix": "prefix/",
        "default_model": "gpt-4o-mini",
        "pass_through_params": false,
        "skip_api_key": true,
        "organization": "org-like",
        "api_version": "2024-12-01",
        "headers": {
            "x-base-header": "base"
        },
        "custom_headers": {
            "x-custom-header": "custom"
        }
    });

    let oai_like = build_openai_like_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("openai_like config should parse: {err}"));

    assert_eq!(
        oai_like.base.api_base.as_deref(),
        Some("https://openai-like.example.test/v1")
    );
    assert_eq!(oai_like.base.api_key.as_deref(), Some("sk-openai-like"));
    assert_eq!(oai_like.provider_name, "custom-like");
    assert_eq!(oai_like.base.timeout, 55);
    assert_eq!(oai_like.base.max_retries, 4);
    assert_eq!(oai_like.model_prefix.as_deref(), Some("prefix/"));
    assert_eq!(oai_like.default_model.as_deref(), Some("gpt-4o-mini"));
    assert!(!oai_like.pass_through_params);
    assert!(oai_like.skip_api_key);
    assert_eq!(oai_like.base.organization.as_deref(), Some("org-like"));
    assert_eq!(oai_like.base.api_version.as_deref(), Some("2024-12-01"));
    assert_eq!(
        oai_like
            .base
            .headers
            .get("x-base-header")
            .map(String::as_str),
        Some("base")
    );
    assert_eq!(
        oai_like
            .custom_headers
            .get("x-custom-header")
            .map(String::as_str),
        Some("custom")
    );
}

#[test]
fn test_build_openai_like_config_from_factory_requires_api_base() {
    let config = serde_json::json!({
        "api_key": "sk-openai-like"
    });

    let err = build_openai_like_config_from_factory(&config)
        .err()
        .unwrap_or_else(|| panic!("missing base_url should return an error"));
    assert!(err.to_string().contains("base_url"));
}
