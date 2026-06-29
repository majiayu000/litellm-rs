//! Rerank endpoint.

use crate::config::models::provider::ProviderConfig;
use crate::core::rerank::{
    CohereRerankProvider, JinaRerankProvider, RerankRequest, RerankResponse, RerankService,
};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

use super::{openai_errors, provider_config};

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
    let selected = select_rerank_provider(state.config().gateway.providers.as_slice(), &request)?;
    let served_model = served_rerank_model(&request.model);

    super::spend::ensure_budget_available(
        &state.budget_limits,
        &selected.provider_name,
        served_model,
    )?;

    let service = build_rerank_service(&selected)?;
    service.rerank(request).await
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
            let mut rerank_provider = CohereRerankProvider::new(provider.api_key.clone());
            if let Some(base_url) = provider.base_url.as_deref() {
                rerank_provider = rerank_provider.with_base_url(base_url.trim_end_matches('/'));
            }
            service.register_provider("cohere", Arc::new(rerank_provider));
        }
        RerankProviderKind::Jina => {
            let mut rerank_provider = JinaRerankProvider::new(provider.api_key.clone());
            if let Some(base_url) = provider.base_url.as_deref() {
                rerank_provider = rerank_provider.with_base_url(base_url.trim_end_matches('/'));
            }
            service.register_provider("jina", Arc::new(rerank_provider));
        }
    }

    Ok(service)
}

fn select_rerank_provider(
    providers: &[ProviderConfig],
    request: &RerankRequest,
) -> Result<SelectedRerankProvider, GatewayError> {
    let candidates = providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter_map(|provider| rerank_provider_kind(provider).map(|kind| (provider, kind)))
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return Err(GatewayError::NotFound(
            "No configured rerank provider found; configure a cohere or jina provider".to_string(),
        ));
    }

    let Some((provider, kind)) = candidates
        .into_iter()
        .find(|(provider, kind)| rerank_provider_supports_model(provider, *kind, &request.model))
    else {
        return Err(GatewayError::NotFound(format!(
            "No configured rerank provider supports model '{}'",
            request.model
        )));
    };

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
    })
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

fn served_rerank_model(model: &str) -> &str {
    split_rerank_model(model).1
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
}
