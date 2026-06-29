//! Shared helpers for OpenAI-compatible provider route configuration.

use crate::config::models::provider::ProviderConfig;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::HttpResponse;
use actix_web::http::{StatusCode, header};
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::{Client, RequestBuilder};
use std::sync::OnceLock;

static PROXY_HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

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

pub(super) fn proxy_http_client() -> &'static Client {
    PROXY_HTTP_CLIENT.get_or_init(Client::new)
}

pub(super) async fn proxy_response_to_http_response(
    response: reqwest::Response,
) -> Result<HttpResponse, GatewayError> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|error| GatewayError::internal(format!("Invalid upstream status: {error}")))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let body = response.bytes().await?;

    Ok(HttpResponse::build(status)
        .insert_header((header::CONTENT_TYPE, content_type))
        .body(body))
}

pub(super) fn apply_proxy_headers(
    mut request: RequestBuilder,
    headers: &[(HeaderName, HeaderValue)],
) -> RequestBuilder {
    for (name, value) in headers {
        request = request.header(name.clone(), value.clone());
    }
    request
}

pub(super) fn push_proxy_header(
    headers: &mut Vec<(HeaderName, HeaderValue)>,
    context: &str,
    name: impl AsRef<str>,
    value: impl AsRef<str>,
) -> Result<(), GatewayError> {
    let name = HeaderName::from_bytes(name.as_ref().as_bytes())
        .map_err(|error| GatewayError::Config(format!("Invalid {context} header: {error}")))?;
    let value = HeaderValue::from_str(value.as_ref()).map_err(|error| {
        GatewayError::Config(format!("Invalid {context} header value: {error}"))
    })?;
    headers.push((name, value));
    Ok(())
}
