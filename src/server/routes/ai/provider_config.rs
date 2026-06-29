//! Shared helpers for OpenAI-compatible provider route configuration.

use crate::config::models::provider::ProviderConfig;
use crate::utils::error::gateway_error::GatewayError;

pub(super) fn append_string_header_map(
    provider: &ProviderConfig,
    settings_key: &str,
    mut append: impl FnMut(&str, &str) -> Result<(), GatewayError>,
) -> Result<(), GatewayError> {
    let Some(header_map) = provider
        .settings
        .get(settings_key)
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(());
    };

    for (key, value) in header_map {
        if let Some(value) = value.as_str() {
            append(key, value)?;
        }
    }
    Ok(())
}

pub(super) fn normalize_provider_selector(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['_', '-'], "")
}
