//! Alert manager implementation

use super::channels::{NotificationChannel, SlackChannel};
use super::types::{AlertRule, AlertStats, AlertStorage};
use crate::config::models::file_storage::AlertingConfig;
use crate::monitoring::types::{Alert, AlertSeverity};
use crate::utils::error::gateway_error::Result;
use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock as TokioRwLock;
use tracing::{debug, info};

/// Alert manager for handling and dispatching alerts
#[derive(Debug)]
pub struct AlertManager {
    /// Configuration
    config: AlertingConfig,
    /// Consolidated storage for all alert-related data
    pub(super) storage: Arc<RwLock<AlertStorage>>,
    /// Pending alerts queue (separate for fast lock-free access)
    pub(super) pending_alerts: Arc<Mutex<VecDeque<Alert>>>,
    /// Notification channels - using tokio RwLock because we need to hold across await points
    pub(super) notification_channels: Arc<TokioRwLock<Vec<Box<dyn NotificationChannel>>>>,
    /// Whether the alert manager is active - using AtomicBool for lock-free access
    pub(super) active: AtomicBool,
}

impl AlertManager {
    /// Create a new alert manager
    pub async fn new(config: &AlertingConfig) -> Result<Self> {
        let mut notification_channels: Vec<Box<dyn NotificationChannel>> = Vec::new();

        // Add Slack channel if configured
        if let Some(webhook_url) = &config.slack_webhook {
            notification_channels.push(Box::new(SlackChannel::new(
                webhook_url.clone(),
                None,
                Some("Gateway Alert".to_string()),
                AlertSeverity::Info,
            )));
        }

        // Add email channel if configured
        // NOTE: email configuration support not yet implemented

        Ok(Self {
            config: config.clone(),
            storage: Arc::new(RwLock::new(AlertStorage::default())),
            pending_alerts: Arc::new(Mutex::new(VecDeque::new())),
            notification_channels: Arc::new(TokioRwLock::new(notification_channels)),
            active: AtomicBool::new(false),
        })
    }

    /// Start the alert manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting alert manager");

        self.active.store(true, Ordering::Release);

        // Start alert processing task
        self.start_alert_processing().await;

        // Start rule evaluation task
        self.start_rule_evaluation().await;

        Ok(())
    }

    /// Stop the alert manager
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping alert manager");
        self.active.store(false, Ordering::Release);
        Ok(())
    }

    /// Check if alert manager is active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Send an alert
    pub async fn send_alert(&self, alert: Alert) -> Result<()> {
        debug!("Queuing alert: {} - {}", alert.severity, alert.title);

        // Add to pending queue
        {
            self.pending_alerts.lock().push_back(alert.clone());
        }

        // Update statistics and history in a single lock
        {
            let mut storage = self.storage.write();

            // Update statistics
            storage.stats.total_alerts += 1;
            *storage
                .stats
                .alerts_by_severity
                .entry(format!("{:?}", alert.severity))
                .or_insert(0) += 1;
            *storage
                .stats
                .alerts_by_source
                .entry(alert.source.clone())
                .or_insert(0) += 1;
            storage.stats.last_alert = Some(alert.timestamp);

            // Add to history
            storage.history.push_back(alert);

            // Keep only recent alerts (last 1000)
            if storage.history.len() > 1000 {
                storage.history.pop_front();
            }
        }

        Ok(())
    }

    /// Process pending alerts
    pub async fn process_pending(&self) -> Result<()> {
        let mut alerts_to_process = Vec::new();

        // Get pending alerts - using parking_lot Mutex (no await needed)
        {
            let mut pending = self.pending_alerts.lock();
            while let Some(alert) = pending.pop_front() {
                alerts_to_process.push(alert);
            }
        }

        // Process each alert
        for alert in alerts_to_process {
            if let Err(e) = self.process_alert(&alert).await {
                tracing::error!("Failed to process alert {}: {}", alert.id, e);

                // Update failed notification count
                self.storage.write().stats.failed_notifications += 1;
            }
        }

        Ok(())
    }

    /// Add an alert rule
    pub async fn add_rule(&self, rule: AlertRule) -> Result<()> {
        info!("Adding alert rule: {}", rule.name);

        self.storage.write().rules.insert(rule.id.clone(), rule);

        Ok(())
    }

    /// Remove an alert rule
    pub async fn remove_rule(&self, rule_id: &str) -> Result<()> {
        info!("Removing alert rule: {}", rule_id);

        self.storage.write().rules.remove(rule_id);

        Ok(())
    }

    /// Get alert statistics
    pub async fn get_stats(&self) -> AlertStats {
        self.storage.read().stats.clone()
    }

    /// Get alert history
    pub async fn get_history(&self, limit: Option<usize>) -> Vec<Alert> {
        let storage = self.storage.read();
        let limit = limit.unwrap_or(100);

        storage.history.iter().rev().take(limit).cloned().collect()
    }
}

impl Clone for AlertManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: self.storage.clone(),
            pending_alerts: self.pending_alerts.clone(),
            notification_channels: self.notification_channels.clone(),
            active: AtomicBool::new(self.active.load(Ordering::Acquire)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn default_alerting_config() -> AlertingConfig {
        AlertingConfig {
            enabled: true,
            slack_webhook: None,
            email: None,
        }
    }

    fn create_test_alert(severity: AlertSeverity, title: &str) -> Alert {
        Alert {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            description: "Test description".to_string(),
            severity,
            source: "test_source".to_string(),
            timestamp: Utc::now(),
            metadata: serde_json::json!({}),
            resolved: false,
        }
    }

    // ==================== AlertManager Creation Tests ====================

    #[tokio::test]
    async fn test_alert_manager_new() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        assert!(!manager.is_active());
    }

    #[tokio::test]
    async fn test_alert_manager_with_slack_webhook() {
        let config = AlertingConfig {
            enabled: true,
            slack_webhook: Some("https://hooks.slack.com/test".to_string()),
            email: None,
        };

        let manager = AlertManager::new(&config).await.unwrap();

        // Should have one notification channel (Slack)
        let channels = manager.notification_channels.read().await;
        assert_eq!(channels.len(), 1);
    }

    #[tokio::test]
    async fn test_alert_manager_no_channels() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let channels = manager.notification_channels.read().await;
        assert!(channels.is_empty());
    }

    // ==================== AlertManager Lifecycle Tests ====================

    #[tokio::test]
    async fn test_alert_manager_start() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        assert!(!manager.is_active());

        manager.start().await.unwrap();

        assert!(manager.is_active());
    }

    #[tokio::test]
    async fn test_alert_manager_stop() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        manager.start().await.unwrap();
        assert!(manager.is_active());

        manager.stop().await.unwrap();
        assert!(!manager.is_active());
    }

    #[tokio::test]
    async fn test_alert_manager_start_stop_cycle() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        for _ in 0..3 {
            manager.start().await.unwrap();
            assert!(manager.is_active());

            manager.stop().await.unwrap();
            assert!(!manager.is_active());
        }
    }

    // ==================== AlertManager Send Alert Tests ====================

    #[tokio::test]
    async fn test_send_alert() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let alert = create_test_alert(AlertSeverity::Warning, "Test Alert");

        manager.send_alert(alert.clone()).await.unwrap();

        // Check stats updated
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_alerts, 1);

        // Check history
        let history = manager.get_history(None).await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].title, "Test Alert");
    }

    #[tokio::test]
    async fn test_send_multiple_alerts() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        for i in 0..5 {
            let alert = create_test_alert(AlertSeverity::Info, &format!("Alert {}", i));
            manager.send_alert(alert).await.unwrap();
        }

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_alerts, 5);

        let history = manager.get_history(None).await;
        assert_eq!(history.len(), 5);
    }

    #[tokio::test]
    async fn test_send_alerts_different_severities() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let severities = [
            AlertSeverity::Info,
            AlertSeverity::Warning,
            AlertSeverity::Critical,
            AlertSeverity::Emergency,
        ];

        for severity in severities {
            let alert = create_test_alert(severity, "Test");
            manager.send_alert(alert).await.unwrap();
        }

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_alerts, 4);
        assert_eq!(stats.alerts_by_severity.len(), 4);
    }

    #[tokio::test]
    async fn test_send_alerts_different_sources() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let sources = ["api", "worker", "scheduler", "database"];

        for source in sources {
            let mut alert = create_test_alert(AlertSeverity::Info, "Test");
            alert.source = source.to_string();
            manager.send_alert(alert).await.unwrap();
        }

        let stats = manager.get_stats().await;
        assert_eq!(stats.alerts_by_source.len(), 4);
    }

    // ==================== AlertManager History Tests ====================

    #[tokio::test]
    async fn test_get_history_default_limit() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        for i in 0..150 {
            let alert = create_test_alert(AlertSeverity::Info, &format!("Alert {}", i));
            manager.send_alert(alert).await.unwrap();
        }

        // Default limit is 100
        let history = manager.get_history(None).await;
        assert_eq!(history.len(), 100);
    }

    #[tokio::test]
    async fn test_get_history_custom_limit() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        for i in 0..50 {
            let alert = create_test_alert(AlertSeverity::Info, &format!("Alert {}", i));
            manager.send_alert(alert).await.unwrap();
        }

        let history = manager.get_history(Some(10)).await;
        assert_eq!(history.len(), 10);
    }

    #[tokio::test]
    async fn test_get_history_returns_recent_first() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        for i in 0..5 {
            let alert = create_test_alert(AlertSeverity::Info, &format!("Alert {}", i));
            manager.send_alert(alert).await.unwrap();
        }

        let history = manager.get_history(Some(5)).await;
        // Most recent should be first (reversed order)
        assert_eq!(history[0].title, "Alert 4");
    }

    // ==================== AlertManager Rule Tests ====================

    #[tokio::test]
    async fn test_add_rule() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let rule = AlertRule {
            id: "rule-1".to_string(),
            name: "High CPU".to_string(),
            description: "CPU usage too high".to_string(),
            metric: "cpu.usage".to_string(),
            threshold: 90.0,
            operator: super::super::types::ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Warning,
            interval: std::time::Duration::from_secs(60),
            enabled: true,
            channels: vec!["slack".to_string()],
        };

        manager.add_rule(rule).await.unwrap();

        let storage = manager.storage.read();
        assert_eq!(storage.rules.len(), 1);
        assert!(storage.rules.contains_key("rule-1"));
    }

    #[tokio::test]
    async fn test_add_multiple_rules() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        for i in 0..5 {
            let rule = AlertRule {
                id: format!("rule-{}", i),
                name: format!("Rule {}", i),
                description: "Test rule".to_string(),
                metric: "test.metric".to_string(),
                threshold: i as f64 * 10.0,
                operator: super::super::types::ComparisonOperator::GreaterThan,
                severity: AlertSeverity::Info,
                interval: std::time::Duration::from_secs(60),
                enabled: true,
                channels: vec![],
            };
            manager.add_rule(rule).await.unwrap();
        }

        let storage = manager.storage.read();
        assert_eq!(storage.rules.len(), 5);
    }

    #[tokio::test]
    async fn test_remove_rule() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let rule = AlertRule {
            id: "rule-to-remove".to_string(),
            name: "Temporary Rule".to_string(),
            description: "".to_string(),
            metric: "test".to_string(),
            threshold: 50.0,
            operator: super::super::types::ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Info,
            interval: std::time::Duration::from_secs(60),
            enabled: true,
            channels: vec![],
        };

        manager.add_rule(rule).await.unwrap();
        assert_eq!(manager.storage.read().rules.len(), 1);

        manager.remove_rule("rule-to-remove").await.unwrap();
        assert_eq!(manager.storage.read().rules.len(), 0);
    }

    #[tokio::test]
    async fn test_remove_nonexistent_rule() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        // Should not error
        let result = manager.remove_rule("nonexistent").await;
        assert!(result.is_ok());
    }

    // ==================== AlertManager Stats Tests ====================

    #[tokio::test]
    async fn test_get_stats_empty() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let stats = manager.get_stats().await;

        assert_eq!(stats.total_alerts, 0);
        assert_eq!(stats.failed_notifications, 0);
        assert!(stats.alerts_by_severity.is_empty());
        assert!(stats.alerts_by_source.is_empty());
        assert!(stats.last_alert.is_none());
    }

    #[tokio::test]
    async fn test_get_stats_after_alerts() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let alert = create_test_alert(AlertSeverity::Critical, "Critical Alert");
        manager.send_alert(alert).await.unwrap();

        let stats = manager.get_stats().await;

        assert_eq!(stats.total_alerts, 1);
        assert!(stats.last_alert.is_some());
        assert_eq!(stats.alerts_by_severity.get("Critical"), Some(&1));
    }

    // ==================== AlertManager Clone Tests ====================

    #[tokio::test]
    async fn test_alert_manager_clone() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        manager.start().await.unwrap();

        let cloned = manager.clone();

        assert!(cloned.is_active());
    }

    #[tokio::test]
    async fn test_alert_manager_clone_shares_storage() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let cloned = manager.clone();

        // Add alert through original
        let alert = create_test_alert(AlertSeverity::Info, "Shared Alert");
        manager.send_alert(alert).await.unwrap();

        // Check stats through clone
        let stats = cloned.get_stats().await;
        assert_eq!(stats.total_alerts, 1);
    }

    // ==================== AlertManager Process Pending Tests ====================

    #[tokio::test]
    async fn test_process_pending_empty() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        // Should not error with no pending alerts
        let result = manager.process_pending().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_pending_with_alerts() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        // Add alerts to pending queue
        let alert = create_test_alert(AlertSeverity::Warning, "Pending Alert");
        manager.send_alert(alert).await.unwrap();

        // Process pending
        let result = manager.process_pending().await;
        assert!(result.is_ok());

        // Pending queue should be empty after processing
        let pending = manager.pending_alerts.lock();
        assert!(pending.is_empty());
    }

    // ==================== AlertManager Debug Tests ====================

    #[tokio::test]
    async fn test_alert_manager_debug() {
        let config = default_alerting_config();
        let manager = AlertManager::new(&config).await.unwrap();

        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("AlertManager"));
    }
}
