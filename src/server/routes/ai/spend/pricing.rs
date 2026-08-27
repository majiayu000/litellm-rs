use uuid::Uuid;

use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::KeyManager;
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::providers::Provider;
use crate::core::providers::provider_type::ProviderType;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::embedding::EmbeddingInput;
use crate::utils::ai::counter::token_counter::TokenCounter;

pub(in crate::server::routes::ai) fn pricing_identity_for_provider(
    pricing_service: &PricingService,
    provider: &Provider,
    model: &str,
) -> (String, String) {
    let provider_name = provider.name();
    let mut provider_candidates = vec![provider_name.to_string()];

    match provider.provider_type() {
        ProviderType::OpenAI => provider_candidates.push("openai".to_string()),
        ProviderType::OpenAICompatible => {
            provider_candidates.push("openai_like".to_string());
            provider_candidates.push("openai".to_string());
        }
        ProviderType::Azure => provider_candidates.push("azure".to_string()),
        ProviderType::AzureAI => provider_candidates.push("azure_ai".to_string()),
        other => provider_candidates.push(other.to_string()),
    }

    let provider_candidates =
        provider_candidates
            .into_iter()
            .fold(Vec::new(), |mut unique, candidate| {
                if !unique.contains(&candidate) {
                    unique.push(candidate);
                }
                unique
            });

    let mut model_candidates = vec![model.to_string()];
    let mapped_model = if let Provider::OpenAI(provider) = provider {
        let mapped = provider.config.get_model_mapping(model);
        if !model_candidates.contains(&mapped) {
            model_candidates.insert(0, mapped.clone());
        }
        Some(mapped)
    } else {
        None
    };

    for pricing_provider in &provider_candidates {
        for pricing_model in &model_candidates {
            if let Some((resolved_model, _)) =
                pricing_service.get_model_info_for_provider(pricing_provider, pricing_model)
            {
                return (pricing_provider.clone(), resolved_model);
            }
        }
    }

    if let Some(identity) = mapped_model.as_deref().and_then(|mapped| {
        unpriced_openai_mapping_identity(&provider.provider_type(), model, mapped)
    }) {
        return identity;
    }

    (provider_name.to_string(), model.to_string())
}

fn unpriced_openai_mapping_identity(
    provider_type: &ProviderType,
    requested_model: &str,
    mapped_model: &str,
) -> Option<(String, String)> {
    (mapped_model != requested_model
        && matches!(
            provider_type,
            ProviderType::OpenAI | ProviderType::OpenAICompatible
        ))
    .then(|| ("openai".to_string(), mapped_model.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::server::routes::ai) fn reserve_embedding_budget_with_policy(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    input: &EmbeddingInput,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let prompt_tokens = estimate_embedding_input_tokens(pricing_model, input);
    reserve_completion_budget_with_split_pricing(
        pricing_service,
        pricing_config,
        budget_limits,
        budget_provider,
        budget_model,
        pricing_provider,
        pricing_model,
        prompt_tokens,
        Some(0),
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::server::routes::ai) fn reserve_pricing_usage_budget_with_policy(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    usage: &PricingUsage,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let cost = match pricing_service.calculate_loaded_usage_cost_for_provider(
        pricing_provider,
        pricing_model,
        usage,
    ) {
        Ok(breakdown) => breakdown.total_cost,
        Err(error) => {
            tracing::error!(
                "cost estimation failed for pricing provider '{pricing_provider}' budget provider \
                 '{budget_provider}' model '{budget_model}': {error}; applying unpriced model policy"
            );
            return super::unpriced::reserve_unpriced_usage_budget(
                pricing_config,
                budget_limits,
                budget_provider,
                budget_model,
                usage,
                error,
            );
        }
    };

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
pub(in crate::server::routes::ai) async fn record_pricing_usage_spend_with_reservation_with_policy(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<Uuid>,
    budget_provider: &str,
    budget_model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    usage: &PricingUsage,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) {
    let cost = match pricing_service.calculate_loaded_settlement_cost_for_provider(
        pricing_provider,
        pricing_model,
        usage,
    ) {
        Ok(breakdown) => breakdown.total_cost,
        Err(error) => {
            tracing::error!(
                "cost calculation failed for pricing provider '{pricing_provider}' budget provider \
                 '{budget_provider}' model '{budget_model}': {error}; settling through unpriced \
                 model policy"
            );
            super::unpriced::settle_unpriced_usage(
                pricing_config,
                budget_limits,
                key_manager,
                api_key_id,
                budget_provider,
                budget_model,
                usage,
                budget_reservation,
                key_budget_reservation,
                "usage spend pricing unavailable",
            )
            .await;
            return;
        }
    };

    if let Some(reservation) = budget_reservation {
        if let Err(error) = reservation.settle(cost) {
            tracing::error!(
                "failed to settle reserved budget for '{budget_provider}'/'{budget_model}': \
                 {error:?}; spend not recorded because reservation settlement failed"
            );
        }
    } else {
        budget_limits.record_spend(budget_provider, budget_model, cost);
    }
    super::settle_api_key_budget_reservation(
        key_budget_reservation,
        cost,
        &format!("{budget_provider}/{budget_model}"),
    );

    if let Some(key_id) = api_key_id {
        let total_tokens = super::unpriced::usage_units(usage);
        if let Err(error) = key_manager
            .record_usage(key_id, u64::from(total_tokens), cost)
            .await
        {
            tracing::error!("failed to record usage for key {key_id}: {error}");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reserve_completion_budget_with_split_pricing(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let estimate = match pricing_service.estimate_loaded_completion_cost_for_provider(
        pricing_provider,
        pricing_model,
        estimated_prompt_tokens,
        max_output_tokens,
    ) {
        Ok(estimate) => estimate,
        Err(error) => {
            tracing::error!(
                "cost estimation failed for pricing provider '{pricing_provider}' budget provider \
                 '{budget_provider}' model '{budget_model}': {error}; applying unpriced model policy"
            );
            return super::unpriced::reserve_unpriced_completion_budget(
                pricing_config,
                budget_limits,
                budget_provider,
                budget_model,
                estimated_prompt_tokens,
                max_output_tokens,
                error,
            );
        }
    };

    if estimate.max_cost <= 0.0 {
        super::ensure_budget_available(budget_limits, budget_provider, budget_model)?;
        return Ok(None);
    }

    budget_limits
        .reserve_spend(budget_provider, budget_model, estimate.max_cost)
        .map(Some)
        .map_err(|error| {
            super::reservation_error_to_provider_error(error, budget_provider, budget_model)
        })
}

fn estimate_embedding_input_tokens(model: &str, input: &EmbeddingInput) -> u32 {
    let counter = TokenCounter::new();
    input.iter().fold(0u32, |total, text| {
        let tokens = counter
            .count_completion_tokens(model, text)
            .map(|estimate| estimate.input_tokens)
            .unwrap_or_else(|error| {
                tracing::warn!(
                    "embedding token estimation failed for model '{model}': {error}; \
                     using fallback estimate"
                );
                u32::try_from(text.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
            });
        total.saturating_add(tokens)
    })
}

#[cfg(test)]
mod mapped_identity_tests {
    use super::*;

    #[test]
    fn unpriced_openai_mapping_retains_canonical_identity_only_for_real_mapping() {
        assert_eq!(
            unpriced_openai_mapping_identity(
                &ProviderType::OpenAICompatible,
                "public-alias",
                "canonical-model",
            ),
            Some(("openai".to_string(), "canonical-model".to_string()))
        );
        assert_eq!(
            unpriced_openai_mapping_identity(
                &ProviderType::OpenAICompatible,
                "same-model",
                "same-model",
            ),
            None
        );
        assert_eq!(
            unpriced_openai_mapping_identity(
                &ProviderType::Anthropic,
                "public-alias",
                "canonical-model",
            ),
            None
        );
    }

    #[test]
    fn retained_mapping_identity_does_not_price_non_image_requests() {
        let pricing = PricingService::new(None);
        let (provider, model) = unpriced_openai_mapping_identity(
            &ProviderType::OpenAICompatible,
            "public-alias",
            "canonical-model",
        )
        .expect("real mapping should retain canonical identity");

        let error = pricing
            .calculate_loaded_usage_cost_for_provider(&provider, &model, &PricingUsage::new(10, 5))
            .expect_err("identity retention must not invent a non-image price");
        assert!(error.to_string().contains("Model not found"));
    }
}
