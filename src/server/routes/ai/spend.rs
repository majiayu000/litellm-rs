//! Spend and usage recording for completed requests.
//!
//! Wires the otherwise-dead budget and per-key usage tracking into the request
//! path: once a completion succeeds and its token usage is known, the served
//! provider/model budget spend and the calling key's usage are recorded.

mod completion;
mod key_budget;
mod pricing;
mod unpriced;

use uuid::Uuid;

use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{
    BudgetReservation, BudgetReservationError, UnifiedBudgetLimits, UnifiedBudgetReservation,
};
use crate::core::keys::KeyManager;
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::request_ledger::{SharedRequestLedgerFacts, apply_settlement, current_facts};
use crate::core::types::responses::{ChatChunk, Usage};
#[cfg(test)]
use std::sync::LazyLock;

pub(super) use completion::{
    ChatCompletionBudgetRequest, reserve_chat_completion_budget_with_request_pricing,
    try_estimate_chat_prompt_tokens,
};
#[cfg(test)]
pub(super) use completion::{
    IMAGE_HIGH_DETAIL_PROMPT_TOKENS, catalog_max_output_tokens, estimate_chat_prompt_tokens,
    provider_effective_max_output_tokens, reserve_chat_completion_budget,
    reserve_completion_budget, reserve_completion_budget_with_policy,
    reserve_completion_budget_with_pricing,
};
pub(in crate::server::routes::ai) use key_budget::{
    reserve_api_key_budget, reserve_api_key_budget_for_reservation,
    settle_api_key_budget_reservation,
};
pub(super) use pricing::{
    RequestPricing, estimate_embedding_input_tokens,
    record_pricing_usage_spend_with_request_pricing, request_pricing_for_provider,
    reserve_embedding_budget_with_request_pricing,
    reserve_pricing_usage_budget_with_request_pricing,
};
pub(super) use unpriced::{
    fallback_cost_for_usage, is_model_not_priced_error, model_not_priced_error,
    reserve_unpriced_usage_budget, settle_unpriced_usage,
};

pub(super) fn stream_chunk_has_candidate_output(chunk: &ChatChunk) -> bool {
    !chunk.choices.is_empty()
}

pub(super) fn token_count_error(
    provider: &str,
    model: &str,
    identity: &crate::utils::ai::counter::token_counter::TokenizerIdentity,
    error: impl std::fmt::Display,
) -> ProviderError {
    ProviderError::invalid_request(
        "token_count",
        format!(
            "token counting failed for selected deployment '{provider}/{model}' using token identity '{}/{}': {error}",
            identity.provider(),
            identity.model()
        ),
    )
}

/// Reject a request before it reaches the upstream provider when the served
/// provider or model budget is already exhausted.
///
/// No-ops when budgets are disabled or unconfigured (the availability checks
/// return true). Returns a non-retryable `QuotaExceeded` error (HTTP 402) so
/// the router does not pointlessly retry an over-budget request.
pub(super) fn ensure_budget_available(
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
) -> Result<(), ProviderError> {
    if !budget_limits.is_provider_available(provider) {
        return Err(ProviderError::quota_exceeded(
            "budget",
            format!("provider '{provider}' budget exceeded"),
        ));
    }
    if !budget_limits.is_model_available(model) {
        return Err(ProviderError::quota_exceeded(
            "budget",
            format!("model '{model}' budget exceeded"),
        ));
    }
    Ok(())
}

pub(in crate::server::routes::ai) fn reservation_error_to_provider_error(
    error: BudgetReservationError,
    provider: &str,
    model: &str,
) -> ProviderError {
    match error {
        BudgetReservationError::ProviderBudgetExceeded => ProviderError::quota_exceeded(
            "budget",
            format!("provider '{provider}' budget exceeded"),
        ),
        BudgetReservationError::ModelBudgetExceeded => {
            ProviderError::quota_exceeded("budget", format!("model '{model}' budget exceeded"))
        }
        BudgetReservationError::BudgetExceeded => ProviderError::quota_exceeded(
            "budget",
            format!("budget exceeded for provider '{provider}' model '{model}'"),
        ),
        BudgetReservationError::InvalidAmount(error) => ProviderError::invalid_request(
            "budget",
            format!("invalid budget reservation amount for '{provider}'/'{model}': {error}"),
        ),
        BudgetReservationError::ActualExceedsReservation => ProviderError::invalid_request(
            "budget",
            format!("actual spend exceeded reserved budget for '{provider}'/'{model}'"),
        ),
        BudgetReservationError::BackendUnavailable => ProviderError::provider_unavailable(
            "budget",
            format!("budget backend unavailable for '{provider}'/'{model}'"),
        ),
    }
}

/// Record provider/model budget spend and per-key usage for a completed request.
///
/// Best-effort and non-fatal: the completion already succeeded, so failures here
/// are logged at error level (never silently swallowed) but do not fail the
/// response. When the cost cannot be priced, token usage is still recorded but
/// budget spend is skipped rather than booked at $0 — under-counting a budget is
/// worse than leaving it unchanged with a loud error.
pub(super) struct UsageSpendSettlement<'a> {
    pub(super) budget_limits: &'a UnifiedBudgetLimits,
    pub(super) key_manager: &'a KeyManager,
    pub(super) api_key_id: Option<Uuid>,
    pub(super) provider: &'a str,
    pub(super) model: &'a str,
    pub(super) pricing_provider: &'a str,
    pub(super) pricing_model: &'a str,
    pub(super) request_pricing: Option<RequestPricing>,
    pub(super) usage: Option<&'a Usage>,
    pub(super) budget_reservation: Option<UnifiedBudgetReservation>,
    pub(super) key_budget_reservation: Option<BudgetReservation>,
    pub(super) ledger_facts: Option<SharedRequestLedgerFacts>,
}

pub(super) fn usage_spend_settlement<'a>(
    core: (&'a UnifiedBudgetLimits, &'a KeyManager, Option<Uuid>),
    usage: (&'a str, &'a str, Option<&'a Usage>),
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) -> UsageSpendSettlement<'a> {
    let (provider, model, usage) = usage;
    usage_spend_settlement_with_pricing(
        core,
        (provider, model, usage),
        (provider, model),
        budget_reservation,
        key_budget_reservation,
    )
}

pub(super) fn usage_spend_settlement_with_pricing<'a>(
    core: (&'a UnifiedBudgetLimits, &'a KeyManager, Option<Uuid>),
    usage: (&'a str, &'a str, Option<&'a Usage>),
    pricing: (&'a str, &'a str),
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) -> UsageSpendSettlement<'a> {
    let (budget_limits, key_manager, api_key_id) = core;
    let (provider, model, usage) = usage;
    let (pricing_provider, pricing_model) = pricing;
    UsageSpendSettlement {
        budget_limits,
        key_manager,
        api_key_id,
        provider,
        model,
        pricing_provider,
        pricing_model,
        request_pricing: None,
        usage,
        budget_reservation,
        key_budget_reservation,
        ledger_facts: current_facts(),
    }
}

impl UsageSpendSettlement<'_> {
    pub(super) fn with_ledger_facts(mut self, facts: Option<SharedRequestLedgerFacts>) -> Self {
        if facts.is_some() {
            self.ledger_facts = facts;
        }
        self
    }
}

fn capture_ledger_settlement(
    facts: Option<&SharedRequestLedgerFacts>,
    provider: &str,
    model: &str,
    usage: Option<&Usage>,
    cost: Option<f64>,
) {
    let prompt_tokens = usage.map(|usage| i64::from(usage.prompt_tokens));
    let completion_tokens = usage.map(|usage| i64::from(usage.completion_tokens));
    let total_tokens = usage.map(|usage| i64::from(usage.total_tokens));
    match facts {
        Some(facts) => apply_settlement(
            facts,
            provider,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cost,
        ),
        None => crate::core::request_ledger::record_current_settlement(
            provider,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cost,
        ),
    }
}

pub(super) fn usage_spend_settlement_with_request_pricing<'a>(
    core: (&'a UnifiedBudgetLimits, &'a KeyManager, Option<Uuid>),
    usage: (&'a str, &'a str, Option<&'a Usage>),
    request_pricing: RequestPricing,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) -> UsageSpendSettlement<'a> {
    let mut settlement =
        usage_spend_settlement(core, usage, budget_reservation, key_budget_reservation);
    settlement.request_pricing = Some(request_pricing);
    settlement
}

#[cfg(test)]
pub(super) async fn record_completion_spend_with_reservation(settlement: UsageSpendSettlement<'_>) {
    record_completion_spend_with_reservation_with_pricing(
        default_spend_pricing_service(),
        settlement,
    )
    .await;
}

#[cfg(test)]
pub(super) async fn record_completion_spend_with_reservation_with_pricing(
    pricing_service: &PricingService,
    settlement: UsageSpendSettlement<'_>,
) {
    let pricing_config = GatewayPricingConfig::default();
    record_completion_spend_with_reservation_with_policy(
        pricing_service,
        &pricing_config,
        settlement,
    )
    .await;
}

pub(super) async fn record_completion_spend_with_reservation_with_policy(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    settlement: UsageSpendSettlement<'_>,
) {
    let UsageSpendSettlement {
        budget_limits,
        key_manager,
        api_key_id,
        provider,
        model,
        pricing_provider,
        pricing_model,
        request_pricing,
        usage,
        budget_reservation,
        key_budget_reservation,
        ledger_facts,
    } = settlement;

    let Some(usage) = usage else {
        capture_ledger_settlement(ledger_facts.as_ref(), provider, model, None, None);
        record_reserved_spend_without_usage(
            key_manager,
            api_key_id,
            provider,
            model,
            budget_reservation,
            key_budget_reservation,
            "provider returned no usage for a successful completion",
        )
        .await;
        return;
    };

    let total_tokens = u64::from(usage.total_tokens);
    let usage_tokens = PricingUsage::from(usage);

    let priced = match request_pricing.as_ref() {
        Some(request_pricing) => request_pricing.calculate_settlement(&usage_tokens),
        None => pricing_service.calculate_loaded_settlement_cost_for_provider(
            pricing_provider,
            pricing_model,
            &usage_tokens,
        ),
    };
    let cost = match priced {
        Ok(breakdown) => breakdown.total_cost,
        Err(e) => {
            tracing::error!(
                "cost calculation failed for pricing provider '{pricing_provider}' model \
                 '{pricing_model}' budget provider '{provider}' model '{model}': {e}; \
                 settling through unpriced model policy"
            );
            unpriced::settle_unpriced_usage(
                pricing_config,
                budget_limits,
                key_manager,
                api_key_id,
                provider,
                model,
                &usage_tokens,
                budget_reservation,
                key_budget_reservation,
                "completion spend pricing unavailable",
            )
            .await;
            capture_ledger_settlement(ledger_facts.as_ref(), provider, model, Some(usage), None);
            return;
        }
    };

    capture_ledger_settlement(
        ledger_facts.as_ref(),
        provider,
        model,
        Some(usage),
        Some(cost),
    );

    if let Some(reservation) = budget_reservation {
        if let Err(error) = reservation.settle(cost) {
            tracing::error!(
                "failed to settle reserved budget for '{provider}'/'{model}': {error:?}; \
                 spend not recorded because reservation settlement failed"
            );
        }
    } else {
        budget_limits.record_spend(provider, model, cost);
    }
    settle_api_key_budget_reservation(key_budget_reservation, cost, &format!("{provider}/{model}"));

    if let Some(key_id) = api_key_id
        && let Err(e) = key_manager.record_usage(key_id, total_tokens, cost).await
    {
        tracing::error!("failed to record usage for key {key_id}: {e}");
    }
}

pub(in crate::server::routes::ai) async fn record_reserved_spend_without_usage(
    key_manager: &KeyManager,
    api_key_id: Option<Uuid>,
    provider: &str,
    model: &str,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    context: &str,
) {
    let provider_reserved = budget_reservation
        .as_ref()
        .map(UnifiedBudgetReservation::reserved_amount);
    let key_reserved = key_budget_reservation
        .as_ref()
        .map(BudgetReservation::reserved_amount);
    let recorded_cost = key_reserved
        .filter(|amount| *amount > 0.0)
        .or_else(|| provider_reserved.filter(|amount| *amount > 0.0));
    if let Some(api_key_usage_fallback_cost) = recorded_cost {
        tracing::error!(
            event = "billing_no_usage_reserved_fallback",
            provider = %provider,
            model = %model,
            reason = %context,
            provider_reserved_amount = ?provider_reserved,
            key_reserved_amount = ?key_reserved,
            api_key_usage_fallback_cost,
            api_key_usage_target_present = api_key_id.is_some(),
            "trusted provider usage unavailable; settling reserved spend fallback"
        );
    }
    if let (Some(reservation), Some(reserved)) = (budget_reservation, provider_reserved)
        && let Err(error) = reservation.settle(reserved)
    {
        tracing::error!(
            "failed to settle reserved budget without usage for '{provider}'/'{model}': {error:?}"
        );
    }
    if let Some(reservation) = key_budget_reservation {
        settle_api_key_budget_reservation(Some(reservation), recorded_cost.unwrap_or(0.0), context);
    }
    let Some(recorded_cost) = recorded_cost else {
        tracing::error!(
            "{context} for provider '{provider}' model '{model}'; \
             no positive reserved spend was available, so key usage was not recorded"
        );
        return;
    };
    if let Some(key_id) = api_key_id
        && let Err(error) = key_manager.record_usage(key_id, 0, recorded_cost).await
    {
        tracing::error!("failed to record reserved usage for key {key_id}: {error}");
    }
}

#[cfg(test)]
pub(super) async fn record_stream_disconnect_spend_with_reservation(
    settlement: UsageSpendSettlement<'_>,
) {
    record_stream_disconnect_spend_with_reservation_with_pricing(
        default_spend_pricing_service(),
        settlement,
    )
    .await;
}

#[cfg(test)]
pub(super) async fn record_stream_disconnect_spend_with_reservation_with_pricing(
    pricing_service: &PricingService,
    settlement: UsageSpendSettlement<'_>,
) {
    let pricing_config = GatewayPricingConfig::default();
    record_stream_disconnect_spend_with_reservation_with_policy(
        pricing_service,
        &pricing_config,
        settlement,
    )
    .await;
}

pub(super) async fn record_stream_disconnect_spend_with_reservation_with_policy(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    settlement: UsageSpendSettlement<'_>,
) {
    let UsageSpendSettlement {
        budget_limits,
        key_manager,
        api_key_id,
        provider,
        model,
        pricing_provider,
        pricing_model,
        request_pricing,
        usage,
        budget_reservation,
        key_budget_reservation,
        ledger_facts,
    } = settlement;

    if let Some(usage) = usage {
        let settlement = if let Some(request_pricing) = request_pricing {
            usage_spend_settlement_with_request_pricing(
                (budget_limits, key_manager, api_key_id),
                (provider, model, Some(usage)),
                request_pricing,
                budget_reservation,
                key_budget_reservation,
            )
        } else {
            usage_spend_settlement_with_pricing(
                (budget_limits, key_manager, api_key_id),
                (provider, model, Some(usage)),
                (pricing_provider, pricing_model),
                budget_reservation,
                key_budget_reservation,
            )
        }
        .with_ledger_facts(ledger_facts);
        record_completion_spend_with_reservation_with_policy(
            pricing_service,
            pricing_config,
            settlement,
        )
        .await;
        return;
    }

    capture_ledger_settlement(ledger_facts.as_ref(), provider, model, None, None);
    record_reserved_spend_without_usage(
        key_manager,
        api_key_id,
        provider,
        model,
        budget_reservation,
        key_budget_reservation,
        "client disconnected before provider returned usage",
    )
    .await;
}

pub(super) struct StreamSpendSettlement<'a> {
    pub(super) budget_limits: &'a UnifiedBudgetLimits,
    pub(super) key_manager: &'a KeyManager,
    pub(super) api_key_id: Option<Uuid>,
    pub(super) provider: &'a str,
    pub(super) model: &'a str,
    pub(super) request_pricing: RequestPricing,
    pub(super) usage: Option<&'a Usage>,
    pub(super) saw_upstream_output: bool,
    pub(super) budget_reservation: Option<UnifiedBudgetReservation>,
    pub(super) key_budget_reservation: Option<BudgetReservation>,
}

#[cfg(test)]
pub(super) async fn record_finished_stream_spend_with_reservation(
    settlement: StreamSpendSettlement<'_>,
) {
    record_finished_stream_spend_with_reservation_with_pricing(
        default_spend_pricing_service(),
        settlement,
    )
    .await;
}

#[cfg(test)]
pub(super) async fn record_finished_stream_spend_with_reservation_with_pricing(
    pricing_service: &PricingService,
    settlement: StreamSpendSettlement<'_>,
) {
    let pricing_config = GatewayPricingConfig::default();
    record_finished_stream_spend_with_reservation_with_policy(
        pricing_service,
        &pricing_config,
        settlement,
    )
    .await;
}

pub(super) async fn record_finished_stream_spend_with_reservation_with_policy(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    settlement: StreamSpendSettlement<'_>,
) {
    let StreamSpendSettlement {
        budget_limits,
        key_manager,
        api_key_id,
        provider,
        model,
        request_pricing,
        usage,
        saw_upstream_output,
        budget_reservation,
        key_budget_reservation,
    } = settlement;

    if usage.is_some() || saw_upstream_output {
        record_stream_disconnect_spend_with_reservation_with_policy(
            pricing_service,
            pricing_config,
            usage_spend_settlement_with_request_pricing(
                (budget_limits, key_manager, api_key_id),
                (provider, model, usage),
                request_pricing,
                budget_reservation,
                key_budget_reservation,
            ),
        )
        .await;
        return;
    }

    record_completion_spend_with_reservation_with_policy(
        pricing_service,
        pricing_config,
        usage_spend_settlement_with_request_pricing(
            (budget_limits, key_manager, api_key_id),
            (provider, model, usage),
            request_pricing,
            budget_reservation,
            key_budget_reservation,
        ),
    )
    .await;
}

#[cfg(test)]
fn default_spend_pricing_service() -> &'static PricingService {
    static DEFAULT_SPEND_PRICING_SERVICE: LazyLock<PricingService> = LazyLock::new(|| {
        PricingService::with_embedded_default().unwrap_or_else(|error| {
            tracing::error!("failed to initialize embedded spend PricingService: {error}");
            PricingService::new(None)
        })
    });
    &DEFAULT_SPEND_PRICING_SERVICE
}

#[cfg(test)]
#[path = "spend_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "spend_provider_reservation_tests.rs"]
mod provider_reservation_tests;

#[cfg(test)]
#[path = "spend_provider_output_cap_tests.rs"]
mod provider_output_cap_tests;

#[cfg(test)]
#[path = "spend_stream_disconnect_tests.rs"]
mod stream_disconnect_tests;

#[cfg(test)]
#[path = "spend_no_usage_tests.rs"]
mod no_usage_tests;

#[cfg(test)]
#[path = "spend_runtime_pricing_tests.rs"]
mod runtime_pricing_tests;
