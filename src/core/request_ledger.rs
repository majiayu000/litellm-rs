//! Metadata-only request ledger facts captured during a gateway request.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::models::request_ledger::RequestLedgerWriteFailure;

tokio::task_local! {
    static REQUEST_LEDGER_FACTS: SharedRequestLedgerFacts;
}

/// Shared mutable facts attached to one in-flight request.
pub type SharedRequestLedgerFacts = Arc<Mutex<RequestLedgerFacts>>;

/// Accounting facts, distinct from a supplier-verified invoice amount.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RequestBilling {
    /// Independent scopes enforcing the same request. Never sum these holds.
    pub provider_reserved_amount: Option<f64>,
    pub model_reserved_amount: Option<f64>,
    /// Existing Redis lease identities for operational inspection (not secrets).
    pub provider_lease_id: Option<String>,
    pub model_lease_id: Option<String>,
    /// Independent API key budget hold for the same request.
    pub key_reserved_amount: Option<f64>,
    pub reserved_at: Option<DateTime<Utc>>,
    /// Why the supplier's actual charge is not yet known.
    pub unknown_reason: Option<String>,
    pub awaiting_since: Option<DateTime<Utc>>,
    /// `gateway_pricing`, `reserved_estimate`, or `fallback_pricing`.
    pub charge_basis: Option<String>,
    pub provider_charge_amount: Option<f64>,
    pub model_charge_amount: Option<f64>,
    pub key_charge_amount: Option<f64>,
    /// Only acknowledged operations are marked `settled`.
    pub provider_settlement: Option<String>,
    pub model_settlement: Option<String>,
    pub key_settlement: Option<String>,
    pub settlement_updated_at: Option<DateTime<Utc>>,
    /// Realtime session subtotal, never a complete actual when a response is unknown/pending.
    pub known_cost_subtotal: Option<f64>,
    pub unknown_response_count: Option<u64>,
    pub pending_response_count: Option<u64>,
}

impl RequestBilling {
    /// Verify both independent budget scopes without adding their charges.
    pub fn requires_budget_review(&self, actual_cost: f64) -> bool {
        [
            (
                self.provider_reserved_amount,
                self.provider_charge_amount,
                &self.provider_settlement,
            ),
            (
                self.model_reserved_amount,
                self.model_charge_amount,
                &self.model_settlement,
            ),
            (
                self.key_reserved_amount,
                self.key_charge_amount,
                &self.key_settlement,
            ),
        ]
        .into_iter()
        .any(|(reserved, charged, status)| {
            reserved.is_some()
                && (status.as_deref() != Some("settled") || charged != Some(actual_cost))
        })
    }
}

/// An administrator's supplier verification; does not alter budget counters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestReconciliation {
    pub verified_actual_cost: f64,
    /// A receipt/invoice reference, never credentials or response contents.
    pub evidence_reference: String,
    pub verified_at: DateTime<Utc>,
    pub budget_review_required: bool,
}

/// Update an explicitly captured handle, or the current HTTP task's facts.
pub fn update_billing(
    facts: Option<&SharedRequestLedgerFacts>,
    update: impl FnOnce(&mut RequestBilling),
) {
    let handle = facts.cloned().or_else(current_facts);
    if let Some(handle) = handle {
        let mut facts = handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        update(facts.billing.get_or_insert_with(RequestBilling::default));
    }
}

/// Selected routing and settlement metadata. Never holds bodies or secrets.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RequestLedgerFacts {
    /// Requested or served model id.
    pub model: Option<String>,
    /// Selected provider name.
    pub provider: Option<String>,
    /// Selected deployment name when distinct from the provider.
    pub deployment: Option<String>,
    /// Prompt / input tokens.
    pub prompt_tokens: Option<i64>,
    /// Completion / output tokens.
    pub completion_tokens: Option<i64>,
    /// Total tokens.
    pub total_tokens: Option<i64>,
    /// Settled cost in the gateway pricing currency.
    pub cost: Option<f64>,
    /// Budget responsibility and pending supplier reconciliation.
    pub billing: Option<RequestBilling>,
}

impl RequestLedgerFacts {
    /// Merge settlement metadata into the request-scoped facts handle.
    pub fn record_settlement(
        &mut self,
        provider: &str,
        model: &str,
        prompt_tokens: Option<i64>,
        completion_tokens: Option<i64>,
        total_tokens: Option<i64>,
        cost: Option<f64>,
    ) {
        self.provider = Some(provider.to_string());
        self.model = Some(model.to_string());
        if self.deployment.is_none() {
            self.deployment = Some(provider.to_string());
        }
        if prompt_tokens.is_some() {
            self.prompt_tokens = prompt_tokens;
        }
        if completion_tokens.is_some() {
            self.completion_tokens = completion_tokens;
        }
        if total_tokens.is_some() {
            self.total_tokens = total_tokens;
        }
        let billing = self.billing.get_or_insert_with(RequestBilling::default);
        if cost.is_some() {
            self.cost = cost;
            billing.charge_basis = Some("gateway_pricing".into());
            billing.unknown_reason = None;
            billing.awaiting_since = None;
        } else if billing.charge_basis.as_deref() != Some("fallback_pricing") {
            billing.charge_basis = Some("reserved_estimate".into());
            billing.unknown_reason = Some(
                if prompt_tokens.is_some() {
                    "pricing_unavailable"
                } else {
                    "provider_usage_missing"
                }
                .into(),
            );
        }
        if cost.is_none() {
            billing.awaiting_since.get_or_insert_with(Utc::now);
        }
    }
}

/// One terminal metadata row. Column names are the persistence contract: no
/// prompt, body, Authorization, raw API key, or provider secret fields exist.
#[derive(Debug, Clone, Serialize)]
pub struct RequestLedgerRecord {
    /// Gateway request id.
    pub request_id: String,
    /// Request start timestamp.
    pub started_at: DateTime<Utc>,
    /// Terminal timestamp.
    pub finished_at: DateTime<Utc>,
    /// HTTP method.
    pub method: String,
    /// Request path.
    pub endpoint: String,
    /// Served model when known.
    pub model: Option<String>,
    /// Selected provider when known.
    pub provider: Option<String>,
    /// Selected deployment when known.
    pub deployment: Option<String>,
    /// HTTP status code, or 0 when the request future failed without a response.
    pub status_code: i32,
    /// `completed`, `failed`, or `cancelled`.
    pub terminal_status: String,
    /// End-to-end latency in milliseconds.
    pub latency_ms: i64,
    /// Prompt tokens when settled.
    pub prompt_tokens: Option<i64>,
    /// Completion tokens when settled.
    pub completion_tokens: Option<i64>,
    /// Total tokens when settled.
    pub total_tokens: Option<i64>,
    /// Settled cost when priced.
    pub cost: Option<f64>,
    /// Budget responsibility and pending supplier reconciliation.
    pub billing: Option<RequestBilling>,
    /// Authenticated user id.
    pub user_id: Option<String>,
    /// API key id (never the raw secret).
    pub api_key_id: Option<String>,
    /// Team id.
    pub team_id: Option<String>,
}

/// Persistence backend for terminal request-ledger rows.
#[async_trait::async_trait]
pub trait RequestLedgerWriter: Send + Sync {
    /// Insert or merge one terminal metadata row.
    async fn persist(&self, record: RequestLedgerRecord) -> Result<(), String>;
}

/// Runtime handle passed to HTTP middleware when the ledger is enabled.
#[derive(Clone)]
pub struct RequestLedgerRuntime {
    /// Persistence backend.
    pub writer: Arc<dyn RequestLedgerWriter>,
    /// Fail or continue when a write fails.
    pub write_failure: RequestLedgerWriteFailure,
}

/// Apply the configured write-failure policy. Continue logs at error level.
pub async fn persist_with_policy(
    writer: &dyn RequestLedgerWriter,
    record: RequestLedgerRecord,
    policy: RequestLedgerWriteFailure,
) -> Result<(), String> {
    let request_id = record.request_id.clone();
    match writer.persist(record).await {
        Ok(()) => Ok(()),
        Err(error) => match policy {
            RequestLedgerWriteFailure::Continue => {
                tracing::error!(
                    request_id = %request_id,
                    error = %error,
                    "request ledger persist failed; continuing"
                );
                Ok(())
            }
            RequestLedgerWriteFailure::Fail => {
                tracing::error!(
                    request_id = %request_id,
                    error = %error,
                    "request ledger persist failed; failing request"
                );
                Err(error)
            }
        },
    }
}

/// Install request-scoped facts for the remainder of the current task.
pub async fn scope_facts<F>(facts: SharedRequestLedgerFacts, future: F) -> F::Output
where
    F: std::future::Future,
{
    REQUEST_LEDGER_FACTS.scope(facts, future).await
}

/// Current request facts when the caller is inside [`scope_facts`].
pub fn current_facts() -> Option<SharedRequestLedgerFacts> {
    REQUEST_LEDGER_FACTS.try_with(Arc::clone).ok()
}

/// Update the current request facts from spend settlement.
pub fn record_current_settlement(
    provider: &str,
    model: &str,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    cost: Option<f64>,
) {
    let Some(facts) = current_facts() else {
        return;
    };
    apply_settlement(
        &facts,
        provider,
        model,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cost,
    );
}

/// Update a captured facts handle from spend settlement.
pub fn apply_settlement(
    facts: &SharedRequestLedgerFacts,
    provider: &str,
    model: &str,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    cost: Option<f64>,
) {
    let mut facts = match facts.lock() {
        Ok(facts) => facts,
        Err(poisoned) => poisoned.into_inner(),
    };
    facts.record_settlement(
        provider,
        model,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cost,
    );
}

/// Snapshot facts without cloning mutex poison into callers.
pub fn snapshot_facts(facts: &SharedRequestLedgerFacts) -> RequestLedgerFacts {
    match facts.lock() {
        Ok(facts) => facts.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FailingWriter;

    #[async_trait::async_trait]
    impl RequestLedgerWriter for FailingWriter {
        async fn persist(&self, _record: RequestLedgerRecord) -> Result<(), String> {
            Err("disk full".to_string())
        }
    }

    fn sample_record() -> RequestLedgerRecord {
        RequestLedgerRecord {
            request_id: "req-1".to_string(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            method: "POST".to_string(),
            endpoint: "/v1/chat/completions".to_string(),
            model: Some("gpt-4".to_string()),
            provider: Some("openai".to_string()),
            deployment: Some("openai".to_string()),
            status_code: 200,
            terminal_status: "completed".to_string(),
            latency_ms: 12,
            prompt_tokens: Some(3),
            completion_tokens: Some(5),
            total_tokens: Some(8),
            cost: Some(0.01),
            billing: None,
            user_id: None,
            api_key_id: Some("key-1".to_string()),
            team_id: None,
        }
    }

    #[test]
    fn serialized_record_has_no_body_or_secret_fields() {
        let json = serde_json::to_value(sample_record()).expect("record serializes");
        let keys: Vec<&str> = json
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        for forbidden in [
            "body",
            "prompt",
            "authorization",
            "header",
            "secret",
            "api_key",
            "raw_key",
        ] {
            assert!(
                !keys.contains(&forbidden),
                "{forbidden} must not be a ledger column, got {keys:?}"
            );
        }
        assert!(keys.contains(&"api_key_id"));
        assert!(keys.contains(&"request_id"));
    }

    #[tokio::test]
    async fn continue_policy_logs_and_succeeds() {
        persist_with_policy(
            &FailingWriter,
            sample_record(),
            RequestLedgerWriteFailure::Continue,
        )
        .await
        .expect("continue must not fail the caller");
    }

    #[tokio::test]
    async fn fail_policy_returns_the_write_error() {
        let error = persist_with_policy(
            &FailingWriter,
            sample_record(),
            RequestLedgerWriteFailure::Fail,
        )
        .await
        .expect_err("fail must surface the write error");
        assert_eq!(error, "disk full");
    }

    #[test]
    fn settlement_does_not_store_authorization_or_bodies() {
        let facts = SharedRequestLedgerFacts::new(Mutex::new(RequestLedgerFacts::default()));
        apply_settlement(
            &facts,
            "openai",
            "gpt-4",
            Some(1),
            Some(2),
            Some(3),
            Some(0.2),
        );
        let snapshot = snapshot_facts(&facts);
        let json = serde_json::to_value(&snapshot).expect("facts serialize");
        let text = json.to_string().to_lowercase();
        assert!(!text.contains("authorization"));
        assert!(!text.contains("bearer"));
        assert!(!text.contains("sk-"));
        assert_eq!(snapshot.model.as_deref(), Some("gpt-4"));
    }
}
