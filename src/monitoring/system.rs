//! Core MonitoringSystem implementation

use crate::config::models::monitoring::MonitoringConfig;
use crate::storage::StorageLayer;
use crate::utils::error::gateway_error::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use super::types::*;
use super::{alerts, metrics};

/// Main monitoring system
#[derive(Clone)]
pub struct MonitoringSystem {
    /// Monitoring configuration
    pub(super) _config: Arc<MonitoringConfig>,
    /// Storage layer for persistence
    pub(super) _storage: Arc<StorageLayer>,
    /// Metrics collector
    pub(super) metrics: Arc<metrics::MetricsCollector>,
    /// Alert manager
    pub(super) alerts: Option<Arc<alerts::AlertManager>>,
    /// System start time
    pub(super) start_time: Instant,
}

impl MonitoringSystem {
    /// Create a new monitoring system
    pub async fn new(config: &MonitoringConfig, storage: Arc<StorageLayer>) -> Result<Self> {
        info!("Initializing monitoring system");

        let config = Arc::new(config.clone());

        // Initialize metrics collector
        let metrics = Arc::new(metrics::MetricsCollector::new(&config).await?);

        // Initialize alert manager (if enabled)
        let alerts = None; // NOTE: alerting config not yet added to MonitoringConfig

        info!("Monitoring system initialized successfully");

        Ok(Self {
            _config: config,
            _storage: storage,
            metrics,
            alerts,
            start_time: Instant::now(),
        })
    }

    /// Start the monitoring system
    pub async fn start(&self) -> Result<()> {
        info!("Starting monitoring system");

        // Start metrics collection
        self.metrics.start().await?;

        // Start alert manager
        if let Some(alerts) = &self.alerts {
            alerts.start().await?;
        }

        // Start background tasks
        self.start_background_tasks().await?;

        info!("Monitoring system started successfully");
        Ok(())
    }

    /// Stop the monitoring system
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping monitoring system");

        // Stop metrics collection
        self.metrics.stop().await?;

        // Stop alert manager
        if let Some(alerts) = &self.alerts {
            alerts.stop().await?;
        }

        info!("Monitoring system stopped");
        Ok(())
    }

    /// Get current system metrics
    pub async fn get_metrics(&self) -> Result<SystemMetrics> {
        debug!("Collecting system metrics");

        let timestamp = chrono::Utc::now();

        // Collect metrics from various sources
        let requests = self.collect_request_metrics().await?;
        let providers = self.collect_provider_metrics().await?;
        let system = self.collect_system_metrics().await?;
        let errors = self.collect_error_metrics().await?;
        let performance = self.collect_performance_metrics().await?;

        Ok(SystemMetrics {
            timestamp,
            requests,
            providers,
            system,
            errors,
            performance,
        })
    }

    /// Record a request metric
    pub async fn record_request(
        &self,
        method: &str,
        path: &str,
        status_code: u16,
        response_time: Duration,
        user_id: Option<uuid::Uuid>,
        api_key_id: Option<uuid::Uuid>,
    ) -> Result<()> {
        self.metrics
            .record_request(
                method,
                path,
                status_code,
                response_time,
                user_id,
                api_key_id,
            )
            .await
    }

    /// Record a provider request metric
    pub async fn record_provider_request(
        &self,
        provider: &str,
        model: &str,
        tokens_used: u32,
        cost: f64,
        response_time: Duration,
        success: bool,
    ) -> Result<()> {
        self.metrics
            .record_provider_request(provider, model, tokens_used, cost, response_time, success)
            .await
    }

    /// Record an error
    pub async fn record_error(
        &self,
        error_type: &str,
        error_message: &str,
        context: Option<serde_json::Value>,
    ) -> Result<()> {
        self.metrics
            .record_error(error_type, error_message, context)
            .await
    }

    /// Send an alert
    pub async fn send_alert(&self, alert: Alert) -> Result<()> {
        if let Some(alerts) = &self.alerts {
            alerts.send_alert(alert).await
        } else {
            warn!("Alert manager not configured, skipping alert");
            Ok(())
        }
    }

    /// Get system uptime
    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Collect request metrics
    pub(super) async fn collect_request_metrics(&self) -> Result<MonitoringRequestMetrics> {
        self.metrics.get_request_metrics().await
    }

    /// Collect provider metrics
    pub(super) async fn collect_provider_metrics(&self) -> Result<ProviderMetrics> {
        self.metrics.get_provider_metrics().await
    }

    /// Collect system resource metrics
    pub(super) async fn collect_system_metrics(&self) -> Result<SystemResourceMetrics> {
        self.metrics.get_system_metrics().await
    }

    /// Collect error metrics
    pub(super) async fn collect_error_metrics(&self) -> Result<ErrorMetrics> {
        self.metrics.get_error_metrics().await
    }

    /// Collect performance metrics
    pub(super) async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics> {
        self.metrics.get_performance_metrics().await
    }
}
