use crate::config::models::provider::ProviderConfig;
use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::providers::{GeminiNativeRequest, Provider, ProviderError};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::super::budgeted::ApiKeyBudgetPolicy;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(super) struct GeminiRouteProvider {
    pub(super) provider_name: String,
    pub(super) pricing_provider: String,
    pub(super) model: String,
}

impl GeminiRouteProvider {
    pub(super) fn selected(provider: &Provider, requested_model: &str) -> Self {
        Self {
            provider_name: provider.name().to_string(),
            pricing_provider: "gemini".to_string(),
            model: requested_model.to_string(),
        }
    }
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
    let provider_name = provider
        .settings
        .get("provider_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&provider.name);
    let provider_name = super::super::provider_config::normalize_provider_selector(provider_name);

    matches!(
        provider_type.as_str(),
        "gemini" | "googleai" | "googleaistudio"
    ) || matches!(
        provider_name.as_str(),
        "gemini" | "googleai" | "googleaistudio"
    )
}

pub(super) async fn send_gemini_request(
    state: &AppState,
    selected_provider: &Provider,
    provider: &GeminiRouteProvider,
    native_request: GeminiNativeRequest,
    api_key_budget_id: Option<Uuid>,
) -> Result<
    (
        Option<UnifiedBudgetReservation>,
        Option<BudgetReservation>,
        reqwest::Response,
    ),
    ProviderError,
> {
    let budgeted = state.budgeted.clone();
    let pricing = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let budget_limits = budgeted.budget_limits();
    let budget_request = native_request.body.clone();
    let (response, reservations) = budgeted
        .for_selected_with_api_key_budget(
            provider.provider_name.clone(),
            provider.model.clone(),
            api_key_budget_id,
            ApiKeyBudgetPolicy::FromProviderReservation,
        )
        .reserve_call(
            |_budget| {
                super::spend::reserve_gemini_budget(
                    pricing.as_ref(),
                    &pricing_config,
                    budget_limits.as_ref(),
                    provider,
                    &budget_request,
                )
                .map_err(gemini_gateway_error_to_provider_error)
            },
            || selected_provider.gemini_generate_content(native_request),
        )
        .await?;
    let (budget_reservation, key_budget_reservation) = reservations.into_parts();
    Ok((budget_reservation, key_budget_reservation, response))
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
        GatewayError::HttpClient(_) | GatewayError::Network(_) => {
            ProviderError::network("gemini_proxy", "Gemini upstream request failed")
        }
        GatewayError::Unavailable(message) => {
            ProviderError::provider_unavailable("gemini_proxy", message)
        }
        other => ProviderError::api_error("gemini_proxy", 500, other.to_string()),
    }
}
