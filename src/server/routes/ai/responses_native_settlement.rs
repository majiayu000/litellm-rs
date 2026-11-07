use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn settle(
    state: &AppState,
    context: &RequestContext,
    provider: &str,
    model: &str,
    pricing: spend::RequestPricing,
    usage: Option<&Usage>,
    pricing_usage: Option<crate::core::pricing_service::PricingUsage>,
    reservation: Option<UnifiedBudgetReservation>,
    key_reservation: Option<BudgetReservation>,
    facts: Option<SharedRequestLedgerFacts>,
) {
    let captured_facts = facts.clone();
    let work = async move {
        let budgeted = &state.budgeted;
        let limits = budgeted.budget_limits();
        let keys = budgeted.key_manager();
        if pricing_usage.is_none() {
            spend::capture_ledger_settlement(facts.as_ref(), provider, model, usage, None);
            // Preserve the budget upper bound without presenting it as an actual bill.
            if let Some(reservation) = key_reservation {
                let reserved = reservation.reserved_amount();
                spend::settle_api_key_budget_reservation(
                    Some(reservation),
                    reserved,
                    "Responses usage unknown",
                )
                .await;
            }
            if let Some(usage) = usage {
                super::super::execution::completion::observe_usage(u64::from(usage.total_tokens));
            }
            let budget_settlement = async {
                if let Some(reservation) = reservation {
                    let reserved = reservation.reserved_amount();
                    if let Err(error) =
                        super::super::execution::completion::settle_budget(reservation, reserved)
                            .await
                    {
                        tracing::error!(%provider, %model, ?error, "failed to retain unknown Responses budget");
                    }
                }
            };
            let usage_record = async {
                if let Some(key_id) = context.api_key_id()
                    && let Err(error) = keys
                        .record_usage_record(
                            key_id,
                            crate::core::keys::UsageRecord::unpriced(
                                usage.map_or(0, |usage| u64::from(usage.total_tokens)),
                                0.0,
                                "responses_usage_unknown",
                            ),
                        )
                        .await
                {
                    tracing::error!(%key_id, %error, "failed to record unknown Responses usage");
                }
            };
            tokio::join!(budget_settlement, usage_record);
            return;
        }
        let settlement = spend::usage_spend_settlement_with_request_pricing(
            (&limits, &keys, context.api_key_id()),
            (provider, model, usage),
            pricing,
            reservation,
            key_reservation,
        )
        .with_pricing_usage(pricing_usage)
        .with_ledger_facts(facts);
        spend::record_completion_spend_with_reservation_with_policy(
            &budgeted.pricing(),
            &state.config().gateway.pricing,
            settlement,
        )
        .await;
    };
    match captured_facts {
        Some(facts) => crate::core::request_ledger::scope_facts(facts, work).await,
        None => work.await,
    }
}
