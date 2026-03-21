//! Alert types and data structures

use crate::monitoring::types::AlertSeverity;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

/// Consolidated alert storage - single lock for related data
#[derive(Debug, Default)]
pub(super) struct AlertStorage {
    /// Alert history
    pub history: VecDeque<crate::monitoring::types::Alert>,
    /// Alert rules
    pub rules: HashMap<String, AlertRule>,
    /// Alert statistics
    pub stats: AlertStats,
}

/// Alert rule for automated alerting
#[derive(Debug, Clone)]
pub struct AlertRule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Metric to monitor
    pub metric: String,
    /// Threshold value
    pub threshold: f64,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Evaluation interval
    pub interval: Duration,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Notification channels for this rule
    pub channels: Vec<String>,
}

/// Comparison operators for alert rules
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equal,
    NotEqual,
}

/// Alert statistics
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct AlertStats {
    /// Total alerts sent
    pub total_alerts: u64,
    /// Alerts by severity
    pub alerts_by_severity: HashMap<String, u64>,
    /// Alerts by source
    pub alerts_by_source: HashMap<String, u64>,
    /// Failed notifications
    pub failed_notifications: u64,
    /// Last alert timestamp
    pub last_alert: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== AlertStorage Tests ====================

    #[test]
    fn test_alert_storage_default() {
        let storage = AlertStorage::default();
        assert!(storage.history.is_empty());
        assert!(storage.rules.is_empty());
        assert_eq!(storage.stats.total_alerts, 0);
    }

    #[test]
    fn test_alert_storage_add_rule() {
        let mut storage = AlertStorage::default();
        let rule = AlertRule {
            id: "rule-1".to_string(),
            name: "High CPU".to_string(),
            description: "Alert when CPU exceeds threshold".to_string(),
            metric: "cpu_usage".to_string(),
            threshold: 90.0,
            operator: ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Warning,
            interval: Duration::from_secs(60),
            enabled: true,
            channels: vec!["email".to_string()],
        };

        storage.rules.insert(rule.id.clone(), rule);
        assert_eq!(storage.rules.len(), 1);
        assert!(storage.rules.contains_key("rule-1"));
    }

    #[test]
    fn test_alert_storage_multiple_rules() {
        let mut storage = AlertStorage::default();

        for i in 0..5 {
            let rule = AlertRule {
                id: format!("rule-{}", i),
                name: format!("Rule {}", i),
                description: "Test rule".to_string(),
                metric: "test_metric".to_string(),
                threshold: i as f64 * 10.0,
                operator: ComparisonOperator::GreaterThan,
                severity: AlertSeverity::Info,
                interval: Duration::from_secs(30),
                enabled: true,
                channels: vec![],
            };
            storage.rules.insert(rule.id.clone(), rule);
        }

        assert_eq!(storage.rules.len(), 5);
    }

    // ==================== AlertRule Tests ====================

    #[test]
    fn test_alert_rule_creation() {
        let rule = AlertRule {
            id: "cpu-high".to_string(),
            name: "High CPU Alert".to_string(),
            description: "Triggers when CPU usage is too high".to_string(),
            metric: "system.cpu.percent".to_string(),
            threshold: 85.0,
            operator: ComparisonOperator::GreaterThanOrEqual,
            severity: AlertSeverity::Critical,
            interval: Duration::from_secs(120),
            enabled: true,
            channels: vec!["slack".to_string(), "pagerduty".to_string()],
        };

        assert_eq!(rule.id, "cpu-high");
        assert_eq!(rule.name, "High CPU Alert");
        assert_eq!(rule.threshold, 85.0);
        assert_eq!(rule.operator, ComparisonOperator::GreaterThanOrEqual);
        assert!(rule.enabled);
        assert_eq!(rule.channels.len(), 2);
    }

    #[test]
    fn test_alert_rule_disabled() {
        let rule = AlertRule {
            id: "test".to_string(),
            name: "Disabled Rule".to_string(),
            description: "This rule is disabled".to_string(),
            metric: "test".to_string(),
            threshold: 50.0,
            operator: ComparisonOperator::Equal,
            severity: AlertSeverity::Info,
            interval: Duration::from_secs(60),
            enabled: false,
            channels: vec![],
        };

        assert!(!rule.enabled);
    }

    #[test]
    fn test_alert_rule_clone() {
        let original = AlertRule {
            id: "mem-low".to_string(),
            name: "Low Memory".to_string(),
            description: "Memory below threshold".to_string(),
            metric: "memory.available".to_string(),
            threshold: 1024.0,
            operator: ComparisonOperator::LessThan,
            severity: AlertSeverity::Warning,
            interval: Duration::from_secs(30),
            enabled: true,
            channels: vec!["email".to_string()],
        };

        let cloned = original.clone();
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.threshold, cloned.threshold);
        assert_eq!(original.operator, cloned.operator);
    }

    #[test]
    fn test_alert_rule_various_thresholds() {
        // Zero threshold
        let rule = AlertRule {
            id: "zero".to_string(),
            name: "Zero".to_string(),
            description: "".to_string(),
            metric: "errors".to_string(),
            threshold: 0.0,
            operator: ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Info,
            interval: Duration::from_secs(60),
            enabled: true,
            channels: vec![],
        };
        assert_eq!(rule.threshold, 0.0);

        // Negative threshold
        let rule = AlertRule {
            id: "neg".to_string(),
            name: "Negative".to_string(),
            description: "".to_string(),
            metric: "temperature".to_string(),
            threshold: -40.0,
            operator: ComparisonOperator::LessThan,
            severity: AlertSeverity::Warning,
            interval: Duration::from_secs(60),
            enabled: true,
            channels: vec![],
        };
        assert_eq!(rule.threshold, -40.0);

        // Large threshold
        let rule = AlertRule {
            id: "large".to_string(),
            name: "Large".to_string(),
            description: "".to_string(),
            metric: "requests".to_string(),
            threshold: 1_000_000.0,
            operator: ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Critical,
            interval: Duration::from_secs(60),
            enabled: true,
            channels: vec![],
        };
        assert_eq!(rule.threshold, 1_000_000.0);
    }

    // ==================== ComparisonOperator Tests ====================

    #[test]
    fn test_comparison_operator_greater_than() {
        let op = ComparisonOperator::GreaterThan;
        assert_eq!(op, ComparisonOperator::GreaterThan);
    }

    #[test]
    fn test_comparison_operator_less_than() {
        let op = ComparisonOperator::LessThan;
        assert_eq!(op, ComparisonOperator::LessThan);
    }

    #[test]
    fn test_comparison_operator_greater_than_or_equal() {
        let op = ComparisonOperator::GreaterThanOrEqual;
        assert_eq!(op, ComparisonOperator::GreaterThanOrEqual);
    }

    #[test]
    fn test_comparison_operator_less_than_or_equal() {
        let op = ComparisonOperator::LessThanOrEqual;
        assert_eq!(op, ComparisonOperator::LessThanOrEqual);
    }

    #[test]
    fn test_comparison_operator_equal() {
        let op = ComparisonOperator::Equal;
        assert_eq!(op, ComparisonOperator::Equal);
    }

    #[test]
    fn test_comparison_operator_not_equal() {
        let op = ComparisonOperator::NotEqual;
        assert_eq!(op, ComparisonOperator::NotEqual);
    }

    #[test]
    fn test_comparison_operator_clone() {
        let op = ComparisonOperator::GreaterThan;
        let cloned = op;
        assert_eq!(op, cloned);
    }

    #[test]
    fn test_comparison_operator_copy() {
        let op1 = ComparisonOperator::LessThan;
        let op2 = op1; // Copy
        assert_eq!(op1, op2);
    }

    #[test]
    fn test_comparison_operators_inequality() {
        assert_ne!(
            ComparisonOperator::GreaterThan,
            ComparisonOperator::LessThan
        );
        assert_ne!(ComparisonOperator::Equal, ComparisonOperator::NotEqual);
        assert_ne!(
            ComparisonOperator::GreaterThanOrEqual,
            ComparisonOperator::LessThanOrEqual
        );
    }

    // ==================== AlertStats Tests ====================

    #[test]
    fn test_alert_stats_default() {
        let stats = AlertStats::default();
        assert_eq!(stats.total_alerts, 0);
        assert!(stats.alerts_by_severity.is_empty());
        assert!(stats.alerts_by_source.is_empty());
        assert_eq!(stats.failed_notifications, 0);
        assert!(stats.last_alert.is_none());
    }

    #[test]
    fn test_alert_stats_with_data() {
        let mut alerts_by_severity = HashMap::new();
        alerts_by_severity.insert("critical".to_string(), 10);
        alerts_by_severity.insert("warning".to_string(), 30);
        alerts_by_severity.insert("info".to_string(), 60);

        let mut alerts_by_source = HashMap::new();
        alerts_by_source.insert("api".to_string(), 50);
        alerts_by_source.insert("worker".to_string(), 50);

        let stats = AlertStats {
            total_alerts: 100,
            failed_notifications: 5,
            alerts_by_severity,
            alerts_by_source,
            last_alert: Some(chrono::Utc::now()),
        };

        assert_eq!(stats.total_alerts, 100);
        assert_eq!(stats.failed_notifications, 5);
        assert_eq!(stats.alerts_by_severity.len(), 3);
        assert_eq!(stats.alerts_by_source.len(), 2);
        assert!(stats.last_alert.is_some());
    }

    #[test]
    fn test_alert_stats_clone() {
        let mut alerts_by_severity = HashMap::new();
        alerts_by_severity.insert("warning".to_string(), 20);
        let original = AlertStats {
            total_alerts: 42,
            alerts_by_severity,
            ..Default::default()
        };

        let cloned = original.clone();
        assert_eq!(original.total_alerts, cloned.total_alerts);
        assert_eq!(
            original.alerts_by_severity.get("warning"),
            cloned.alerts_by_severity.get("warning")
        );
    }

    #[test]
    fn test_alert_stats_serialize() {
        let mut alerts_by_severity = HashMap::new();
        alerts_by_severity.insert("info".to_string(), 50);
        let stats = AlertStats {
            total_alerts: 50,
            failed_notifications: 2,
            alerts_by_severity,
            ..Default::default()
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("total_alerts"));
        assert!(json.contains("50"));
        assert!(json.contains("failed_notifications"));
        assert!(json.contains("alerts_by_severity"));
    }

    #[test]
    fn test_alert_stats_with_timestamp() {
        let mut stats = AlertStats::default();
        let now = chrono::Utc::now();
        stats.last_alert = Some(now);

        assert_eq!(stats.last_alert, Some(now));
    }

    #[test]
    fn test_alert_stats_increment_counters() {
        let mut stats = AlertStats::default();

        for _ in 0..10 {
            stats.total_alerts += 1;
        }
        assert_eq!(stats.total_alerts, 10);

        for _ in 0..3 {
            stats.failed_notifications += 1;
        }
        assert_eq!(stats.failed_notifications, 3);
    }

    #[test]
    fn test_alert_stats_severity_counts() {
        let mut stats = AlertStats::default();

        *stats
            .alerts_by_severity
            .entry("critical".to_string())
            .or_insert(0) += 5;
        *stats
            .alerts_by_severity
            .entry("warning".to_string())
            .or_insert(0) += 15;
        *stats
            .alerts_by_severity
            .entry("critical".to_string())
            .or_insert(0) += 3;

        assert_eq!(stats.alerts_by_severity.get("critical"), Some(&8));
        assert_eq!(stats.alerts_by_severity.get("warning"), Some(&15));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_alert_storage_with_stats() {
        let mut storage = AlertStorage::default();

        // Add a rule
        let rule = AlertRule {
            id: "test-rule".to_string(),
            name: "Test Rule".to_string(),
            description: "A test rule".to_string(),
            metric: "test.metric".to_string(),
            threshold: 100.0,
            operator: ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Warning,
            interval: Duration::from_secs(60),
            enabled: true,
            channels: vec!["test-channel".to_string()],
        };
        storage.rules.insert(rule.id.clone(), rule);

        // Update stats
        storage.stats.total_alerts = 10;
        storage
            .stats
            .alerts_by_severity
            .insert("warning".to_string(), 10);

        assert_eq!(storage.rules.len(), 1);
        assert_eq!(storage.stats.total_alerts, 10);
    }

    #[test]
    fn test_alert_rule_with_multiple_channels() {
        let rule = AlertRule {
            id: "multi-channel".to_string(),
            name: "Multi Channel Alert".to_string(),
            description: "Alerts to multiple channels".to_string(),
            metric: "errors.rate".to_string(),
            threshold: 10.0,
            operator: ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Critical,
            interval: Duration::from_secs(30),
            enabled: true,
            channels: vec![
                "slack".to_string(),
                "pagerduty".to_string(),
                "email".to_string(),
                "sms".to_string(),
            ],
        };

        assert_eq!(rule.channels.len(), 4);
        assert!(rule.channels.contains(&"slack".to_string()));
        assert!(rule.channels.contains(&"sms".to_string()));
    }

    #[test]
    fn test_alert_rule_interval_variations() {
        // Short interval
        let rule = AlertRule {
            id: "short".to_string(),
            name: "Short Interval".to_string(),
            description: "".to_string(),
            metric: "test".to_string(),
            threshold: 1.0,
            operator: ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Info,
            interval: Duration::from_secs(5),
            enabled: true,
            channels: vec![],
        };
        assert_eq!(rule.interval, Duration::from_secs(5));

        // Long interval
        let rule = AlertRule {
            id: "long".to_string(),
            name: "Long Interval".to_string(),
            description: "".to_string(),
            metric: "test".to_string(),
            threshold: 1.0,
            operator: ComparisonOperator::GreaterThan,
            severity: AlertSeverity::Info,
            interval: Duration::from_secs(3600),
            enabled: true,
            channels: vec![],
        };
        assert_eq!(rule.interval, Duration::from_secs(3600));
    }
}
