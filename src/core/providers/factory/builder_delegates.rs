//! OpenAI-like delegate config builders
//!
//! Provider-specific builders that produce `OpenAILikeConfig` for providers
//! routed through the OpenAILike variant in the factory. Separated from
//! `builder.rs` to keep each file under the 800-line limit.

use super::builder::{config_str, config_u32, config_u64, merge_string_headers};
use super::super::unified_provider::ProviderError;
use super::super::{macros, openai_like};

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
        .or_else(|| config_str(config, "azure_endpoint"))
        .ok_or_else(|| {
            ProviderError::configuration(
                "azure",
                "base_url (or azure_endpoint) is required",
            )
        })?;

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(api_base, api_key);
    oai_config.provider_name = "azure".to_string();

    if let Some(api_version) = config_str(config, "api_version") {
        oai_config.base.api_version = Some(api_version.to_string());
    } else {
        oai_config.base.api_version = Some("2024-02-01".to_string());
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
    let aws_region = config_str(config, "aws_region").unwrap_or("us-east-1");
    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!(
                "https://bedrock-runtime.{}.amazonaws.com/model",
                aws_region
            )
        });

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(&api_base, api_key);
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
    let project_id = config_str(config, "project_id")
        .or_else(|| config_str(config, "project"))
        .unwrap_or("default");
    let location = config_str(config, "location").unwrap_or("us-central1");
    let api_version = config_str(config, "api_version").unwrap_or("v1");

    let api_base = config_str(config, "base_url")
        .or_else(|| config_str(config, "api_base"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!(
                "https://{}-aiplatform.googleapis.com/{}/projects/{}/locations/{}",
                location, api_version, project_id, location
            )
        });

    let mut oai_config = openai_like::OpenAILikeConfig::with_api_key(&api_base, api_key);
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
