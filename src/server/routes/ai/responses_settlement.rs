//! Durable, bounded reconciliation of native Responses dispatches; never replays POST.
use super::responses_native::{lifecycle, response_usage};
use super::spend::RequestPricing;
use crate::core::budget::{ResponseBudgetLeases, UnifiedBudgetReservation};
use crate::core::pricing_service::{LiteLLMModelInfo, PricingService, PricingUsage};
use crate::core::providers::base::HttpMethod;
use crate::core::types::context::RequestContext;
use crate::server::state::AppState;
use crate::storage::database::entities::{response, response_settlement::Model};
use crate::utils::error::gateway_error::{GatewayError, Result};

pub(crate) struct ResponseSettlementTask(tokio::task::JoinHandle<()>);
impl Drop for ResponseSettlementTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(crate) fn start(state: AppState) -> Option<ResponseSettlementTask> {
    if !state.config().gateway.storage.database.enabled {
        return None;
    }
    Some(ResponseSettlementTask(tokio::spawn(async move {
        loop {
            if let Err(error) = recover(&state).await {
                tracing::error!(%error, "Response settlement recovery failed; will retry");
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    })))
}

pub(super) async fn prepare(
    state: &AppState,
    context: &RequestContext,
    storage: &mut lifecycle::NativeResponseStorage,
    provider: &str,
    model: &str,
    pricing: &RequestPricing,
    reservation: Option<&UnifiedBudgetReservation>,
) -> Result<()> {
    let leases = reservation.map(UnifiedBudgetReservation::response_leases).transpose()
        .map_err(|_| GatewayError::validation("Background Responses with provider/model limits require the existing shared Redis budget backend"))?;
    let now = chrono::Utc::now().timestamp();
    let id = uuid::Uuid::new_v4().to_string();
    let pricing_json = serde_json::to_string(&pricing.priced_parts().zip(pricing.model_info()))?;
    let row = Model {
        id: id.clone(),
        owner: storage.owner.clone(),
        api_key_id: context.api_key_id(),
        provider: provider.into(),
        model: model.into(),
        deployment_id: storage.deployment.clone(),
        deployment_binding: storage.binding.clone(),
        response_id: None,
        pricing_json,
        leases_json: serde_json::to_string(&leases)?,
        reserved: leases.as_ref().map_or(0.0, |l| l.reserved),
        cost: None,
        tokens: 0,
        outcome: "pending".into(),
        key_settled: false,
        complete: false,
        deadline: now + 540,
        next_attempt: now,
        lease_until: now + 540,
        revision: 0,
    };
    state
        .storage
        .database
        .insert_response_settlement(row)
        .await?;
    storage.settlement_id = Some(id);
    Ok(())
}

/// Preserve already-observed terminal usage before a concurrent upstream DELETE.
pub(super) async fn submit_usage(
    state: &AppState,
    id: &str,
    usage: Option<&crate::core::types::responses::Usage>,
) -> Result<()> {
    let mut row = state
        .storage
        .database
        .response_settlement(id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Response settlement missing"))?;
    if row.cost.is_none() {
        let priced = match usage {
            Some(usage) => frozen_cost(&row, usage)?
                .map(|cost| (cost, u64::from(usage.total_tokens), "actual")),
            None => None,
        }
        .unwrap_or((row.reserved, 0, "reserved_unknown"));
        state
            .storage
            .database
            .price_response_settlement(&mut row, priced.0, priced.1, priced.2)
            .await?;
    }
    state.storage.database.wake_response_settlement(id).await?;
    recover(state).await
}

fn frozen_cost(row: &Model, usage: &crate::core::types::responses::Usage) -> Result<Option<f64>> {
    let frozen: Option<((String, String), LiteLLMModelInfo)> =
        serde_json::from_str(&row.pricing_json)?;
    let Some(((provider, model), info)) = frozen else {
        return Ok(None);
    };
    let pricing = PricingService::new(None);
    pricing.add_custom_model(model.clone(), info);
    Ok(Some(
        pricing
            .calculate_loaded_settlement_cost_for_provider(
                &provider,
                &model,
                &PricingUsage::from(usage),
            )?
            .total_cost,
    ))
}

pub(crate) async fn recover(state: &AppState) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    for mut row in state
        .storage
        .database
        .pending_response_settlements(now)
        .await?
    {
        if !state
            .storage
            .database
            .claim_response_settlement(&mut row, chrono::Utc::now().timestamp())
            .await?
        {
            continue;
        }
        let result = reconcile(state, &mut row).await;
        let complete = match result {
            Ok(complete) => complete,
            Err(error) => {
                tracing::error!(settlement_id = %row.id, %error, "Response settlement remains pending");
                false
            }
        };
        state
            .storage
            .database
            .finish_response_settlement_attempt(&row, complete, chrono::Utc::now().timestamp())
            .await?;
    }
    Ok(())
}

async fn reconcile(state: &AppState, row: &mut Model) -> Result<bool> {
    if row.cost.is_none() {
        let mut priced = None;
        if let Some(id) = &row.response_id {
            let record = response::Model {
                id: id.clone(),
                owner: row.owner.clone(),
                response_json: "{}".into(),
                input_json: "{}".into(),
                deployment_id: Some(row.deployment_id.clone()),
                deployment_binding: Some(row.deployment_binding.clone()),
                background: true,
                status: "in_progress".into(),
                expires_at: row.deadline,
                lease_until: None,
                revision: 0,
            };
            let fetched = async {
                let provider = lifecycle::bound_provider(state, &record)?;
                let mut response = provider
                    .native_response_lifecycle(id, HttpMethod::GET, None, "")
                    .await?;
                lifecycle::read_json(&mut response).await
            };
            if let Ok(Ok(value)) =
                tokio::time::timeout(std::time::Duration::from_secs(20), fetched).await
            {
                if value.get("id").and_then(serde_json::Value::as_str) != Some(id.as_str()) {
                    return Err(GatewayError::validation(
                        "Background settlement received a different response ID",
                    ));
                }
                if super::responses_native::background::is_terminal(
                    value
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                ) {
                    if let Some(usage) = response_usage(&value)
                        && let Some(cost) = frozen_cost(row, &usage)?
                    {
                        priced = Some((cost, u64::from(usage.total_tokens), "actual"));
                    }
                    if priced.is_none() {
                        priced = Some((row.reserved, 0, "reserved_unknown"));
                    }
                }
            }
        }
        if priced.is_none() && chrono::Utc::now().timestamp() >= row.deadline {
            // Missing ID/terminal usage is a conservative budget commitment, not
            // verified supplier spend. Keep the unresolved accounting outcome.
            priced = Some((row.reserved, 0, "reserved_unknown"));
        }
        let Some((cost, tokens, outcome)) = priced else {
            return Ok(false);
        };
        if !state
            .storage
            .database
            .price_response_settlement(row, cost, tokens, outcome)
            .await?
        {
            return Ok(false);
        }
    }
    let leases: Option<ResponseBudgetLeases> = serde_json::from_str(&row.leases_json)?;
    if let Some(leases) = leases {
        state
            .budget_limits
            .settle_response_leases(&row.provider, &row.model, &leases, row.cost.unwrap_or(0.0))
            .map_err(|error| {
                GatewayError::Storage(format!("Response budget settlement failed: {error:?}"))
            })?;
    }
    state.storage.database.settle_response_key(row).await?;
    Ok(true)
}

#[cfg(all(test, feature = "sqlite"))]
#[path = "responses_settlement_tests.rs"]
mod tests;
