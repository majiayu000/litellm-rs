use uuid::Uuid;

use crate::config::models::gateway::{GatewayPricingConfig, UnpricedModelPolicy};
use crate::core::budget::{BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::{KeyManager, UsageRecord};
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::unified_provider::ProviderError;

const MODEL_NOT_PRICED_PREFIX: &str = "model_not_priced:";

pub(in crate::server::routes::ai) fn model_not_priced_error(
    provider: &str,
    model: &str,
    error: impl std::fmt::Display,
) -> ProviderError {
    crate::server::middleware::record_unpriced_event(provider, model, "reject", "reject_preflight");
    tracing::error!(
        provider = %provider,
        model = %model,
        model_bucket = crate::server::middleware::unpriced_model_bucket(model),
        policy = "reject",
        outcome = "reject_preflight",
        error = %error,
        "unpriced model rejected before provider call"
    );
    ProviderError::invalid_request(
        "pricing",
        format!(
            "{MODEL_NOT_PRICED_PREFIX} pricing unavailable for '{provider}'/'{model}': {error}"
        ),
    )
}

pub(in crate::server::routes::ai) fn is_model_not_priced_error(error: &ProviderError) -> bool {
    matches!(
        error,
        ProviderError::InvalidRequest {
            provider: "pricing",
            message,
        } if is_model_not_priced_message(message)
    )
}

pub(in crate::server::routes::ai) fn is_model_not_priced_message(message: &str) -> bool {
    message.starts_with(MODEL_NOT_PRICED_PREFIX)
}

pub(in crate::server::routes::ai) fn unpriced_policy_name(
    pricing_config: &GatewayPricingConfig,
) -> &'static str {
    match pricing_config.unpriced_model_policy {
        UnpricedModelPolicy::Reject => "reject",
        UnpricedModelPolicy::AllowUnpriced => "allow_unpriced",
    }
}

pub(in crate::server::routes::ai) fn usage_units(usage: &PricingUsage) -> u32 {
    usage
        .total_tokens
        .saturating_add(usage.audio_token_count())
        .saturating_add(usage.image_tokens.unwrap_or(0))
}

pub(in crate::server::routes::ai) fn fallback_cost_for_usage(
    pricing_config: &GatewayPricingConfig,
    usage: &PricingUsage,
) -> f64 {
    fallback_cost_for_units(pricing_config, usage_units(usage))
}

pub(in crate::server::routes::ai) fn fallback_cost_for_completion_estimate(
    pricing_config: &GatewayPricingConfig,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> f64 {
    let usage = PricingUsage::new(
        estimated_prompt_tokens,
        max_output_tokens.unwrap_or_else(|| estimated_prompt_tokens.saturating_div(2).max(1)),
    );
    fallback_cost_for_usage(pricing_config, &usage)
}

fn fallback_cost_for_units(pricing_config: &GatewayPricingConfig, usage_units: u32) -> f64 {
    pricing_config
        .unpriced_fallback_cost_per_1k_tokens
        .unwrap_or(0.0)
        * f64::from(usage_units)
        / 1000.0
}

pub(in crate::server::routes::ai) fn reserve_unpriced_completion_budget(
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
    error: impl std::fmt::Display,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    match pricing_config.unpriced_model_policy {
        UnpricedModelPolicy::Reject => {
            Err(model_not_priced_error(budget_provider, budget_model, error))
        }
        UnpricedModelPolicy::AllowUnpriced => {
            let cost = fallback_cost_for_completion_estimate(
                pricing_config,
                estimated_prompt_tokens,
                max_output_tokens,
            );
            reserve_unpriced_fallback_budget(budget_limits, budget_provider, budget_model, cost)
        }
    }
}

pub(in crate::server::routes::ai) fn reserve_unpriced_usage_budget(
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    usage: &PricingUsage,
    error: impl std::fmt::Display,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    match pricing_config.unpriced_model_policy {
        UnpricedModelPolicy::Reject => {
            Err(model_not_priced_error(budget_provider, budget_model, error))
        }
        UnpricedModelPolicy::AllowUnpriced => {
            let cost = fallback_cost_for_usage(pricing_config, usage);
            reserve_unpriced_fallback_budget(budget_limits, budget_provider, budget_model, cost)
        }
    }
}

fn reserve_unpriced_fallback_budget(
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    cost: f64,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    if cost <= 0.0 {
        super::ensure_budget_available(budget_limits, budget_provider, budget_model)?;
        return Ok(None);
    }

    budget_limits
        .reserve_spend(budget_provider, budget_model, cost)
        .map(Some)
        .map_err(|error| {
            super::reservation_error_to_provider_error(error, budget_provider, budget_model)
        })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::server::routes::ai) async fn settle_unpriced_usage(
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<Uuid>,
    budget_provider: &str,
    budget_model: &str,
    usage: &PricingUsage,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    context: &str,
) {
    let cost = fallback_cost_for_usage(pricing_config, usage);
    crate::server::middleware::record_unpriced_spend(
        budget_provider,
        budget_model,
        unpriced_policy_name(pricing_config),
        "fallback_settled",
        cost,
    );
    tracing::error!(
        provider = %budget_provider,
        model = %budget_model,
        model_bucket = crate::server::middleware::unpriced_model_bucket(budget_model),
        policy = unpriced_policy_name(pricing_config),
        outcome = "fallback_settled",
        cost,
        context = %context,
        "unpriced model settled through fallback pricing"
    );
    if let Some(reservation) = budget_reservation {
        if let Err(error) = reservation.settle(cost) {
            tracing::error!(
                "failed to settle unpriced budget for '{budget_provider}'/'{budget_model}': \
                 {error:?}; spend not recorded because reservation settlement failed"
            );
        }
    } else {
        budget_limits.record_spend(budget_provider, budget_model, cost);
    }
    super::settle_api_key_budget_reservation(
        key_budget_reservation,
        cost,
        &format!("{context}: {budget_provider}/{budget_model}"),
    );

    if let Some(key_id) = api_key_id {
        let mut record = UsageRecord::unpriced(
            u64::from(usage_units(usage)),
            cost,
            unpriced_policy_name(pricing_config),
        );
        record.provider = Some(budget_provider.to_string());
        record.model = Some(budget_model.to_string());
        if let Err(error) = key_manager.record_usage_record(key_id, record).await {
            tracing::error!("failed to record unpriced usage for key {key_id}: {error}");
        }
    }
}
