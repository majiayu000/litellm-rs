//! Batch API route handlers.
//!
//! These endpoints proxy OpenAI-compatible batch lifecycle requests to a
//! configured OpenAI or OpenAI-compatible provider.

use crate::config::models::provider::ProviderConfig;
use crate::core::providers::ProviderError;
use crate::core::router::execution::is_retryable_error;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpResponse, Result as ActixResult, http::StatusCode, http::header, web};
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::{Client, RequestBuilder, Url};
use serde::Deserialize;
use serde_json::Value;
use std::future::Future;
use std::sync::OnceLock;
use tracing::error;

use super::openai_errors;
use super::provider_config;

const OPENAI_BATCH_BASE_URL: &str = "https://api.openai.com/v1";
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

#[derive(Debug, Deserialize)]
pub struct ListBatchesQuery {
    after: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
struct BatchProxyProvider {
    provider_name: String,
    api_key: String,
    base_url: String,
    headers: Vec<(HeaderName, HeaderValue)>,
}

/// Create a batch request.
pub async fn create_batch(
    state: web::Data<AppState>,
    request: web::Json<Value>,
) -> ActixResult<HttpResponse> {
    match proxy_batch_create(state.get_ref(), request.into_inner()).await {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Batch create error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

/// List batches.
pub async fn list_batches(
    state: web::Data<AppState>,
    query: web::Query<ListBatchesQuery>,
) -> ActixResult<HttpResponse> {
    match proxy_batch_list(state.get_ref(), query.into_inner()).await {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Batch list error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

/// Get a batch by ID.
pub async fn get_batch(
    state: web::Data<AppState>,
    batch_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    match proxy_batch_get(state.get_ref(), &batch_id).await {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Batch retrieve error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

/// Cancel a batch by ID.
pub async fn cancel_batch(
    state: web::Data<AppState>,
    batch_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    match proxy_batch_cancel(state.get_ref(), &batch_id).await {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Batch cancel error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

async fn proxy_batch_create(
    state: &AppState,
    request: Value,
) -> Result<HttpResponse, GatewayError> {
    validate_create_batch_request(&request)?;
    execute_batch_proxy_request(state, move |provider| {
        let request = request.clone();
        async move {
            let url = batch_url(&provider, None, None)?;
            apply_provider_headers(http_client().post(url), &provider)
                .json(&request)
                .send()
                .await
                .map_err(GatewayError::from)
        }
    })
    .await
}

async fn proxy_batch_list(
    state: &AppState,
    query: ListBatchesQuery,
) -> Result<HttpResponse, GatewayError> {
    execute_batch_proxy_request(state, move |provider| {
        let query = ListBatchesQuery {
            after: query.after.clone(),
            limit: query.limit,
        };
        async move {
            let mut url = batch_url(&provider, None, None)?;
            {
                let mut pairs = url.query_pairs_mut();
                if let Some(after) = query.after.filter(|after| !after.trim().is_empty()) {
                    pairs.append_pair("after", &after);
                }
                if let Some(limit) = query.limit {
                    pairs.append_pair("limit", &limit.to_string());
                }
            }

            apply_provider_headers(http_client().get(url), &provider)
                .send()
                .await
                .map_err(GatewayError::from)
        }
    })
    .await
}

async fn proxy_batch_get(state: &AppState, batch_id: &str) -> Result<HttpResponse, GatewayError> {
    let batch_id = batch_id.to_string();
    execute_batch_proxy_request(state, move |provider| {
        let batch_id = batch_id.clone();
        async move {
            let url = batch_url(&provider, Some(&batch_id), None)?;
            apply_provider_headers(http_client().get(url), &provider)
                .send()
                .await
                .map_err(GatewayError::from)
        }
    })
    .await
}

async fn proxy_batch_cancel(
    state: &AppState,
    batch_id: &str,
) -> Result<HttpResponse, GatewayError> {
    let batch_id = batch_id.to_string();
    execute_batch_proxy_request(state, move |provider| {
        let batch_id = batch_id.clone();
        async move {
            let url = batch_url(&provider, Some(&batch_id), Some("cancel"))?;
            apply_provider_headers(http_client().post(url), &provider)
                .send()
                .await
                .map_err(GatewayError::from)
        }
    })
    .await
}

async fn execute_batch_proxy_request<F, Fut>(
    state: &AppState,
    operation: F,
) -> Result<HttpResponse, GatewayError>
where
    F: Fn(BatchProxyProvider) -> Fut,
    Fut: Future<Output = Result<reqwest::Response, GatewayError>>,
{
    let config = state.config();
    let provider_configs = select_batch_proxy_provider_configs(config.gateway.providers.as_slice());
    if provider_configs.is_empty() {
        return Err(missing_batch_provider_error());
    }

    let provider_count = provider_configs.len();
    let mut last_error = None;
    for (index, provider_config) in provider_configs.into_iter().enumerate() {
        let is_last_provider = index + 1 == provider_count;
        let provider = batch_proxy_provider_from_config(provider_config)?;
        if let Err(error) =
            super::spend::ensure_budget_available(&state.budget_limits, &provider.provider_name, "")
                .map_err(GatewayError::Provider)
        {
            if !is_last_provider && is_retryable_batch_error(&error) {
                last_error = Some(error);
                continue;
            }
            return Err(error);
        }

        match operation(provider).await {
            Ok(response) if should_try_next_batch_provider(response.status()) => {
                if is_last_provider {
                    return response_to_http_response(response).await;
                }
                last_error = Some(batch_upstream_gateway_error(response).await);
            }
            Ok(response) => return response_to_http_response(response).await,
            Err(error) if !is_last_provider && is_retryable_batch_error(&error) => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(missing_batch_provider_error))
}

async fn response_to_http_response(
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

fn is_retryable_batch_error(error: &GatewayError) -> bool {
    match error {
        GatewayError::Provider(error) => is_retryable_error(error),
        GatewayError::HttpClient(_)
        | GatewayError::Network(_)
        | GatewayError::Timeout(_)
        | GatewayError::Unavailable(_)
        | GatewayError::RateLimit { .. } => true,
        _ => false,
    }
}

fn should_try_next_batch_provider(status: reqwest::StatusCode) -> bool {
    matches!(
        status.as_u16(),
        408 | 429 | 500 | 502 | 503 | 504 | 507 | 529
    )
}

async fn batch_upstream_gateway_error(response: reqwest::Response) -> GatewayError {
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .unwrap_or_else(|error| format!("failed to read batch upstream error body: {error}"));
    let message = if body.trim().is_empty() {
        format!("Batch upstream returned HTTP {status}")
    } else {
        format!("Batch upstream returned HTTP {status}: {body}")
    };

    let provider_error = match status {
        408 | 504 => ProviderError::timeout("batch_proxy", message),
        429 => ProviderError::rate_limit_with_retry("batch_proxy", message, None),
        502 | 503 => ProviderError::provider_unavailable("batch_proxy", message),
        _ => ProviderError::api_error("batch_proxy", status, message),
    };
    GatewayError::Provider(provider_error)
}

#[cfg(test)]
fn select_batch_proxy_provider(
    providers: &[ProviderConfig],
) -> Result<Option<BatchProxyProvider>, GatewayError> {
    select_batch_proxy_provider_configs(providers)
        .into_iter()
        .next()
        .map(batch_proxy_provider_from_config)
        .transpose()
}

fn select_batch_proxy_provider_configs(providers: &[ProviderConfig]) -> Vec<&ProviderConfig> {
    providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter(|provider| is_openai_batch_provider(provider))
        .collect()
}

fn batch_proxy_provider_from_config(
    provider: &ProviderConfig,
) -> Result<BatchProxyProvider, GatewayError> {
    if provider.api_key.trim().is_empty() {
        return Err(GatewayError::Config(format!(
            "Batch provider '{}' is missing api_key",
            provider.name
        )));
    }

    Ok(BatchProxyProvider {
        provider_name: provider.name.clone(),
        api_key: provider.api_key.clone(),
        base_url: batch_base_url(provider)?,
        headers: batch_provider_headers(provider)?,
    })
}

fn batch_base_url(provider: &ProviderConfig) -> Result<String, GatewayError> {
    if let Some(base_url) = provider.base_url.as_deref() {
        let trimmed = base_url.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    if provider_config::normalize_provider_selector(&provider.provider_type) == "openai"
        || provider_config::normalize_provider_selector(&provider.name) == "openai"
    {
        return Ok(OPENAI_BATCH_BASE_URL.to_string());
    }

    Err(GatewayError::Config(format!(
        "Batch provider '{}' is missing base_url",
        provider.name
    )))
}

fn batch_url(
    provider: &BatchProxyProvider,
    batch_id: Option<&str>,
    action: Option<&str>,
) -> Result<Url, GatewayError> {
    if let Some(batch_id) = batch_id {
        validate_batch_id(batch_id)?;
    }
    if let Some(action) = action
        && action != "cancel"
    {
        return Err(GatewayError::validation("Unsupported batch action"));
    }

    let mut url = format!("{}/batches", provider.base_url);
    if let Some(batch_id) = batch_id {
        url.push('/');
        url.push_str(batch_id);
    }
    if let Some(action) = action {
        url.push('/');
        url.push_str(action);
    }

    Url::parse(&url).map_err(|error| GatewayError::Config(format!("Invalid batch URL: {error}")))
}

fn missing_batch_provider_error() -> GatewayError {
    GatewayError::BadRequest(
        "Batch API requires an enabled openai or openai_compatible provider".to_string(),
    )
}

fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(Client::new)
}

fn apply_provider_headers(
    mut request: RequestBuilder,
    provider: &BatchProxyProvider,
) -> RequestBuilder {
    for (name, value) in &provider.headers {
        request = request.header(name.clone(), value.clone());
    }
    request
}

fn batch_provider_headers(
    provider: &ProviderConfig,
) -> Result<Vec<(HeaderName, HeaderValue)>, GatewayError> {
    let mut headers = Vec::new();
    push_header(
        &mut headers,
        "Authorization",
        format!("Bearer {}", provider.api_key),
    )?;
    if let Some(organization) = provider.organization.as_deref() {
        push_header(&mut headers, "OpenAI-Organization", organization)?;
    }
    if let Some(project) = provider.project.as_deref() {
        push_header(&mut headers, "OpenAI-Project", project)?;
    }
    provider_config::append_string_header_map(provider, "headers", |key, value| {
        push_header(&mut headers, key, value)
    })?;
    provider_config::append_string_header_map(provider, "custom_headers", |key, value| {
        push_header(&mut headers, key, value)
    })?;
    Ok(headers)
}

fn push_header(
    headers: &mut Vec<(HeaderName, HeaderValue)>,
    name: impl AsRef<str>,
    value: impl AsRef<str>,
) -> Result<(), GatewayError> {
    let name = HeaderName::from_bytes(name.as_ref().as_bytes())
        .map_err(|error| GatewayError::Config(format!("Invalid batch provider header: {error}")))?;
    let value = HeaderValue::from_str(value.as_ref()).map_err(|error| {
        GatewayError::Config(format!("Invalid batch provider header value: {error}"))
    })?;
    headers.push((name, value));
    Ok(())
}

fn validate_create_batch_request(request: &Value) -> Result<(), GatewayError> {
    for field in ["input_file_id", "endpoint", "completion_window"] {
        if request
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(GatewayError::validation(format!("{field} is required")));
        }
    }
    Ok(())
}

fn validate_batch_id(batch_id: &str) -> Result<(), GatewayError> {
    if batch_id.trim().is_empty() {
        return Err(GatewayError::validation("batch_id is required"));
    }

    if batch_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Ok(());
    }

    Err(GatewayError::validation(
        "batch_id must be a single safe path segment",
    ))
}

fn is_openai_batch_provider(provider: &ProviderConfig) -> bool {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);

    provider_type == "openai"
        || provider_type == "openaicompatible"
        || provider_name == "openai"
        || provider_name == "openaicompatible"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn provider(name: &str, provider_type: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: provider_type.to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some("https://batch.example.test/v1/".to_string()),
            ..ProviderConfig::default()
        }
    }

    fn provider_with_headers(base_url: &str) -> ProviderConfig {
        let mut provider = provider("mock-openai-compatible", "openai_compatible");
        provider.base_url = Some(base_url.to_string());
        provider.organization = Some("org-test".to_string());
        provider.project = Some("proj-test".to_string());
        provider.settings = HashMap::from([
            (
                "headers".to_string(),
                json!({
                    "X-Base-Header": "base-value",
                    "X-Ignored-Non-String": false
                }),
            ),
            (
                "custom_headers".to_string(),
                json!({
                    "X-Custom-Header": "custom-value"
                }),
            ),
        ]);
        provider
    }

    #[test]
    fn selects_openai_compatible_batch_provider() {
        let selected = select_batch_proxy_provider(&[provider("primary", "openai_compatible")])
            .expect("provider selection should succeed")
            .expect("provider should select");

        assert_eq!(selected.base_url, "https://batch.example.test/v1");
        assert_eq!(selected.api_key, "sk-test");
        assert_eq!(selected.provider_name, "primary");
    }

    #[test]
    fn selected_provider_preserves_openai_and_custom_headers() {
        let provider = provider_with_headers("https://batch.example.test/v1");
        let selected = select_batch_proxy_provider(&[provider])
            .expect("provider selection should succeed")
            .expect("provider should select");
        let headers: HashMap<_, _> = selected
            .headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_string(), value.to_string()))
            })
            .collect();

        assert_eq!(headers["authorization"], "Bearer sk-test");
        assert_eq!(headers["openai-organization"], "org-test");
        assert_eq!(headers["openai-project"], "proj-test");
        assert_eq!(headers["x-base-header"], "base-value");
        assert_eq!(headers["x-custom-header"], "custom-value");
        assert!(!headers.contains_key("x-ignored-non-string"));
    }

    #[test]
    fn openai_provider_defaults_base_url() {
        let mut provider = provider("openai", "openai");
        provider.base_url = None;

        let selected = select_batch_proxy_provider(&[provider])
            .expect("provider selection should succeed")
            .expect("provider should select");

        assert_eq!(selected.base_url, OPENAI_BATCH_BASE_URL);
    }

    #[test]
    fn compatible_provider_requires_base_url() {
        let mut provider = provider("local", "openai_compatible");
        provider.base_url = None;

        let error = select_batch_proxy_provider(&[provider]).unwrap_err();

        assert!(error.to_string().contains("missing base_url"));
    }

    #[test]
    fn no_batch_provider_is_explicitly_unconfigured() {
        let selected = select_batch_proxy_provider(&[]);

        assert_eq!(selected.unwrap(), None);
        assert!(
            missing_batch_provider_error()
                .to_string()
                .contains("Batch API requires")
        );
    }

    #[test]
    fn builds_batch_urls() {
        let provider = BatchProxyProvider {
            provider_name: "openai".to_string(),
            api_key: "sk-test".to_string(),
            base_url: "https://batch.example.test/v1".to_string(),
            headers: Vec::new(),
        };

        assert_eq!(
            batch_url(&provider, None, None)
                .expect("url should build")
                .as_str(),
            "https://batch.example.test/v1/batches"
        );
        assert_eq!(
            batch_url(&provider, Some("batch_123"), Some("cancel"))
                .expect("url should build")
                .as_str(),
            "https://batch.example.test/v1/batches/batch_123/cancel"
        );
    }

    #[test]
    fn validates_create_batch_request_required_fields() {
        validate_create_batch_request(&serde_json::json!({
            "input_file_id": "file_123",
            "endpoint": "/v1/chat/completions",
            "completion_window": "24h"
        }))
        .expect("request should validate");

        let error = validate_create_batch_request(&serde_json::json!({
            "endpoint": "/v1/chat/completions",
            "completion_window": "24h"
        }))
        .unwrap_err();
        assert!(error.to_string().contains("input_file_id is required"));
    }

    #[test]
    fn rejects_unsafe_batch_id() {
        let error = validate_batch_id("../batch_123").unwrap_err();

        assert!(error.to_string().contains("safe path segment"));
    }
}
