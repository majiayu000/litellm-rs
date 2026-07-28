use super::*;
use crate::core::budget::{
    BudgetConfig, BudgetManager, BudgetScope, ModelLimitConfig, ProviderLimitConfig, ResetPeriod,
};
use crate::core::keys::{CreateKeyConfig, InMemoryKeyRepository};

#[tokio::test]
async fn successful_completion_without_usage_settles_reserved_budget() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let (key_id, _) = keys
        .generate_key(CreateKeyConfig {
            name: "completion key".to_string(),
            ..Default::default()
        })
        .await
        .expect("test key should be created");
    let reservation = reserve_completion_budget(&budget, "openai", "gpt-4o", 0, Some(100))
        .expect("reservation should succeed")
        .expect("priced model should reserve budget");
    let reserved = reservation.reserved_amount();

    record_completion_spend_with_reservation(usage_spend_settlement(
        (&budget, &keys, Some(key_id)),
        ("openai", "gpt-4o", None),
        Some(reservation),
        None,
    ))
    .await;

    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        reserved
    );
    assert_eq!(
        budget
            .models
            .get_model_usage("gpt-4o")
            .unwrap()
            .current_spend,
        reserved
    );
    let stats = keys
        .get_usage_stats(key_id)
        .await
        .expect("usage stats should be readable");
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.total_tokens, 0);
    assert_eq!(stats.total_cost, reserved);
}

async fn assert_no_usage_reservation_case(
    provider_amount: Option<f64>,
    key_amount: Option<f64>,
    key_budget_enabled: bool,
) {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    budget
        .models
        .set_model_limit("gpt-4o", ModelLimitConfig::new(10.0, ResetPeriod::Monthly));
    let provider_reservation = provider_amount.map(|amount| {
        budget
            .reserve_spend("openai", "gpt-4o", amount)
            .expect("provider reservation")
    });

    let budget_manager = BudgetManager::new();
    let key_scope = BudgetScope::ApiKey("no-usage-key-budget".to_string());
    let mut key_budget_config = BudgetConfig::new("no usage key budget", 10.0);
    key_budget_config.enabled = Some(key_budget_enabled);
    budget_manager
        .create_budget(key_scope.clone(), key_budget_config)
        .await
        .expect("key budget");
    let key_reservation = key_amount.map(|amount| {
        budget_manager
            .tracker()
            .reserve_spend(&key_scope, amount)
            .expect("key reservation")
    });
    let key_reserved = key_reservation
        .as_ref()
        .map(BudgetReservation::reserved_amount);
    let has_key_reservation = key_reservation.is_some();

    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let (key_id, _) = keys
        .generate_key(CreateKeyConfig {
            name: "no usage matrix key".to_string(),
            ..Default::default()
        })
        .await
        .expect("test key");
    record_reserved_spend_without_usage(
        &keys,
        Some(key_id),
        "openai",
        "gpt-4o",
        provider_reservation,
        key_reservation,
        "no usage matrix",
    )
    .await;

    let provider_spend = budget
        .providers
        .get_provider_usage("openai")
        .expect("provider budget")
        .current_spend;
    let model_spend = budget
        .models
        .get_model_usage("gpt-4o")
        .expect("model budget")
        .current_spend;
    let expected_provider = provider_amount.unwrap_or(0.0);
    assert!((provider_spend - expected_provider).abs() < f64::EPSILON);
    assert!((model_spend - expected_provider).abs() < f64::EPSILON);
    let expected_cost = key_reserved
        .filter(|amount| *amount > 0.0)
        .or_else(|| provider_amount.filter(|amount| *amount > 0.0));
    let expected_key_spend = if has_key_reservation {
        expected_cost.unwrap_or(0.0)
    } else {
        0.0
    };
    assert!(
        (budget_manager.get_current_spend(&key_scope) - expected_key_spend).abs() < f64::EPSILON
    );

    let stats = keys.get_usage_stats(key_id).await.expect("key usage stats");
    let expected_requests = u64::from(expected_cost.is_some());
    assert_eq!(stats.total_requests, expected_requests);
    assert_eq!(stats.total_tokens, 0);
    assert!((stats.total_cost - expected_cost.unwrap_or(0.0)).abs() < f64::EPSILON);
}

#[tokio::test]
async fn no_usage_settlement_uses_each_reservation_own_amount() {
    assert_no_usage_reservation_case(Some(0.4), Some(0.2), true).await;
    assert_no_usage_reservation_case(Some(0.4), None, true).await;
    assert_no_usage_reservation_case(None, Some(0.2), true).await;
    assert_no_usage_reservation_case(None, None, true).await;
    assert_no_usage_reservation_case(Some(0.4), Some(0.2), false).await;
    assert_no_usage_reservation_case(None, Some(0.2), false).await;
}
