//! Rerank endpoint.

use crate::config::models::provider::ProviderConfig;
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::{Provider, ProviderError};
use crate::core::rerank::{
    CohereRerankProvider, JinaRerankProvider, RerankProvider, RerankRequest, RerankResponse,
    RerankService, VoyageRerankProvider,
};
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

use super::{
    budgeted::{SettlementMode, run_unary},
    openai_errors, provider_config,
};

/// Rerank documents against a query.
pub async fn rerank(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<RerankRequest>,
) -> ActixResult<HttpResponse> {
    info!("Rerank request for model: {}", request.model);

    if let Err(error) = ensure_rerank_route_authorized(state.get_ref(), &req) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    if let Err(error) =
        super::context::enforce_api_key_model_and_token_limits(&req, &request.model, None)
    {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let context = match super::context::get_request_context(&req) {
        Ok(context) => context,
        Err(error) => {
            return Ok(openai_errors::gateway_error_response(&GatewayError::Auth(
                error.to_string(),
            )));
        }
    };
    let api_key_id = context
        .api_key_id()
        .or_else(|| super::context::get_authenticated_api_key(&req).map(|key| key.metadata.id));

    match handle_rerank_with_state(
        state.get_ref(),
        request.into_inner(),
        api_key_id,
        context.api_key_budget_id(),
    )
    .await
    {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(error) => {
            error!("Rerank error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

async fn handle_rerank_with_state(
    state: &AppState,
    mut request: RerankRequest,
    api_key_id: Option<uuid::Uuid>,
    api_key_budget_id: Option<uuid::Uuid>,
) -> Result<RerankResponse, GatewayError> {
    let requested_model = state.unified_router.resolve_model_name(&request.model);
    request.model = requested_model.clone();
    ensure_rerank_provider_candidate_configured(
        state.config().gateway.providers.as_slice(),
        &requested_model,
    )?;
    let router_models = rerank_router_models(
        state.config().gateway.providers.as_slice(),
        &requested_model,
    );

    let mut last_router_error = None;
    let budgeted = state.budgeted.clone();
    let key_manager = budgeted.key_manager();
    let pricing_service = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    for router_model in router_models {
        let result = run_unary(
            &state.unified_router,
            &router_model,
            ProviderCapability::Rerank,
            {
                let request = request.clone();
                let requested_model = requested_model.clone();
                let budgeted = budgeted.clone();
                let key_manager = key_manager.clone();
                let pricing_service = pricing_service.clone();
                let pricing_config = pricing_config.clone();
                move |selected_provider, selected_model, selected_deployment_id| {
                    let request = request.clone();
                    let requested_model = requested_model.clone();
                    let budgeted = budgeted.clone();
                    let key_manager = key_manager.clone();
                    let pricing_service = pricing_service.clone();
                    let pricing_config = pricing_config.clone();
                    async move {
                        if let Provider::Voyage(provider) = &selected_provider {
                            let request_pricing = super::spend::request_pricing_for_provider(
                                &pricing_service,
                                &selected_provider,
                                &selected_model,
                                ProviderCapability::Rerank,
                            )?;
                            if request_pricing.priced_parts().is_none() {
                                return Err(super::spend::model_not_priced_error(
                                    selected_provider.name(),
                                    &selected_model,
                                    "the selected Voyage model has no exact catalog price",
                                ));
                            }
                            let estimated_usage = estimated_rerank_usage(&request);
                            let voyage = provider.rerank_provider();
                            let mut request_for_provider = request;
                            request_for_provider.model = selected_model.clone();
                            let reserve_pricing = request_pricing.clone();
                            let settle_pricing = request_pricing;
                            let reserve_usage = estimated_usage;
                            let reserve_pricing_config = pricing_config.clone();
                            let settle_pricing_config = pricing_config;
                            let settle_key_manager = key_manager;
                            return budgeted
                                .for_selected_with_api_key_budget(
                                    "voyage",
                                    selected_model,
                                    api_key_budget_id,
                                    SettlementMode::Metered,
                                )
                                .reserve_call_settle(
                                    |budget| {
                                        super::spend::reserve_pricing_usage_budget_with_request_pricing(
                                            &reserve_pricing,
                                            &reserve_pricing_config,
                                            budget.budget_limits(),
                                            budget.provider(),
                                            budget.model(),
                                            &reserve_usage,
                                        )
                                    },
                                    || async move {
                                        let response = voyage
                                            .rerank(request_for_provider)
                                            .await
                                            .map_err(rerank_gateway_error_to_provider_error)?;
                                        let total_tokens = response
                                            .usage
                                            .as_ref()
                                            .and_then(|usage| usage.total_tokens)
                                            .ok_or_else(|| {
                                                ProviderError::response_parsing(
                                                    "voyage",
                                                    "Voyage rerank response omitted total token usage",
                                                )
                                            })?;
                                        Ok((response, total_tokens))
                                    },
                                    |(response, total_tokens), reservations, budget| async move {
                                        let usage = PricingUsage::new(total_tokens, 0);
                                        let (budget_reservation, key_budget_reservation) =
                                            reservations.into_parts();
                                        super::spend::record_pricing_usage_spend_with_request_pricing(
                                            &settle_pricing,
                                            &settle_pricing_config,
                                            budget.budget_limits(),
                                            &settle_key_manager,
                                            api_key_id,
                                            budget.provider(),
                                            budget.model(),
                                            &usage,
                                            budget_reservation,
                                            key_budget_reservation,
                                        )
                                        .await;
                                        ((response, total_tokens), u64::from(total_tokens))
                                    },
                                )
                                .await
                                .map(|((response, _total_tokens), tokens)| (response, tokens));
                        }
                        let selected = selected_rerank_provider(
                            state.config().gateway.providers.as_slice(),
                            &selected_deployment_id,
                            &selected_provider,
                            &selected_model,
                            &requested_model,
                        )?;
                        let served_model = served_rerank_model(&requested_model);
                        let budget_provider = selected.provider_name.clone();
                        budgeted
                            .for_selected(budget_provider, served_model.to_string())
                            .with_settlement_mode(SettlementMode::AvailabilityOnly)
                            .reserve_call_settle(
                                |_budget| Ok(None),
                                || async move {
                                    let service = build_rerank_service(&selected)
                                        .map_err(rerank_gateway_error_to_provider_error)?;
                                    service
                                        .rerank(request)
                                        .await
                                        .map_err(rerank_gateway_error_to_provider_error)
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

    Err(last_router_error.unwrap_or_else(missing_rerank_provider_error))
}

fn estimated_rerank_usage(request: &RerankRequest) -> PricingUsage {
    let total_tokens = std::iter::once(request.query.as_str())
        .chain(request.documents.iter().map(|document| document.get_text()))
        .map(estimated_text_tokens)
        .fold(0_u32, u32::saturating_add);
    PricingUsage::new(total_tokens, 0)
}

fn estimated_text_tokens(text: &str) -> u32 {
    u32::try_from(text.len().div_ceil(4))
        .unwrap_or(u32::MAX)
        .max(1)
}

fn ensure_rerank_route_authorized(state: &AppState, req: &HttpRequest) -> Result<(), GatewayError> {
    let _context = super::context::get_request_context(req)
        .map_err(|_| GatewayError::Auth("Unauthorized".to_string()))?;
    let auth = &state.config().gateway.auth;
    if !auth.enable_jwt && !auth.enable_api_key && auth.allow_anonymous {
        return Ok(());
    }

    let user = super::context::get_authenticated_user(req);
    let api_key = super::context::get_authenticated_api_key(req);
    if super::context::check_permission(user.as_ref(), api_key.as_ref(), "rerank") {
        Ok(())
    } else {
        Err(GatewayError::Auth("Unauthorized".to_string()))
    }
}

fn build_rerank_service(provider: &SelectedRerankProvider) -> Result<RerankService, GatewayError> {
    let mut service = RerankService::new();
    service
        .set_default_provider(provider.kind.as_str())
        .set_timeout(provider.timeout);

    match provider.kind {
        RerankProviderKind::Cohere => {
            let rerank_provider = CohereRerankProvider::new_with_endpoint(
                provider.api_key.clone(),
                provider
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.cohere.ai/v1"),
                provider.endpoint_access,
                provider.timeout.as_secs(),
            )?;
            service.register_provider("cohere", Arc::new(rerank_provider));
        }
        RerankProviderKind::Jina => {
            let rerank_provider = JinaRerankProvider::new_with_endpoint(
                provider.api_key.clone(),
                provider
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.jina.ai/v1"),
                provider.endpoint_access,
                provider.timeout.as_secs(),
            )?;
            service.register_provider("jina", Arc::new(rerank_provider));
        }
        RerankProviderKind::Voyage => {
            let rerank_provider = VoyageRerankProvider::new_with_endpoint(
                provider.api_key.clone(),
                provider
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.voyageai.com/v1"),
                provider.endpoint_access,
                provider.timeout.as_secs(),
            )?;
            service.register_provider("voyage", Arc::new(rerank_provider));
        }
    }

    Ok(service)
}

#[cfg(test)]
fn select_rerank_provider(
    providers: &[ProviderConfig],
    request: &RerankRequest,
) -> Result<SelectedRerankProvider, GatewayError> {
    ensure_rerank_provider_candidate_configured(providers, &request.model)?;

    let Some((provider, kind)) = rerank_candidate_configs(providers)
        .into_iter()
        .find(|(provider, kind)| rerank_provider_supports_model(provider, *kind, &request.model))
    else {
        return Err(missing_rerank_provider_error());
    };

    selected_rerank_provider_from_config(provider, kind)
}

fn ensure_rerank_provider_candidate_configured(
    providers: &[ProviderConfig],
    requested_model: &str,
) -> Result<(), GatewayError> {
    let candidates = rerank_candidate_configs(providers);
    if candidates.is_empty() {
        return Err(missing_rerank_provider_error());
    }

    if candidates
        .iter()
        .any(|(provider, kind)| rerank_provider_supports_model(provider, *kind, requested_model))
    {
        Ok(())
    } else {
        Err(GatewayError::NotFound(format!(
            "No configured rerank provider supports model '{requested_model}'"
        )))
    }
}

fn selected_rerank_provider_from_config(
    provider: &ProviderConfig,
    kind: RerankProviderKind,
) -> Result<SelectedRerankProvider, GatewayError> {
    if provider.api_key.trim().is_empty() {
        return Err(GatewayError::Config(format!(
            "Rerank provider '{}' is missing api_key",
            provider.name
        )));
    }

    Ok(SelectedRerankProvider {
        provider_name: provider.name.clone(),
        kind,
        api_key: provider.api_key.clone(),
        base_url: provider.configured_endpoint().map(str::to_string),
        timeout: Duration::from_secs(provider.timeout),
        endpoint_access: provider.endpoint_access,
    })
}

fn selected_rerank_provider(
    providers: &[ProviderConfig],
    selected_deployment_id: &str,
    selected_provider: &Provider,
    selected_model: &str,
    requested_model: &str,
) -> Result<SelectedRerankProvider, ProviderError> {
    let selected_provider_name = selected_provider.name();
    let candidates = rerank_candidate_configs(providers);
    let supported_candidates = candidates
        .iter()
        .copied()
        .filter(|(provider, kind)| rerank_provider_supports_model(provider, *kind, requested_model))
        .collect::<Vec<_>>();
    let matching = supported_candidates
        .iter()
        .copied()
        .find(|(provider, _)| {
            selected_deployment_matches_provider_config(
                selected_deployment_id,
                provider,
                selected_model,
            )
        })
        .or_else(|| {
            supported_candidates.iter().copied().find(|(provider, kind)| {
                provider.name == selected_provider_name
                || provider
                    .settings
                    .get("provider_name")
                    .and_then(|value| value.as_str())
                    == Some(selected_provider_name)
                || (provider.models.is_empty() && provider.name == selected_model)
                || (provider.models.iter().any(|model| model == selected_model)
                    && kind.as_str() == selected_provider_name)
            })
        })
        .ok_or_else(|| {
            ProviderError::configuration(
                "rerank_proxy",
                format!(
                    "selected rerank provider '{selected_provider_name}' for model '{selected_model}' has no matching gateway provider config"
                ),
            )
        })?;

    selected_rerank_provider_from_config(matching.0, matching.1)
        .map_err(rerank_gateway_error_to_provider_error)
}

fn selected_deployment_matches_provider_config(
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

fn rerank_router_models(providers: &[ProviderConfig], requested_model: &str) -> Vec<String> {
    let (_, served_model) = split_rerank_model(requested_model);
    let mut router_models = Vec::new();

    for (provider, kind) in rerank_candidate_configs(providers) {
        if !rerank_provider_supports_model(provider, kind, requested_model) {
            continue;
        }

        if provider.models.is_empty() {
            if rerank_provider_uses_registry_models(provider) {
                push_unique_model(&mut router_models, served_model);
                push_unique_model(&mut router_models, requested_model);
            } else {
                push_unique_model(&mut router_models, &provider.name);
            }
            continue;
        }

        for model in &provider.models {
            if model == requested_model || model == served_model {
                push_unique_model(&mut router_models, model);
            }
        }
    }

    if router_models.is_empty() {
        push_unique_model(&mut router_models, served_model);
    }

    router_models
}

fn push_unique_model(router_models: &mut Vec<String>, model: &str) {
    if !router_models.iter().any(|existing| existing == model) {
        router_models.push(model.to_string());
    }
}

fn rerank_candidate_configs(
    providers: &[ProviderConfig],
) -> Vec<(&ProviderConfig, RerankProviderKind)> {
    providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter_map(|provider| rerank_provider_kind(provider).map(|kind| (provider, kind)))
        .collect()
}

fn rerank_provider_supports_model(
    provider: &ProviderConfig,
    kind: RerankProviderKind,
    requested_model: &str,
) -> bool {
    let (requested_provider, served_model) = split_rerank_model(requested_model);
    if let Some(requested_provider) = requested_provider
        && !rerank_qualifier_matches(kind, requested_provider)
    {
        return false;
    }

    let explicitly_configured = provider
        .models
        .iter()
        .any(|model| model == requested_model || model == served_model);

    if !provider.models.is_empty() {
        return explicitly_configured;
    }

    kind.supports_model(served_model)
}

fn rerank_qualifier_matches(kind: RerankProviderKind, qualifier: &str) -> bool {
    crate::core::providers::registry::entry_for_name(qualifier).map_or_else(
        || qualifier == kind.as_str(),
        |entry| entry.canonical_name == kind.as_str(),
    )
}

fn rerank_provider_kind(provider: &ProviderConfig) -> Option<RerankProviderKind> {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);

    if provider_type == "voyage" || provider_name == "voyage" {
        return Some(RerankProviderKind::Voyage);
    }

    if provider_type.contains("cohere") || provider_name.contains("cohere") {
        return Some(RerankProviderKind::Cohere);
    }

    if provider_type.contains("jina") || provider_name.contains("jina") {
        return Some(RerankProviderKind::Jina);
    }

    None
}

fn rerank_provider_uses_registry_models(provider: &ProviderConfig) -> bool {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    matches!(provider_type.as_str(), "cohere" | "voyage")
}

fn served_rerank_model(model: &str) -> &str {
    split_rerank_model(model).1
}

fn rerank_gateway_error_to_provider_error(error: GatewayError) -> ProviderError {
    match error {
        GatewayError::Provider(error) => error,
        GatewayError::Validation(message) | GatewayError::BadRequest(message) => {
            ProviderError::invalid_request("rerank_proxy", message)
        }
        GatewayError::Config(message) => ProviderError::configuration("rerank_proxy", message),
        GatewayError::Auth(message) => ProviderError::authentication("rerank_proxy", message),
        GatewayError::Forbidden(message) => ProviderError::api_error("rerank_proxy", 403, message),
        GatewayError::Timeout(message) => ProviderError::timeout("rerank_proxy", message),
        GatewayError::RateLimit {
            message,
            retry_after,
            ..
        } => ProviderError::rate_limit_with_retry("rerank_proxy", message, retry_after),
        GatewayError::HttpClient(error) => {
            ProviderError::network("rerank_proxy", error.to_string())
        }
        GatewayError::Network(message) => ProviderError::network("rerank_proxy", message),
        GatewayError::Unavailable(message) => {
            ProviderError::provider_unavailable("rerank_proxy", message)
        }
        other => ProviderError::api_error("rerank_proxy", 500, other.to_string()),
    }
}

fn missing_rerank_provider_error() -> GatewayError {
    GatewayError::NotFound(
        "No configured rerank provider found; configure a cohere, jina, or voyage provider"
            .to_string(),
    )
}

fn split_rerank_model(model: &str) -> (Option<&str>, &str) {
    match model.split_once('/') {
        Some((provider, model)) if !provider.trim().is_empty() && !model.trim().is_empty() => {
            (Some(provider), model)
        }
        _ => (None, model),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RerankProviderKind {
    Cohere,
    Jina,
    Voyage,
}

impl RerankProviderKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cohere => "cohere",
            Self::Jina => "jina",
            Self::Voyage => "voyage",
        }
    }

    fn supports_model(self, model: &str) -> bool {
        match self {
            Self::Cohere => matches!(
                model,
                "rerank-english-v3.0"
                    | "rerank-multilingual-v3.0"
                    | "rerank-english-v2.0"
                    | "rerank-multilingual-v2.0"
            ),
            Self::Jina => matches!(
                model,
                "jina-reranker-v2-base-multilingual"
                    | "jina-reranker-v1-base-en"
                    | "jina-reranker-v1-turbo-en"
            ),
            Self::Voyage => matches!(
                model,
                "rerank-2.5"
                    | "rerank-2.5-lite"
                    | "rerank-2"
                    | "rerank-2-lite"
                    | "rerank-1"
                    | "rerank-lite-1"
            ),
        }
    }
}

#[derive(Debug)]
struct SelectedRerankProvider {
    provider_name: String,
    kind: RerankProviderKind,
    api_key: String,
    base_url: Option<String>,
    timeout: Duration,
    endpoint_access: crate::core::net::ProviderEndpointAccess,
}

#[cfg(test)]
#[path = "rerank/tests.rs"]
mod tests;
