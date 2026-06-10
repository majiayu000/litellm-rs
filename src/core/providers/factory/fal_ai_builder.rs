//! Fal AI provider config builder.

use super::builder::{config_bool, config_str, config_u32, config_u64, env_str_any};
use crate::core::providers::{fal_ai, unified_provider::ProviderError};
use crate::core::traits::provider::ProviderConfig as _;

pub(super) fn build_fal_ai_config_from_factory(
    config: &serde_json::Value,
) -> Result<fal_ai::FalAIConfig, ProviderError> {
    let api_key = config_str(config, "api_key")
        .map(str::to_string)
        .or_else(|| env_str_any(&["FAL_AI_API_KEY"]))
        .ok_or_else(|| {
            ProviderError::configuration("fal_ai", "api_key or FAL_AI_API_KEY is required")
        })?;

    let mut fal_config = fal_ai::FalAIConfig::with_api_key(api_key);

    if let Some(api_base) =
        config_str(config, "base_url").or_else(|| config_str(config, "api_base"))
    {
        fal_config.base.api_base = Some(api_base.to_string());
    }
    if let Some(timeout) = config_u64(config, "timeout") {
        fal_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        fal_config.base.max_retries = max_retries;
    }
    if let Some(output_format) = config_str(config, "output_format") {
        fal_config.output_format = output_format.trim().to_ascii_lowercase();
    }
    if let Some(sync_mode) = config_bool(config, "sync_mode") {
        fal_config.sync_mode = sync_mode;
    }

    fal_config
        .validate()
        .map_err(|err| ProviderError::configuration("fal_ai", err))?;
    Ok(fal_config)
}
