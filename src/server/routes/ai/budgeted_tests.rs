use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{
    BudgetConfig, BudgetManager, BudgetScope, ResetPeriod, UnifiedBudgetReservation,
};
use crate::core::budget::{ModelLimitConfig, ProviderLimitConfig, UnifiedBudgetLimits};
use crate::core::keys::{InMemoryKeyRepository, KeyManager};
use crate::core::pricing_service::PricingService;

use super::{ApiKeyBudgetPolicy, BudgetedCall, SettledStream};

pub(super) fn limited_budget() -> Arc<UnifiedBudgetLimits> {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(1.0, ResetPeriod::Monthly));
    Arc::new(limits)
}

#[tokio::test]
async fn reserve_failure_prevents_provider_call() {
    let limits = limited_budget();
    let called = Arc::new(AtomicBool::new(false));

    let result = BudgetedCall::new(limits.clone(), "openai", "gpt-4")
        .reserve_call_settle(
            async |context| {
                limits
                    .reserve_spend(context.provider(), context.model(), 2.0)
                    .map(Some)
                    .map_err(|error| {
                        super::spend::reservation_error_to_provider_error(
                            error,
                            context.provider(),
                            context.model(),
                        )
                    })
            },
            {
                let called = called.clone();
                move || async move {
                    called.store(true, Ordering::Relaxed);
                    Ok::<_, crate::core::providers::ProviderError>("should not run")
                }
            },
            |value, _reservations, _context| async move { (value, 0) },
        )
        .await;

    assert!(result.is_err());
    assert!(!called.load(Ordering::Relaxed));
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .map(|usage| usage.current_spend)
            .unwrap_or_default(),
        0.0
    );
}

#[tokio::test]
async fn provider_failure_cancels_provider_model_and_key_reservations() {
    let limits = limited_budget();
    let budget_manager = Arc::new(BudgetManager::new());
    let scope = BudgetScope::ApiKey("key-budget".to_string());
    let budget = budget_manager
        .create_budget(scope.clone(), BudgetConfig::new("key budget", 1.0))
        .await
        .expect("key budget should be created");
    let budget_id = uuid::Uuid::parse_str(&budget.id).expect("budget id should be UUID");

    let result = BudgetedCall::new(limits.clone(), "openai", "gpt-4")
        .with_api_key_budget(
            budget_manager.clone(),
            Some(budget_id),
            ApiKeyBudgetPolicy::FromProviderReservation,
        )
        .reserve_call_settle(
            async |context| {
                limits
                    .reserve_spend(context.provider(), context.model(), 0.25)
                    .map(Some)
                    .map_err(|error| {
                        super::spend::reservation_error_to_provider_error(
                            error,
                            context.provider(),
                            context.model(),
                        )
                    })
            },
            || async {
                Err::<(), _>(crate::core::providers::ProviderError::timeout(
                    "test",
                    "upstream failed",
                ))
            },
            |value, _reservations, _context| async move { (value, 0) },
        )
        .await;

    assert!(result.is_err());
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .expect("provider usage should exist")
            .current_spend,
        0.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .expect("model usage should exist")
            .current_spend,
        0.0
    );
    assert_eq!(budget_manager.get_current_spend(&scope), 0.0);
}

#[tokio::test]
async fn key_reservation_mode_uses_precomputed_cost_without_provider_model_reservation() {
    let limits = limited_budget();
    let budget_manager = Arc::new(BudgetManager::new());
    let scope = BudgetScope::ApiKey("image-proxy-key-budget".to_string());
    let budget = budget_manager
        .create_budget(
            scope.clone(),
            BudgetConfig::new("image proxy key budget", 1.0),
        )
        .await
        .expect("key budget should be created");
    let budget_id = uuid::Uuid::parse_str(&budget.id).expect("budget id should be UUID");

    let ((), reservations) = BudgetedCall::new(limits.clone(), "openai", "gpt-image-1-mini")
        .with_api_key_budget(
            budget_manager.clone(),
            Some(budget_id),
            ApiKeyBudgetPolicy::RequirePricedReservation,
        )
        .with_precomputed_api_key_budget_cost(Some(0.25))
        .reserve_call(async |_context| Ok(None), {
            let limits = limits.clone();
            let budget_manager = budget_manager.clone();
            let scope = scope.clone();
            move || async move {
                assert_eq!(
                    limits
                        .providers
                        .get_provider_usage("openai")
                        .map(|usage| usage.current_spend)
                        .unwrap_or_default(),
                    0.0
                );
                assert_eq!(
                    limits
                        .models
                        .get_model_usage("gpt-image-1-mini")
                        .map(|usage| usage.current_spend)
                        .unwrap_or_default(),
                    0.0
                );
                assert_eq!(budget_manager.get_current_spend(&scope), 0.25);
                Ok::<_, crate::core::providers::ProviderError>(())
            }
        })
        .await
        .expect("precomputed key budget should reserve without provider/model reservation");

    let (budget, key) = reservations.into_parts();
    assert!(budget.is_none());
    super::spend::settle_api_key_budget_reservation(key, 0.10, "image proxy test").await;
    assert_eq!(budget_manager.get_current_spend(&scope), 0.10);
}

#[tokio::test]
async fn provider_model_reservation_blocks_concurrent_budget_overrun() {
    let limits = limited_budget();

    let first = BudgetedCall::new(limits.clone(), "openai", "gpt-4")
        .reserve_call(
            async |context| context.reserve_spend(0.75).await.map(Some),
            {
                let limits = limits.clone();
                move || async move {
                    assert!(
                        limits.reserve_spend("openai", "gpt-4", 0.50).is_err(),
                        "in-flight reservation must count against remaining budget"
                    );
                    Ok::<_, crate::core::providers::ProviderError>(())
                }
            },
        )
        .await;
    assert!(first.is_ok(), "first reservation should fit the budget");
    let (_, reservations) = match first {
        Ok(value) => value,
        Err(error) => panic!("first reservation failed unexpectedly: {error}"),
    };

    let (reservation, key_reservation) = reservations.into_parts();
    assert!(key_reservation.is_none());
    let Some(reservation) = reservation else {
        panic!("provider/model reservation should be present");
    };
    assert!(
        reservation.settle(0.75).is_ok(),
        "settlement should fit reservation"
    );
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .map(|usage| usage.current_spend),
        Some(0.75)
    );
}

#[test]
fn availability_only_ensure_available_does_not_record_spend() {
    let limits = limited_budget();

    BudgetedCall::new(limits.clone(), "openai", "gpt-4")
        .ensure_available()
        .expect("availability check should pass without recording spend");

    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .map(|usage| usage.current_spend)
            .unwrap_or_default(),
        0.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .map(|usage| usage.current_spend)
            .unwrap_or_default(),
        0.0
    );
}

#[test]
fn ensure_available_rejects_metered_mode() {
    let error = BudgetedCall::new(limited_budget(), "openai", "gpt-4")
        .with_settlement_mode(ApiKeyBudgetPolicy::FromProviderReservation)
        .ensure_available()
        .expect_err("metered budget calls must use reservation settlement");

    assert_eq!(error.http_status(), 400);
}

#[tokio::test]
async fn success_hands_reservations_to_settlement() {
    let limits = limited_budget();
    let settled = Arc::new(AtomicBool::new(false));

    let (value, tokens) = BudgetedCall::new(limits.clone(), "openai", "gpt-4")
        .reserve_call_settle(
            async |context| {
                limits
                    .reserve_spend(context.provider(), context.model(), 0.25)
                    .map(Some)
                    .map_err(|error| {
                        super::spend::reservation_error_to_provider_error(
                            error,
                            context.provider(),
                            context.model(),
                        )
                    })
            },
            || async { Ok::<_, crate::core::providers::ProviderError>("ok") },
            {
                let settled = settled.clone();
                move |value, reservations, _context| async move {
                    let (budget, key) = reservations.into_parts();
                    let settlement_failed = budget
                        .expect("provider/model reservation should be present")
                        .settle(f64::NAN)
                        .is_err();
                    assert!(key.is_none());
                    assert!(settlement_failed);
                    settled.store(true, Ordering::Relaxed);
                    (value, 17)
                }
            },
        )
        .await
        .expect("provider success should return response even when settlement logs internally");

    assert_eq!(value, "ok");
    assert_eq!(tokens, 17);
    assert!(settled.load(Ordering::Relaxed));
}

pub(super) fn settled_stream(
    budget_limits: Arc<UnifiedBudgetLimits>,
    reservation: Option<UnifiedBudgetReservation>,
) -> SettledStream {
    let pricing_service = Arc::new(
        PricingService::with_embedded_default()
            .expect("embedded pricing should load for settled stream tests"),
    );
    let request_pricing = crate::server::routes::ai::spend::RequestPricing::from_exact(
        pricing_service.as_ref(),
        "openai",
        "gpt-4",
    );
    SettledStream {
        pricing_service,
        pricing_config: GatewayPricingConfig::default(),
        budget_limits,
        key_manager: KeyManager::new(InMemoryKeyRepository::new()),
        api_key_id: None,
        provider: "openai".to_string(),
        model: "gpt-4".to_string(),
        request_pricing,
        budget_reservation: reservation,
        key_budget_reservation: None,
        ledger_facts: None,
    }
}

#[tokio::test]
async fn settled_stream_completion_without_usage_settles_reserved_output() {
    let limits = limited_budget();
    let reservation = limits
        .reserve_spend("openai", "gpt-4", 0.25)
        .expect("reservation should fit test budget");

    settled_stream(limits.clone(), Some(reservation))
        .record_completion(None, true)
        .await;

    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .expect("provider usage should be recorded")
            .current_spend,
        0.25
    );
}

#[tokio::test]
async fn settled_stream_completion_without_output_settles_reserved_output() {
    let limits = limited_budget();
    let reservation = limits
        .reserve_spend("openai", "gpt-4", 0.25)
        .expect("reservation should fit test budget");

    settled_stream(limits.clone(), Some(reservation))
        .record_completion(None, false)
        .await;

    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .expect("provider reservation should exist")
            .current_spend,
        0.25
    );
}

#[tokio::test]
async fn persistent_key_budget_timeout_retains_hold_but_explicit_rejection_refunds() {
    use crate::core::keys::{CreateKeyConfig, DatabaseKeyRepository, KeyManager};
    let mut config = crate::config::Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    let storage = Arc::new(
        crate::storage::StorageLayer::new(&config.gateway.storage)
            .await
            .unwrap(),
    );
    storage.migrate().await.unwrap();
    let keys = KeyManager::new(DatabaseKeyRepository::new(storage.clone()));
    let (key, _) = keys
        .generate_key(CreateKeyConfig {
            name: "timeout-budget".into(),
            max_budget: Some(1.0),
            ..Default::default()
        })
        .await
        .unwrap();
    let id = keys.get_key(key).await.unwrap().unwrap().budget_id.unwrap();
    let manager = Arc::new(BudgetManager::new().with_api_key_database(storage.database.clone()));
    for error in [
        ProviderError::timeout("test", "no reply"),
        ProviderError::invalid_request("test", "rejected"),
    ] {
        let call = BudgetedCall::new(Arc::new(UnifiedBudgetLimits::new()), "test", "model")
            .with_api_key_budget(
                manager.clone(),
                Some(id),
                SettlementMode::KeyReservationThenPostSuccessRecord,
            )
            .with_precomputed_api_key_budget_cost(Some(0.4));
        let result = call
            .reserve_call(async |_| Ok(None), || async { Err::<(), _>(error) })
            .await;
        assert!(result.is_err());
        assert_eq!(
            storage
                .database
                .api_key_budget(id)
                .await
                .unwrap()
                .unwrap()
                .used_amount,
            400_000_000
        );
    }
}
