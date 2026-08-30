use crate::core::providers::{ProviderError, voyage::VoyageProvider};

use super::builder::{config_endpoint_access, config_str};

pub(super) fn build_voyage_provider(
    config: &serde_json::Value,
) -> Result<VoyageProvider, ProviderError> {
    let api_key = config_str(config, "api_key")
        .ok_or_else(|| ProviderError::configuration("voyage", "api_key is required"))?
        .to_string();
    let api_base = config_str(config, "api_base").or_else(|| config_str(config, "base_url"));
    let timeout = config
        .get("timeout")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(60);
    let max_retries = config
        .get("max_retries")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(3);
    let models = config
        .get("models")
        .and_then(serde_json::Value::as_array)
        .map(|models| {
            models
                .iter()
                .map(|model| {
                    model.as_str().map(str::to_owned).ok_or_else(|| {
                        ProviderError::configuration("voyage", "models entries must be strings")
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    VoyageProvider::new(
        api_key,
        api_base,
        config_endpoint_access(config, "voyage")?,
        timeout,
        max_retries,
        &models,
    )
}
