//! Gemini SDK-compatible native generation routes.

use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, http::StatusCode, web};
use bytes::Bytes;
use futures::StreamExt;
use reqwest::{
    Client, Url,
    header::{CONTENT_TYPE, HeaderName, HeaderValue},
};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::error;

use crate::config::models::provider::ProviderConfig;
use crate::core::budget::UnifiedBudgetReservation;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::{openai_errors, provider_config};

mod spend;
use spend::{
    GeminiSpendState, extract_gemini_sse_usage, record_gemini_spend, reserve_gemini_budget,
    settle_gemini_stream_spend,
};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";
const GEMINI_V1: &str = "v1";
const GEMINI_V1BETA: &str = "v1beta";
const GEMINI_MAX_JSON_BYTES: usize = 16 * 1024 * 1024;

static GEMINI_HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

#[derive(Debug, Clone)]
struct GeminiRouteProvider {
    provider_name: String,
    pricing_provider: String,
    model: String,
    api_key: String,
    base_url: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    timeout: Duration,
}

macro_rules! gemini_route_handler {
    ($name:ident, $version:expr, $method:literal, $stream:expr) => {
        pub async fn $name(
            state: web::Data<AppState>,
            req: HttpRequest,
            model: web::Path<String>,
            request: web::Json<Value>,
        ) -> ActixResult<HttpResponse> {
            proxy_gemini_route(
                state.get_ref(),
                &req,
                $version,
                model.into_inner(),
                $method,
                $stream,
                request.into_inner(),
            )
            .await
        }
    };
}

gemini_route_handler!(
    gemini_generate_content_v1beta,
    GEMINI_V1BETA,
    "generateContent",
    false
);
gemini_route_handler!(
    gemini_stream_generate_content_v1beta,
    GEMINI_V1BETA,
    "streamGenerateContent",
    true
);
gemini_route_handler!(
    gemini_generate_content_v1,
    GEMINI_V1,
    "generateContent",
    false
);
gemini_route_handler!(
    gemini_stream_generate_content_v1,
    GEMINI_V1,
    "streamGenerateContent",
    true
);

async fn proxy_gemini_route(
    state: &AppState,
    req: &HttpRequest,
    api_version: &str,
    requested_model: String,
    method: &'static str,
    stream: bool,
    request: Value,
) -> ActixResult<HttpResponse> {
    match proxy_gemini_route_inner(
        state,
        req,
        api_version,
        requested_model,
        method,
        stream,
        request,
    )
    .await
    {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Gemini SDK route error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

async fn proxy_gemini_route_inner(
    state: &AppState,
    req: &HttpRequest,
    api_version: &str,
    requested_model: String,
    method: &'static str,
    stream: bool,
    request: Value,
) -> Result<HttpResponse, GatewayError> {
    let context = ensure_gemini_route_authorized(state, req)?;
    validate_api_version(api_version)?;
    validate_method(method)?;
    validate_model_segment(&requested_model)?;
    validate_gemini_request_size(&request)?;

    let provider = select_gemini_provider(state.config().providers(), &requested_model)?;
    super::spend::ensure_budget_available(
        &state.budget_limits,
        &provider.provider_name,
        &provider.model,
    )?;
    let budget_reservation = reserve_gemini_budget(state, &provider, &request)?;

    let url = gemini_url(&provider, api_version, method, stream)?;
    let response = apply_gemini_headers(gemini_http_client().post(url), &provider)
        .header(CONTENT_TYPE, "application/json")
        .json(&request)
        .timeout(provider.timeout)
        .send()
        .await
        .map_err(gemini_http_error)?;

    gemini_upstream_response_to_http_response(
        state,
        context,
        provider,
        budget_reservation,
        response,
        stream,
    )
    .await
}

async fn gemini_upstream_response_to_http_response(
    state: &AppState,
    context: crate::core::types::context::RequestContext,
    provider: GeminiRouteProvider,
    budget_reservation: Option<UnifiedBudgetReservation>,
    response: reqwest::Response,
    stream: bool,
) -> Result<HttpResponse, GatewayError> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|error| GatewayError::internal(format!("Invalid upstream status: {error}")))?;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or(if stream {
            "text/event-stream"
        } else {
            "application/json"
        })
        .to_string();

    if stream && !status.is_success() {
        let body = response.bytes().await.map_err(gemini_http_error)?;
        return Ok(HttpResponse::build(status)
            .insert_header((actix_web::http::header::CONTENT_TYPE, content_type))
            .body(sanitize_gemini_error_body(body, &provider)));
    }

    if stream {
        return Ok(gemini_streaming_response(
            state,
            context,
            provider,
            budget_reservation,
            response,
            status,
            content_type,
        ));
    }

    let mut body = response.bytes().await.map_err(gemini_http_error)?;
    if status.is_success() {
        let spend_state = GeminiSpendState {
            pricing: state.pricing.as_ref(),
            budget_limits: &state.budget_limits,
            key_manager: &state.key_manager,
            api_key_id: context.api_key_id(),
        };
        record_gemini_spend(&spend_state, &provider, &body, budget_reservation, true).await;
    } else {
        body = sanitize_gemini_error_body(body, &provider);
    }

    Ok(HttpResponse::build(status)
        .insert_header((actix_web::http::header::CONTENT_TYPE, content_type))
        .body(body))
}

fn gemini_streaming_response(
    state: &AppState,
    context: crate::core::types::context::RequestContext,
    provider: GeminiRouteProvider,
    mut budget_reservation: Option<UnifiedBudgetReservation>,
    response: reqwest::Response,
    status: StatusCode,
    content_type: String,
) -> HttpResponse {
    let (tx, rx) = mpsc::channel::<Bytes>(8);
    let pricing = state.pricing.clone();
    let budget_limits = state.budget_limits.clone();
    let key_manager = state.key_manager.clone();
    let api_key_id = context.api_key_id();
    let should_record_spend = status.is_success();

    tokio::spawn(async move {
        let mut upstream = response.bytes_stream();
        let mut sse_buffer = String::new();
        let mut final_usage = None;
        let mut saw_upstream_output = false;

        while let Some(chunk_result) = upstream.next().await {
            let bytes = match chunk_result {
                Ok(bytes) => bytes,
                Err(_) => {
                    error!("Gemini SDK upstream stream error; closing client stream");
                    let spend_state = GeminiSpendState {
                        pricing: pricing.as_ref(),
                        budget_limits: &budget_limits,
                        key_manager: &key_manager,
                        api_key_id,
                    };
                    settle_gemini_stream_spend(
                        &spend_state,
                        &provider,
                        should_record_spend.then(|| final_usage.take()).flatten(),
                        budget_reservation.take(),
                        false,
                    )
                    .await;
                    if tx
                        .send(Bytes::from_static(
                            b"event: error\ndata: Gemini upstream stream error\n\n",
                        ))
                        .await
                        .is_err()
                    {
                        error!("client disconnected before Gemini stream error could be sent");
                    }
                    return;
                }
            };
            saw_upstream_output = true;
            if let Some(usage) = extract_gemini_sse_usage(&bytes, &mut sse_buffer) {
                final_usage = Some(usage);
            }
            if tx.send(bytes).await.is_err() {
                let spend_state = GeminiSpendState {
                    pricing: pricing.as_ref(),
                    budget_limits: &budget_limits,
                    key_manager: &key_manager,
                    api_key_id,
                };
                settle_gemini_stream_spend(
                    &spend_state,
                    &provider,
                    should_record_spend.then(|| final_usage.take()).flatten(),
                    budget_reservation.take(),
                    should_record_spend && saw_upstream_output,
                )
                .await;
                return;
            }
        }

        let spend_state = GeminiSpendState {
            pricing: pricing.as_ref(),
            budget_limits: &budget_limits,
            key_manager: &key_manager,
            api_key_id,
        };
        settle_gemini_stream_spend(
            &spend_state,
            &provider,
            if should_record_spend {
                final_usage
            } else {
                None
            },
            budget_reservation.take(),
            should_record_spend && saw_upstream_output,
        )
        .await;
    });

    let upstream =
        tokio_stream::wrappers::ReceiverStream::new(rx).map(Ok::<_, actix_web::error::Error>);
    HttpResponse::build(status)
        .insert_header((actix_web::http::header::CONTENT_TYPE, content_type))
        .streaming(upstream)
}

fn select_gemini_provider(
    providers: &[ProviderConfig],
    requested_model: &str,
) -> Result<GeminiRouteProvider, GatewayError> {
    let Some(provider) = providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter(|provider| is_gemini_provider(provider))
        .find(|provider| provider_supports_requested_model(provider, requested_model))
    else {
        return Err(GatewayError::Config(format!(
            "Gemini SDK route provider for model '{requested_model}' is not configured"
        )));
    };

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

fn provider_supports_requested_model(provider: &ProviderConfig, requested_model: &str) -> bool {
    provider.models.is_empty() || provider.models.iter().any(|model| model == requested_model)
}

fn is_gemini_provider(provider: &ProviderConfig) -> bool {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);

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
    provider_config::append_string_header_map(provider, "headers", |key, value| {
        push_gemini_header(&mut headers, key, value)
    })?;
    provider_config::append_string_header_map(provider, "custom_headers", |key, value| {
        push_gemini_header(&mut headers, key, value)
    })?;
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

fn ensure_gemini_route_authorized(
    state: &AppState,
    req: &HttpRequest,
) -> Result<crate::core::types::context::RequestContext, GatewayError> {
    let context = super::context::get_request_context(req)
        .map_err(|_| GatewayError::Auth("Unauthorized".to_string()))?;
    let auth = &state.config().gateway.auth;
    if !auth.enable_jwt && !auth.enable_api_key && auth.allow_anonymous {
        return Ok(context);
    }

    let user = super::context::get_authenticated_user(req);
    let api_key = super::context::get_authenticated_api_key(req);
    if super::context::check_permission(user.as_ref(), api_key.as_ref(), "chat") {
        Ok(context)
    } else {
        Err(GatewayError::Auth("Unauthorized".to_string()))
    }
}

fn validate_api_version(api_version: &str) -> Result<(), GatewayError> {
    if matches!(api_version, GEMINI_V1 | GEMINI_V1BETA) {
        return Ok(());
    }

    Err(GatewayError::validation("Unsupported Gemini API version"))
}

fn validate_method(method: &str) -> Result<(), GatewayError> {
    if matches!(method, "generateContent" | "streamGenerateContent") {
        return Ok(());
    }

    Err(GatewayError::validation("Unsupported Gemini method"))
}

fn validate_model_segment(model: &str) -> Result<(), GatewayError> {
    if model.trim().is_empty() {
        return Err(GatewayError::validation("Gemini model is required"));
    }

    if model
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Ok(());
    }

    Err(GatewayError::validation(
        "Gemini model must be a single safe path segment",
    ))
}

fn validate_gemini_request_size(request: &Value) -> Result<(), GatewayError> {
    let len = serde_json::to_vec(request)?.len();
    if len > GEMINI_MAX_JSON_BYTES {
        return Err(GatewayError::validation("Gemini request body too large"));
    }
    Ok(())
}

fn gemini_http_client() -> &'static Client {
    GEMINI_HTTP_CLIENT.get_or_init(Client::new)
}

fn gemini_http_error(error: reqwest::Error) -> GatewayError {
    if error.is_timeout() {
        GatewayError::timeout("Gemini upstream request timed out")
    } else {
        GatewayError::network("Gemini upstream request failed")
    }
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
