//! Gemini provider config builder.

#![cfg(feature = "providers-extended")]

use super::builder::{
    config_bool, config_str, config_str_any, config_u32, config_u64, env_str_any,
    merge_string_headers,
};
use crate::core::providers::{gemini, unified_provider::ProviderError};
use crate::core::traits::provider::ProviderConfig as _;

pub(super) fn build_gemini_config_from_factory(
    config: &serde_json::Value,
) -> Result<gemini::GeminiConfig, ProviderError> {
    if config_bool(config, "use_vertex_ai").unwrap_or(false)
        || config_str_any(config, &["project_id", "location", "service_account_json"]).is_some()
    {
        return Err(ProviderError::invalid_request(
            "gemini",
            "Gemini provider selector uses Google AI Studio API-key auth; use vertex_ai for Vertex AI project/location credentials",
        ));
    }

    let api_key = config_str_any(config, &["api_key", "google_api_key", "gemini_api_key"])
        .map(str::to_string)
        .or_else(|| env_str_any(&["GEMINI_API_KEY", "GOOGLE_API_KEY"]))
        .ok_or_else(|| {
            ProviderError::configuration("gemini", "api_key or GEMINI_API_KEY is required")
        })?;

    let mut gemini_config = gemini::GeminiConfig::new_google_ai(api_key);

    if let Some(base_url) =
        config_str(config, "base_url").or_else(|| config_str(config, "api_base"))
    {
        gemini_config.base_url = base_url.to_string();
    }
    if let Some(api_version) = config_str(config, "api_version") {
        gemini_config.api_version = api_version.to_string();
    }
    if let Some(timeout) = config_u64(config, "timeout") {
        gemini_config.request_timeout = timeout;
    }
    if let Some(connect_timeout) = config_u64(config, "connect_timeout") {
        gemini_config.connect_timeout = connect_timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        gemini_config.max_retries = max_retries;
    }
    if let Some(retry_delay_ms) = config_u64(config, "retry_delay_ms") {
        gemini_config.retry_delay_ms = retry_delay_ms;
    }
    if let Some(enable_caching) = config_bool(config, "enable_caching") {
        gemini_config.enable_caching = enable_caching;
    }
    if let Some(cache_ttl_seconds) = config_u64(config, "cache_ttl_seconds") {
        gemini_config.cache_ttl_seconds = cache_ttl_seconds;
    }
    if let Some(enable_search_grounding) = config_bool(config, "enable_search_grounding") {
        gemini_config.enable_search_grounding = enable_search_grounding;
    }
    if let Some(proxy_url) = config_str(config, "proxy_url").or_else(|| config_str(config, "proxy"))
    {
        gemini_config.proxy_url = Some(proxy_url.to_string());
    }
    if let Some(debug) = config_bool(config, "debug") {
        gemini_config.debug = debug;
    }

    merge_string_headers(&mut gemini_config.custom_headers, config, "headers");
    merge_string_headers(&mut gemini_config.custom_headers, config, "custom_headers");

    gemini_config
        .validate()
        .map_err(|err| ProviderError::configuration("gemini", err))?;
    Ok(gemini_config)
}
