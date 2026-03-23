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

pub(super) fn build_meta_llama_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "meta_llama")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .unwrap_or("https://api.llama.com/compat/v1");

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "meta_llama".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_v0_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "v0")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .unwrap_or("https://api.v0.dev/v1");

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "v0".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_azure_ai_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "azure_ai")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .or_else(|| config_str(config, "endpoint"))
        .ok_or_else(|| {
            ProviderError::configuration("azure_ai", "base_url (or endpoint) is required")
        })?;

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "azure_ai".to_string();

    if let Some(api_version) = config_str(config, "api_version") {
        oai_config.base.api_version = Some(api_version.to_string());
    }
    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_amazon_nova_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "amazon_nova")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .unwrap_or("https://api.nova.amazon.com/v1");

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "amazon_nova".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_fal_ai_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "fal_ai")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .unwrap_or("https://fal.run");

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "fal_ai".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_azure_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "azure")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .or_else(|| config_str(config, "endpoint"))
        .ok_or_else(|| {
            ProviderError::configuration("azure", "base_url (or endpoint) is required")
        })?;

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "azure".to_string();

    if let Some(api_version) = config_str(config, "api_version") {
        oai_config.base.api_version = Some(api_version.to_string());
    }
    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_bedrock_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "bedrock")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .unwrap_or("https://bedrock-runtime.us-east-1.amazonaws.com");

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "bedrock".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_vertex_ai_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "vertex_ai")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .ok_or_else(|| ProviderError::configuration("vertex_ai", "base_url is required"))?;

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "vertex_ai".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_replicate_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "replicate")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .unwrap_or("https://api.replicate.com/v1");

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "replicate".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_github_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "github")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .unwrap_or("https://models.inference.ai.azure.com");

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "github".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
}

pub(super) fn build_github_copilot_config_from_factory(
    config: &serde_json::Value,
) -> Result<openai_like::OpenAILikeConfig, ProviderError> {
    let api_key = macros::require_config_str(config, "api_key", "github_copilot")?;
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .unwrap_or("https://api.githubcopilot.com");

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "github_copilot".to_string();

    if let Some(timeout) = config_u64(config, "timeout") {
        oai_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        oai_config.base.max_retries = max_retries;
    }
    merge_string_headers(&mut oai_config.base.headers, config, "headers");
    merge_string_headers(&mut oai_config.custom_headers, config, "custom_headers");

    Ok(oai_config)
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
