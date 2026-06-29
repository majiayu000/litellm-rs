//! OpenAI-compatible moderation API route.

use crate::config::models::provider::ProviderConfig;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use reqwest::Url;
use reqwest::header::{HeaderName, HeaderValue};
use serde_json::Value;
use std::time::Duration;
use tracing::error;

use super::openai_errors;
use super::provider_config;

const DEFAULT_MODERATION_MODEL: &str = "omni-moderation-latest";
const OPENAI_MODERATION_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone, PartialEq)]
struct ModerationProxyProvider {
    provider_name: String,
    base_url: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    timeout: Duration,
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

    match proxy_moderation(state.get_ref(), request.into_inner()).await {
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
    mut request: Value,
) -> Result<HttpResponse, GatewayError> {
    let resolved_model = validate_moderation_request(&request)?;
    apply_resolved_moderation_model(&mut request, &resolved_model);
    let Some(provider) = select_moderation_proxy_provider(
        state.config().gateway.providers.as_slice(),
        &resolved_model,
    )?
    else {
        return Err(missing_moderation_provider_error());
    };

    super::spend::ensure_budget_available(
        &state.budget_limits,
        &provider.provider_name,
        &resolved_model,
    )?;

    let response = provider_config::apply_proxy_headers(
        provider_config::proxy_http_client().post(moderation_url(&provider)?),
        &provider.headers,
    )
    .timeout(provider.timeout)
    .json(&request)
    .send()
    .await?;

    provider_config::proxy_response_to_http_response(response).await
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

fn select_moderation_proxy_provider(
    providers: &[ProviderConfig],
    requested_model: &str,
) -> Result<Option<ModerationProxyProvider>, GatewayError> {
    let candidates = providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter(|provider| is_openai_moderation_provider(provider))
        .collect::<Vec<_>>();

    let Some(provider) = candidates
        .iter()
        .copied()
        .find(|provider| moderation_provider_supports_requested_model(provider, requested_model))
    else {
        if candidates.is_empty() {
            return Ok(None);
        }
        return Err(GatewayError::Config(format!(
            "Moderation provider for model '{requested_model}' is not configured"
        )));
    };

    if provider.api_key.trim().is_empty() {
        return Err(GatewayError::Config(format!(
            "Moderation provider '{}' is missing api_key",
            provider.name
        )));
    }

    Ok(Some(ModerationProxyProvider {
        provider_name: provider.name.clone(),
        base_url: moderation_base_url(provider)?,
        headers: moderation_provider_headers(provider)?,
        timeout: Duration::from_secs(provider.timeout),
    }))
}

fn moderation_provider_supports_requested_model(
    provider: &ProviderConfig,
    requested_model: &str,
) -> bool {
    provider.models.is_empty() || provider.models.iter().any(|model| model == requested_model)
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
