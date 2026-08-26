//! OpenAI-compatible moderation API route.

use crate::config::models::provider::ProviderConfig;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::{Provider, ProviderError};
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use reqwest::Url;
use reqwest::header::{HeaderName, HeaderValue};
use serde_json::Value;
use std::time::Duration;
use tracing::error;

use super::budgeted::{SettlementMode, run_unary};
use super::openai_errors;
use super::{provider_config, route_http::RouteHttpClient};

const DEFAULT_MODERATION_MODEL: &str = "omni-moderation-latest";
const OPENAI_MODERATION_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone, PartialEq)]
struct ModerationProxyProvider {
    provider_name: String,
    base_url: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    timeout: Duration,
    endpoint_access: ProviderEndpointAccess,
}

/// Create a moderation request.
pub async fn create_moderation(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<Value>,
) -> ActixResult<HttpResponse> {
    if let Err(error) = ensure_moderation_route_authorized(state.get_ref(), &req) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    match proxy_moderation(state.get_ref(), &req, request.into_inner()).await {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Moderation route error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

fn ensure_moderation_route_authorized(
    state: &AppState,
    req: &HttpRequest,
) -> Result<(), GatewayError> {
    let _context = super::context::get_request_context(req)
        .map_err(|_| GatewayError::Auth("Unauthorized".to_string()))?;
    let auth = &state.config().gateway.auth;
    if !auth.enable_jwt && !auth.enable_api_key && auth.allow_anonymous {
        return Ok(());
    }

    let user = super::context::get_authenticated_user(req);
    let api_key = super::context::get_authenticated_api_key(req);
    if super::context::check_permission(user.as_ref(), api_key.as_ref(), "moderations") {
        Ok(())
    } else {
        Err(GatewayError::Auth("Unauthorized".to_string()))
    }
}

async fn proxy_moderation(
    state: &AppState,
    req: &HttpRequest,
    mut request: Value,
) -> Result<HttpResponse, GatewayError> {
    let requested_model = validate_moderation_request(&request)?;
    super::context::enforce_api_key_model_and_token_limits(req, &requested_model, None)?;
    let resolved_model = state.unified_router.resolve_model_name(&requested_model);
    apply_resolved_moderation_model(&mut request, &resolved_model);
    ensure_moderation_proxy_candidate_configured(
        state.config().gateway.providers.as_slice(),
        &resolved_model,
    )?;
    let router_models =
        moderation_router_models(state.config().gateway.providers.as_slice(), &resolved_model);

    let mut last_router_error = None;
    let budgeted = state.budgeted.clone();
    for router_model in router_models {
        let result = run_unary(
            &state.unified_router,
            &router_model,
            ProviderCapability::Moderation,
            {
                let request = request.clone();
                let resolved_model = resolved_model.clone();
                let budgeted = budgeted.clone();
                move |selected_provider, selected_model, _deployment_id| {
                    let request = request.clone();
                    let resolved_model = resolved_model.clone();
                    let budgeted = budgeted.clone();
                    async move {
                        let provider = selected_moderation_proxy_provider(
                            state.config().gateway.providers.as_slice(),
                            &selected_provider,
                            &selected_model,
                            &resolved_model,
                        )?;
                        let budget_provider = provider.provider_name.clone();
                        budgeted
                            .for_selected(budget_provider, resolved_model.clone())
                            .with_settlement_mode(SettlementMode::AvailabilityOnly)
                            .reserve_call_settle(
                                |_budget| Ok(None),
                                || async move {
                                    let url = moderation_url(&provider)
                                        .map_err(moderation_gateway_error_to_provider_error)?;
                                    let client = RouteHttpClient::new(
                                        "moderation_proxy",
                                        provider.base_url.clone(),
                                        provider.endpoint_access,
                                        provider.timeout.as_secs(),
                                    )
                                    .map_err(moderation_gateway_error_to_provider_error)?;
                                    let request_builder = client
                                        .ordinary_post(url)
                                        .map_err(moderation_gateway_error_to_provider_error)?;
                                    let response = provider_config::apply_proxy_headers(
                                        request_builder,
                                        &provider.headers,
                                    )
                                    .json(&request)
                                    .send()
                                    .await
                                    .map_err(|error| {
                                        ProviderError::network(
                                            "moderation_proxy",
                                            error.to_string(),
                                        )
                                    })?;

                                    if !response.status().is_success() {
                                        return Err(moderation_upstream_error(response).await);
                                    }

                                    provider_config::proxy_response_to_http_response(response)
                                        .await
                                        .map_err(moderation_gateway_error_to_provider_error)
                                },
                                |response, _reservations, _budget| async move { (response, 0) },
                            )
                            .await
                    }
                }
            },
        )
        .await;
        match result {
            Ok(response) => return Ok(response),
            Err(GatewayError::Provider(ProviderError::QuotaExceeded {
                provider: "budget",
                message,
            })) if message.starts_with("provider ") => {
                last_router_error = Some(GatewayError::Provider(ProviderError::QuotaExceeded {
                    provider: "budget",
                    message,
                }));
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_router_error.unwrap_or_else(missing_moderation_provider_error))
}

fn validate_moderation_request(request: &Value) -> Result<String, GatewayError> {
    let object = request
        .as_object()
        .ok_or_else(|| GatewayError::validation("request body must be a JSON object"))?;

    for key in object.keys() {
        if key != "input" && key != "model" {
            return Err(GatewayError::validation(format!(
                "Unknown /v1/moderations field: {key}"
            )));
        }
    }

    match object.get("input") {
        Some(Value::String(input)) if !input.trim().is_empty() => {}
        Some(Value::Array(inputs)) if !inputs.is_empty() => {}
        Some(Value::String(_)) => return Err(GatewayError::validation("input cannot be empty")),
        Some(Value::Array(_)) => return Err(GatewayError::validation("input cannot be empty")),
        Some(_) => {
            return Err(GatewayError::validation(
                "input must be a string or non-empty array",
            ));
        }
        None => return Err(GatewayError::validation("input is required")),
    }

    match object.get("model") {
        Some(Value::String(model)) if !model.trim().is_empty() => Ok(model.trim().to_string()),
        Some(Value::String(_)) => Err(GatewayError::validation("model cannot be empty")),
        Some(Value::Null) | None => Ok(DEFAULT_MODERATION_MODEL.to_string()),
        Some(_) => Err(GatewayError::validation("model must be a string")),
    }
}

fn apply_resolved_moderation_model(request: &mut Value, resolved_model: &str) {
    if let Some(object) = request.as_object_mut() {
        object.insert(
            "model".to_string(),
            Value::String(resolved_model.to_string()),
        );
    }
}

fn ensure_moderation_proxy_candidate_configured(
    providers: &[ProviderConfig],
    requested_model: &str,
) -> Result<(), GatewayError> {
    let candidates = moderation_candidate_configs(providers);
    if candidates.is_empty() {
        return Err(missing_moderation_provider_error());
    }

    if candidates
        .iter()
        .any(|provider| moderation_provider_supports_requested_model(provider, requested_model))
    {
        Ok(())
    } else {
        Err(GatewayError::Config(format!(
            "Moderation provider for model '{requested_model}' is not configured"
        )))
    }
}

fn moderation_router_models(providers: &[ProviderConfig], requested_model: &str) -> Vec<String> {
    let candidates = moderation_candidate_configs(providers);
    let mut router_models = Vec::new();

    if candidates.iter().any(|provider| {
        provider.models.iter().any(|model| model == requested_model)
            || (provider.models.is_empty() && moderation_provider_uses_registry_models(provider))
    }) {
        router_models.push(requested_model.to_string());
    }

    router_models.extend(
        candidates
            .iter()
            .filter(|provider| {
                provider.models.is_empty() && !moderation_provider_uses_registry_models(provider)
            })
            .map(|provider| provider.name.clone()),
    );

    if router_models.is_empty() {
        router_models.push(requested_model.to_string());
    }

    router_models
}

fn selected_moderation_proxy_provider(
    providers: &[ProviderConfig],
    selected_provider: &Provider,
    selected_model: &str,
    requested_model: &str,
) -> Result<ModerationProxyProvider, ProviderError> {
    let provider_name = selected_provider.name();
    let candidates = moderation_candidate_configs(providers);
    let matching = candidates
        .iter()
        .copied()
        .filter(|provider| moderation_provider_supports_requested_model(provider, requested_model))
        .find(|provider| {
            provider.name == provider_name
                || provider
                    .settings
                    .get("provider_name")
                    .and_then(|value| value.as_str())
                    == Some(provider_name)
                || (provider.models.is_empty() && provider.name == selected_model)
        })
        .ok_or_else(|| {
            ProviderError::configuration(
                "moderation_proxy",
                format!(
                    "selected moderation provider '{provider_name}' for model '{selected_model}' has no matching gateway provider config"
                ),
            )
        })?;

    moderation_proxy_provider_from_config(matching)
        .map_err(moderation_gateway_error_to_provider_error)
}

fn moderation_candidate_configs(providers: &[ProviderConfig]) -> Vec<&ProviderConfig> {
    providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter(|provider| is_openai_moderation_provider(provider))
        .collect()
}

fn moderation_proxy_provider_from_config(
    provider: &ProviderConfig,
) -> Result<ModerationProxyProvider, GatewayError> {
    if provider.api_key.trim().is_empty() {
        return Err(GatewayError::Config(format!(
            "Moderation provider '{}' is missing api_key",
            provider.name
        )));
    }

    Ok(ModerationProxyProvider {
        provider_name: provider.name.clone(),
        base_url: moderation_base_url(provider)?,
        headers: moderation_provider_headers(provider)?,
        timeout: Duration::from_secs(provider.timeout),
        endpoint_access: provider.endpoint_access,
    })
}

fn moderation_provider_supports_requested_model(
    provider: &ProviderConfig,
    requested_model: &str,
) -> bool {
    provider.models.is_empty() || provider.models.iter().any(|model| model == requested_model)
}

fn moderation_provider_uses_registry_models(provider: &ProviderConfig) -> bool {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);

    provider_type == "openai" || (provider_type.is_empty() && provider_name == "openai")
}

fn moderation_base_url(provider: &ProviderConfig) -> Result<String, GatewayError> {
    if let Some(base_url) = provider.base_url.as_deref() {
        let trimmed = base_url.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    if provider_config::normalize_provider_selector(&provider.provider_type) == "openai"
        || provider_config::normalize_provider_selector(&provider.name) == "openai"
    {
        return Ok(OPENAI_MODERATION_BASE_URL.to_string());
    }

    Err(GatewayError::Config(format!(
        "Moderation provider '{}' is missing base_url",
        provider.name
    )))
}

fn moderation_url(provider: &ModerationProxyProvider) -> Result<Url, GatewayError> {
    let mut url = Url::parse(&provider.base_url)
        .map_err(|error| GatewayError::Config(format!("Invalid moderation URL: {error}")))?;
    url.path_segments_mut()
        .map_err(|_| GatewayError::Config("Invalid moderation URL".to_string()))?
        .extend(["moderations"]);
    Ok(url)
}

fn moderation_gateway_error_to_provider_error(error: GatewayError) -> ProviderError {
    match error {
        GatewayError::Provider(error) => error,
        GatewayError::Validation(message) | GatewayError::BadRequest(message) => {
            ProviderError::invalid_request("moderation_proxy", message)
        }
        GatewayError::Config(message) => ProviderError::configuration("moderation_proxy", message),
        GatewayError::Auth(message) => ProviderError::authentication("moderation_proxy", message),
        GatewayError::Forbidden(message) => {
            ProviderError::api_error("moderation_proxy", 403, message)
        }
        GatewayError::Timeout(message) => ProviderError::timeout("moderation_proxy", message),
        GatewayError::RateLimit {
            message,
            retry_after,
            ..
        } => ProviderError::rate_limit_with_retry("moderation_proxy", message, retry_after),
        GatewayError::HttpClient(error) => {
            ProviderError::network("moderation_proxy", error.to_string())
        }
        GatewayError::Network(message) => ProviderError::network("moderation_proxy", message),
        GatewayError::Unavailable(message) => {
            ProviderError::provider_unavailable("moderation_proxy", message)
        }
        other => ProviderError::api_error("moderation_proxy", 500, other.to_string()),
    }
}

async fn moderation_upstream_error(response: reqwest::Response) -> ProviderError {
    let status = response.status().as_u16();
    let message = response
        .text()
        .await
        .unwrap_or_else(|error| format!("Failed to read moderation error body: {error}"));

    match status {
        400 => ProviderError::invalid_request("moderation_proxy", message),
        401 => ProviderError::authentication("moderation_proxy", message),
        403 => ProviderError::api_error("moderation_proxy", status, message),
        402 => ProviderError::quota_exceeded("moderation_proxy", message),
        404 => ProviderError::model_not_found("moderation_proxy", message),
        408 | 504 => ProviderError::timeout("moderation_proxy", message),
        429 => ProviderError::rate_limit_with_retry("moderation_proxy", message, None),
        502 | 503 => ProviderError::provider_unavailable("moderation_proxy", message),
        _ => ProviderError::api_error("moderation_proxy", status, message),
    }
}

fn missing_moderation_provider_error() -> GatewayError {
    GatewayError::Config(
        "Moderation API requires an enabled openai or openai_compatible provider".to_string(),
    )
}

fn moderation_provider_headers(
    provider: &ProviderConfig,
) -> Result<Vec<(HeaderName, HeaderValue)>, GatewayError> {
    let mut headers = Vec::new();
    provider_config::push_proxy_header(
        &mut headers,
        "moderation provider",
        "Authorization",
        format!("Bearer {}", provider.api_key),
    )?;
    if let Some(organization) = provider.organization.as_deref() {
        provider_config::push_proxy_header(
            &mut headers,
            "moderation provider",
            "OpenAI-Organization",
            organization,
        )?;
    }
    if let Some(project) = provider.project.as_deref() {
        provider_config::push_proxy_header(
            &mut headers,
            "moderation provider",
            "OpenAI-Project",
            project,
        )?;
    }
    provider_config::append_string_header_map(provider, "headers", |key, value| {
        provider_config::push_proxy_header(&mut headers, "moderation provider", key, value)
    })?;
    provider_config::append_string_header_map(provider, "custom_headers", |key, value| {
        provider_config::push_proxy_header(&mut headers, "moderation provider", key, value)
    })?;
    Ok(headers)
}

fn is_openai_moderation_provider(provider: &ProviderConfig) -> bool {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);

    provider_type == "openai"
        || provider_type == "openaicompatible"
        || provider_name == "openai"
        || provider_name == "openaicompatible"
}
