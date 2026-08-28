use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::KeyManager;
use crate::core::pricing_service::PricingUsage;
use tracing::error;

use super::ImageProxyProvider;

#[allow(clippy::too_many_arguments)]
pub(super) async fn record_image_proxy_spend(
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
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
        super::super::spend::settle_unpriced_usage(
            pricing_config,
            budget_limits,
            key_manager,
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
        budget_limits.record_spend(&provider.provider_name, model, cost);
    }
    if let Some(api_key_id) = api_key_id {
        let total_tokens = u64::from(
            usage
                .total_tokens
                .saturating_add(usage.image_tokens.unwrap_or(0)),
        );
        if let Err(error) = key_manager
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
