//! Model listing and retrieval endpoints

use crate::core::models::openai::{Model, ModelListResponse};
use crate::core::router::UnifiedRouter;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpResponse, Result as ActixResult, web};
use std::collections::BTreeSet;
use tracing::{debug, error};

use super::openai_errors;

/// List available models
///
/// Returns a list of available AI models across all configured providers.
pub async fn list_models(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    debug!("Listing available models");

    let unified_router = state.unified_router();

    match get_models_from_router(&unified_router).await {
        Ok(models) => {
            let response = ModelListResponse {
                object: "list".to_string(),
                data: models,
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            error!("Failed to list models: {}", e);
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}

/// Get specific model information
///
/// Returns detailed information about a specific model.
pub async fn get_model(
    state: web::Data<AppState>,
    model_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    debug!("Getting model info for: {}", model_id);

    let unified_router = state.unified_router();

    match get_model_from_router(&unified_router, &model_id).await {
        Ok(Some(model)) => Ok(HttpResponse::Ok().json(model)),
        Ok(None) => Ok(openai_errors::gateway_error_response(
            &GatewayError::not_found(format!("Model not found: {}", model_id)),
        )),
        Err(e) => {
            error!("Failed to get model {}: {}", model_id, e);
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}

/// Get all models from UnifiedRouter
pub async fn get_models_from_router(router: &UnifiedRouter) -> Result<Vec<Model>, GatewayError> {
    let mut models = Vec::new();
    let model_names = router
        .list_models()
        .into_iter()
        .chain(router.model_aliases().into_keys())
        .collect::<BTreeSet<_>>();

    for model_name in model_names {
        let Some(owned_by) = router
            .get_deployments_for_model(&model_name)
            .into_iter()
            .find_map(|deployment_id| {
                router
                    .get_deployment(&deployment_id)
                    .map(|deployment| deployment.provider.name().to_string())
            })
        else {
            continue;
        };

        models.push(Model {
            id: model_name,
            object: "model".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            owned_by,
        });
    }

    Ok(models)
}

/// Get specific model from UnifiedRouter
pub async fn get_model_from_router(
    router: &UnifiedRouter,
    model_id: &str,
) -> Result<Option<Model>, GatewayError> {
    let deployment_ids = router.get_deployments_for_model(model_id);
    if let Some(owner) = deployment_ids.into_iter().find_map(|deployment_id| {
        router
            .get_deployment(&deployment_id)
            .map(|deployment| deployment.provider.name().to_string())
    }) {
        return Ok(Some(Model {
            id: model_id.to_string(),
            object: "model".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            owned_by: owner,
        }));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::provider::ProviderConfig;
    use std::collections::HashMap;

    #[tokio::test]
    async fn model_inventory_contains_sorted_unique_aliases_and_canonical_models() {
        let provider = ProviderConfig {
            name: "primary".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["gpt-4o".to_string()],
            ..ProviderConfig::default()
        };
        let aliases = HashMap::from([
            ("stable-chat".to_string(), "gpt-4o".to_string()),
            ("chat".to_string(), "stable-chat".to_string()),
        ]);
        let router = UnifiedRouter::from_gateway_config_with_aliases(&[provider], None, &aliases)
            .await
            .expect("inventory fixture router should build");

        let models = get_models_from_router(&router)
            .await
            .expect("model inventory should build");
        assert_eq!(
            models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            vec!["chat", "gpt-4o", "stable-chat"]
        );
        assert!(models.iter().all(|model| model.owned_by == "primary"));

        let alias = get_model_from_router(&router, "chat")
            .await
            .expect("alias lookup should succeed")
            .expect("configured alias should be discoverable");
        assert_eq!(alias.id, "chat");
        assert_eq!(alias.owned_by, "primary");
    }

    #[tokio::test]
    async fn model_inventory_excludes_runtime_aliases_without_live_deployments() {
        let provider = ProviderConfig {
            name: "primary".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["gpt-4o".to_string()],
            ..ProviderConfig::default()
        };
        let router = UnifiedRouter::from_gateway_config(&[provider], None)
            .await
            .expect("inventory fixture router should build");

        router
            .add_model_alias("missing", "not-deployed")
            .expect("runtime alias shape should be valid");
        router
            .add_model_alias("live", "gpt-4o")
            .expect("live runtime alias should install");

        let models = get_models_from_router(&router)
            .await
            .expect("model inventory should build");
        assert_eq!(
            models
                .iter()
                .map(|model| (model.id.as_str(), model.owned_by.as_str()))
                .collect::<Vec<_>>(),
            vec![("gpt-4o", "primary"), ("live", "primary")]
        );
        assert!(
            get_model_from_router(&router, "missing")
                .await
                .expect("missing alias lookup should complete")
                .is_none()
        );

        router
            .remove_deployment("primary-gpt-4o")
            .expect("live deployment should be removable");
        assert!(
            get_models_from_router(&router)
                .await
                .expect("post-removal inventory should build")
                .is_empty()
        );
        assert!(
            get_model_from_router(&router, "live")
                .await
                .expect("removed alias lookup should complete")
                .is_none()
        );
    }
}
