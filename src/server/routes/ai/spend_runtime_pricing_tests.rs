use super::*;
use crate::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
use crate::core::keys::InMemoryKeyRepository;
use crate::core::pricing_service::LiteLLMModelInfo;
use crate::core::types::responses::Usage;
use std::collections::HashMap;

fn response_usage(prompt: u32, completion: u32) -> Usage {
    Usage {
        prompt_tokens: prompt,
        completion_tokens: completion,
        total_tokens: prompt + completion,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
    }
}

fn runtime_test_pricing_service(provider: &str) -> PricingService {
    let service = PricingService::new(None);
    service.add_custom_model(
        "runtime-only-priced-model".to_string(),
        LiteLLMModelInfo {
            max_tokens: Some(8192),
            max_input_tokens: Some(8192),
            max_output_tokens: Some(2048),
            input_cost_per_token: Some(0.00001),
            output_cost_per_token: Some(0.00003),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(true),
            supports_vision: Some(false),
            supports_streaming: Some(true),
            supports_parallel_function_calling: Some(true),
            supports_system_message: Some(true),
            extra: HashMap::new(),
        },
    );
    service
}

#[test]
fn reserve_completion_budget_uses_runtime_pricing_service() {
    let pricing = runtime_test_pricing_service("runtime_provider");
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "runtime_provider",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "runtime-only-priced-model",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );

    let reservation = match reserve_completion_budget_with_pricing(
        &pricing,
        &budget,
        "runtime_provider",
        "runtime-only-priced-model",
        1000,
        Some(500),
    ) {
        Ok(Some(reservation)) => reservation,
        Ok(None) => panic!("priced runtime model should create a budget reservation"),
        Err(error) => panic!("runtime pricing reservation should succeed: {error}"),
    };

    assert_eq!(reservation.reserved_amount(), 0.025);
    assert_eq!(
        budget
            .providers
            .get_provider_usage("runtime_provider")
            .map(|usage| usage.current_spend),
        Some(0.025)
    );
    reservation.cancel();
}

#[test]
fn reserve_completion_budget_prices_xai_openai_like_prefix() {
    let pricing = default_spend_pricing_service();
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai_like",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "xai/grok-4.3",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );

    let reservation = match reserve_completion_budget_with_pricing(
        pricing,
        &budget,
        "openai_like",
        "xai/grok-4.3",
        1000,
        Some(500),
    ) {
        Ok(Some(reservation)) => reservation,
        Ok(None) => panic!("priced xAI OpenAI-like model should create a budget reservation"),
        Err(error) => panic!("xAI OpenAI-like budget reservation should succeed: {error}"),
    };

    assert!((reservation.reserved_amount() - 0.0025).abs() < f64::EPSILON);
    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai_like")
            .map(|usage| usage.current_spend),
        Some(0.0025)
    );
    reservation.cancel();
}

#[cfg(feature = "providers-extended")]
#[test]
fn reserve_completion_budget_prices_amazon_nova_short_alias() {
    let pricing = default_spend_pricing_service();
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "amazon_nova",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "nova-2-lite",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );

    let reservation = match reserve_completion_budget_with_pricing(
        pricing,
        &budget,
        "amazon_nova",
        "nova-2-lite",
        1000,
        Some(500),
    ) {
        Ok(Some(reservation)) => reservation,
        Ok(None) => panic!("priced Amazon Nova short alias should create a budget reservation"),
        Err(error) => panic!("Amazon Nova short alias reservation should succeed: {error}"),
    };

    assert!((reservation.reserved_amount() - 0.00155).abs() < f64::EPSILON);
    reservation.cancel();
}

#[tokio::test]
async fn record_completion_spend_uses_runtime_pricing_service() {
    let pricing = runtime_test_pricing_service("runtime_provider");
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "runtime_provider",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "runtime-only-priced-model",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());

    record_completion_spend_with_reservation_with_pricing(
        &pricing,
        &budget,
        &keys,
        None,
        "runtime_provider",
        "runtime-only-priced-model",
        Some(&response_usage(1000, 500)),
        None,
        None,
    )
    .await;

    assert_eq!(
        budget
            .providers
            .get_provider_usage("runtime_provider")
            .map(|usage| usage.current_spend),
        Some(0.025)
    );
    assert_eq!(
        budget
            .models
            .get_model_usage("runtime-only-priced-model")
            .map(|usage| usage.current_spend),
        Some(0.025)
    );
}

#[tokio::test]
async fn record_completion_spend_prices_xai_openai_like_prefix() {
    let pricing = default_spend_pricing_service();
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai_like",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "xai/grok-4.3",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());

    record_completion_spend_with_reservation_with_pricing(
        pricing,
        &budget,
        &keys,
        None,
        "openai_like",
        "xai/grok-4.3",
        Some(&response_usage(1000, 500)),
        None,
        None,
    )
    .await;

    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai_like")
            .map(|usage| usage.current_spend),
        Some(0.0025)
    );
    assert_eq!(
        budget
            .models
            .get_model_usage("xai/grok-4.3")
            .map(|usage| usage.current_spend),
        Some(0.0025)
    );
}

#[test]
fn reserve_completion_budget_rejects_missing_pricing_when_budget_requires_cost() {
    let pricing = PricingService::new(None);
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "runtime_provider",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );

    let error = match reserve_completion_budget_with_pricing(
        &pricing,
        &budget,
        "runtime_provider",
        "missing-priced-model",
        1000,
        Some(500),
    ) {
        Ok(_) => panic!("budgeted requests without pricing should fail closed"),
        Err(error) => error,
    };

    assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    assert_eq!(
        budget
            .providers
            .get_provider_usage("runtime_provider")
            .map(|usage| usage.current_spend)
            .unwrap_or(0.0),
        0.0
    );
}

#[test]
fn reserve_completion_budget_allows_missing_pricing_when_limits_are_disabled() {
    let pricing = PricingService::new(None);
    let budget = UnifiedBudgetLimits::new();
    let mut provider_config = ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly);
    provider_config.enabled = false;
    budget
        .providers
        .set_provider_limit("runtime_provider", provider_config);
    let mut model_config = ModelLimitConfig::new(1000.0, ResetPeriod::Monthly);
    model_config.enabled = false;
    budget
        .models
        .set_model_limit("missing-priced-model", model_config);

    let reservation = match reserve_completion_budget_with_pricing(
        &pricing,
        &budget,
        "runtime_provider",
        "missing-priced-model",
        1000,
        Some(500),
    ) {
        Ok(reservation) => reservation,
        Err(error) => panic!("disabled budget limits should not require pricing: {error}"),
    };

    assert!(reservation.is_none());
}

#[test]
fn reserve_completion_budget_allows_missing_pricing_when_budget_manager_disabled() {
    let pricing = PricingService::new(None);
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "runtime_provider",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.providers.set_enabled(false);

    let reservation = match reserve_completion_budget_with_pricing(
        &pricing,
        &budget,
        "runtime_provider",
        "missing-priced-model",
        1000,
        Some(500),
    ) {
        Ok(reservation) => reservation,
        Err(error) => panic!("disabled budget manager should not require pricing: {error}"),
    };

    assert!(reservation.is_none());
}
