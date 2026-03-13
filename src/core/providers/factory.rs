//! Provider factory: creation from configuration
//!
//! Contains `create_provider`, `from_config_async`, config builder helpers,
//! and `is_provider_selector_supported`.

use super::provider_type::ProviderType;
use super::unified_provider::ProviderError;
use super::{Provider, anthropic, cloudflare, macros, mistral, openai, openai_like, registry};
use tracing::warn;

/// Returns true if a provider selector can be instantiated by the current runtime.
///
/// The selector is resolved using the same precedence as `create_provider`:
/// 1. Tier-1 data-driven catalog names
/// 2. Built-in factory provider types
pub fn is_provider_selector_supported(selector: &str) -> bool {
    let normalized = selector.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }

    if registry::get_definition(&normalized).is_some() {
        return true;
    }

    let provider_type = ProviderType::from(normalized.as_str());
    if matches!(provider_type, ProviderType::Custom(_)) {
        return false;
    }

    Provider::factory_supported_provider_types().contains(&provider_type)
}

/// Create a provider from configuration
///
/// This is the main factory function for creating providers
pub async fn create_provider(
    config: crate::config::models::provider::ProviderConfig,
) -> Result<Provider, ProviderError> {
    use serde_json::Value;

    let crate::config::models::provider::ProviderConfig {
        name,
        provider_type,
        api_key,
        base_url,
        api_version,
        organization,
        project,
        timeout,
        max_retries,
        settings,
        ..
    } = config;

    let provider_selector = if provider_type.trim().is_empty() {
        name.as_str()
    } else {
        provider_type.as_str()
    };
    let provider_type_enum = ProviderType::from(provider_selector);

    // --- Tier 1: check the data-driven catalog first ---
    let provider_name_lower = provider_selector.to_lowercase();
    if let Some(def) = registry::get_definition(&provider_name_lower) {
        let effective_key = if api_key.is_empty() {
            def.resolve_api_key(None)
        } else {
            Some(api_key.clone())
        };
        let mut oai_config =
            def.to_openai_like_config(effective_key.as_deref(), base_url.as_deref());
        oai_config.base.timeout = timeout;
        oai_config.base.max_retries = max_retries;

        if let Some(version) = api_version.filter(|v| !v.trim().is_empty()) {
            oai_config.base.api_version = Some(version);
        }
        if let Some(org) = organization.filter(|v| !v.trim().is_empty()) {
            oai_config.base.organization = Some(org);
        }

        let ignored_settings = apply_tier1_openai_like_overrides(&mut oai_config, &settings);
        if !ignored_settings.is_empty() {
            warn!(
                provider = def.name,
                ignored_settings = ?ignored_settings,
                "Tier-1 catalog provider has unsupported settings that were ignored"
            );
        }
        if let Some(project) = project.filter(|v| !v.trim().is_empty()) {
            warn!(
                provider = def.name,
                project = %project,
                "Provider project field is ignored for Tier-1 catalog providers"
            );
        }

        let provider = openai_like::OpenAILikeProvider::new(oai_config)
            .await
            .map_err(|e| ProviderError::initialization(def.name, e.to_string()))?;
        return Ok(Provider::OpenAILike(provider));
    }

    // --- Tier 2/3: existing factory logic ---
    if let ProviderType::Custom(custom_name) = &provider_type_enum {
        return Err(ProviderError::not_implemented(
            "unknown",
            format!(
                "Unknown provider type '{}' (name='{}'). Add a supported provider_type or implementation.",
                custom_name, name
            ),
        ));
    }
    if !Provider::factory_supported_provider_types().contains(&provider_type_enum) {
        return Err(ProviderError::not_implemented(
            "unknown",
            format!("Factory for {:?} not yet implemented", provider_type_enum),
        ));
    }

    let mut factory_config = serde_json::Map::new();

    if !api_key.is_empty() {
        factory_config.insert("api_key".to_string(), Value::String(api_key.clone()));
    }
    if let Some(value) = base_url.filter(|v| !v.is_empty()) {
        factory_config.insert("base_url".to_string(), Value::String(value));
    }
    if let Some(value) = api_version.filter(|v| !v.is_empty()) {
        factory_config.insert("api_version".to_string(), Value::String(value));
    }
    if let Some(value) = organization.filter(|v| !v.is_empty()) {
        factory_config.insert("organization".to_string(), Value::String(value.clone()));
        factory_config
            .entry("account_id".to_string())
            .or_insert(Value::String(value));
    }
    if let Some(value) = project.filter(|v| !v.is_empty()) {
        factory_config.insert("project".to_string(), Value::String(value));
    }

    for (key, value) in settings {
        factory_config.entry(key).or_insert(value);
    }

    if matches!(provider_type_enum, ProviderType::Cloudflare)
        && !factory_config.contains_key("api_token")
        && !api_key.is_empty()
    {
        factory_config.insert("api_token".to_string(), Value::String(api_key));
    }

    Provider::from_config_async(provider_type_enum, Value::Object(factory_config)).await
}

// ==================== Config Extraction Helpers ====================

fn config_str<'a>(config: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    config
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn config_u32(config: &serde_json::Value, key: &str) -> Option<u32> {
    config
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn config_u64(config: &serde_json::Value, key: &str) -> Option<u64> {
    config.get(key).and_then(serde_json::Value::as_u64)
}

fn config_bool(config: &serde_json::Value, key: &str) -> Option<bool> {
    config.get(key).and_then(serde_json::Value::as_bool)
}

fn merge_string_headers(
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

fn merge_string_headers_value(
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

fn apply_tier1_openai_like_overrides(
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

// ==================== Provider::from_config_async ====================

impl Provider {
    /// Create provider from configuration asynchronously
    ///
    /// This is the preferred method for creating providers from configuration.
    /// It supports all provider types and handles async initialization properly.
    pub async fn from_config_async(
        provider_type: ProviderType,
        config: serde_json::Value,
    ) -> Result<Self, ProviderError> {
        match provider_type {
            ProviderType::OpenAI => {
                let openai_config = build_openai_config_from_factory(&config)?;
                let provider = openai::OpenAIProvider::new(openai_config)
                    .await
                    .map_err(|e| ProviderError::initialization("openai", e.to_string()))?;
                Ok(Provider::OpenAI(provider))
            }
            ProviderType::Anthropic => {
                let anthropic_config = build_anthropic_config_from_factory(&config)?;
                let provider = anthropic::AnthropicProvider::new(anthropic_config)?;
                Ok(Provider::Anthropic(provider))
            }
            ProviderType::Mistral => {
                let mistral_config = build_mistral_config_from_factory(&config)?;
                let provider = mistral::MistralProvider::new(mistral_config)
                    .await
                    .map_err(|e| ProviderError::initialization("mistral", e.to_string()))?;
                Ok(Provider::Mistral(provider))
            }
            ProviderType::Cloudflare => {
                let cf_config = build_cloudflare_config_from_factory(&config)?;
                let provider = cloudflare::CloudflareProvider::new(cf_config)
                    .await
                    .map_err(|e| ProviderError::initialization("cloudflare", e.to_string()))?;
                Ok(Provider::Cloudflare(provider))
            }
            ProviderType::OpenAICompatible => {
                let oai_like = build_openai_like_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_like)
                    .await
                    .map_err(|e| {
                        ProviderError::initialization("openai_compatible", e.to_string())
                    })?;
                Ok(Provider::OpenAILike(provider))
            }
            _ => Err(ProviderError::not_implemented(
                "unknown",
                format!("Factory for {:?} not yet implemented", provider_type),
            )),
        }
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    fn supported_factory_provider_types() -> Vec<ProviderType> {
        Provider::factory_supported_provider_types().to_vec()
    }

    #[tokio::test]
    async fn test_from_config_async_supported_variants_do_not_fallthrough_to_not_implemented() {
        for provider_type in supported_factory_provider_types() {
            let err = Provider::from_config_async(provider_type.clone(), serde_json::json!({}))
                .await
                .expect_err("Expected empty config to fail");
            assert!(
                !matches!(err, ProviderError::NotImplemented { .. }),
                "{:?} unexpectedly fell through to NotImplemented: {}",
                provider_type,
                err
            );
        }
    }

    #[tokio::test]
    async fn test_from_config_async_unsupported_variants_return_not_implemented() {
        let supported = supported_factory_provider_types();

        for provider_type in super::super::provider_type::all_non_custom_provider_types() {
            if supported.contains(&provider_type) {
                continue;
            }

            let err = Provider::from_config_async(provider_type.clone(), serde_json::json!({}))
                .await
                .expect_err("Expected unsupported provider to fail");
            assert!(
                matches!(err, ProviderError::NotImplemented { .. }),
                "Expected NotImplemented for {:?}, got {}",
                provider_type,
                err
            );
        }
    }

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

    #[tokio::test]
    async fn test_from_config_async_cloudflare_accepts_alias_fields() {
        let config = serde_json::json!({
            "organization": "acct-alias",
            "api_key": "token-alias"
        });

        let provider = Provider::from_config_async(ProviderType::Cloudflare, config)
            .await
            .unwrap_or_else(|err| {
                panic!("cloudflare should be creatable from alias fields: {err}")
            });
        assert!(matches!(provider, Provider::Cloudflare(_)));
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

    #[tokio::test]
    async fn test_from_config_async_openai_compatible_accepts_api_base_alias() {
        let config = serde_json::json!({
            "api_base": "http://localhost:11434/v1",
            "skip_api_key": true,
            "provider_name": "local-openai-like"
        });

        let provider = Provider::from_config_async(ProviderType::OpenAICompatible, config)
            .await
            .unwrap_or_else(|err| panic!("openai_compatible should be creatable: {err}"));
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    #[test]
    fn test_provider_selector_support_detection() {
        assert!(is_provider_selector_supported("openai"));
        assert!(is_provider_selector_supported("openai_compatible"));
        assert!(is_provider_selector_supported("groq")); // Tier-1 catalog
        assert!(!is_provider_selector_supported("totally_unknown_provider"));
    }

    #[test]
    fn test_catalog_entries_are_supported_selectors() {
        for name in registry::PROVIDER_CATALOG.keys() {
            assert!(
                is_provider_selector_supported(name),
                "Catalog provider '{}' must be a supported selector",
                name
            );
        }
    }

    #[tokio::test]
    async fn test_catalog_entries_are_creatable_via_factory() {
        for (name, def) in registry::PROVIDER_CATALOG.iter() {
            let config = crate::config::models::provider::ProviderConfig {
                name: (*name).to_string(),
                provider_type: (*name).to_string(),
                api_key: if def.skip_api_key {
                    String::new()
                } else {
                    "test-key".to_string()
                },
                ..Default::default()
            };

            let provider = create_provider(config).await.unwrap_or_else(|e| {
                panic!("Catalog provider '{}' should be creatable: {}", name, e)
            });

            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Catalog provider '{}' must create OpenAILike variant",
                name
            );
        }
    }

    #[tokio::test]
    async fn test_create_provider_prefers_provider_type_over_name() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "openai".to_string(),
            provider_type: "pydantic_ai".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unsupported provider type to fail");
        assert!(
            matches!(err, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented error, got {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_provider_falls_back_to_name_when_provider_type_empty() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "pydantic_ai".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unsupported provider name to fail");
        assert!(
            matches!(err, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented error, got {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_provider_tier1_catalog_creates_openai_like() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "perplexity".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let provider = create_provider(config)
            .await
            .expect("Tier 1 provider should succeed");
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    #[tokio::test]
    async fn test_create_provider_tier1_catalog_applies_openai_like_overrides() {
        let mut config = crate::config::models::provider::ProviderConfig {
            name: "perplexity".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            timeout: 42,
            max_retries: 6,
            api_version: Some("2024-01-01".to_string()),
            organization: Some("org-top-level".to_string()),
            ..Default::default()
        };
        config
            .settings
            .insert("model_prefix".to_string(), serde_json::json!("pplx/"));
        config.settings.insert(
            "default_model".to_string(),
            serde_json::json!("llama-3.1-sonar-small"),
        );
        config
            .settings
            .insert("pass_through_params".to_string(), serde_json::json!(false));
        config.settings.insert(
            "headers".to_string(),
            serde_json::json!({"x-test-header": "ok"}),
        );
        config.settings.insert(
            "custom_headers".to_string(),
            serde_json::json!({"x-custom-header": "ok"}),
        );

        let provider = create_provider(config)
            .await
            .expect("Tier 1 provider should accept openai-like overrides");

        match provider {
            Provider::OpenAILike(provider) => {
                let cfg = provider.config();
                assert_eq!(cfg.provider_name, "perplexity");
                assert_eq!(cfg.base.timeout, 42);
                assert_eq!(cfg.base.max_retries, 6);
                assert_eq!(cfg.base.api_version.as_deref(), Some("2024-01-01"));
                assert_eq!(cfg.base.organization.as_deref(), Some("org-top-level"));
                assert_eq!(cfg.model_prefix.as_deref(), Some("pplx/"));
                assert_eq!(cfg.default_model.as_deref(), Some("llama-3.1-sonar-small"));
                assert!(!cfg.pass_through_params);
                assert_eq!(
                    cfg.base.headers.get("x-test-header").map(String::as_str),
                    Some("ok")
                );
                assert_eq!(
                    cfg.custom_headers
                        .get("x-custom-header")
                        .map(String::as_str),
                    Some("ok")
                );
            }
            _ => panic!("Expected OpenAILike provider"),
        }
    }

    #[test]
    fn test_b1_first_batch_selectors_are_supported() {
        for selector in ["aiml_api", "anyscale", "bytez", "comet_api"] {
            assert!(
                is_provider_selector_supported(selector),
                "Expected selector '{}' to be supported",
                selector
            );
        }
    }

    #[tokio::test]
    async fn test_b1_first_batch_create_provider_from_name() {
        for provider_name in ["aiml_api", "anyscale", "bytez", "comet_api"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: provider_name.to_string(),
                provider_type: "".to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config)
                .await
                .unwrap_or_else(|e| panic!("Expected '{}' to be creatable: {}", provider_name, e));
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected '{}' to create OpenAILike provider",
                provider_name
            );
        }
    }

    #[tokio::test]
    async fn test_b1_first_batch_create_provider_from_provider_type() {
        for provider_type in ["aiml_api", "anyscale", "bytez", "comet_api"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: "openai".to_string(),
                provider_type: provider_type.to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config).await.unwrap_or_else(|e| {
                panic!(
                    "Expected '{}' provider_type to be creatable: {}",
                    provider_type, e
                )
            });
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected provider_type '{}' to create OpenAILike provider",
                provider_type
            );
        }
    }

    #[test]
    fn test_b2_second_batch_selectors_are_supported() {
        for selector in ["compactifai", "aleph_alpha", "yi", "lambda_ai"] {
            assert!(
                is_provider_selector_supported(selector),
                "Expected selector '{}' to be supported",
                selector
            );
        }
    }

    #[tokio::test]
    async fn test_b2_second_batch_create_provider_from_name() {
        for provider_name in ["compactifai", "aleph_alpha", "yi", "lambda_ai"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: provider_name.to_string(),
                provider_type: "".to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config)
                .await
                .unwrap_or_else(|e| panic!("Expected '{}' to be creatable: {}", provider_name, e));
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected '{}' to create OpenAILike provider",
                provider_name
            );
        }
    }

    #[tokio::test]
    async fn test_b2_second_batch_create_provider_from_provider_type() {
        for provider_type in ["compactifai", "aleph_alpha", "yi", "lambda_ai"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: "openai".to_string(),
                provider_type: provider_type.to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config).await.unwrap_or_else(|e| {
                panic!(
                    "Expected '{}' provider_type to be creatable: {}",
                    provider_type, e
                )
            });
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected provider_type '{}' to create OpenAILike provider",
                provider_type
            );
        }
    }

    #[test]
    fn test_b3_third_batch_selectors_are_supported() {
        for selector in ["ovhcloud", "maritalk", "siliconflow", "lemonade"] {
            assert!(
                is_provider_selector_supported(selector),
                "Expected selector '{}' to be supported",
                selector
            );
        }
    }

    #[tokio::test]
    async fn test_b3_third_batch_create_provider_from_name() {
        for provider_name in ["ovhcloud", "maritalk", "siliconflow", "lemonade"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: provider_name.to_string(),
                provider_type: "".to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config)
                .await
                .unwrap_or_else(|e| panic!("Expected '{}' to be creatable: {}", provider_name, e));
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected '{}' to create OpenAILike provider",
                provider_name
            );
        }
    }

    #[tokio::test]
    async fn test_b3_third_batch_create_provider_from_provider_type() {
        for provider_type in ["ovhcloud", "maritalk", "siliconflow", "lemonade"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: "openai".to_string(),
                provider_type: provider_type.to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config).await.unwrap_or_else(|e| {
                panic!(
                    "Expected '{}' provider_type to be creatable: {}",
                    provider_type, e
                )
            });
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected provider_type '{}' to create OpenAILike provider",
                provider_type
            );
        }
    }

    #[tokio::test]
    async fn test_create_provider_reports_unknown_custom_provider() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "my-custom-provider".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unknown custom provider to fail");
        assert!(
            matches!(err, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented error, got {}",
            err
        );
        assert!(
            err.to_string().contains("my-custom-provider"),
            "Expected custom provider name in error, got {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_provider_openai_compatible_factory() {
        let mut config = crate::config::models::provider::ProviderConfig {
            name: "local-openai-like".to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "".to_string(),
            base_url: Some("http://localhost:11434/v1".to_string()),
            ..Default::default()
        };
        config
            .settings
            .insert("skip_api_key".to_string(), serde_json::Value::Bool(true));

        let provider = create_provider(config)
            .await
            .expect("openai_compatible provider should be creatable");
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }
}
