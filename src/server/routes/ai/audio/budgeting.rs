use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::KeyManager;
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::ProviderError;

const ESTIMATED_AUDIO_BYTES_PER_SECOND: usize = 16_000;

#[derive(Clone)]
pub(super) enum AudioPricingUnits {
    Time {
        seconds: f64,
        surface: crate::core::types::model::ProviderCapability,
    },
    Characters(f64),
}

pub(super) fn speech_usage(input: &str) -> PricingUsage {
    let tokens = estimated_audio_text_tokens(input);
    PricingUsage::new(tokens, tokens)
}

pub(super) fn audio_file_usage(file: &[u8], prompt: Option<&str>) -> PricingUsage {
    let file_tokens = u32::try_from(file.len().div_ceil(4))
        .unwrap_or(u32::MAX)
        .max(1);
    let prompt_tokens = prompt.map(estimated_audio_text_tokens).unwrap_or(0);
    let mut usage = PricingUsage::new(prompt_tokens, 0);
    usage.audio_tokens = Some(file_tokens);
    usage
}

pub(super) fn estimated_audio_file_seconds(file: &[u8]) -> f64 {
    file.len().max(1).div_ceil(ESTIMATED_AUDIO_BYTES_PER_SECOND) as f64
}

#[allow(clippy::too_many_arguments)]
pub(super) fn reserve_audio_provider_budget_with_pricing(
    request_pricing: &super::super::spend::RequestPricing,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    pricing_units: Option<AudioPricingUnits>,
    usage: &PricingUsage,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    super::super::spend::ensure_budget_available(budget_limits, budget_provider, budget_model)?;
    let budget_reservation = if let Some(cost) = audio_unit_cost(request_pricing, pricing_units) {
        match cost {
            Ok(cost) if cost.total_cost > 0.0 => budget_limits
                .reserve_spend(budget_provider, budget_model, cost.total_cost)
                .map(Some)
                .map_err(|error| {
                    super::super::spend::reservation_error_to_provider_error(
                        error,
                        budget_provider,
                        budget_model,
                    )
                })?,
            Ok(_) => {
                super::super::spend::ensure_budget_available(
                    budget_limits,
                    budget_provider,
                    budget_model,
                )?;
                None
            }
            Err(error) => super::super::spend::reserve_unpriced_usage_budget(
                pricing_config,
                budget_limits,
                budget_provider,
                budget_model,
                usage,
                error,
            )?,
        }
    } else {
        super::super::spend::reserve_pricing_usage_budget_with_request_pricing(
            request_pricing,
            pricing_config,
            budget_limits,
            budget_provider,
            budget_model,
            usage,
        )?
    };
    Ok(budget_reservation)
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn record_audio_spend(
    request_pricing: &super::super::spend::RequestPricing,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<uuid::Uuid>,
    budget_provider: &str,
    budget_model: &str,
    pricing_units: Option<AudioPricingUnits>,
    usage: &PricingUsage,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) {
    if let Some(cost) = audio_unit_cost(request_pricing, pricing_units) {
        match cost {
            Ok(cost) => {
                settle_audio_budget_or_record(
                    budget_limits,
                    budget_provider,
                    budget_model,
                    budget_reservation,
                    cost.total_cost,
                    "time-based audio spend",
                );
                super::super::spend::settle_api_key_budget_reservation(
                    key_budget_reservation,
                    cost.total_cost,
                    "time-based audio spend",
                );
                record_key_usage(key_manager, api_key_id, usage, cost.total_cost).await;
            }
            Err(error) => {
                tracing::error!(
                    "unit-based audio spend calculation failed for pricing provider \
                     budget provider '{budget_provider}' model \
                     '{budget_model}': {error}; skipping budget spend"
                );
                super::super::spend::settle_unpriced_usage(
                    pricing_config,
                    budget_limits,
                    key_manager,
                    api_key_id,
                    budget_provider,
                    budget_model,
                    usage,
                    budget_reservation,
                    key_budget_reservation,
                    "unit-based audio spend calculation failed",
                )
                .await;
            }
        }
        return;
    }

    super::super::spend::record_pricing_usage_spend_with_request_pricing(
        request_pricing,
        pricing_config,
        budget_limits,
        key_manager,
        api_key_id,
        budget_provider,
        budget_model,
        usage,
        budget_reservation,
        key_budget_reservation,
    )
    .await;
}

fn audio_unit_cost(
    request_pricing: &super::super::spend::RequestPricing,
    pricing_units: Option<AudioPricingUnits>,
) -> Option<crate::utils::error::gateway_error::Result<crate::core::pricing_service::CostResult>> {
    let units = pricing_units?;
    match units {
        AudioPricingUnits::Time { seconds, surface }
            if request_pricing.has_time_pricing(&surface) =>
        {
            Some(request_pricing.calculate_time(seconds, &surface))
        }
        AudioPricingUnits::Characters(characters) if request_pricing.has_character_pricing() => {
            Some(request_pricing.calculate_characters(characters))
        }
        _ => None,
    }
}

fn settle_audio_budget_or_record(
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    budget_reservation: Option<UnifiedBudgetReservation>,
    cost: f64,
    context: &str,
) {
    if let Some(reservation) = budget_reservation {
        if let Err(error) = reservation.settle(cost) {
            tracing::error!("failed to settle {context}: {error:?}");
        }
    } else {
        budget_limits.record_spend(budget_provider, budget_model, cost);
    }
}

async fn record_key_usage(
    key_manager: &KeyManager,
    api_key_id: Option<uuid::Uuid>,
    usage: &PricingUsage,
    cost: f64,
) {
    if let Some(key_id) = api_key_id {
        let total_tokens = usage.total_tokens.saturating_add(usage.audio_token_count());
        if let Err(error) = key_manager
            .record_usage(key_id, u64::from(total_tokens), cost)
            .await
        {
            tracing::error!("failed to record usage for key {key_id}: {error}");
        }
    }
}

fn estimated_audio_text_tokens(text: &str) -> u32 {
    u32::try_from(text.chars().count().div_ceil(4))
        .unwrap_or(u32::MAX)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::budget::{BudgetConfig, BudgetManager, BudgetScope};
    use crate::core::keys::InMemoryKeyRepository;
    use crate::core::pricing_service::PricingService;

    #[test]
    fn audio_file_usage_keeps_audio_tokens_out_of_total_tokens() {
        let usage = audio_file_usage(&[0; 16], Some("guide"));

        assert_eq!(usage.prompt_tokens, 2);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 2);
        assert_eq!(usage.audio_tokens, Some(4));
    }

    #[tokio::test]
    async fn record_audio_spend_settles_api_key_budget_reservation() {
        let pricing = PricingService::with_embedded_default()
            .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));
        let budget_limits = UnifiedBudgetLimits::new();
        let key_manager = KeyManager::new(InMemoryKeyRepository::new());
        let budget_manager = BudgetManager::new();
        let scope = BudgetScope::ApiKey("audio-key-budget".to_string());
        budget_manager
            .create_budget(scope.clone(), BudgetConfig::new("audio key", 1.0))
            .await
            .unwrap_or_else(|error| panic!("API key budget should be created: {error}"));
        let key_budget_reservation = budget_manager
            .tracker()
            .reserve_spend(&scope, 0.5)
            .unwrap_or_else(|error| panic!("API key budget should reserve: {error:?}"));
        let request_pricing = crate::server::routes::ai::spend::RequestPricing::from_exact(
            &pricing, "openai", "gpt-4o",
        );

        record_audio_spend(
            &request_pricing,
            &GatewayPricingConfig::default(),
            &budget_limits,
            &key_manager,
            None,
            "openai",
            "gpt-4o",
            None,
            &PricingUsage::new(10, 5),
            None,
            Some(key_budget_reservation),
        )
        .await;

        let spend = budget_manager.get_current_spend(&scope);
        assert!(spend > 0.0, "API key budget spend should be recorded");
        assert!(spend < 0.5, "reservation should settle to actual spend");
    }
}
