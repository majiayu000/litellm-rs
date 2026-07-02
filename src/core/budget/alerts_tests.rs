use super::*;
use crate::core::budget::types::{BudgetScope, BudgetStatus};

fn create_test_budget() -> Budget {
    Budget::new("test-budget", "Test Budget", BudgetScope::Global, 100.0)
}

#[tokio::test]
async fn test_alert_manager_creation() {
    let manager = BudgetAlertManager::new();
    assert!(manager.is_enabled().await);
}

#[tokio::test]
async fn test_create_soft_limit_alert() {
    let manager = BudgetAlertManager::new();
    let mut budget = create_test_budget();
    budget.current_spend = 85.0;

    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Ok,
        new_status: BudgetStatus::Warning,
        current_spend: 85.0,
        max_budget: 100.0,
        remaining: 15.0,
        should_alert_soft_limit: true,
        should_alert_exceeded: false,
    };

    manager.process_spend_result(&result, &budget).await;

    let alerts = manager.get_alerts_for_budget(&budget.id).await;
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::SoftLimitReached);
    assert_eq!(alerts[0].severity, AlertSeverity::Warning);
}

#[tokio::test]
async fn test_create_exceeded_alert() {
    let manager = BudgetAlertManager::new();
    let mut budget = create_test_budget();
    budget.current_spend = 110.0;

    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Warning,
        new_status: BudgetStatus::Exceeded,
        current_spend: 110.0,
        max_budget: 100.0,
        remaining: 0.0,
        should_alert_soft_limit: false,
        should_alert_exceeded: true,
    };

    manager.process_spend_result(&result, &budget).await;

    let alerts = manager.get_alerts_for_budget(&budget.id).await;
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::BudgetExceeded);
    assert_eq!(alerts[0].severity, AlertSeverity::Critical);
}

#[tokio::test]
async fn test_create_reset_alert() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    manager.create_reset_alert(&budget).await;

    let alerts = manager.get_alerts_for_budget(&budget.id).await;
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::BudgetReset);
    assert_eq!(alerts[0].severity, AlertSeverity::Info);
}

#[tokio::test]
async fn test_acknowledge_alert() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    manager.create_reset_alert(&budget).await;

    let alerts = manager.get_unacknowledged_alerts().await;
    assert_eq!(alerts.len(), 1);

    let alert_id = &alerts[0].id;
    assert!(manager.acknowledge_alert(alert_id).await);

    let unacked = manager.get_unacknowledged_alerts().await;
    assert_eq!(unacked.len(), 0);
}

#[tokio::test]
async fn test_acknowledge_alerts_for_budget() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    // Create multiple alerts
    manager.create_reset_alert(&budget).await;

    let mut budget2 = budget.clone();
    budget2.current_spend = 85.0;
    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Ok,
        new_status: BudgetStatus::Warning,
        current_spend: 85.0,
        max_budget: 100.0,
        remaining: 15.0,
        should_alert_soft_limit: true,
        should_alert_exceeded: false,
    };
    manager.process_spend_result(&result, &budget2).await;

    let unacked_before = manager.get_unacknowledged_alerts().await;
    assert_eq!(unacked_before.len(), 2);

    let count = manager.acknowledge_alerts_for_budget(&budget.id).await;
    assert_eq!(count, 2);

    let unacked_after = manager.get_unacknowledged_alerts().await;
    assert_eq!(unacked_after.len(), 0);
}

#[tokio::test]
async fn test_get_alerts_by_severity() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    // Create a reset alert (Info)
    manager.create_reset_alert(&budget).await;

    // Create a soft limit alert (Warning)
    let mut budget2 = budget.clone();
    budget2.current_spend = 85.0;
    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Ok,
        new_status: BudgetStatus::Warning,
        current_spend: 85.0,
        max_budget: 100.0,
        remaining: 15.0,
        should_alert_soft_limit: true,
        should_alert_exceeded: false,
    };
    manager.process_spend_result(&result, &budget2).await;

    let info_alerts = manager.get_alerts_by_severity(AlertSeverity::Info).await;
    assert_eq!(info_alerts.len(), 1);

    let warning_alerts = manager.get_alerts_by_severity(AlertSeverity::Warning).await;
    assert_eq!(warning_alerts.len(), 1);

    let critical_alerts = manager
        .get_alerts_by_severity(AlertSeverity::Critical)
        .await;
    assert_eq!(critical_alerts.len(), 0);
}

#[tokio::test]
async fn test_get_alert_stats() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    manager.create_reset_alert(&budget).await;

    let stats = manager.get_alert_stats().await;

    assert_eq!(stats.total_alerts, 1);
    assert_eq!(stats.unacknowledged, 1);
    assert_eq!(stats.info_count, 1);
    assert_eq!(stats.reset_alerts, 1);
}

#[tokio::test]
async fn test_clear_alerts() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    manager.create_reset_alert(&budget).await;
    assert_eq!(manager.get_all_alerts().await.len(), 1);

    manager.clear_alerts().await;
    assert_eq!(manager.get_all_alerts().await.len(), 0);
}

#[tokio::test]
async fn test_clear_acknowledged_alerts() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    // Create two alerts
    manager.create_reset_alert(&budget).await;

    let mut budget2 = budget.clone();
    budget2.current_spend = 85.0;
    let result = SpendResult {
        budget_id: budget.id.clone(),
        scope: budget.scope.clone(),
        previous_status: BudgetStatus::Ok,
        new_status: BudgetStatus::Warning,
        current_spend: 85.0,
        max_budget: 100.0,
        remaining: 15.0,
        should_alert_soft_limit: true,
        should_alert_exceeded: false,
    };
    manager.process_spend_result(&result, &budget2).await;

    // Acknowledge one
    let alerts = manager.get_all_alerts().await;
    manager.acknowledge_alert(&alerts[0].id).await;

    // Clear acknowledged
    let cleared = manager.clear_acknowledged_alerts().await;
    assert_eq!(cleared, 1);

    // Should have 1 remaining
    assert_eq!(manager.get_all_alerts().await.len(), 1);
}

#[tokio::test]
async fn test_add_webhook() {
    let manager = BudgetAlertManager::new();

    let webhook = WebhookConfig {
        url: "https://example.com/webhook".to_string(),
        ..Default::default()
    };

    manager.add_webhook(webhook).await;

    // Webhook is added (internal state)
    let webhooks = manager.webhooks.read().await;
    assert_eq!(webhooks.len(), 1);
}

#[tokio::test]
async fn test_config_management() {
    let manager = BudgetAlertManager::new();

    let config = manager.get_config().await;
    assert!(config.enabled);

    manager.set_enabled(false).await;
    assert!(!manager.is_enabled().await);

    let new_config = AlertConfig {
        enabled: true,
        soft_limit_percentage: 0.9,
        warning_thresholds: vec![0.95],
        max_history_size: 500,
        duplicate_suppression_secs: 1800,
    };

    manager.update_config(new_config).await;

    let updated_config = manager.get_config().await;
    assert_eq!(updated_config.soft_limit_percentage, 0.9);
    assert_eq!(updated_config.max_history_size, 500);
}

#[tokio::test]
async fn test_alert_history() {
    let manager = BudgetAlertManager::new();
    let budget = create_test_budget();

    // Create multiple alerts
    for _ in 0..5 {
        manager.create_reset_alert(&budget).await;
    }

    let history = manager.get_alert_history(Some(3)).await;
    assert_eq!(history.len(), 3);

    let full_history = manager.get_alert_history(None).await;
    assert_eq!(full_history.len(), 5);
}

#[tokio::test]
async fn test_disabled_alerting() {
    let manager = BudgetAlertManager::new();
    manager.set_enabled(false).await;

    let budget = create_test_budget();
    manager.create_reset_alert(&budget).await;

    // No alerts should be created when disabled
    let alerts = manager.get_all_alerts().await;
    assert_eq!(alerts.len(), 0);
}
