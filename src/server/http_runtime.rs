//! Runtime pricing and routing construction for the HTTP server.

use crate::config::Config;
use crate::core::pricing_service::PricingService;
use crate::core::router::UnifiedRouter;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::sync::Arc;
use tracing::{error, info};

pub(super) async fn initialize_pricing(config: &Config) -> Result<Arc<PricingService>> {
    let pricing = Arc::new(PricingService::new(config.gateway.pricing.source.clone()));
    if let Err(error) = pricing.initialize().await {
        // A `None` pricing source is disabled. A configured source failure is
        // fatal unless the operator explicitly permits degraded startup.
        let is_configured = config.gateway.pricing.source.is_some();
        if !is_configured || config.gateway.pricing.allow_degraded {
            error!(
                "Pricing service initial load failed; gateway will serve traffic without pricing \
                 data (configured={}, allow_degraded={}). Error: {}",
                is_configured, config.gateway.pricing.allow_degraded, error
            );
        } else {
            return Err(GatewayError::Config(format!(
                "Pricing service initial load failed and pricing.allow_degraded=false: {error}"
            )));
        }
    } else {
        info!("Pricing service initial load completed");
    }
    info!("Pricing auto-refresh task is managed by on-demand refresh checks");
    Ok(pricing)
}

pub(super) async fn build_router_from_config(
    config: &Config,
    pricing: Arc<PricingService>,
) -> Result<UnifiedRouter> {
    let router_config = crate::core::router::gateway_config::runtime_router_config_from_gateway(
        &config.gateway.router,
    )
    .map_err(|error| GatewayError::Config(format!("Invalid router config: {error}")))?;
    UnifiedRouter::from_gateway_config_with_aliases_and_pricing(
        &config.gateway.providers,
        Some(router_config),
        &config.gateway.model_aliases,
        pricing,
    )
    .await
    .map_err(|error| {
        GatewayError::Config(format!(
            "Failed to initialize unified router from config: {error}"
        ))
    })
}
