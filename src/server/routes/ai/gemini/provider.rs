use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::providers::{GeminiNativeRequest, Provider, ProviderError};
use crate::core::router::UnifiedRouter;
use crate::core::types::model::ProviderCapability;
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

pub(super) fn gemini_router_models(router: &UnifiedRouter, requested_model: &str) -> Vec<String> {
    let mut runtime_models = router.list_models();
    runtime_models.sort();
    runtime_models.dedup();

    let mut candidates = Vec::new();
    if runtime_models
        .binary_search_by(|model| model.as_str().cmp(requested_model))
        .is_ok()
        && gemini_runtime_model_supported(router, requested_model, requested_model)
    {
        candidates.push(requested_model.to_string());
    }
    candidates.extend(runtime_models.into_iter().filter(|model| {
        model != requested_model && gemini_runtime_model_supported(router, model, requested_model)
    }));
    candidates
}

fn gemini_runtime_model_supported(
    router: &UnifiedRouter,
    model: &str,
    requested_model: &str,
) -> bool {
    router
        .get_deployments_for_model(model)
        .into_iter()
        .filter_map(|deployment_id| router.get_deployment(&deployment_id))
        .any(|deployment| {
            let exact_requested_key = model == requested_model;
            let empty_model_alias = deployment.id == model
                && deployment.model == model
                && deployment.model_name == model;
            (exact_requested_key || empty_model_alias)
                && deployment.provider.supports_capability_for_model(
                    &deployment.model,
                    &ProviderCapability::GeminiGenerateContent,
                )
        })
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
