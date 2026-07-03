use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::KeyManager;
use crate::core::models::openai::Usage;
use crate::core::pricing_service::PricingService;

use super::super::spend;

#[allow(clippy::too_many_arguments)]
pub(super) async fn settle_stream_spend(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<uuid::Uuid>,
    provider: &str,
    model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    usage: Option<&Usage>,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    saw_upstream_output: bool,
) {
    spend::record_finished_stream_spend_with_reservation_with_policy(
        pricing_service,
        pricing_config,
        spend::StreamSpendSettlement {
            budget_limits,
            key_manager,
            api_key_id,
            provider,
            model,
            pricing_provider,
            pricing_model,
            usage,
            saw_upstream_output,
            budget_reservation,
            key_budget_reservation,
        },
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn settle_stream_spend_if_chargeable(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<uuid::Uuid>,
    provider: &str,
    model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    usage: Option<&Usage>,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    saw_upstream_output: bool,
) {
    if usage.is_some() || saw_upstream_output {
        settle_stream_spend(
            pricing_service,
            pricing_config,
            budget_limits,
            key_manager,
            api_key_id,
            provider,
            model,
            pricing_provider,
            pricing_model,
            usage,
            budget_reservation,
            key_budget_reservation,
            saw_upstream_output,
        )
        .await;
    }
}
