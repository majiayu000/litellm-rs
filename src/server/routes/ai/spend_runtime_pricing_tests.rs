use super::*;
use crate::config::models::gateway::{GatewayPricingConfig, UnpricedModelPolicy};
use crate::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
use crate::core::keys::InMemoryKeyRepository;
use crate::core::pricing_service::LiteLLMModelInfo;
use crate::core::types::responses::{PromptTokensDetails, Usage};
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
        usage_spend_settlement(
            (&budget, &keys, None),
            (
                "runtime_provider",
                "runtime-only-priced-model",
                Some(&response_usage(1000, 500)),
            ),
            None,
            None,
        ),
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
async fn record_completion_spend_settles_text_cost_when_modal_price_is_missing() {
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
    let mut usage = response_usage(1000, 500);
    usage.prompt_tokens_details = Some(PromptTokensDetails {
        cached_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        audio_tokens: Some(250),
    });

    record_completion_spend_with_reservation_with_pricing(
        &pricing,
        usage_spend_settlement(
            (&budget, &keys, None),
            (
                "runtime_provider",
                "runtime-only-priced-model",
                Some(&usage),
            ),
            None,
            None,
        ),
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
        usage_spend_settlement(
            (&budget, &keys, None),
            (
                "openai_like",
                "xai/grok-4.3",
                Some(&response_usage(1000, 500)),
            ),
            None,
            None,
        ),
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
fn reserve_completion_budget_rejects_missing_pricing_when_limits_are_disabled() {
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

    let error = match reserve_completion_budget_with_pricing(
        &pricing,
        &budget,
        "runtime_provider",
        "missing-priced-model",
        1000,
        Some(500),
    ) {
        Ok(_) => panic!("unpriced requests should fail closed by default"),
        Err(error) => error,
    };

    assert!(super::is_model_not_priced_error(&error));
}

#[test]
fn reserve_completion_budget_rejects_missing_pricing_when_budget_manager_disabled() {
    let pricing = PricingService::new(None);
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "runtime_provider",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.providers.set_enabled(false);

    let error = match reserve_completion_budget_with_pricing(
        &pricing,
        &budget,
        "runtime_provider",
        "missing-priced-model",
        1000,
        Some(500),
    ) {
        Ok(_) => panic!("unpriced requests should fail closed by default"),
        Err(error) => error,
    };

    assert!(super::is_model_not_priced_error(&error));
}

#[tokio::test]
async fn reserve_completion_budget_reject_records_unpriced_metric() {
    let _metrics_guard = crate::server::middleware::MetricsMiddleware::test_lock().await;
    crate::server::middleware::reset_unpriced_metrics_for_tests();
    let pricing = PricingService::new(None);
    let budget = UnifiedBudgetLimits::new();

    let error = match reserve_completion_budget_with_pricing(
        &pricing,
        &budget,
        "metrics-reject-provider",
        "tenant-private-model-831",
        1000,
        Some(500),
    ) {
        Ok(_) => panic!("unpriced requests should fail closed by default"),
        Err(error) => error,
    };

    assert!(super::is_model_not_priced_error(&error));
    let rendered = crate::server::middleware::MetricsMiddleware::render_prometheus();
    assert!(rendered.contains(
        "gateway_unpriced_events_total{provider=\"metrics-reject-provider\",model_bucket=\"other\",policy=\"reject\",outcome=\"reject_preflight\"} 1"
    ));
    assert!(!rendered.contains("tenant-private-model-831"));
}

#[test]
fn reserve_completion_budget_allow_unpriced_uses_fallback_per_1k_units() {
    let pricing = PricingService::new(None);
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "runtime_provider",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let mut pricing_config = GatewayPricingConfig::default();
    pricing_config.unpriced_model_policy = UnpricedModelPolicy::AllowUnpriced;
    pricing_config.unpriced_fallback_cost_per_1k_tokens = Some(0.5);

    let reservation = reserve_completion_budget_with_policy(
        &pricing,
        &pricing_config,
        &budget,
        "runtime_provider",
        "missing-priced-model",
        1000,
        Some(500),
    )
    .expect("allow_unpriced should use fallback")
    .expect("non-zero fallback should reserve");

    assert_eq!(reservation.reserved_amount(), 0.75);
    reservation.cancel();
}

#[tokio::test]
async fn settle_unpriced_usage_records_unpriced_spend_metric() {
    let _metrics_guard = crate::server::middleware::MetricsMiddleware::test_lock().await;
    crate::server::middleware::reset_unpriced_metrics_for_tests();
    let budget = UnifiedBudgetLimits::new();
    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let mut pricing_config = GatewayPricingConfig::default();
    pricing_config.unpriced_model_policy = UnpricedModelPolicy::AllowUnpriced;
    pricing_config.unpriced_fallback_cost_per_1k_tokens = Some(1.0);
    let usage = PricingUsage::new(10, 5);

    settle_unpriced_usage(
        &pricing_config,
        &budget,
        &keys,
        None,
        "metrics-allow-provider",
        "tenant-private-model-832",
        &usage,
        None,
        None,
        "metrics test",
    )
    .await;

    let rendered = crate::server::middleware::MetricsMiddleware::render_prometheus();
    assert!(rendered.contains(
        "gateway_unpriced_events_total{provider=\"metrics-allow-provider\",model_bucket=\"other\",policy=\"allow_unpriced\",outcome=\"fallback_settled\"} 1"
    ));
    assert!(rendered.contains(
        "gateway_unpriced_spend_total{provider=\"metrics-allow-provider\",model_bucket=\"other\",policy=\"allow_unpriced\",outcome=\"fallback_settled\"} 0.015000000"
    ));
    assert!(!rendered.contains("tenant-private-model-832"));
}
