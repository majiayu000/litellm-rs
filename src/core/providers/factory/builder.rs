//! Provider-specific config builders
//!
//! Contains config extraction helpers and per-provider config builder functions
//! used by the factory when constructing provider instances.

use super::super::unified_provider::ProviderError;
use super::super::{anthropic, cloudflare, macros, mistral, openai, openai_like};

// ==================== Config Extraction Helpers ====================

pub(super) fn config_str<'a>(config: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    config
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

pub(super) fn config_u32(config: &serde_json::Value, key: &str) -> Option<u32> {
    config
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

pub(super) fn config_u64(config: &serde_json::Value, key: &str) -> Option<u64> {
    config.get(key).and_then(serde_json::Value::as_u64)
}

pub(super) fn config_bool(config: &serde_json::Value, key: &str) -> Option<bool> {
    config.get(key).and_then(serde_json::Value::as_bool)
}

pub(super) fn merge_string_headers(
    target: &mut std::collections::HashMap<String, String>,
    config: &serde_json::Value,
    key: &str,
) {
    if let Some(header_map) = config.get(key).and_then(serde_json::Value::as_object) {
        for (header_key, header_value) in header_map {
            if let Some(header_value) = header_value.as_str() {
                target.insert(header_key.clone(), header_value.to_string());
            }
        }
    }
}

pub(super) fn merge_string_headers_value(
    target: &mut std::collections::HashMap<String, String>,
    value: &serde_json::Value,
) -> bool {
    if let Some(header_map) = value.as_object() {
        for (header_key, header_value) in header_map {
            if let Some(header_value) = header_value.as_str() {
                target.insert(header_key.clone(), header_value.to_string());
            }
        }
        return true;
    }
    false
}

pub(super) fn apply_tier1_openai_like_overrides(
    config: &mut openai_like::OpenAILikeConfig,
    settings: &std::collections::HashMap<String, serde_json::Value>,
) -> Vec<String> {
    let mut ignored = Vec::new();

    for (key, value) in settings {
        let consumed = match key.as_str() {
            "headers" => merge_string_headers_value(&mut config.base.headers, value),
            "custom_headers" => merge_string_headers_value(&mut config.custom_headers, value),
            "model_prefix" => {
                if let Some(v) = value.as_str().filter(|v| !v.trim().is_empty()) {
                    config.model_prefix = Some(v.to_string());
                    true
                } else {
                    false
                }
            }
            "default_model" => {
                if let Some(v) = value.as_str().filter(|v| !v.trim().is_empty()) {
                    config.default_model = Some(v.to_string());
                    true
                } else {
                    false
                }
            }
            "pass_through_params" => {
                if let Some(v) = value.as_bool() {
                    config.pass_through_params = v;
                    true
                } else {
                    false
                }
            }
            "skip_api_key" => {
                if let Some(v) = value.as_bool() {
                    config.skip_api_key = v;
                    true
                } else {
                    false
                }
            }
            "timeout" => {
                if let Some(v) = value.as_u64() {
                    config.base.timeout = v;
                    true
                } else {
                    false
                }
            }
            "max_retries" => {
                if let Some(v) = value.as_u64().and_then(|n| u32::try_from(n).ok()) {
                    config.base.max_retries = v;
                    true
                } else {
                    false
                }
            }
            "organization" => {
                if let Some(v) = value.as_str().filter(|v| !v.trim().is_empty()) {
                    config.base.organization = Some(v.to_string());
                    true
                } else {
                    false
                }
            }
            "api_version" => {
                if let Some(v) = value.as_str().filter(|v| !v.trim().is_empty()) {
                    config.base.api_version = Some(v.to_string());
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        if !consumed {
            ignored.push(key.clone());
        }
    }

    ignored.sort();
    ignored
}

// ==================== Provider-Specific Config Builders ====================

pub(super) fn build_openai_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai::OpenAIConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "openai")?;
    let mut openai_config = openai::OpenAIConfig::default();
    openai_config.base.api_key = Some(api_key.to_string());

    if let Some(base_url) =
        config_str(config, "base_url").or_else(|| config_str(config, "api_base"))
    {
        openai_config.base.api_base = Some(base_url.to_string());
    }
    if let Some(timeout) = config_u64(config, "timeout") {
        openai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        openai_config.base.max_retries = max_retries;
    }
    if let Some(organization) = config_str(config, "organization") {
        openai_config.organization = Some(organization.to_string());
    }
    if let Some(project) = config_str(config, "project") {
        openai_config.project = Some(project.to_string());
    }

    merge_string_headers(&mut openai_config.base.headers, config, "headers");
    merge_string_headers(&mut openai_config.base.headers, config, "custom_headers");

    if let Some(model_mappings) = config
        .get("model_mappings")
        .and_then(serde_json::Value::as_object)
    {
        for (from_model, to_model) in model_mappings {
            if let Some(to_model) = to_model.as_str() {
                openai_config
                    .model_mappings
                    .insert(from_model.clone(), to_model.to_string());
            }
        }
    }

    Ok(openai_config)
}

pub(super) fn build_anthropic_config_from_factory(
    config: &serde_json::Value,
) -> Result<anthropic::AnthropicConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "anthropic")?;
    let mut anthropic_config = anthropic::AnthropicConfig::default().with_api_key(api_key);

    if let Some(base_url) =
        config_str(config, "base_url").or_else(|| config_str(config, "api_base"))
    {
        anthropic_config.base_url = base_url.to_string();
    }
    if let Some(api_version) = config_str(config, "api_version") {
        anthropic_config.api_version = api_version.to_string();
    }
    if let Some(timeout) = config_u64(config, "timeout") {
        anthropic_config.request_timeout = timeout;
    }
    if let Some(connect_timeout) = config_u64(config, "connect_timeout") {
        anthropic_config.connect_timeout = connect_timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        anthropic_config.max_retries = max_retries;
    }
    if let Some(retry_delay_base) = config_u64(config, "retry_delay_base") {
        anthropic_config.retry_delay_base = retry_delay_base;
    }
    if let Some(proxy_url) = config_str(config, "proxy_url").or_else(|| config_str(config, "proxy"))
    {
        anthropic_config.proxy_url = Some(proxy_url.to_string());
    }

    merge_string_headers(&mut anthropic_config.custom_headers, config, "headers");
    merge_string_headers(
        &mut anthropic_config.custom_headers,
        config,
        "custom_headers",
    );

    if let Some(enable_multimodal) = config_bool(config, "enable_multimodal") {
        anthropic_config.enable_multimodal = enable_multimodal;
    }
    if let Some(enable_cache_control) = config_bool(config, "enable_cache_control") {
        anthropic_config.enable_cache_control = enable_cache_control;
    }
    if let Some(enable_computer_use) = config_bool(config, "enable_computer_use") {
        anthropic_config.enable_computer_use = enable_computer_use;
    }
    if let Some(enable_experimental) = config_bool(config, "enable_experimental") {
        anthropic_config.enable_experimental = enable_experimental;
    }

    Ok(anthropic_config)
}

pub(super) fn build_mistral_config_from_factory(
    config: &serde_json::Value,
) -> Result<mistral::MistralConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "mistral")?;
    let mut mistral_config = mistral::MistralConfig {
        api_key: api_key.to_string(),
        ..Default::default()
    };

    if let Some(base_url) =
        config_str(config, "base_url").or_else(|| config_str(config, "api_base"))
    {
        mistral_config.api_base = base_url.to_string();
    }
    if let Some(timeout) = config_u64(config, "timeout") {
        mistral_config.timeout_seconds = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        mistral_config.max_retries = max_retries;
    }

    Ok(mistral_config)
}

pub(super) fn build_cloudflare_config_from_factory(
    config: &serde_json::Value,
) -> Result<cloudflare::CloudflareConfig, ProviderError> {
    let account_id = config_str(config, "account_id")
        .or_else(|| config_str(config, "organization"))
        .ok_or_else(|| ProviderError::configuration("cloudflare", "account_id is required"))?;
    let api_token = config_str(config, "api_token")
        .or_else(|| config_str(config, "api_key"))
        .ok_or_else(|| ProviderError::configuration("cloudflare", "api_token is required"))?;

    let mut cf_config = cloudflare::CloudflareConfig {
        account_id: Some(account_id.to_string()),
        api_token: Some(api_token.to_string()),
        ..Default::default()
    };

    if let Some(base_url) =
        config_str(config, "base_url").or_else(|| config_str(config, "api_base"))
    {
        cf_config.api_base = Some(base_url.to_string());
    }
    if let Some(timeout) = config_u64(config, "timeout") {
        cf_config.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        cf_config.max_retries = max_retries;
    }
    if let Some(debug) = config_bool(config, "debug") {
        cf_config.debug = debug;
    }

    Ok(cf_config)
}

pub(super) fn build_openai_like_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .ok_or_else(|| {
            ProviderError::configuration("openai_compatible", "base_url (or api_base) is required")
        })?;

    let api_key = config_str(config, "api_key");
    let skip_api_key = config_bool(config, "skip_api_key").unwrap_or(api_key.is_none());

    let mut oai_like = if let Some(api_key) = api_key {
        openai_like::OpenAILikeConfig::with_api_key(api_base, api_key)
    } else {
        openai_like::OpenAILikeConfig::new(api_base).with_skip_api_key(skip_api_key)
    };

    oai_like.skip_api_key = skip_api_key;
    oai_like.provider_name = config_str(config, "provider_name")
        .unwrap_or("openai_compatible")
        .to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_like.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_like.base.max_retries = max_retries;
    }
    if let Some(prefix) = config_str(config, "model_prefix") {
        oai_like.model_prefix = Some(prefix.to_string());
    }
    if let Some(default_model) = config_str(config, "default_model") {
        oai_like.default_model = Some(default_model.to_string());
    }
    if let Some(pass_through) = config_bool(config, "pass_through_params") {
        oai_like.pass_through_params = pass_through;
    }
    if let Some(organization) = config_str(config, "organization") {
        oai_like.base.organization = Some(organization.to_string());
    }
    if let Some(api_version) = config_str(config, "api_version") {
        oai_like.base.api_version = Some(api_version.to_string());
    }

    merge_string_headers(&mut oai_like.base.headers, config, "headers");
    merge_string_headers(&mut oai_like.custom_headers, config, "custom_headers");

    Ok(oai_like)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
