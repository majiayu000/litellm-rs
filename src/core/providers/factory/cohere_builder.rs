//! Cohere provider config builder.

use super::builder::{config_str, config_u32, config_u64, env_str_any};
use crate::core::providers::{cohere, unified_provider::ProviderError};
use crate::core::traits::provider::ProviderConfig as _;

pub(super) fn build_cohere_config_from_factory(
    config: &serde_json::Value,
) -> Result<cohere::CohereConfig, ProviderError> {
    let api_key = config_str(config, "api_key")
        .map(str::to_string)
        .or_else(|| env_str_any(&["COHERE_API_KEY"]))
        .ok_or_else(|| {
            ProviderError::configuration("cohere", "api_key or COHERE_API_KEY is required")
        })?;

    let mut cohere_config = cohere::CohereConfig::new(api_key);

    if let Some(api_base) =
        config_str(config, "base_url").or_else(|| config_str(config, "api_base"))
    {
        cohere_config.api_base = api_base.to_string();
    }
    if let Some(api_version) = config_str(config, "api_version") {
        cohere_config.api_version = match api_version.trim().to_ascii_lowercase().as_str() {
            "v1" | "1" => cohere::CohereApiVersion::V1,
            "v2" | "2" => cohere::CohereApiVersion::V2,
            _ => {
                return Err(ProviderError::configuration(
                    "cohere",
                    "api_version must be v1 or v2",
                ));
            }
        };
    }
    if let Some(timeout) =
        config_u64(config, "timeout_seconds").or_else(|| config_u64(config, "timeout"))
    {
        cohere_config.timeout_seconds = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        cohere_config.max_retries = max_retries;
    }
    if let Some(input_type) = config_str(config, "default_embedding_input_type") {
        cohere_config.default_embedding_input_type = input_type.to_string();
    }

    cohere_config
        .validate()
        .map_err(|err| ProviderError::configuration("cohere", err))?;
    Ok(cohere_config)
}
