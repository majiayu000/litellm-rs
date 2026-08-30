use serde_json::Value;

use crate::core::providers::ProviderError;
use crate::core::providers::base::{BaseConfig, BaseHttpClient, HeaderPair, header_owned};

pub(super) fn native_audio_headers(
    client: &BaseHttpClient,
    auth_header: &'static str,
    auth_value: String,
) -> Vec<HeaderPair> {
    let mut headers = Vec::with_capacity(client.config().headers.len() + 1);
    for (key, value) in &client.config().headers {
        if key.eq_ignore_ascii_case(auth_header) || key.eq_ignore_ascii_case("content-type") {
            continue;
        }
        headers.push(header_owned(key.clone(), value.clone()));
    }
    headers.push(header_owned(auth_header.to_string(), auth_value));
    headers
}

pub(crate) fn native_audio_base_config(
    config: &Value,
    provider: &'static str,
) -> Result<BaseConfig, ProviderError> {
    let mut base = BaseConfig::from_env(provider);
    let retry_env_key = format!("{}_MAX_RETRIES", provider.to_ascii_uppercase());
    if std::env::var_os(retry_env_key).is_some() && base.max_retries != 0 {
        return Err(unsupported_retries(provider));
    }
    if config
        .get("max_retries")
        .is_some_and(|value| value.as_u64() != Some(u64::from(BaseConfig::default().max_retries)))
    {
        return Err(unsupported_retries(provider));
    }
    base.max_retries = 0;
    base.api_key = config
        .get("api_key")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(base.api_key);
    if let Some(api_base) = config
        .get("base_url")
        .or_else(|| config.get("api_base"))
        .and_then(Value::as_str)
    {
        base.api_base = Some(api_base.trim_end_matches('/').to_string());
    }
    if let Some(timeout) = config.get("timeout").and_then(Value::as_u64) {
        base.timeout = timeout;
    }
    if let Some(access) = config.get("endpoint_access") {
        base.endpoint_access = serde_json::from_value(access.clone()).map_err(|error| {
            ProviderError::configuration(provider, format!("invalid endpoint_access: {error}"))
        })?;
    }
    if let Some(headers) = config.get("headers") {
        base.headers = serde_json::from_value(headers.clone()).map_err(|error| {
            ProviderError::configuration(provider, format!("invalid headers: {error}"))
        })?;
    }
    Ok(base)
}

fn unsupported_retries(provider: &'static str) -> ProviderError {
    ProviderError::configuration(
        provider,
        "max_retries is unsupported for native audio providers",
    )
}
