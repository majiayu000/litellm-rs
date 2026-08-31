use reqwest::header::{HeaderName, HeaderValue};

use crate::core::providers::base::BaseConfig;
use crate::core::providers::unified_provider::ProviderError;

pub(crate) enum MediaCredential {
    Bearer,
    Raw,
}

pub(crate) fn validate_media_config(
    provider: &'static str,
    config: &mut BaseConfig,
    credential: MediaCredential,
    replaced_headers: &[&str],
) -> Result<(), ProviderError> {
    config.api_key = config.api_key.take().and_then(|api_key| {
        let api_key = api_key.trim();
        (!api_key.is_empty()).then(|| api_key.to_string())
    });

    if let Some(api_base) = config.api_base.as_deref() {
        let parsed = url::Url::parse(api_base).map_err(|error| {
            ProviderError::configuration(provider, format!("invalid API base URL: {error}"))
        })?;
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(ProviderError::configuration(
                provider,
                "API base URL must not contain userinfo, query, or fragment",
            ));
        }
    }

    for (name, value) in &config.headers {
        if replaced_headers
            .iter()
            .any(|replaced| name.eq_ignore_ascii_case(replaced))
        {
            continue;
        }
        HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
            ProviderError::configuration(provider, format!("invalid header name: {error}"))
        })?;
        HeaderValue::from_str(value).map_err(|error| {
            ProviderError::configuration(provider, format!("invalid header value: {error}"))
        })?;
    }

    if let Some(api_key) = config.api_key.as_deref() {
        let value = match credential {
            MediaCredential::Bearer => format!("Bearer {api_key}"),
            MediaCredential::Raw => api_key.to_string(),
        };
        HeaderValue::from_str(&value).map_err(|error| {
            ProviderError::configuration(provider, format!("invalid API credential: {error}"))
        })?;
    }
    Ok(())
}
