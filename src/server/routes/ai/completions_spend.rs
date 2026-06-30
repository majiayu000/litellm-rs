use crate::core::budget::{BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::KeyManager;
use crate::core::models::openai::Usage;
use crate::core::pricing_service::PricingService;

use super::super::spend;

#[allow(clippy::too_many_arguments)]
pub(super) async fn settle_stream_spend(
    pricing_service: &PricingService,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<uuid::Uuid>,
    provider: &str,
    model: &str,
    usage: Option<&Usage>,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    saw_upstream_output: bool,
) {
    spend::record_finished_stream_spend_with_reservation_with_pricing(
        pricing_service,
        spend::StreamSpendSettlement {
            budget_limits,
            key_manager,
            api_key_id,
            provider,
            model,
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
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<uuid::Uuid>,
    provider: &str,
    model: &str,
    usage: Option<&Usage>,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    saw_upstream_output: bool,
) {
    if usage.is_some() || saw_upstream_output {
        settle_stream_spend(
            pricing_service,
            budget_limits,
            key_manager,
            api_key_id,
            provider,
            model,
            usage,
            budget_reservation,
            key_budget_reservation,
            saw_upstream_output,
        )
        .await;
    }
}
