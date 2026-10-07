//! Realtime uses modality-specific cache rates absent from chat Usage.
use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::pricing_service::LiteLLMModelInfo;
use crate::core::providers::ProviderError;
use crate::core::request_ledger::{
    RequestBilling, RequestLedgerFacts, SharedRequestLedgerFacts, apply_settlement, current_facts,
    scope_facts, snapshot_facts, update_billing,
};
use crate::server::state::AppState;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct Rates {
    input: [f64; 2],
    cached: [f64; 2],
    output: [f64; 2],
    pub max_input: u32,
    pub max_output: u32,
    pub model_max_output: u32,
}
impl Rates {
    pub fn load(info: &LiteLLMModelInfo, output_limit: Option<u32>) -> Result<Self, String> {
        let rate = |value: Option<f64>| {
            value
                .filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or_else(|| "Realtime requires complete text/audio/cache pricing".to_string())
        };
        let extra = |key: &str| info.extra.get(key).and_then(Value::as_f64);
        let max_input = info
            .max_input_tokens
            .filter(|v| *v > 0)
            .ok_or("Missing realtime input limit")?;
        let max_output = info
            .max_output_tokens
            .filter(|v| *v > 0)
            .ok_or("Missing realtime output limit")?;
        let model_max_output = max_output;
        let max_output = output_limit.map_or(max_output, |limit| limit.min(max_output));
        if max_output == 0 {
            return Err("Realtime output limit must be positive".into());
        }
        Ok(Self {
            input: [
                rate(info.input_cost_per_token)?,
                rate(extra("input_cost_per_audio_token"))?,
            ],
            cached: [
                rate(extra("cache_read_input_token_cost"))?,
                rate(extra("cache_read_input_audio_token_cost"))?,
            ],
            output: [
                rate(info.output_cost_per_token)?,
                rate(extra("output_cost_per_audio_token"))?,
            ],
            max_input,
            max_output,
            model_max_output,
        })
    }
    pub fn wire_output_limit(&self, limit: u32) -> Result<Value, &'static str> {
        if (1..=4096).contains(&limit) {
            Ok(Value::from(limit))
        } else if limit == self.model_max_output {
            Ok(Value::from("inf"))
        } else {
            Err("Realtime output policy cannot be represented by the upstream token limit")
        }
    }
    pub fn bound(&self, max_output: u32) -> f64 {
        self.max_input as f64
            * self
                .input
                .into_iter()
                .chain(self.cached)
                .fold(0.0, f64::max)
            + max_output.min(self.model_max_output) as f64
                * self.output.into_iter().fold(0.0, f64::max)
    }
    pub fn cost(&self, usage: &Value, max_output: u32) -> Result<Option<(f64, u64)>, String> {
        // Optional usage cannot establish exact modality pricing. Known limits
        // still apply before selecting the conservative settlement fallback.
        if usage["input_tokens"]
            .as_u64()
            .is_some_and(|v| v > self.max_input as u64)
            || usage["output_tokens"]
                .as_u64()
                .is_some_and(|v| v > max_output.min(self.model_max_output) as u64)
            || usage["input_token_details"]["image_tokens"]
                .as_u64()
                .is_some_and(|v| v != 0)
        {
            return Err("Realtime usage exceeds reserved text/audio scope".into());
        }
        if usage.is_null() {
            return Ok(None);
        }
        if !usage.is_object() {
            return Err("Invalid realtime usage object".into());
        }
        let missing = |pointer| usage.pointer(pointer).is_none_or(Value::is_null);
        if [
            "/input_tokens",
            "/output_tokens",
            "/total_tokens",
            "/input_token_details/text_tokens",
            "/input_token_details/audio_tokens",
            "/input_token_details/cached_tokens",
            "/output_token_details/text_tokens",
            "/output_token_details/audio_tokens",
        ]
        .into_iter()
        .any(missing)
            || (usage["input_token_details"]["cached_tokens"]
                .as_u64()
                .is_some_and(|v| v > 0)
                && [
                    "/input_token_details/cached_tokens_details/text_tokens",
                    "/input_token_details/cached_tokens_details/audio_tokens",
                ]
                .into_iter()
                .any(missing))
        {
            return Ok(None);
        }
        let count = |v: &Value| {
            v.as_u64()
                .ok_or_else(|| "Missing or invalid realtime usage".to_string())
        };
        let input = &usage["input_token_details"];
        let output = &usage["output_token_details"];
        let cached = &input["cached_tokens_details"];
        let i = [
            count(&input["text_tokens"])?,
            count(&input["audio_tokens"])?,
        ];
        let o = [
            count(&output["text_tokens"])?,
            count(&output["audio_tokens"])?,
        ];
        let c = if count(&input["cached_tokens"])? == 0 {
            [0, 0]
        } else {
            [
                count(&cached["text_tokens"])?,
                count(&cached["audio_tokens"])?,
            ]
        };
        let input_total = count(&usage["input_tokens"])?;
        let output_total = count(&usage["output_tokens"])?;
        let total = count(&usage["total_tokens"])?;
        if i[0].checked_add(i[1]) != Some(input_total)
            || o[0].checked_add(o[1]) != Some(output_total)
            || c[0].checked_add(c[1]) != Some(count(&input["cached_tokens"])?)
            || input_total.checked_add(output_total) != Some(total)
            || c.iter().zip(i).any(|(cached, input)| *cached > input)
        {
            return Err("Realtime usage exceeds reserved text/audio scope".into());
        }
        let cost = (0..2)
            .map(|n| {
                (i[n] - c[n]) as f64 * self.input[n]
                    + c[n] as f64 * self.cached[n]
                    + o[n] as f64 * self.output[n]
            })
            .sum();
        Ok(Some((cost, total)))
    }
}

pub(super) struct Pending {
    provider: Option<UnifiedBudgetReservation>,
    key: Option<BudgetReservation>,
    bound: f64,
    pub max_output: u32,
    facts: Option<SharedRequestLedgerFacts>,
    session_facts: Option<SharedRequestLedgerFacts>,
}
impl Pending {
    pub async fn reserve(
        state: &AppState,
        provider: &str,
        model: &str,
        key_budget: Option<Uuid>,
        bound: f64,
        max_output: u32,
    ) -> Result<Self, ProviderError> {
        let session_facts = current_facts();
        let facts = session_facts
            .as_ref()
            .map(|_| Arc::new(Mutex::new(RequestLedgerFacts::default())));
        let reserve = async {
            let reservation = state
                .budget_limits
                .reserve_spend_async(provider, model, bound)
                .await
                .map_err(|error| {
                    super::spend::reservation_error_to_provider_error(error, provider, model)
                })?;
            let key = super::spend::reserve_api_key_budget(
                &state.budget_manager,
                key_budget,
                Some(bound),
            )?;
            Ok::<_, ProviderError>((reservation, key))
        };
        let (reservation, key) = match &facts {
            Some(facts) => scope_facts(facts.clone(), reserve).await?,
            None => reserve.await?,
        };
        if let (Some(session), Some(facts)) = (&session_facts, &facts) {
            start_response(session, &snapshot_facts(facts));
        }
        Ok(Self {
            provider: Some(reservation),
            key,
            bound,
            max_output,
            facts,
            session_facts,
        })
    }
    pub async fn settle(
        mut self,
        state: &AppState,
        key_id: Option<Uuid>,
        actual: Option<(f64, u64)>,
    ) -> Result<u64, String> {
        let (cost, tokens) = actual.unwrap_or((self.bound, 0));
        if let (Some(facts), Some(reservation)) = (&self.facts, &self.provider) {
            apply_settlement(
                facts,
                reservation.provider(),
                reservation.model(),
                None,
                None,
                actual.map(|(_, tokens)| i64::try_from(tokens).unwrap_or(i64::MAX)),
                actual.map(|(cost, _)| cost),
            );
        }
        let key_error = self.key.take().and_then(|reservation| {
            settle_key(reservation, cost, self.facts.as_ref())
                .err()
                .map(|_| "Realtime key budget settlement failed".to_string())
        });
        let reservation = self.provider.take();
        let budget_settlement = async {
            if let Some(reservation) = reservation {
                let settle = reservation.settle_async(cost);
                let result = match self.facts.as_ref() {
                    Some(facts) => scope_facts(facts.clone(), settle).await,
                    None => settle.await,
                };
                result
                    .err()
                    .map(|_| "Realtime budget settlement failed".to_string())
            } else {
                None
            }
        };
        let usage_record = async {
            if let Some(key_id) = key_id
                && let Err(error) = state
                    .budgeted
                    .key_manager()
                    .record_usage(key_id, tokens, cost)
                    .await
            {
                // Attempt usage persistence even when a separate budget ledger failed.
                tracing::error!(%error, %key_id, tokens, cost, "Realtime key usage recording failed");
            }
        };
        let (provider_error, ()) = tokio::join!(budget_settlement, usage_record);
        match (provider_error, key_error) {
            (Some(provider), Some(key)) => Err(format!("{provider}; {key}")),
            (Some(error), None) | (None, Some(error)) => Err(error),
            (None, None) => Ok(tokens),
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        // An aborted transport task must not refund a potentially billable response.
        if (self.provider.is_some() || self.key.is_some())
            && let Some(facts) = &self.facts
        {
            update_billing(Some(facts), |billing| {
                billing.charge_basis = Some("reserved_estimate".into());
                billing.unknown_reason = Some("stream_cancelled_before_usage".into());
                billing.awaiting_since.get_or_insert_with(chrono::Utc::now);
            });
        }
        if let Some(reservation) = self.provider.take() {
            reservation.settle_detached(self.bound, self.facts.clone());
        }
        if let Some(reservation) = self.key.take()
            && let Err(error) = settle_key(reservation, self.bound, self.facts.as_ref())
        {
            tracing::error!(
                ?error,
                "Realtime aborted response fallback settlement failed"
            );
        }
        if let (Some(session), Some(facts)) = (&self.session_facts, &self.facts) {
            finish_response(session, &snapshot_facts(facts));
        }
    }
}

fn settle_key(
    reservation: BudgetReservation,
    cost: f64,
    facts: Option<&SharedRequestLedgerFacts>,
) -> Result<(), crate::core::budget::BudgetReservationError> {
    let tracked = reservation.is_tracked();
    let reserved = reservation.reserved_amount();
    let result = reservation.settle(cost);
    if tracked && facts.is_some() {
        update_billing(facts, |billing| {
            billing.key_reserved_amount = Some(reserved);
            billing.key_settlement = Some(
                if result.is_ok() {
                    "settled"
                } else {
                    "settlement_unconfirmed"
                }
                .into(),
            );
            if result.is_ok() {
                billing.key_charge_amount = Some(cost);
            }
            billing.settlement_updated_at = Some(chrono::Utc::now());
        });
    }
    result.map(|_| ())
}

// Only one Realtime generation may be active. Merge each generation once into
// the existing HTTP session row; this is metadata, not another budget ledger.
fn start_response(session: &SharedRequestLedgerFacts, response: &RequestLedgerFacts) {
    let mut session = session
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    session.provider.clone_from(&response.provider);
    session.model.clone_from(&response.model);
    session.deployment.clone_from(&response.deployment);
    let billing = session.billing.get_or_insert_with(RequestBilling::default);
    billing.known_cost_subtotal.get_or_insert(0.0);
    billing.unknown_response_count.get_or_insert(0);
    *billing.pending_response_count.get_or_insert(0) += 1;
    if billing.unknown_response_count == Some(0) {
        billing.unknown_reason = Some("awaiting_provider_usage".into());
    }
    billing.awaiting_since.get_or_insert_with(chrono::Utc::now);
    if let Some(response) = &response.billing {
        add_amount(
            &mut billing.provider_reserved_amount,
            response.provider_reserved_amount,
        );
        add_amount(
            &mut billing.model_reserved_amount,
            response.model_reserved_amount,
        );
        add_amount(
            &mut billing.key_reserved_amount,
            response.key_reserved_amount,
        );
        billing.reserved_at = response.reserved_at.or(billing.reserved_at);
        billing
            .provider_lease_id
            .clone_from(&response.provider_lease_id);
        billing.model_lease_id.clone_from(&response.model_lease_id);
        mark_pending(
            &mut billing.provider_settlement,
            response.provider_reserved_amount.is_some(),
        );
        mark_pending(
            &mut billing.model_settlement,
            response.model_reserved_amount.is_some(),
        );
        mark_pending(
            &mut billing.key_settlement,
            response.key_reserved_amount.is_some(),
        );
    }
    session.cost = None;
}

fn finish_response(session: &SharedRequestLedgerFacts, response: &RequestLedgerFacts) {
    let mut session = session
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(tokens) = response.total_tokens {
        session.total_tokens = Some(session.total_tokens.unwrap_or(0).saturating_add(tokens));
    }
    let billing = session.billing.get_or_insert_with(RequestBilling::default);
    let pending = billing.pending_response_count.get_or_insert(0);
    *pending = pending.saturating_sub(1);
    match response.cost {
        Some(cost) => *billing.known_cost_subtotal.get_or_insert(0.0) += cost,
        None => {
            if billing.unknown_response_count.unwrap_or(0) == 0 {
                billing.unknown_reason = response
                    .billing
                    .as_ref()
                    .and_then(|response| response.unknown_reason.clone())
                    .or_else(|| Some("provider_usage_missing".into()));
            }
            *billing.unknown_response_count.get_or_insert(0) += 1;
            billing.awaiting_since.get_or_insert_with(chrono::Utc::now);
        }
    }
    if let Some(response) = &response.billing {
        add_amount(
            &mut billing.provider_charge_amount,
            response.provider_charge_amount,
        );
        add_amount(
            &mut billing.model_charge_amount,
            response.model_charge_amount,
        );
        add_amount(&mut billing.key_charge_amount, response.key_charge_amount);
        merge_settlement(
            &mut billing.provider_settlement,
            response.provider_settlement.as_deref(),
        );
        merge_settlement(
            &mut billing.model_settlement,
            response.model_settlement.as_deref(),
        );
        merge_settlement(
            &mut billing.key_settlement,
            response.key_settlement.as_deref(),
        );
    }
    billing.settlement_updated_at = Some(chrono::Utc::now());
    let all_known = billing.unknown_response_count.unwrap_or(0) == 0
        && billing.pending_response_count.unwrap_or(0) == 0;
    let subtotal = billing.known_cost_subtotal;
    if all_known {
        billing.unknown_reason = None;
        billing.awaiting_since = None;
        billing.charge_basis = Some("gateway_pricing".into());
    } else {
        billing.charge_basis = Some("realtime_known_and_unknown".into());
    }
    session.cost = if all_known { subtotal } else { None };
}

fn add_amount(total: &mut Option<f64>, amount: Option<f64>) {
    if let Some(amount) = amount {
        *total = Some(total.unwrap_or(0.0) + amount);
    }
}

fn mark_pending(status: &mut Option<String>, tracked: bool) {
    if tracked && status.as_deref() != Some("settlement_unconfirmed") {
        *status = Some("settlement_pending".into());
    }
}

fn merge_settlement(status: &mut Option<String>, response: Option<&str>) {
    if let Some(response) = response {
        if status.as_deref() == Some("settlement_unconfirmed") || response != "settled" {
            *status = Some("settlement_unconfirmed".into());
        } else {
            *status = Some("settled".into());
        }
    }
}
