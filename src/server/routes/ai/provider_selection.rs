//! Provider selection helpers for AI routes

use crate::core::providers::Provider;
use crate::core::router::UnifiedRouter;
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;
use std::borrow::Cow;

pub struct ProviderSelection<'a> {
    pub provider: Cow<'a, Provider>,
    pub model: String,
}

pub fn select_provider_for_model<'a>(
    router: &UnifiedRouter,
    model: &str,
    capability: ProviderCapability,
) -> Result<ProviderSelection<'a>, GatewayError> {
    if model.trim().is_empty() {
        return Err(GatewayError::validation("Model is required"));
    }

    select_provider_from_unified_router(router, model, capability)
}

pub fn select_provider_for_optional_model<'a>(
    router: &UnifiedRouter,
    model: Option<&str>,
    capability: ProviderCapability,
) -> Result<(Cow<'a, Provider>, String), GatewayError> {
    let model = model.ok_or_else(|| GatewayError::validation("Model is required"))?;
    let selection = select_provider_for_model(router, model, capability)?;
    Ok((selection.provider, selection.model))
}

fn select_provider_from_unified_router<'a>(
    router: &UnifiedRouter,
    model: &str,
    capability: ProviderCapability,
) -> Result<ProviderSelection<'a>, GatewayError> {
    let deployment = router
        .select_capability_deployment(model, &capability)
        .ok_or_else(|| {
            GatewayError::validation(format!(
                "Model '{}' does not support {:?}",
                model, capability
            ))
        })?;

    Ok(ProviderSelection {
        provider: Cow::Owned(deployment.provider),
        model: deployment.model,
    })
}
