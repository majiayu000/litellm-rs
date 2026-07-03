use crate::core::budget::{
    BudgetManager, BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation,
};
use crate::core::keys::KeyManager;
use crate::core::pricing_service::PricingService;
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::ProviderError;
use uuid::Uuid;

const ESTIMATED_AUDIO_BYTES_PER_SECOND: usize = 16_000;

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
pub(super) fn reserve_audio_budget_with_pricing(
    pricing_service: &PricingService,
    budget_manager: &BudgetManager,
    budget_limits: &UnifiedBudgetLimits,
    api_key_budget_id: Option<Uuid>,
    budget_provider: &str,
    budget_model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    total_time_seconds: Option<f64>,
    usage: &PricingUsage,
) -> Result<(Option<UnifiedBudgetReservation>, Option<BudgetReservation>), ProviderError> {
    super::super::spend::ensure_budget_available(budget_limits, budget_provider, budget_model)?;
    let budget_reservation = if is_time_priced_audio(
        pricing_service,
        pricing_provider,
        pricing_model,
        total_time_seconds,
    ) {
        let cost = pricing_service
            .calculate_loaded_completion_cost_for_provider(
                pricing_provider,
                pricing_model,
                0,
                0,
                None,
                None,
                total_time_seconds,
            )
            .map_err(|error| {
                ProviderError::invalid_request(
                    "pricing",
                    format!(
                        "pricing is required for audio budget reservation for \
                         '{budget_provider}'/'{budget_model}': {error}"
                    ),
                )
            })?
            .total_cost;
        if cost > 0.0 {
            budget_limits
                .reserve_spend(budget_provider, budget_model, cost)
                .map(Some)
                .map_err(|error| {
                    super::super::spend::reservation_error_to_provider_error(
                        error,
                        budget_provider,
                        budget_model,
                    )
                })?
        } else {
            super::super::spend::ensure_budget_available(
                budget_limits,
                budget_provider,
                budget_model,
            )?;
            None
        }
    } else {
        super::super::spend::reserve_pricing_usage_budget_with_pricing(
            pricing_service,
            budget_limits,
            budget_provider,
            budget_model,
            pricing_provider,
            pricing_model,
            usage,
        )?
    };
    let key_budget_reservation = super::super::spend::reserve_api_key_budget_for_reservation(
        budget_manager,
        api_key_budget_id,
        budget_reservation.as_ref(),
    )?;
    Ok((budget_reservation, key_budget_reservation))
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn record_audio_spend(
    pricing_service: &PricingService,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<uuid::Uuid>,
    budget_provider: &str,
    budget_model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    total_time_seconds: Option<f64>,
    usage: &PricingUsage,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) {
    if let Some(total_time_seconds) = total_time_seconds
        && is_time_priced_audio(
            pricing_service,
            pricing_provider,
            pricing_model,
            Some(total_time_seconds),
        )
    {
        match pricing_service.calculate_loaded_completion_cost_for_provider(
            pricing_provider,
            pricing_model,
            0,
            0,
            None,
            None,
            Some(total_time_seconds),
        ) {
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
                    "time-based audio spend calculation failed for pricing provider \
                     '{pricing_provider}' budget provider '{budget_provider}' model \
                     '{budget_model}': {error}; skipping budget spend"
                );
                let reserved = settle_reserved_audio_budget_on_error(
                    budget_reservation,
                    key_budget_reservation,
                    "time-based audio spend calculation failed",
                );
                record_key_usage(key_manager, api_key_id, usage, reserved).await;
            }
        }
        return;
    }

    super::super::spend::record_pricing_usage_spend_with_reservation_with_pricing(
        pricing_service,
        budget_limits,
        key_manager,
        api_key_id,
        budget_provider,
        budget_model,
        pricing_provider,
        pricing_model,
        usage,
        budget_reservation,
        key_budget_reservation,
    )
    .await;
}

fn is_time_priced_audio(
    pricing_service: &PricingService,
    pricing_provider: &str,
    pricing_model: &str,
    total_time_seconds: Option<f64>,
) -> bool {
    total_time_seconds.is_some()
        && pricing_service
            .get_model_info_for_provider(pricing_provider, pricing_model)
            .map(|(_, model_info)| model_info.cost_per_second.is_some())
            .unwrap_or(false)
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

fn settle_reserved_audio_budget_on_error(
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    context: &str,
) -> f64 {
    let Some(budget_reservation) = budget_reservation else {
        super::super::spend::settle_api_key_budget_reservation(
            key_budget_reservation,
            0.0,
            context,
        );
        return 0.0;
    };
    let reserved = budget_reservation.reserved_amount();
    if let Err(error) = budget_reservation.settle(reserved) {
        tracing::error!("failed to settle {context}: {error:?}");
    }
    super::super::spend::settle_api_key_budget_reservation(
        key_budget_reservation,
        reserved,
        context,
    );
    reserved
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
    use crate::core::budget::{BudgetConfig, BudgetScope};
    use crate::core::keys::InMemoryKeyRepository;

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

        record_audio_spend(
            &pricing,
            &budget_limits,
            &key_manager,
            None,
            "openai",
            "gpt-4o",
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
