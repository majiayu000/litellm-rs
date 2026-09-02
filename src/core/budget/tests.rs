//! Integration tests for the budget management system

use super::*;
use std::sync::Arc;

#[tokio::test]
async fn test_budget_system_initialization() {
    let (manager, alert_manager) = init_budget_system();

    assert_eq!(manager.budget_count(), 0);
    assert!(alert_manager.is_enabled().await);
}

#[tokio::test]
async fn test_budget_system_with_custom_config() {
    let manager_config = BudgetManagerConfig {
        enabled: true,
        default_soft_limit_percentage: 0.75,
        block_on_exceeded: true,
        auto_reset_enabled: false,
        reset_check_interval_secs: 300,
    };

    let alert_config = AlertConfig {
        enabled: true,
        soft_limit_percentage: 0.75,
        warning_thresholds: vec![0.9],
        max_history_size: 500,
        duplicate_suppression_secs: 1800,
    };

    let (manager, alert_manager) = init_budget_system_with_config(manager_config, alert_config);

    assert!(manager.is_enabled().await);

    let config = alert_manager.get_config().await;
    assert_eq!(config.soft_limit_percentage, 0.75);
}

#[tokio::test]
async fn test_end_to_end_budget_workflow() {
    let manager = Arc::new(BudgetManager::new());
    let alert_manager = Arc::new(BudgetAlertManager::new());

    // Create a user budget
    let config = BudgetConfig::new("User 1 Budget", 100.0)
        .with_reset_period(ResetPeriod::Monthly)
        .with_currency(Currency::USD);

    let budget = manager
        .create_budget(BudgetScope::User("user-1".to_string()), config)
        .await
        .unwrap();

    assert_eq!(budget.name, "User 1 Budget");
    assert_eq!(budget.max_budget, 100.0);
    assert_eq!(budget.soft_limit, 80.0);
    assert_eq!(budget.status(), BudgetStatus::Ok);

    // Record some spending
    let result1 = manager
        .record_spend(&BudgetScope::User("user-1".to_string()), 30.0)
        .await
        .unwrap();

    assert_eq!(result1.current_spend, 30.0);
    assert_eq!(result1.new_status, BudgetStatus::Ok);
    assert!(!result1.should_alert_soft_limit);

    // Record more spending to trigger soft limit
    let result2 = manager
        .record_spend(&BudgetScope::User("user-1".to_string()), 51.0)
        .await
        .unwrap();

    assert_eq!(result2.current_spend, 81.0);
    assert_eq!(result2.new_status, BudgetStatus::Warning);
    assert!(result2.should_alert_soft_limit);

    // Process the alert
    let budget_now = manager
        .get_budget(&BudgetScope::User("user-1".to_string()))
        .unwrap();
    alert_manager
        .process_spend_result(&result2, &budget_now)
        .await;

    let alerts = alert_manager.get_all_alerts().await;
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::SoftLimitReached);

    // Record more to exceed budget
    let result3 = manager
        .record_spend(&BudgetScope::User("user-1".to_string()), 20.0)
        .await
        .unwrap();

    assert_eq!(result3.current_spend, 101.0);
    assert_eq!(result3.new_status, BudgetStatus::Exceeded);
    assert!(result3.should_alert_exceeded);

    // Check budget should now be blocked
    let check_result = manager
        .check_spend(&BudgetScope::User("user-1".to_string()), 1.0)
        .await;
    assert!(!check_result.allowed);

    // Reset the budget
    manager
        .reset_budget(&BudgetScope::User("user-1".to_string()))
        .await
        .unwrap();

    let after_reset = manager
        .get_budget(&BudgetScope::User("user-1".to_string()))
        .unwrap();
    assert_eq!(after_reset.current_spend, 0.0);
    assert_eq!(after_reset.status(), BudgetStatus::Ok);
}

#[tokio::test]
async fn test_multiple_budget_scopes() {
    let manager = BudgetManager::new();

    // Create budgets for different scopes
    manager
        .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 10000.0))
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::Team("team-1".to_string()),
            BudgetConfig::new("Team 1", 1000.0),
        )
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::User("user-1".to_string()),
            BudgetConfig::new("User 1", 100.0),
        )
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::ApiKey("sk-123...".to_string()),
            BudgetConfig::new("API Key 1", 50.0),
        )
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::Provider("openai".to_string()),
            BudgetConfig::new("OpenAI Provider", 5000.0),
        )
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::Model("gpt-4".to_string()),
            BudgetConfig::new("GPT-4 Model", 2000.0),
        )
        .await
        .unwrap();

    assert_eq!(manager.budget_count(), 6);

    // List by type
    let user_budgets = manager.list_budgets_filtered(Some("user"), None);
    assert_eq!(user_budgets.len(), 1);

    let provider_budgets = manager.list_budgets_filtered(Some("provider"), None);
    assert_eq!(provider_budgets.len(), 1);
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn test_budget_recorder() {
    let manager = Arc::new(BudgetManager::new());

    // Create budgets
    manager
        .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 10000.0))
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::User("user-1".to_string()),
            BudgetConfig::new("User 1", 100.0),
        )
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::Model("gpt-4".to_string()),
            BudgetConfig::new("GPT-4", 1000.0),
        )
        .await
        .unwrap();

    let recorder = BudgetRecorder::new(Arc::clone(&manager));

    // Simulate a request that uses GPT-4
    recorder
        .record_request_spend(Some("user-1"), None, None, Some("gpt-4"), None, 0.05)
        .await;

    // Check all scopes were updated
    assert_eq!(manager.get_current_spend(&BudgetScope::Global), 0.05);
    assert_eq!(
        manager.get_current_spend(&BudgetScope::User("user-1".to_string())),
        0.05
    );
    assert_eq!(
        manager.get_current_spend(&BudgetScope::Model("gpt-4".to_string())),
        0.05
    );
}

#[tokio::test]
async fn test_budget_summary() {
    let manager = BudgetManager::new();

    manager
        .create_budget(
            BudgetScope::User("user-1".to_string()),
            BudgetConfig::new("User 1", 100.0),
        )
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::User("user-2".to_string()),
            BudgetConfig::new("User 2", 100.0),
        )
        .await
        .unwrap();

    manager
        .create_budget(
            BudgetScope::User("user-3".to_string()),
            BudgetConfig::new("User 3", 100.0),
        )
        .await
        .unwrap();

    // Record different spend amounts
    manager
        .record_spend(&BudgetScope::User("user-1".to_string()), 20.0)
        .await;
    manager
        .record_spend(&BudgetScope::User("user-2".to_string()), 85.0)
        .await; // Warning
    manager
        .record_spend(&BudgetScope::User("user-3".to_string()), 110.0)
        .await; // Exceeded

    let summary = manager.get_summary();

    assert_eq!(summary.total_budgets, 3);
    assert_eq!(summary.total_allocated, 300.0);
    assert_eq!(summary.total_spent, 215.0);
    assert_eq!(summary.total_remaining, 85.0);
    assert_eq!(summary.ok_count, 1);
    assert_eq!(summary.warning_count, 1);
    assert_eq!(summary.exceeded_count, 1);
}

#[tokio::test]
async fn test_alert_workflow() {
    let manager = Arc::new(BudgetManager::new());
    let alert_manager = Arc::new(BudgetAlertManager::new());

    manager
        .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
        .await
        .unwrap();

    // Record spend that triggers soft limit
    let result = manager
        .record_spend(&BudgetScope::Global, 85.0)
        .await
        .unwrap();

    let budget = manager.get_budget(&BudgetScope::Global).unwrap();
    alert_manager.process_spend_result(&result, &budget).await;

    let stats = alert_manager.get_alert_stats().await;
    assert_eq!(stats.warning_count, 1);
    assert_eq!(stats.soft_limit_alerts, 1);
    assert_eq!(stats.unacknowledged, 1);

    // Acknowledge all alerts
    let alerts = alert_manager.get_all_alerts().await;
    for alert in &alerts {
        alert_manager.acknowledge_alert(&alert.id).await;
    }

    let stats_after = alert_manager.get_alert_stats().await;
    assert_eq!(stats_after.unacknowledged, 0);
}

#[tokio::test]
async fn test_concurrent_budget_operations() {
    use std::sync::Arc;
    use tokio::task;

    let manager = Arc::new(BudgetManager::new());

    manager
        .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 10000.0))
        .await
        .unwrap();

    let mut handles = vec![];

    // Spawn multiple tasks to record spend concurrently
    for _i in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = task::spawn(async move {
            for _j in 0..100 {
                manager_clone.record_spend(&BudgetScope::Global, 1.0).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Should have recorded 1000 spends of 1.0 each
    let spend = manager.get_current_spend(&BudgetScope::Global);
    assert_eq!(spend, 1000.0);
}

#[tokio::test]
async fn test_budget_scope_parsing() {
    // Test scope to key and back
    let scopes = vec![
        BudgetScope::User("user-123".to_string()),
        BudgetScope::Team("team-456".to_string()),
        BudgetScope::ApiKey("sk-abc123".to_string()),
        BudgetScope::Provider("openai".to_string()),
        BudgetScope::Model("gpt-4-turbo".to_string()),
        BudgetScope::Global,
    ];

    for scope in scopes {
        let key = scope.to_key();
        let parsed = BudgetScope::from_key(&key);
        assert_eq!(parsed, Some(scope));
    }
}

#[tokio::test]
async fn test_budget_disabled() {
    let config = BudgetManagerConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = BudgetManager::with_config(config);

    manager
        .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
        .await
        .unwrap();

    // When disabled, check_spend should always return allowed
    manager.record_spend(&BudgetScope::Global, 150.0).await;

    let result = manager.check_spend(&BudgetScope::Global, 50.0).await;
    assert!(result.allowed); // Allowed because manager is disabled
}

#[tokio::test]
async fn test_budget_without_blocking() {
    let config = BudgetManagerConfig {
        block_on_exceeded: false,
        ..Default::default()
    };

    let manager = BudgetManager::with_config(config);

    manager
        .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
        .await
        .unwrap();

    manager.record_spend(&BudgetScope::Global, 150.0).await;

    // Should still be allowed even though exceeded
    let result = manager.check_spend(&BudgetScope::Global, 50.0).await;
    assert!(result.allowed);
    assert_eq!(result.status, BudgetStatus::Exceeded);
}

#[test]
fn test_provider_budget_manager_creation() {
    let manager = ProviderBudgetManager::new();
    assert_eq!(manager.provider_count(), 0);
    assert!(manager.is_enabled());
}

#[test]
fn test_set_provider_limit() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );

    assert!(manager.has_provider_limit("openai"));
    assert_eq!(manager.provider_count(), 1);
}

#[test]
fn test_remove_provider_limit() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );

    assert!(manager.has_provider_limit("openai"));
    assert!(manager.remove_provider_limit("openai"));
    assert!(!manager.has_provider_limit("openai"));
}

#[test]
fn test_check_provider_budget() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );

    assert_eq!(manager.check_provider_budget("openai"), BudgetStatus::Ok);
    assert_eq!(manager.check_provider_budget("unknown"), BudgetStatus::Ok);
}

#[test]
fn test_can_provider_spend() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );

    assert!(manager.can_provider_spend("openai", 50.0));
    assert!(manager.can_provider_spend("openai", 100.0));
    assert!(!manager.can_provider_spend("openai", 101.0));
    assert!(manager.can_provider_spend("unknown", 10000.0));
}

#[test]
fn test_record_provider_spend() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );

    assert_eq!(
        manager.record_provider_spend("openai", 50.0),
        Some(BudgetStatus::Ok)
    );
    assert_eq!(
        manager.record_provider_spend("openai", 30.0),
        Some(BudgetStatus::Warning)
    );
    assert_eq!(
        manager.record_provider_spend("openai", 25.0),
        Some(BudgetStatus::Exceeded)
    );
}

#[test]
fn test_get_provider_usage() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    manager.record_provider_spend("openai", 30.0);

    let usage = manager.get_provider_usage("openai").unwrap();
    assert_eq!(usage.current_spend, 30.0);
    assert_eq!(usage.remaining, 70.0);
    assert_eq!(usage.request_count, 1);
}

#[test]
fn test_get_available_and_exceeded_providers() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    manager.set_provider_limit(
        "anthropic",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    manager.record_provider_spend("openai", 150.0);

    let available = manager.get_available_providers();
    assert!(available.contains(&"anthropic".to_string()));
    assert!(!available.contains(&"openai".to_string()));

    let exceeded = manager.get_exceeded_providers();
    assert_eq!(exceeded, vec!["openai".to_string()]);
}

#[test]
fn test_reset_provider_budget() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    manager.record_provider_spend("openai", 75.0);

    assert!(manager.reset_provider_budget("openai"));
    let usage = manager.get_provider_usage("openai").unwrap();
    assert_eq!(usage.current_spend, 0.0);
    assert_eq!(usage.request_count, 0);
}

#[test]
fn test_disabled_provider_budget_manager_allows_all() {
    let manager = ProviderBudgetManager::new();
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    manager.record_provider_spend("openai", 150.0);
    assert_eq!(
        manager.check_provider_budget("openai"),
        BudgetStatus::Exceeded
    );

    manager.set_enabled(false);
    assert_eq!(manager.check_provider_budget("openai"), BudgetStatus::Ok);
    assert!(manager.can_provider_spend("openai", 1000.0));
}

#[test]
fn test_model_budget_manager_basics() {
    let manager = ModelBudgetManager::new();
    assert_eq!(manager.model_count(), 0);
    assert!(manager.is_enabled());

    manager.set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));
    assert!(manager.has_model_limit("gpt-4"));
    assert_eq!(manager.check_model_budget("gpt-4"), BudgetStatus::Ok);
    assert_eq!(
        manager.record_model_spend("gpt-4", 50.0),
        Some(BudgetStatus::Ok)
    );
    assert_eq!(
        manager.record_model_spend("gpt-4", 55.0),
        Some(BudgetStatus::Exceeded)
    );

    let usage = manager.get_model_usage("gpt-4").unwrap();
    assert_eq!(usage.current_spend, 105.0);
    assert_eq!(usage.request_count, 2);
}

#[test]
fn test_unified_budget_limits() {
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(500.0, ResetPeriod::Monthly));

    assert!(limits.can_spend("openai", "gpt-4", 100.0));
    limits.record_spend("openai", "gpt-4", 100.0);

    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        100.0
    );
    assert_eq!(
        limits
            .models
            .get_model_usage("gpt-4")
            .unwrap()
            .current_spend,
        100.0
    );
}

#[test]
fn test_unified_budget_limits_restore_snapshots() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let limits = UnifiedBudgetLimits::from_snapshots_with_persistence(
        vec![
            BudgetLimitSnapshot {
                kind: BudgetLimitKind::Provider,
                name: "openai".to_string(),
                max_budget: 100.0,
                current_spend: 25.0,
                soft_limit: 80.0,
                reset_period: ResetPeriod::Monthly,
                currency: Currency::USD,
                enabled: true,
                last_reset_at: None,
                request_count: 3,
            },
            BudgetLimitSnapshot {
                kind: BudgetLimitKind::Model,
                name: "gpt-4".to_string(),
                max_budget: 50.0,
                current_spend: 10.0,
                soft_limit: 40.0,
                reset_period: ResetPeriod::Daily,
                currency: Currency::USD,
                enabled: true,
                last_reset_at: None,
                request_count: 2,
            },
        ],
        tx,
    );

    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .request_count,
        3
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
fn test_provider_budget_manager_emits_persistence_events() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let manager = ProviderBudgetManager::new().with_persistence(tx);

    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    manager.record_provider_spend("openai", 12.5);

    assert!(matches!(
        rx.try_recv().expect("set should emit an upsert"),
        BudgetPersistenceEvent::Upsert(BudgetLimitSnapshot {
            kind: BudgetLimitKind::Provider,
            ..
        })
    ));

    match rx.try_recv().expect("spend should emit an upsert") {
        BudgetPersistenceEvent::Upsert(snapshot) => {
            assert_eq!(snapshot.name, "openai");
            assert_eq!(snapshot.current_spend, 12.5);
            assert_eq!(snapshot.request_count, 1);
        }
        BudgetPersistenceEvent::Delete { .. } => panic!("expected upsert"),
    }
}

#[test]
fn test_filter_available_providers() {
    let limits = UnifiedBudgetLimits::new();
    for provider in ["openai", "anthropic", "google"] {
        limits.providers.set_provider_limit(
            provider,
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
    }

    limits.providers.record_provider_spend("openai", 150.0);
    let available = limits.filter_available_providers(vec![
        "openai".to_string(),
        "anthropic".to_string(),
        "google".to_string(),
    ]);

    assert_eq!(available.len(), 2);
    assert!(!available.contains(&"openai".to_string()));
}

#[test]
fn test_provider_budget_manager_concurrent_access() {
    use std::thread;

    let manager = Arc::new(ProviderBudgetManager::new());
    manager.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10000.0, ResetPeriod::Monthly),
    );

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let manager = Arc::clone(&manager);
            thread::spawn(move || {
                for _ in 0..100 {
                    manager.record_provider_spend("openai", 1.0);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let usage = manager.get_provider_usage("openai").unwrap();
    assert_eq!(usage.current_spend, 1000.0);
    assert_eq!(usage.request_count, 1000);
}

#[test]
fn optional_soft_limit_updates_resolve_inside_the_entry() {
    let providers = ProviderBudgetManager::new();
    let mut provider = ProviderLimitConfig::new(100.0, ResetPeriod::Monthly);
    provider.soft_limit_percentage = 0.5;
    providers.set_provider_limit("openai", provider);
    providers.set_provider_limit_optional(
        "openai",
        ProviderLimitConfig::new(200.0, ResetPeriod::Weekly),
        None,
    );
    assert_eq!(
        providers.get_provider_soft_limit_percentage("openai"),
        Some(0.5)
    );

    let models = ModelBudgetManager::new();
    let mut model = ModelLimitConfig::new(100.0, ResetPeriod::Monthly);
    model.soft_limit_percentage = 0.75;
    models.set_model_limit("gpt-4o", model);
    models.set_model_limit_optional(
        "gpt-4o",
        ModelLimitConfig::new(250.0, ResetPeriod::Daily),
        None,
    );
    assert_eq!(models.get_model_soft_limit_percentage("gpt-4o"), Some(0.75));

    let mut new_provider = ProviderLimitConfig::new(50.0, ResetPeriod::Daily);
    new_provider.soft_limit_percentage = 0.6;
    providers.set_provider_limit_optional("anthropic", new_provider, None);
    assert_eq!(
        providers.get_provider_soft_limit_percentage("anthropic"),
        Some(0.6)
    );
}
