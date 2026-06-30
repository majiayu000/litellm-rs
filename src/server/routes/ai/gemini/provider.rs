use bytes::Bytes;
use reqwest::{
    Client, Url,
    header::{CONTENT_TYPE, HeaderName, HeaderValue, RETRY_AFTER},
};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

use crate::config::models::provider::ProviderConfig;
use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::{Provider, ProviderError};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::{validate_api_version, validate_method, validate_model_segment};
use uuid::Uuid;

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

static GEMINI_HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

#[derive(Debug, Clone)]
pub(super) struct GeminiRouteProvider {
    pub(super) provider_name: String,
    pub(super) pricing_provider: String,
    pub(super) model: String,
    api_key: String,
    base_url: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    timeout: Duration,
}

#[cfg(test)]
pub(super) fn test_gemini_route_provider(
    provider_name: impl Into<String>,
    pricing_provider: impl Into<String>,
    model: impl Into<String>,
) -> GeminiRouteProvider {
    GeminiRouteProvider {
        provider_name: provider_name.into(),
        pricing_provider: pricing_provider.into(),
        model: model.into(),
        api_key: String::new(),
        base_url: String::new(),
        headers: Vec::new(),
        timeout: Duration::from_secs(1),
    }
}

pub(super) fn ensure_gemini_provider_candidate_configured(
    providers: &[ProviderConfig],
    requested_model: &str,
) -> Result<(), GatewayError> {
    let candidates = gemini_candidate_configs(providers);
    if candidates.is_empty() {
        return Err(missing_gemini_provider_error(requested_model));
    }

    if candidates
        .iter()
        .any(|provider| gemini_provider_supports_requested_model(provider, requested_model))
    {
        Ok(())
    } else {
        Err(missing_gemini_provider_error(requested_model))
    }
}

pub(super) fn gemini_router_models(
    providers: &[ProviderConfig],
    requested_model: &str,
) -> Vec<String> {
    let mut router_models = Vec::new();

    for provider in gemini_candidate_configs(providers) {
        if !gemini_provider_supports_requested_model(provider, requested_model) {
            continue;
        }

        if provider.models.is_empty() {
            if gemini_provider_uses_registry_models(provider) {
                push_unique_gemini_router_model(&mut router_models, requested_model);
            } else {
                push_unique_gemini_router_model(&mut router_models, &provider.name);
            }
            continue;
        }

        if provider.models.iter().any(|model| model == requested_model) {
            push_unique_gemini_router_model(&mut router_models, requested_model);
        }
    }

    if router_models.is_empty() {
        router_models.push(requested_model.to_string());
    }

    router_models
}

fn push_unique_gemini_router_model(router_models: &mut Vec<String>, model: &str) {
    if !router_models.iter().any(|existing| existing == model) {
        router_models.push(model.to_string());
    }
}

pub(super) fn selected_gemini_provider(
    providers: &[ProviderConfig],
    selected_deployment_id: &str,
    selected_provider: &Provider,
    selected_model: &str,
    requested_model: &str,
) -> Result<GeminiRouteProvider, ProviderError> {
    let selected_provider_name = selected_provider.name();
    let candidates = gemini_candidate_configs(providers);
    let supported_candidates = candidates
        .iter()
        .copied()
        .filter(|provider| gemini_provider_supports_requested_model(provider, requested_model))
        .collect::<Vec<_>>();
    let matching = supported_candidates
        .iter()
        .copied()
        .find(|provider| {
            selected_deployment_matches_gemini_config(
                selected_deployment_id,
                provider,
                selected_model,
            )
        })
        .or_else(|| {
            supported_candidates.iter().copied().find(|provider| {
                provider.name == selected_provider_name
                    || provider
                        .settings
                        .get("provider_name")
                        .and_then(|value| value.as_str())
                        == Some(selected_provider_name)
                    || (provider.models.is_empty() && provider.name == selected_model)
            })
        })
        .ok_or_else(|| {
            ProviderError::configuration(
                "gemini_proxy",
                format!(
                    "selected Gemini provider '{selected_provider_name}' for model '{selected_model}' has no matching gateway provider config"
                ),
            )
        })?;

    gemini_route_provider_from_config(matching, requested_model)
        .map_err(gemini_gateway_error_to_provider_error)
}

fn selected_deployment_matches_gemini_config(
    selected_deployment_id: &str,
    provider: &ProviderConfig,
    selected_model: &str,
) -> bool {
    selected_deployment_id == provider.name
        || selected_deployment_id
            .strip_prefix(provider.name.as_str())
            .and_then(|suffix| suffix.strip_prefix('-'))
            == Some(selected_model)
}

fn gemini_route_provider_from_config(
    provider: &ProviderConfig,
    requested_model: &str,
) -> Result<GeminiRouteProvider, GatewayError> {
    if provider.api_key.trim().is_empty() {
        return Err(GatewayError::Config(format!(
            "Gemini provider '{}' is missing api_key",
            provider.name
        )));
    }

    Ok(GeminiRouteProvider {
        provider_name: provider.name.clone(),
        pricing_provider: "gemini".to_string(),
        model: requested_model.to_string(),
        api_key: provider.api_key.clone(),
        base_url: gemini_base_url(provider),
        headers: gemini_provider_headers(provider)?,
        timeout: Duration::from_secs(provider.timeout),
    })
}

fn gemini_candidate_configs(providers: &[ProviderConfig]) -> Vec<&ProviderConfig> {
    providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter(|provider| is_gemini_provider(provider))
        .collect()
}

fn gemini_provider_supports_requested_model(
    provider: &ProviderConfig,
    requested_model: &str,
) -> bool {
    provider.models.is_empty() || provider.models.iter().any(|model| model == requested_model)
}

fn gemini_provider_uses_registry_models(provider: &ProviderConfig) -> bool {
    matches!(
        super::super::provider_config::normalize_provider_selector(&provider.provider_type)
            .as_str(),
        "gemini" | "googlegemini" | "googleai"
    )
}

fn is_gemini_provider(provider: &ProviderConfig) -> bool {
    let provider_type =
        super::super::provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = super::super::provider_config::normalize_provider_selector(&provider.name);

    matches!(
        provider_type.as_str(),
        "gemini" | "googleai" | "googleaistudio"
    ) || matches!(
        provider_name.as_str(),
        "gemini" | "googleai" | "googleaistudio"
    )
}

fn gemini_base_url(provider: &ProviderConfig) -> String {
    provider
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|base_url| !base_url.is_empty())
        .unwrap_or(GEMINI_BASE_URL)
        .trim_end_matches('/')
        .to_string()
}

fn gemini_provider_headers(
    provider: &ProviderConfig,
) -> Result<Vec<(HeaderName, HeaderValue)>, GatewayError> {
    let mut headers = Vec::new();
    super::super::provider_config::append_string_header_map(provider, "headers", |key, value| {
        push_gemini_header(&mut headers, key, value)
    })?;
    super::super::provider_config::append_string_header_map(
        provider,
        "custom_headers",
        |key, value| push_gemini_header(&mut headers, key, value),
    )?;
    Ok(headers)
}

fn push_gemini_header(
    headers: &mut Vec<(HeaderName, HeaderValue)>,
    name: impl AsRef<str>,
    value: impl AsRef<str>,
) -> Result<(), GatewayError> {
    let name = HeaderName::from_bytes(name.as_ref().as_bytes()).map_err(|error| {
        GatewayError::Config(format!("Invalid Gemini provider header: {error}"))
    })?;
    let value = HeaderValue::from_str(value.as_ref()).map_err(|error| {
        GatewayError::Config(format!("Invalid Gemini provider header value: {error}"))
    })?;
    headers.push((name, value));
    Ok(())
}

fn gemini_url(
    provider: &GeminiRouteProvider,
    api_version: &str,
    method: &'static str,
    stream: bool,
) -> Result<Url, GatewayError> {
    validate_api_version(api_version)?;
    validate_model_segment(&provider.model)?;
    validate_method(method)?;

    let mut url = Url::parse(&format!(
        "{}/{}/models/{}:{}",
        provider.base_url.trim_end_matches('/'),
        api_version,
        provider.model,
        method
    ))
    .map_err(|error| GatewayError::Config(format!("Invalid Gemini base URL: {error}")))?;

    {
        let mut query = url.query_pairs_mut();
        if stream {
            query.append_pair("alt", "sse");
        }
        query.append_pair("key", &provider.api_key);
    }

    Ok(url)
}

pub(super) async fn send_gemini_request(
    state: &AppState,
    provider: &GeminiRouteProvider,
    api_version: &str,
    method: &'static str,
    stream: bool,
    request: &Value,
    api_key_budget_id: Option<Uuid>,
) -> Result<
    (
        Option<UnifiedBudgetReservation>,
        Option<BudgetReservation>,
        reqwest::Response,
    ),
    ProviderError,
> {
    super::super::spend::ensure_budget_available(
        &state.budget_limits,
        &provider.provider_name,
        &provider.model,
    )?;
    let mut budget_reservation = super::spend::reserve_gemini_budget(state, provider, request)
        .map_err(gemini_gateway_error_to_provider_error)?;
    let mut key_budget_reservation = super::super::spend::reserve_api_key_budget_for_reservation(
        &state.budget_manager,
        api_key_budget_id,
        budget_reservation.as_ref(),
    )?;
    let url = gemini_url(provider, api_version, method, stream)
        .map_err(gemini_gateway_error_to_provider_error)?;
    let response_result = apply_gemini_headers(gemini_http_client().post(url), provider)
        .header(CONTENT_TYPE, "application/json")
        .json(request)
        .timeout(provider.timeout)
        .send()
        .await;
    let response = match response_result {
        Ok(response) => response,
        Err(error) => {
            if let Some(reservation) = budget_reservation.take() {
                reservation.cancel();
            }
            if let Some(reservation) = key_budget_reservation.take() {
                reservation.cancel();
            }
            return Err(gemini_gateway_error_to_provider_error(gemini_http_error(
                error,
            )));
        }
    };
    let response = match gemini_response_or_provider_error(response, provider).await {
        Ok(response) => response,
        Err(error) => {
            if let Some(reservation) = budget_reservation.take() {
                reservation.cancel();
            }
            if let Some(reservation) = key_budget_reservation.take() {
                reservation.cancel();
            }
            return Err(error);
        }
    };
    Ok((budget_reservation, key_budget_reservation, response))
}

async fn gemini_response_or_provider_error(
    response: reqwest::Response,
    provider: &GeminiRouteProvider,
) -> Result<reqwest::Response, ProviderError> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let body = response
        .bytes()
        .await
        .map_err(|error| gemini_gateway_error_to_provider_error(gemini_http_error(error)))?;
    let body = sanitize_gemini_error_body(body, provider);
    let body_text = String::from_utf8_lossy(&body).to_string();
    Err(gemini_upstream_status_provider_error(
        status,
        body_text,
        retry_after,
    ))
}

fn gemini_upstream_status_provider_error(
    status: u16,
    body: String,
    retry_after: Option<u64>,
) -> ProviderError {
    let message = if body.trim().is_empty() {
        format!("Gemini upstream returned HTTP {status}")
    } else {
        format!("Gemini upstream returned HTTP {status}: {body}")
    };
    if status == 429 {
        ProviderError::rate_limit_with_retry(
            "gemini_proxy",
            message,
            retry_after.or_else(|| parse_retry_after_from_body(&body)),
        )
    } else {
        ProviderError::api_error("gemini_proxy", status, message)
    }
}

pub(super) fn gemini_http_error(error: reqwest::Error) -> GatewayError {
    if error.is_timeout() {
        GatewayError::timeout("Gemini upstream request timed out")
    } else {
        GatewayError::network("Gemini upstream request failed")
    }
}

pub(super) fn missing_gemini_provider_error(requested_model: &str) -> GatewayError {
    GatewayError::Config(format!(
        "Gemini SDK route provider for model '{requested_model}' is not configured"
    ))
}

pub(super) fn gemini_gateway_error_to_provider_error(error: GatewayError) -> ProviderError {
    match error {
        GatewayError::Provider(error) => error,
        GatewayError::Validation(message) | GatewayError::BadRequest(message) => {
            ProviderError::invalid_request("gemini_proxy", message)
        }
        GatewayError::Config(message) => ProviderError::configuration("gemini_proxy", message),
        GatewayError::Auth(message) => ProviderError::authentication("gemini_proxy", message),
        GatewayError::Forbidden(message) => ProviderError::api_error("gemini_proxy", 403, message),
        GatewayError::Timeout(message) => ProviderError::timeout("gemini_proxy", message),
        GatewayError::RateLimit {
            message,
            retry_after,
            ..
        } => ProviderError::rate_limit_with_retry("gemini_proxy", message, retry_after),
        GatewayError::HttpClient(error) => {
            ProviderError::network("gemini_proxy", error.to_string())
        }
        GatewayError::Network(message) => ProviderError::network("gemini_proxy", message),
        GatewayError::Unavailable(message) => {
            ProviderError::provider_unavailable("gemini_proxy", message)
        }
        other => ProviderError::api_error("gemini_proxy", 500, other.to_string()),
    }
}

fn gemini_http_client() -> &'static Client {
    GEMINI_HTTP_CLIENT.get_or_init(Client::new)
}

fn apply_gemini_headers(
    mut request: reqwest::RequestBuilder,
    provider: &GeminiRouteProvider,
) -> reqwest::RequestBuilder {
    for (name, value) in &provider.headers {
        request = request.header(name.clone(), value.clone());
    }
    request
}

fn sanitize_gemini_error_body(body: Bytes, provider: &GeminiRouteProvider) -> Bytes {
    if provider.api_key.is_empty() || body.is_empty() {
        return body;
    }

    let text = String::from_utf8_lossy(&body);
    let encoded_key: String =
        url::form_urlencoded::byte_serialize(provider.api_key.as_bytes()).collect();
    if !text.contains(&provider.api_key) && !text.contains(&encoded_key) {
        return body;
    }

    let mut sanitized = text.replace(&provider.api_key, "[REDACTED]");
    if encoded_key != provider.api_key {
        sanitized = sanitized.replace(&encoded_key, "[REDACTED]");
    }
    Bytes::from(sanitized)
}
