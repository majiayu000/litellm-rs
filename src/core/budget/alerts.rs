//! Budget alert management system
//!
//! Provides alert tracking, webhook notifications, and alert management
//! for budget thresholds and exceeded limits.

use super::tracker::SpendResult;
use super::types::{AlertSeverity, Budget, BudgetAlert, BudgetAlertType};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::utils::net::http::create_custom_client;
/// Budget alert manager for handling notifications
#[derive(Clone)]
pub struct BudgetAlertManager {
    /// Alert storage
    alerts: Arc<RwLock<AlertStorage>>,
    /// Webhook clients for notifications
    webhooks: Arc<RwLock<Vec<WebhookConfig>>>,
    /// HTTP client for webhook requests
    client: Client,
    /// Configuration
    config: Arc<RwLock<AlertConfig>>,
}

/// Alert storage structure
#[derive(Debug, Default)]
struct AlertStorage {
    /// All alerts indexed by ID
    alerts: HashMap<String, BudgetAlert>,
    /// Alerts indexed by budget ID for quick lookup
    alerts_by_budget: HashMap<String, Vec<String>>,
    /// Alert history (limited to last N alerts)
    history: Vec<BudgetAlert>,
    /// Maximum history size
    max_history_size: usize,
}

impl AlertStorage {
    fn new(max_history_size: usize) -> Self {
        Self {
            alerts: HashMap::new(),
            alerts_by_budget: HashMap::new(),
            history: Vec::new(),
            max_history_size,
        }
    }

    fn add_alert(&mut self, alert: BudgetAlert) {
        let alert_id = alert.id.clone();
        let budget_id = alert.budget_id.clone();

        // Add to main storage
        self.alerts.insert(alert_id.clone(), alert.clone());

        // Add to budget index
        self.alerts_by_budget
            .entry(budget_id)
            .or_default()
            .push(alert_id);

        // Add to history
        self.history.push(alert);

        // Trim history if needed
        if self.history.len() > self.max_history_size {
            let excess = self.history.len() - self.max_history_size;
            self.history.drain(0..excess);
        }
    }

    fn get_alerts_for_budget(&self, budget_id: &str) -> Vec<&BudgetAlert> {
        self.alerts_by_budget
            .get(budget_id)
            .map(|ids| ids.iter().filter_map(|id| self.alerts.get(id)).collect())
            .unwrap_or_default()
    }
}

/// Webhook configuration for alert notifications
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL
    pub url: String,
    /// HTTP headers to include
    pub headers: HashMap<String, String>,
    /// Alert severities to notify for
    pub severities: Vec<AlertSeverity>,
    /// Whether webhook is enabled
    pub enabled: bool,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            headers: HashMap::new(),
            severities: vec![AlertSeverity::Warning, AlertSeverity::Critical],
            enabled: true,
            timeout_secs: 30,
            max_retries: 3,
        }
    }
}

/// Alert manager configuration
#[derive(Debug, Clone)]
pub struct AlertConfig {
    /// Whether alerting is enabled
    pub enabled: bool,
    /// Default soft limit percentage (for warnings)
    pub soft_limit_percentage: f64,
    /// Additional warning thresholds (e.g., 90%, 95%)
    pub warning_thresholds: Vec<f64>,
    /// Maximum alert history size
    pub max_history_size: usize,
    /// Suppress duplicate alerts within this window (seconds)
    pub duplicate_suppression_secs: u64,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            soft_limit_percentage: 0.8,
            warning_thresholds: vec![0.9, 0.95],
            max_history_size: 1000,
            duplicate_suppression_secs: 3600, // 1 hour
        }
    }
}

impl Default for BudgetAlertManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetAlertManager {
    /// Create a new alert manager with default configuration
    pub fn new() -> Self {
        let config = AlertConfig::default();
        Self {
            alerts: Arc::new(RwLock::new(AlertStorage::new(config.max_history_size))),
            webhooks: Arc::new(RwLock::new(Vec::new())),
            client: create_custom_client(Duration::from_secs(30)).unwrap_or_else(|_| Client::new()),
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Create a new alert manager with custom configuration
    pub fn with_config(config: AlertConfig) -> Self {
        Self {
            alerts: Arc::new(RwLock::new(AlertStorage::new(config.max_history_size))),
            webhooks: Arc::new(RwLock::new(Vec::new())),
            client: create_custom_client(Duration::from_secs(30)).unwrap_or_else(|_| Client::new()),
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Add a webhook for alert notifications
    pub async fn add_webhook(&self, config: WebhookConfig) {
        let mut webhooks = self.webhooks.write().await;
        webhooks.push(config);
    }

    /// Remove all webhooks
    pub async fn clear_webhooks(&self) {
        let mut webhooks = self.webhooks.write().await;
        webhooks.clear();
    }

    /// Process a spend result and generate alerts if needed
    pub async fn process_spend_result(&self, result: &SpendResult, budget: &Budget) {
        let config = self.config.read().await;
        if !config.enabled {
            return;
        }
        drop(config);

        if result.should_alert_soft_limit {
            self.create_alert(budget, BudgetAlertType::SoftLimitReached, budget.soft_limit)
                .await;
        }

        if result.should_alert_exceeded {
            self.create_alert(budget, BudgetAlertType::BudgetExceeded, budget.max_budget)
                .await;
            // Don't check warning thresholds if already exceeded
            return;
        }

        // Check additional warning thresholds (only if not exceeded)
        let config = self.config.read().await;
        for &threshold_pct in &config.warning_thresholds {
            let threshold = budget.max_budget * threshold_pct;
            if result.current_spend >= threshold
                && result.current_spend - (result.max_budget - result.remaining) < threshold
            {
                // Just crossed this threshold
                drop(config);
                self.create_alert(budget, BudgetAlertType::ApproachingLimit, threshold)
                    .await;
                break;
            }
        }
    }

    /// Create and store an alert
    async fn create_alert(&self, budget: &Budget, alert_type: BudgetAlertType, threshold: f64) {
        let alert = BudgetAlert::new(budget, alert_type, threshold);

        info!(
            "Budget alert created: {} - {} (severity: {:?})",
            budget.name, alert.message, alert.severity
        );

        // Store the alert
        {
            let mut storage = self.alerts.write().await;
            storage.add_alert(alert.clone());
        }

        // Send webhook notifications
        self.send_webhook_notifications(&alert).await;
    }

    /// Create alert for budget reset
    pub async fn create_reset_alert(&self, budget: &Budget) {
        let config = self.config.read().await;
        if !config.enabled {
            return;
        }
        drop(config);

        let alert = BudgetAlert::new(budget, BudgetAlertType::BudgetReset, 0.0);

        info!("Budget reset alert: {}", alert.message);

        let mut storage = self.alerts.write().await;
        storage.add_alert(alert.clone());
        drop(storage);

        self.send_webhook_notifications(&alert).await;
    }

    /// Send webhook notifications for an alert
    async fn send_webhook_notifications(&self, alert: &BudgetAlert) {
        let webhooks = self.webhooks.read().await;

        for webhook in webhooks.iter() {
            if !webhook.enabled {
                continue;
            }

            if !webhook.severities.contains(&alert.severity) {
                continue;
            }

            self.send_single_webhook(webhook, alert).await;
        }
    }

    /// Send a single webhook notification
    async fn send_single_webhook(&self, webhook: &WebhookConfig, alert: &BudgetAlert) {
        let payload = serde_json::json!({
            "type": "budget_alert",
            "alert": {
                "id": alert.id,
                "budget_id": alert.budget_id,
                "scope": alert.scope.to_string(),
                "alert_type": format!("{:?}", alert.alert_type),
                "severity": format!("{:?}", alert.severity),
                "message": alert.message,
                "current_spend": alert.current_spend,
                "threshold": alert.threshold,
                "max_budget": alert.max_budget,
                "created_at": alert.created_at.to_rfc3339()
            }
        });

        let mut retries = 0;
        let max_retries = webhook.max_retries;

        loop {
            let mut request = self
                .client
                .post(&webhook.url)
                .timeout(Duration::from_secs(webhook.timeout_secs))
                .json(&payload);

            // Add custom headers
            for (key, value) in &webhook.headers {
                request = request.header(key, value);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!("Successfully sent budget alert webhook to {}", webhook.url);
                        return;
                    } else {
                        warn!(
                            "Budget alert webhook returned error status {}: {}",
                            response.status(),
                            webhook.url
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to send budget alert webhook to {}: {}",
                        webhook.url, e
                    );
                }
            }

            retries += 1;
            if retries >= max_retries {
                error!(
                    "Exhausted retries for budget alert webhook: {}",
                    webhook.url
                );
                return;
            }

            // Exponential backoff
            let delay = Duration::from_millis(100 * 2_u64.pow(retries));
            tokio::time::sleep(delay).await;
        }
    }

    /// Get all alerts for a budget
    pub async fn get_alerts_for_budget(&self, budget_id: &str) -> Vec<BudgetAlert> {
        let storage = self.alerts.read().await;
        storage
            .get_alerts_for_budget(budget_id)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Get all alerts
    pub async fn get_all_alerts(&self) -> Vec<BudgetAlert> {
        let storage = self.alerts.read().await;
        storage.alerts.values().cloned().collect()
    }

    /// Get unacknowledged alerts
    pub async fn get_unacknowledged_alerts(&self) -> Vec<BudgetAlert> {
        let storage = self.alerts.read().await;
        storage
            .alerts
            .values()
            .filter(|a| !a.acknowledged)
            .cloned()
            .collect()
    }

    /// Get alerts by severity
    pub async fn get_alerts_by_severity(&self, severity: AlertSeverity) -> Vec<BudgetAlert> {
        let storage = self.alerts.read().await;
        storage
            .alerts
            .values()
            .filter(|a| a.severity == severity)
            .cloned()
            .collect()
    }

    /// Acknowledge an alert
    pub async fn acknowledge_alert(&self, alert_id: &str) -> bool {
        let mut storage = self.alerts.write().await;
        if let Some(alert) = storage.alerts.get_mut(alert_id) {
            alert.acknowledge();
            true
        } else {
            false
        }
    }

    /// Acknowledge all alerts for a budget
    pub async fn acknowledge_alerts_for_budget(&self, budget_id: &str) -> usize {
        let mut storage = self.alerts.write().await;
        let mut count = 0;

        if let Some(alert_ids) = storage.alerts_by_budget.get(budget_id).cloned() {
            for alert_id in alert_ids {
                if let Some(alert) = storage.alerts.get_mut(&alert_id)
                    && !alert.acknowledged
                {
                    alert.acknowledge();
                    count += 1;
                }
            }
        }

        count
    }

    /// Get alert history
    pub async fn get_alert_history(&self, limit: Option<usize>) -> Vec<BudgetAlert> {
        let storage = self.alerts.read().await;
        let limit = limit.unwrap_or(storage.history.len());
        storage.history.iter().rev().take(limit).cloned().collect()
    }

    /// Get alert statistics
    pub async fn get_alert_stats(&self) -> AlertStats {
        let storage = self.alerts.read().await;

        let mut stats = AlertStats::default();

        for alert in storage.alerts.values() {
            stats.total_alerts += 1;

            if !alert.acknowledged {
                stats.unacknowledged += 1;
            }

            match alert.severity {
                AlertSeverity::Info => stats.info_count += 1,
                AlertSeverity::Warning => stats.warning_count += 1,
                AlertSeverity::Critical => stats.critical_count += 1,
            }

            match alert.alert_type {
                BudgetAlertType::SoftLimitReached => stats.soft_limit_alerts += 1,
                BudgetAlertType::BudgetExceeded => stats.exceeded_alerts += 1,
                BudgetAlertType::BudgetReset => stats.reset_alerts += 1,
                BudgetAlertType::ApproachingLimit => stats.approaching_limit_alerts += 1,
            }
        }

        stats
    }

    /// Clear all alerts
    pub async fn clear_alerts(&self) {
        let mut storage = self.alerts.write().await;
        storage.alerts.clear();
        storage.alerts_by_budget.clear();
        // Keep history
    }

    /// Clear acknowledged alerts
    pub async fn clear_acknowledged_alerts(&self) -> usize {
        let mut storage = self.alerts.write().await;

        let acknowledged_ids: Vec<String> = storage
            .alerts
            .iter()
            .filter(|(_, alert)| alert.acknowledged)
            .map(|(id, _)| id.clone())
            .collect();

        let count = acknowledged_ids.len();

        for id in acknowledged_ids {
            storage.alerts.remove(&id);
        }

        // Clean up alerts_by_budget
        // Collect remaining alert IDs first to avoid borrow conflict
        let remaining_ids: std::collections::HashSet<String> =
            storage.alerts.keys().cloned().collect();
        for alerts in storage.alerts_by_budget.values_mut() {
            alerts.retain(|id| remaining_ids.contains(id));
        }

        count
    }

    /// Update configuration
    pub async fn update_config(&self, new_config: AlertConfig) {
        let mut config = self.config.write().await;
        *config = new_config;
    }

    /// Get current configuration
    pub async fn get_config(&self) -> AlertConfig {
        self.config.read().await.clone()
    }

    /// Check if alerting is enabled
    pub async fn is_enabled(&self) -> bool {
        self.config.read().await.enabled
    }

    /// Enable or disable alerting
    pub async fn set_enabled(&self, enabled: bool) {
        let mut config = self.config.write().await;
        config.enabled = enabled;
    }
}

/// Alert statistics
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AlertStats {
    /// Total number of alerts
    pub total_alerts: usize,
    /// Unacknowledged alerts
    pub unacknowledged: usize,
    /// Info severity count
    pub info_count: usize,
    /// Warning severity count
    pub warning_count: usize,
    /// Critical severity count
    pub critical_count: usize,
    /// Soft limit alerts
    pub soft_limit_alerts: usize,
    /// Exceeded alerts
    pub exceeded_alerts: usize,
    /// Reset alerts
    pub reset_alerts: usize,
    /// Approaching limit alerts
    pub approaching_limit_alerts: usize,
}

#[cfg(test)]
mod tests {
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
}
