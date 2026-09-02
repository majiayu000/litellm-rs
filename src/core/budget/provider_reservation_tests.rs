use super::{
    BudgetAmountError, BudgetPersistenceEvent, BudgetReservationError, BudgetStatus,
    ModelLimitConfig, ProviderLimitConfig, ResetPeriod, UnifiedBudgetLimits,
};

#[test]
fn provider_reservation_settle_refunds_unused_amount_and_counts_once() {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );

    let reservation = limits
        .providers
        .reserve_provider_spend("openai", 10.0)
        .unwrap();
    let usage = limits.providers.get_provider_usage("openai").unwrap();
    assert_eq!(usage.current_spend, 10.0);
    assert_eq!(usage.request_count, 0);

    assert_eq!(reservation.settle(3.0).unwrap(), Some(BudgetStatus::Ok));
    let usage = limits.providers.get_provider_usage("openai").unwrap();
    assert_eq!(usage.current_spend, 3.0);
    assert_eq!(usage.request_count, 1);
}

#[test]
fn provider_reservation_cancel_and_drop_release_amount() {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );

    limits
        .providers
        .reserve_provider_spend("openai", 25.0)
        .unwrap()
        .cancel();
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        0.0
    );

    {
        let _reservation = limits
            .providers
            .reserve_provider_spend("openai", 40.0)
            .unwrap();
        assert_eq!(
            limits
                .providers
                .get_provider_usage("openai")
                .unwrap()
                .current_spend,
            40.0
        );
    }
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        0.0
    );
}

#[test]
fn unified_reservation_rolls_back_provider_when_model_fails() {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(5.0, ResetPeriod::Monthly));

    assert!(matches!(
        limits.reserve_spend("openai", "gpt-4", 10.0),
        Err(BudgetReservationError::ModelBudgetExceeded)
    ));
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        0.0
    );
}

#[test]
fn unified_reservation_settles_provider_and_model() {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));

    let reservation = limits.reserve_spend("openai", "gpt-4", 10.0).unwrap();
    assert_eq!(reservation.reserved_amount(), 10.0);
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        10.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        10.0
    );

    assert_eq!(
        reservation.settle(4.0).unwrap(),
        (Some(BudgetStatus::Ok), Some(BudgetStatus::Ok))
    );
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        4.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        4.0
    );
}

#[test]
fn unified_reservation_records_spend_for_disabled_limits() {
    let limits = UnifiedBudgetLimits::new();
    let mut provider_config = ProviderLimitConfig::new(100.0, ResetPeriod::Monthly);
    provider_config.enabled = false;
    limits
        .providers
        .set_provider_limit("openai", provider_config);
    let mut model_config = ModelLimitConfig::new(100.0, ResetPeriod::Monthly);
    model_config.enabled = false;
    limits.models.set_model_limit("gpt-4", model_config);

    let reservation = limits.reserve_spend("openai", "gpt-4", 10.0).unwrap();
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        0.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        0.0
    );

    assert_eq!(
        reservation.settle(4.0).unwrap(),
        (Some(BudgetStatus::Ok), Some(BudgetStatus::Ok))
    );
    let provider_usage = limits.providers.get_provider_usage("openai").unwrap();
    assert_eq!(provider_usage.current_spend, 4.0);
    assert_eq!(provider_usage.request_count, 1);
    let model_usage = limits.models.get_model_usage("gpt-4").unwrap();
    assert_eq!(model_usage.current_spend, 4.0);
    assert_eq!(model_usage.request_count, 1);
}

#[test]
fn unified_reservation_persists_only_final_settlement() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let limits = UnifiedBudgetLimits::with_persistence(tx);
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));
    while rx.try_recv().is_ok() {}

    let reservation = limits.reserve_spend("openai", "gpt-4", 10.0).unwrap();
    assert!(
        rx.try_recv().is_err(),
        "reserve-time spend must stay transient"
    );

    assert_eq!(
        reservation.settle(4.0).unwrap(),
        (Some(BudgetStatus::Ok), Some(BudgetStatus::Ok))
    );
    let mut persisted = Vec::new();
    for _ in 0..2 {
        match rx.try_recv().expect("settle should persist final spend") {
            BudgetPersistenceEvent::Upsert(snapshot) => {
                persisted.push((snapshot.kind, snapshot.name, snapshot.current_spend));
            }
            BudgetPersistenceEvent::Delete { .. } => panic!("expected upsert"),
        }
    }
    assert!(
        persisted
            .iter()
            .any(|(_, name, current_spend)| name == "openai" && *current_spend == 4.0)
    );
    assert!(
        persisted
            .iter()
            .any(|(_, name, current_spend)| name == "gpt-4" && *current_spend == 4.0)
    );
}

#[test]
fn overlapping_reservation_persistence_excludes_other_temporary_holds() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let limits = UnifiedBudgetLimits::with_persistence(tx);
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));
    while rx.try_recv().is_ok() {}

    let first = limits.reserve_spend("openai", "gpt-4", 10.0).unwrap();
    let second = limits.reserve_spend("openai", "gpt-4", 20.0).unwrap();
    assert!(
        rx.try_recv().is_err(),
        "reserve-time holds must stay transient"
    );

    assert_eq!(
        first.settle(4.0).unwrap(),
        (Some(BudgetStatus::Ok), Some(BudgetStatus::Ok))
    );
    let mut persisted = Vec::new();
    for _ in 0..2 {
        match rx
            .try_recv()
            .expect("settle should persist committed spend")
        {
            BudgetPersistenceEvent::Upsert(snapshot) => {
                persisted.push((snapshot.kind, snapshot.name, snapshot.current_spend));
            }
            BudgetPersistenceEvent::Delete { .. } => panic!("expected upsert"),
        }
    }
    assert!(
        persisted
            .iter()
            .any(|(_, name, current_spend)| name == "openai" && *current_spend == 4.0)
    );
    assert!(
        persisted
            .iter()
            .any(|(_, name, current_spend)| name == "gpt-4" && *current_spend == 4.0)
    );
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        24.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        24.0
    );

    second.cancel();
}

#[test]
fn replacing_limits_preserves_committed_spend_and_drops_stale_outstanding_holds() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let limits = UnifiedBudgetLimits::with_persistence(tx);
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));
    assert_eq!(
        limits.providers.record_provider_spend("openai", 6.0),
        Some(BudgetStatus::Ok)
    );
    assert_eq!(
        limits.models.record_model_spend("gpt-4", 6.0),
        Some(BudgetStatus::Ok)
    );
    while rx.try_recv().is_ok() {}

    let reservation = limits.reserve_spend("openai", "gpt-4", 10.0).unwrap();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));

    let mut replacement_snapshots = Vec::new();
    for _ in 0..2 {
        match rx
            .try_recv()
            .expect("replacing limits should persist new snapshots")
        {
            BudgetPersistenceEvent::Upsert(snapshot) => {
                replacement_snapshots.push((snapshot.kind, snapshot.name, snapshot.current_spend));
            }
            BudgetPersistenceEvent::Delete { .. } => panic!("expected upsert"),
        }
    }
    assert!(
        replacement_snapshots
            .iter()
            .any(|(_, name, current_spend)| name == "openai" && *current_spend == 6.0)
    );
    assert!(
        replacement_snapshots
            .iter()
            .any(|(_, name, current_spend)| name == "gpt-4" && *current_spend == 6.0)
    );

    assert_eq!(
        reservation.settle(4.0).unwrap(),
        (Some(BudgetStatus::Ok), Some(BudgetStatus::Ok))
    );
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        10.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        10.0
    );
}

#[test]
fn unified_reservation_settles_actual_above_reserved_amount() {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(10.0, ResetPeriod::Monthly));

    let reservation = limits.reserve_spend("openai", "gpt-4", 5.0).unwrap();
    assert_eq!(
        reservation.settle(12.0).unwrap(),
        (Some(BudgetStatus::Exceeded), Some(BudgetStatus::Exceeded))
    );
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        12.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        12.0
    );
}

#[test]
fn provider_and_model_reservations_do_not_release_new_period_spend_after_reset() {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));

    let reservation = limits.reserve_spend("openai", "gpt-4", 10.0).unwrap();
    assert!(limits.providers.reset_provider_budget("openai"));
    assert!(limits.models.reset_model_budget("gpt-4"));
    assert_eq!(
        limits.providers.record_provider_spend("openai", 5.0),
        Some(BudgetStatus::Ok)
    );
    assert_eq!(
        limits.models.record_model_spend("gpt-4", 7.0),
        Some(BudgetStatus::Ok)
    );

    assert_eq!(
        reservation.settle(3.0).unwrap(),
        (Some(BudgetStatus::Ok), Some(BudgetStatus::Ok))
    );
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        8.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        10.0
    );

    let reservation = limits.reserve_spend("openai", "gpt-4", 10.0).unwrap();
    assert!(limits.providers.reset_provider_budget("openai"));
    assert!(limits.models.reset_model_budget("gpt-4"));
    assert_eq!(
        limits.providers.record_provider_spend("openai", 5.0),
        Some(BudgetStatus::Ok)
    );
    assert_eq!(
        limits.models.record_model_spend("gpt-4", 7.0),
        Some(BudgetStatus::Ok)
    );

    reservation.cancel();
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        5.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        7.0
    );
}

#[test]
fn provider_and_model_reject_invalid_amounts_without_mutation() {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));

    assert!(!limits.can_spend("unknown", "unknown", f64::NAN));
    assert_eq!(
        limits.providers.record_provider_spend("openai", f64::NAN),
        None
    );
    assert_eq!(
        limits.models.record_model_spend("gpt-4", f64::INFINITY),
        None
    );
    assert!(matches!(
        limits.reserve_spend("openai", "gpt-4", f64::INFINITY),
        Err(BudgetReservationError::InvalidAmount(
            BudgetAmountError::NonFinite
        ))
    ));
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        0.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        0.0
    );
}

#[test]
fn provider_and_model_limits_reject_non_finite_configuration() {
    let limits = UnifiedBudgetLimits::new();

    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(f64::NAN, ResetPeriod::Monthly),
    );
    assert!(limits.providers.get_provider_usage("openai").is_none());

    let mut provider_config = ProviderLimitConfig::new(100.0, ResetPeriod::Monthly);
    provider_config.soft_limit_percentage = f64::INFINITY;
    limits
        .providers
        .set_provider_limit("openai", provider_config);
    assert!(limits.providers.get_provider_usage("openai").is_none());

    limits.models.set_model_limit(
        "gpt-4",
        ModelLimitConfig::new(f64::INFINITY, ResetPeriod::Monthly),
    );
    assert!(limits.models.get_model_usage("gpt-4").is_none());
}
