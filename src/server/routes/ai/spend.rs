//! Spend and usage recording for completed requests.
//!
//! Wires the otherwise-dead budget and per-key usage tracking into the request
//! path: once a completion succeeds and its token usage is known, the served
//! provider/model budget spend and the calling key's usage are recorded.

use uuid::Uuid;

use crate::core::budget::UnifiedBudgetLimits;
use crate::core::cost::calculator::generic_cost_per_token;
use crate::core::cost::types::UsageTokens;
use crate::core::keys::KeyManager;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::Usage;

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

/// Record provider/model budget spend and per-key usage for a completed request.
///
/// Best-effort and non-fatal: the completion already succeeded, so failures here
/// are logged at error level (never silently swallowed) but do not fail the
/// response. When the cost cannot be priced, token usage is still recorded but
/// budget spend is skipped rather than booked at $0 — under-counting a budget is
/// worse than leaving it unchanged with a loud error.
pub(super) async fn record_completion_spend(
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<Uuid>,
    provider: &str,
    model: &str,
    usage: Option<&Usage>,
) {
    let Some(usage) = usage else {
        tracing::error!(
            "provider '{provider}' returned no usage for model '{model}'; spend not recorded"
        );
        return;
    };

    let total_tokens = u64::from(usage.total_tokens);
    let usage_tokens: UsageTokens = usage.clone().into();

    let cost = match generic_cost_per_token(model, &usage_tokens, provider) {
        Ok(breakdown) => Some(breakdown.total_cost),
        Err(e) => {
            tracing::error!(
                "cost calculation failed for '{provider}'/'{model}': {e}; \
                 recording token usage without cost and skipping budget spend"
            );
            None
        }
    };

    if let Some(cost) = cost {
        budget_limits.record_spend(provider, model, cost);
    }

    if let Some(key_id) = api_key_id {
        // Token counts are factual even when pricing is unavailable; record them
        // with the cost we have (0.0 only when pricing failed, already logged).
        if let Err(e) = key_manager
            .record_usage(key_id, total_tokens, cost.unwrap_or(0.0))
            .await
        {
            tracing::error!("failed to record usage for key {key_id}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::budget::{ProviderLimitConfig, ResetPeriod};
    use crate::core::keys::InMemoryKeyRepository;
    use crate::core::types::responses::Usage;

    fn usage(prompt: u32, completion: u32) -> Usage {
        Usage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        }
    }

    #[tokio::test]
    async fn records_provider_spend_for_priced_model() {
        let budget = UnifiedBudgetLimits::new();
        budget.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
        );
        let keys = KeyManager::new(InMemoryKeyRepository::new());

        record_completion_spend(
            &budget,
            &keys,
            None,
            "openai",
            "gpt-4o",
            Some(&usage(1000, 1000)),
        )
        .await;

        let spent = budget
            .providers
            .get_provider_usage("openai")
            .map(|u| u.current_spend)
            .unwrap_or(0.0);
        assert!(spent > 0.0, "priced completion must record provider spend");
    }

    #[tokio::test]
    async fn unpriced_model_records_no_budget_spend() {
        let budget = UnifiedBudgetLimits::new();
        budget.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
        );
        let keys = KeyManager::new(InMemoryKeyRepository::new());

        // Unknown model has no pricing: budget spend must stay at 0 rather than
        // being booked at a fabricated $0 cost silently.
        record_completion_spend(
            &budget,
            &keys,
            None,
            "openai",
            "definitely-not-a-real-model-xyz",
            Some(&usage(1000, 1000)),
        )
        .await;

        let spent = budget
            .providers
            .get_provider_usage("openai")
            .map(|u| u.current_spend)
            .unwrap_or(0.0);
        assert_eq!(spent, 0.0);
    }
    #[test]
    fn budget_available_when_unconfigured() {
        // No limits set: precheck must allow the request through.
        let budget = UnifiedBudgetLimits::new();
        assert!(ensure_budget_available(&budget, "openai", "gpt-4o").is_ok());
    }

    #[test]
    fn budget_rejects_when_provider_exhausted() {
        let budget = UnifiedBudgetLimits::new();
        budget.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        // Drive the provider over its limit.
        budget.providers.record_provider_spend("openai", 2.0);

        let err = ensure_budget_available(&budget, "openai", "gpt-4o")
            .expect_err("exhausted provider budget must be rejected");
        assert!(matches!(err, ProviderError::QuotaExceeded { .. }));
    }
}
