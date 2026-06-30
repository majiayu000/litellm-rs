use std::sync::Arc;

use uuid::Uuid;

use crate::core::budget::{BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::KeyManager;
use crate::core::models::openai::Usage as ChatUsage;
use crate::core::pricing_service::PricingService;

use super::spend;

pub(super) struct StreamBudgetSettlement {
    pub(super) pricing_service: Arc<PricingService>,
    pub(super) budget_limits: Arc<UnifiedBudgetLimits>,
    pub(super) key_manager: KeyManager,
    pub(super) api_key_id: Option<Uuid>,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) reservation: Option<UnifiedBudgetReservation>,
    pub(super) key_budget_reservation: Option<BudgetReservation>,
}

impl StreamBudgetSettlement {
    pub(super) async fn record_completion(
        mut self,
        usage: Option<&ChatUsage>,
        saw_upstream_output: bool,
    ) {
        spend::record_finished_stream_spend_with_reservation_with_pricing(
            self.pricing_service.as_ref(),
            spend::StreamSpendSettlement {
                budget_limits: self.budget_limits.as_ref(),
                key_manager: &self.key_manager,
                api_key_id: self.api_key_id,
                provider: &self.provider,
                model: &self.model,
                usage,
                saw_upstream_output,
                budget_reservation: self.reservation.take(),
                key_budget_reservation: self.key_budget_reservation.take(),
            },
        )
        .await;
    }

    pub(super) async fn record_disconnect(&mut self, usage: Option<&ChatUsage>) {
        spend::record_stream_disconnect_spend_with_reservation_with_pricing(
            self.pricing_service.as_ref(),
            spend::usage_spend_settlement(
                (
                    self.budget_limits.as_ref(),
                    &self.key_manager,
                    self.api_key_id,
                ),
                (&self.provider, &self.model, usage),
                self.reservation.take(),
                self.key_budget_reservation.take(),
            ),
        )
        .await;
    }
}
