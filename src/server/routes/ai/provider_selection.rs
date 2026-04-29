//! Provider selection helpers for AI routes

use crate::core::router::UnifiedRouter;
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;

pub fn select_provider_for_model(
    router: &UnifiedRouter,
    model: &str,
    capability: ProviderCapability,
) -> Result<String, GatewayError> {
    if model.trim().is_empty() {
        return Err(GatewayError::validation("Model is required"));
    }

    let deployment = router
        .select_capability_deployment(model, &capability)
        .ok_or_else(|| {
            GatewayError::validation(format!(
                "Model '{}' does not support {:?}",
                model, capability
            ))
        })?;

    Ok(deployment.model)
}

pub fn select_provider_for_optional_model(
    router: &UnifiedRouter,
    model: Option<&str>,
    capability: ProviderCapability,
) -> Result<String, GatewayError> {
    let model = model.ok_or_else(|| GatewayError::validation("Model is required"))?;
    select_provider_for_model(router, model, capability)
}
