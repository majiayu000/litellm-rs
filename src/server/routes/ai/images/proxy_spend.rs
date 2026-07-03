use crate::config::models::gateway::{GatewayPricingConfig, UnpricedModelPolicy};
use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use tracing::error;

use super::ImageProxyProvider;

#[allow(clippy::too_many_arguments)]
pub(super) async fn record_image_proxy_spend(
    state: &AppState,
    provider: &ImageProxyProvider,
    model: &str,
    usage: &PricingUsage,
    cost: f64,
    unpriced: bool,
    budget_reservation: Option<UnifiedBudgetReservation>,
    api_key_id: Option<uuid::Uuid>,
    key_budget_reservation: Option<BudgetReservation>,
) {
    if unpriced {
        let config = state.config();
        super::super::spend::settle_unpriced_usage(
            &config.gateway.pricing,
            &state.budget_limits,
            &state.key_manager,
            api_key_id,
            &provider.provider_name,
            model,
            usage,
            budget_reservation,
            key_budget_reservation,
            "image proxy pricing unavailable",
        )
        .await;
        return;
    }

    if let Some(reservation) = budget_reservation {
        if let Err(error) = reservation.settle(cost) {
            error!(
                "failed to settle image proxy budget for provider '{}' model '{}': {error:?}",
                provider.provider_name, model
            );
        }
    } else {
        state
            .budget_limits
            .record_spend(&provider.provider_name, model, cost);
    }
    if let Some(api_key_id) = api_key_id {
        let total_tokens = u64::from(
            usage
                .total_tokens
                .saturating_add(usage.image_tokens.unwrap_or(0)),
        );
        if let Err(error) = state
            .key_manager
            .record_usage(api_key_id, total_tokens, cost)
            .await
        {
            error!("failed to record image proxy usage for key {api_key_id}: {error}");
        }
    }
    super::super::spend::settle_api_key_budget_reservation(
        key_budget_reservation,
        cost,
        "image proxy",
    );
}

pub(super) fn image_proxy_cost(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    provider: &str,
    model: &str,
    usage: &PricingUsage,
) -> Result<(f64, bool), GatewayError> {
    match pricing_service.calculate_loaded_usage_cost_for_provider(provider, model, usage) {
        Ok(breakdown) => Ok((breakdown.total_cost, false)),
        Err(error) => match pricing_config.unpriced_model_policy {
            UnpricedModelPolicy::Reject => Err(GatewayError::Provider(
                super::super::spend::model_not_priced_error(provider, model, error),
            )),
            UnpricedModelPolicy::AllowUnpriced => Ok((
                super::super::spend::fallback_cost_for_usage(pricing_config, usage),
                true,
            )),
        },
    }
}
