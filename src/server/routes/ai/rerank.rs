//! Rerank endpoint.

use crate::config::models::provider::ProviderConfig;
use crate::core::providers::{Provider, ProviderError};
use crate::core::rerank::{RerankProvider, RerankRequest, RerankResponse};
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use std::sync::Arc;
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
    mut request: RerankRequest,
) -> Result<RerankResponse, GatewayError> {
    let requested_model = state.unified_router.resolve_model_name(&request.model);
    request.model = requested_model.clone();
    let config = state.config();
    ensure_rerank_provider_candidate_configured(
        config.gateway.providers.as_slice(),
        &requested_model,
    )?;
    let router_models = rerank_router_models(config.gateway.providers.as_slice(), &requested_model);
    drop(config);

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
                move |selected_provider, _selected_model, _selected_deployment_id| {
                    let request = request.clone();
                    let requested_model = requested_model.clone();
                    let budgeted = budgeted.clone();
                    async move {
                        let runtime = selected_rerank_runtime(&selected_provider)?;
                        let served_model = served_rerank_model(&requested_model);
                        let budget_provider = runtime.provider_name().to_string();
                        budgeted
                            .for_selected(budget_provider, served_model.to_string())
                            .with_settlement_mode(SettlementMode::AvailabilityOnly)
                            .reserve_call_settle(
                                |_budget| Ok(None),
                                || async move {
                                    runtime
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

fn selected_rerank_runtime(provider: &Provider) -> Result<Arc<dyn RerankProvider>, ProviderError> {
    provider.rerank_adapter()
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

#[cfg(test)]
fn selected_rerank_provider_from_config(
    provider: &ProviderConfig,
    kind: RerankProviderKind,
) -> Result<SelectedRerankProvider, GatewayError> {
    if kind != RerankProviderKind::Oci && provider.api_key.trim().is_empty() {
        return Err(GatewayError::Config(format!(
            "Rerank provider '{}' is missing api_key",
            provider.name
        )));
    }

    Ok(SelectedRerankProvider {
        provider_name: provider.name.clone(),
        kind,
    })
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
    if provider_type == "watsonx" || provider_name == "watsonx" {
        return Some(RerankProviderKind::Watsonx);
    }
    if provider_type == "oci" || provider_name == "oci" {
        return Some(RerankProviderKind::Oci);
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
        "No configured rerank provider found; configure cohere, jina, watsonx, or OCI native retrieval".to_string(),
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
    Watsonx,
    Oci,
}

impl RerankProviderKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cohere => "cohere",
            Self::Jina => "jina",
            Self::Watsonx => "watsonx",
            Self::Oci => "oci",
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
            Self::Watsonx | Self::Oci => false,
        }
    }
}

#[cfg(test)]
#[derive(Debug)]
struct SelectedRerankProvider {
    provider_name: String,
    kind: RerankProviderKind,
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

    #[tokio::test]
    async fn selected_rerank_runtime_comes_from_typed_provider_snapshot() {
        let provider = crate::core::providers::factory::create_provider(ProviderConfig {
            name: "watsonx".to_string(),
            provider_type: "watsonx".to_string(),
            project: Some("runtime-project".to_string()),
            models: vec!["ibm-rerank".to_string()],
            settings: serde_json::from_value(serde_json::json!({
                "access_token": "runtime-access-token",
                "region": "us-south"
            }))
            .expect("settings object"),
            ..Default::default()
        })
        .await
        .expect("typed watsonx runtime");

        let runtime = selected_rerank_runtime(&provider)
            .expect("typed selected provider should expose its rerank runtime");
        assert_eq!(runtime.provider_name(), "watsonx");
        assert!(runtime.supports_model("ibm-rerank"));
    }

    #[test]
    fn detects_provider_kind_from_type_or_name() {
        let by_type = provider_config("primary", "cohere_rerank", Vec::new());
        let by_name = provider_config("jina-reranker", "custom", Vec::new());
        let unsupported = provider_config("voyage", "voyage", Vec::new());
        let watsonx = provider_config("primary", "watsonx", vec!["ibm-rerank"]);
        let oci = provider_config("oci", "oci", vec!["cohere.rerank-v3-5"]);

        assert_eq!(
            rerank_provider_kind(&by_type),
            Some(RerankProviderKind::Cohere)
        );
        assert_eq!(
            rerank_provider_kind(&by_name),
            Some(RerankProviderKind::Jina)
        );
        assert_eq!(rerank_provider_kind(&unsupported), None);
        assert_eq!(
            rerank_provider_kind(&watsonx),
            Some(RerankProviderKind::Watsonx)
        );
        assert_eq!(rerank_provider_kind(&oci), Some(RerankProviderKind::Oci));
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
}
