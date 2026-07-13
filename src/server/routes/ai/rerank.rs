//! Rerank endpoint.

use crate::config::models::provider::ProviderConfig;
use crate::core::providers::{Provider, ProviderError};
use crate::core::rerank::{
    CohereRerankProvider, JinaRerankProvider, RerankRequest, RerankResponse, RerankService,
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

    match handle_rerank_with_state(state.get_ref(), request.into_inner()).await {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(error) => {
            error!("Rerank error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

async fn handle_rerank_with_state(
    state: &AppState,
    request: RerankRequest,
) -> Result<RerankResponse, GatewayError> {
    let requested_model = request.model.clone();
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
    for router_model in router_models {
        let result = run_unary(
            &state.unified_router,
            &router_model,
            ProviderCapability::Rerank,
            {
                let request = request.clone();
                let requested_model = requested_model.clone();
                let budgeted = budgeted.clone();
                move |selected_provider, selected_model, selected_deployment_id| {
                    let request = request.clone();
                    let requested_model = requested_model.clone();
                    let budgeted = budgeted.clone();
                    async move {
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
        base_url: provider.base_url.clone(),
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
        && provider_config::normalize_provider_selector(requested_provider) != kind.as_str()
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

fn rerank_provider_kind(provider: &ProviderConfig) -> Option<RerankProviderKind> {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);

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
    provider_type == "cohere"
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
        "No configured rerank provider found; configure a cohere or jina provider".to_string(),
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
}

impl RerankProviderKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cohere => "cohere",
            Self::Jina => "jina",
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
mod tests {
    use super::*;

    fn provider_config(name: &str, provider_type: &str, models: Vec<&str>) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: provider_type.to_string(),
            api_key: "test-key".to_string(),
            models: models.into_iter().map(ToString::to_string).collect(),
            ..ProviderConfig::default()
        }
    }

    #[test]
    fn detects_provider_kind_from_type_or_name() {
        let by_type = provider_config("primary", "cohere_rerank", Vec::new());
        let by_name = provider_config("jina-reranker", "custom", Vec::new());
        let unsupported = provider_config("voyage", "voyage", Vec::new());

        assert_eq!(
            rerank_provider_kind(&by_type),
            Some(RerankProviderKind::Cohere)
        );
        assert_eq!(
            rerank_provider_kind(&by_name),
            Some(RerankProviderKind::Jina)
        );
        assert_eq!(rerank_provider_kind(&unsupported), None);
    }

    #[test]
    fn provider_model_filter_accepts_prefixed_and_unprefixed_models() {
        let provider = provider_config("cohere", "cohere", vec!["rerank-english-v3.0"]);

        assert!(rerank_provider_supports_model(
            &provider,
            RerankProviderKind::Cohere,
            "rerank-english-v3.0"
        ));
        assert!(rerank_provider_supports_model(
            &provider,
            RerankProviderKind::Cohere,
            "cohere/rerank-english-v3.0"
        ));
        assert!(!rerank_provider_supports_model(
            &provider,
            RerankProviderKind::Cohere,
            "jina/rerank-english-v3.0"
        ));
        assert!(!rerank_provider_supports_model(
            &provider,
            RerankProviderKind::Cohere,
            "rerank-multilingual-v3.0"
        ));
    }

    #[test]
    fn provider_model_filter_allows_explicit_new_provider_models() {
        let cohere = provider_config("cohere", "cohere", vec!["rerank-v4.0-pro"]);
        let jina = provider_config("jina", "jina", vec!["jina-colbert-v2"]);

        assert!(rerank_provider_supports_model(
            &cohere,
            RerankProviderKind::Cohere,
            "rerank-v4.0-pro"
        ));
        assert!(rerank_provider_supports_model(
            &cohere,
            RerankProviderKind::Cohere,
            "cohere/rerank-v4.0-pro"
        ));
        assert!(rerank_provider_supports_model(
            &jina,
            RerankProviderKind::Jina,
            "jina-colbert-v2"
        ));
        assert!(rerank_provider_supports_model(
            &jina,
            RerankProviderKind::Jina,
            "jina/jina-colbert-v2"
        ));
    }

    #[test]
    fn provider_model_filter_rejects_unconfigured_unknown_models() {
        let cohere = provider_config("cohere", "cohere", Vec::new());

        assert!(!rerank_provider_supports_model(
            &cohere,
            RerankProviderKind::Cohere,
            "rerank-v4.0-pro"
        ));
    }

    #[test]
    fn selects_matching_enabled_provider() {
        let wrong = provider_config("wrong-cohere", "cohere", vec!["rerank-multilingual-v3.0"]);
        let selected = provider_config("right-cohere", "cohere", vec!["rerank-english-v3.0"]);
        let request = RerankRequest {
            model: "rerank-english-v3.0".to_string(),
            query: "hello".to_string(),
            documents: vec!["doc".into()],
            ..RerankRequest::default()
        };

        let selected = select_rerank_provider(&[wrong, selected], &request)
            .expect("matching provider should be selected");

        assert_eq!(selected.provider_name, "right-cohere");
        assert_eq!(selected.kind, RerankProviderKind::Cohere);
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn selected_provider_uses_deployment_id_for_native_cohere_duplicate_model() {
        let primary = provider_config("cohere-a", "cohere", vec!["rerank-english-v3.0"]);
        let fallback = provider_config("cohere-b", "cohere", vec!["rerank-english-v3.0"]);
        let cohere =
            match crate::core::providers::cohere::CohereProvider::with_api_key("test-key").await {
                Ok(provider) => provider,
                Err(error) => panic!("native cohere provider should build: {error}"),
            };
        let provider = Provider::Cohere(cohere);

        let selected = match selected_rerank_provider(
            &[primary, fallback],
            "cohere-b-rerank-english-v3.0",
            &provider,
            "rerank-english-v3.0",
            "rerank-english-v3.0",
        ) {
            Ok(selected) => selected,
            Err(error) => {
                panic!("selected deployment id should identify the fallback config: {error}")
            }
        };

        assert_eq!(selected.provider_name, "cohere-b");
        assert_eq!(selected.kind, RerankProviderKind::Cohere);
    }
}
