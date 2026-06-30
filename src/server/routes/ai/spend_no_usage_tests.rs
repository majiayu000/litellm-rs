use super::*;
use crate::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
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

    record_completion_spend_with_reservation(
        &budget,
        &keys,
        Some(key_id),
        "openai",
        "gpt-4o",
        None,
        Some(reservation),
        None,
    )
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
