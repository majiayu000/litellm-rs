//! Alert processing logic

use super::manager::AlertManager;
use super::types::{AlertRule, ComparisonOperator};
use crate::monitoring::types::Alert;
use crate::utils::error::gateway_error::Result;
use std::time::Duration;
use tracing::{debug, error, warn};

impl AlertManager {
    /// Process a single alert
    pub(super) async fn process_alert(&self, alert: &Alert) -> Result<()> {
        debug!("Processing alert: {}", alert.id);

        // Using tokio RwLock here - safe to hold across await points
        let channels = self.notification_channels.read().await;

        for channel in channels.iter() {
            if channel.supports_severity(alert.severity) {
                if let Err(e) = channel.send(alert).await {
                    warn!("Failed to send alert via {}: {}", channel.name(), e);
                } else {
                    debug!("Alert sent via {}", channel.name());
                }
            }
        }

        Ok(())
    }

    /// Start alert processing task
    pub(super) async fn start_alert_processing(&self) {
        let alert_manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                if !alert_manager.is_active() {
                    break;
                }

                if let Err(e) = alert_manager.process_pending().await {
                    error!("Failed to process pending alerts: {}", e);
                }
            }
        });
    }

    /// Start rule evaluation task
    pub(super) async fn start_rule_evaluation(&self) {
        let alert_manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                if !alert_manager.is_active() {
                    break;
                }

                if let Err(e) = alert_manager.evaluate_rules().await {
                    error!("Failed to evaluate alert rules: {}", e);
                }
            }
        });
    }

    /// Evaluate alert rules
    pub(super) async fn evaluate_rules(&self) -> Result<()> {
        debug!("Evaluating alert rules");

        let rules = self.storage.read().rules.clone();

        for rule in rules.values() {
            if rule.enabled
                && let Err(e) = self.evaluate_rule(rule).await
            {
                warn!("Failed to evaluate rule {}: {}", rule.name, e);
            }
        }

        Ok(())
    }

    /// Evaluate a single alert rule
    pub(super) async fn evaluate_rule(&self, rule: &AlertRule) -> Result<()> {
        // TODO: Implement metric evaluation
        // This would involve getting the current metric value and comparing it to the threshold

        debug!("Evaluating rule: {}", rule.name);

        // Placeholder implementation
        let metric_value = 0.0; // Get actual metric value
        let threshold_exceeded = match rule.operator {
            ComparisonOperator::GreaterThan => metric_value > rule.threshold,
            ComparisonOperator::LessThan => metric_value < rule.threshold,
            ComparisonOperator::GreaterThanOrEqual => metric_value >= rule.threshold,
            ComparisonOperator::LessThanOrEqual => metric_value <= rule.threshold,
            ComparisonOperator::Equal => (metric_value - rule.threshold).abs() < f64::EPSILON,
            ComparisonOperator::NotEqual => (metric_value - rule.threshold).abs() >= f64::EPSILON,
        };

        if threshold_exceeded {
            let alert = Alert {
                id: uuid::Uuid::new_v4().to_string(),
                severity: rule.severity,
                title: format!("Alert Rule Triggered: {}", rule.name),
                description: format!(
                    "Rule '{}' triggered: {} {} {} (current value: {})",
                    rule.name,
                    rule.metric,
                    format!("{:?}", rule.operator).to_lowercase(),
                    rule.threshold,
                    metric_value
                ),
                timestamp: chrono::Utc::now(),
                source: "alert_rule".to_string(),
                metadata: serde_json::json!({
                    "rule_id": rule.id,
                    "metric": rule.metric,
                    "threshold": rule.threshold,
                    "current_value": metric_value,
                    "operator": format!("{:?}", rule.operator)
                }),
                resolved: false,
            };

            self.send_alert(alert).await?;
        }

        Ok(())
    }
}
