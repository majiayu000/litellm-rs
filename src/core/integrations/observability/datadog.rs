//! DataDog APM Integration
//!
//! Provides integration with DataDog for APM, metrics, and logging.

use crate::config::models::defaults::default_true;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::core::traits::integration::{
    CacheHitEvent, EmbeddingEndEvent, EmbeddingStartEvent, Integration, IntegrationError,
    IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
};
use crate::utils::net::http::create_custom_client;

/// DataDog configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDogConfig {
    /// DataDog API key
    pub api_key: String,

    /// DataDog site (e.g., datadoghq.com, datadoghq.eu)
    #[serde(default = "default_site")]
    pub site: String,

    /// Service name for APM
    #[serde(default = "default_service")]
    pub service: String,

    /// Environment (e.g., production, staging)
    #[serde(default)]
    pub env: Option<String>,

    /// Version tag
    #[serde(default)]
    pub version: Option<String>,

    /// Enable metrics
    #[serde(default = "default_true")]
    pub enable_metrics: bool,

    /// Enable APM traces
    #[serde(default = "default_true")]
    pub enable_traces: bool,

    /// Enable logs
    #[serde(default = "default_true")]
    pub enable_logs: bool,

    /// Batch size for sending events
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Flush interval in milliseconds
    #[serde(default = "default_flush_interval")]
    pub flush_interval_ms: u64,

    /// Additional tags
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

fn default_site() -> String {
    "datadoghq.com".to_string()
}

const SUPPORTED_DATADOG_SITES: &[&str] = &[
    "datadoghq.com",
    "us3.datadoghq.com",
    "us5.datadoghq.com",
    "datadoghq.eu",
    "ap1.datadoghq.com",
    "ap2.datadoghq.com",
    "uk1.datadoghq.com",
    "ddog-gov.com",
    "us2.ddog-gov.com",
];

fn default_service() -> String {
    "litellm-gateway".to_string()
}

fn default_batch_size() -> usize {
    100
}

fn default_flush_interval() -> u64 {
    10000
}

impl Default for DataDogConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            site: default_site(),
            service: default_service(),
            env: None,
            version: None,
            enable_metrics: true,
            enable_traces: true,
            enable_logs: true,
            batch_size: default_batch_size(),
            flush_interval_ms: default_flush_interval(),
            tags: HashMap::new(),
        }
    }
}

impl DataDogConfig {
    /// Create a new DataDog configuration
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            ..Default::default()
        }
    }

    /// Set the DataDog site
    pub fn site(mut self, site: impl Into<String>) -> Self {
        self.site = site.into();
        self
    }

    pub(crate) fn is_supported_site(site: &str) -> bool {
        SUPPORTED_DATADOG_SITES.contains(&site)
    }

    /// Set the service name
    pub fn service(mut self, service: impl Into<String>) -> Self {
        self.service = service.into();
        self
    }

    /// Set the environment
    pub fn env(mut self, env: impl Into<String>) -> Self {
        self.env = Some(env.into());
        self
    }

    /// Set the version
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Add a tag
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("DD_API_KEY")
            .or_else(|_| std::env::var("DATADOG_API_KEY"))
            .ok()?;

        Some(Self {
            api_key,
            site: std::env::var("DD_SITE").unwrap_or_else(|_| default_site()),
            service: std::env::var("DD_SERVICE").unwrap_or_else(|_| default_service()),
            env: std::env::var("DD_ENV").ok(),
            version: std::env::var("DD_VERSION").ok(),
            ..Default::default()
        })
    }

    /// Get the metrics API URL
    pub fn metrics_url(&self) -> String {
        format!("https://api.{}/api/v2/series", self.site)
    }

    /// Get the logs API URL
    pub fn logs_url(&self) -> String {
        format!("https://http-intake.logs.{}/api/v2/logs", self.site)
    }

    /// Get the traces API URL
    pub fn traces_url(&self) -> String {
        format!("https://trace.agent.{}/api/v0.2/traces", self.site)
    }
}

/// DataDog metric point
#[derive(Debug, Clone, Serialize)]
struct MetricPoint {
    timestamp: i64,
    value: f64,
}

/// DataDog metric series
#[derive(Debug, Clone, Serialize)]
struct MetricSeries {
    metric: String,
    #[serde(rename = "type")]
    metric_type: i32,
    points: Vec<MetricPoint>,
    tags: Vec<String>,
    unit: Option<String>,
}

/// DataDog metrics payload
#[derive(Debug, Clone, Serialize)]
struct MetricsPayload {
    series: Vec<MetricSeries>,
}

/// DataDog log entry
#[derive(Debug, Clone, Serialize)]
struct DataDogLogRecord {
    ddsource: String,
    ddtags: String,
    hostname: String,
    message: String,
    service: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<i64>,
}

/// Buffered event for batching
#[derive(Debug, Clone)]
enum BufferedEvent {
    Metric(MetricSeries),
    Log(DataDogLogRecord),
}

/// DataDog APM Integration
pub struct DataDogIntegration {
    config: DataDogConfig,
    http_client: Client,
    buffer: Arc<RwLock<Vec<BufferedEvent>>>,
    enabled: bool,
}

impl std::fmt::Debug for DataDogIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataDogIntegration")
            .field("service", &self.config.service)
            .field("site", &self.config.site)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl DataDogIntegration {
    /// Create a new DataDog integration
    pub fn new(config: DataDogConfig) -> IntegrationResult<Self> {
        if config.api_key.trim().is_empty() {
            return Err(IntegrationError::config(
                "DataDog API key is required".to_string(),
            ));
        }
        if !DataDogConfig::is_supported_site(&config.site) {
            return Err(IntegrationError::config(
                "DataDog site is not a supported Datadog site".to_string(),
            ));
        }

        let http_client = create_custom_client(Duration::from_secs(30)).map_err(|e| {
            IntegrationError::connection(format!("Failed to create HTTP client: {}", e))
        })?;

        info!(
            "DataDog integration initialized for service: {}",
            config.service
        );

        Ok(Self {
            config,
            http_client,
            buffer: Arc::new(RwLock::new(Vec::new())),
            enabled: true,
        })
    }

    /// Create from environment variables
    pub fn from_env() -> IntegrationResult<Self> {
        let config = DataDogConfig::from_env()
            .ok_or_else(|| IntegrationError::config("DD_API_KEY not set".to_string()))?;
        Self::new(config)
    }

    /// Get current timestamp in seconds
    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// Build tags list
    fn build_tags(&self, extra_tags: &[(&str, &str)]) -> Vec<String> {
        let mut tags = Vec::new();

        // Add service tag
        tags.push(format!("service:{}", self.config.service));

        // Add env tag
        if let Some(env) = &self.config.env {
            tags.push(format!("env:{}", env));
        }

        // Add version tag
        if let Some(version) = &self.config.version {
            tags.push(format!("version:{}", version));
        }

        // Add configured tags
        for (key, value) in &self.config.tags {
            tags.push(format!("{}:{}", key, value));
        }

        // Add extra tags
        for (key, value) in extra_tags {
            tags.push(format!("{}:{}", key, value));
        }

        tags
    }

    /// Build tags string for logs
    fn build_tags_string(&self, extra_tags: &[(&str, &str)]) -> String {
        self.build_tags(extra_tags).join(",")
    }

    /// Record a metric
    async fn record_metric(
        &self,
        name: &str,
        value: f64,
        metric_type: i32,
        tags: &[(&str, &str)],
        unit: Option<&str>,
    ) -> IntegrationResult<()> {
        if !self.config.enable_metrics {
            return Ok(());
        }

        let metric = MetricSeries {
            metric: format!("litellm.{}", name),
            metric_type,
            points: vec![MetricPoint {
                timestamp: Self::current_timestamp(),
                value,
            }],
            tags: self.build_tags(tags),
            unit: unit.map(String::from),
        };

        let mut buffer = self.buffer.write().await;
        buffer.push(BufferedEvent::Metric(metric));

        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            self.flush().await?;
        }
        Ok(())
    }

    /// Record a log entry
    async fn record_log(
        &self,
        message: &str,
        status: &str,
        tags: &[(&str, &str)],
    ) -> IntegrationResult<()> {
        if !self.config.enable_logs {
            return Ok(());
        }

        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("HOST"))
            .unwrap_or_else(|_| "unknown".to_string());

        let log = DataDogLogRecord {
            ddsource: "litellm-gateway".to_string(),
            ddtags: self.build_tags_string(tags),
            hostname,
            message: message.to_string(),
            service: self.config.service.clone(),
            status: status.to_string(),
            timestamp: Some(Self::current_timestamp() * 1000), // milliseconds
        };

        let mut buffer = self.buffer.write().await;
        buffer.push(BufferedEvent::Log(log));

        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            self.flush().await?;
        }
        Ok(())
    }

    /// Send metrics to DataDog
    async fn send_metrics(&self, metrics: Vec<MetricSeries>) -> IntegrationResult<()> {
        if metrics.is_empty() {
            return Ok(());
        }

        let payload = MetricsPayload { series: metrics };

        let response = self
            .http_client
            .post(self.config.metrics_url())
            .header("DD-API-KEY", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| IntegrationError::connection(format!("Failed to send metrics: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let response_bytes = response.bytes().await.map(|bytes| bytes.len()).unwrap_or(0);
            return Err(IntegrationError::connection(format!(
                "DataDog metrics API returned {status} ({response_bytes} response bytes)"
            )));
        }

        Ok(())
    }

    /// Send logs to DataDog
    async fn send_logs(&self, logs: Vec<DataDogLogRecord>) -> IntegrationResult<()> {
        if logs.is_empty() {
            return Ok(());
        }

        let response = self
            .http_client
            .post(self.config.logs_url())
            .header("DD-API-KEY", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&logs)
            .send()
            .await
            .map_err(|e| IntegrationError::connection(format!("Failed to send logs: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let response_bytes = response.bytes().await.map(|bytes| bytes.len()).unwrap_or(0);
            return Err(IntegrationError::connection(format!(
                "DataDog logs API returned {status} ({response_bytes} response bytes)"
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl Integration for DataDogIntegration {
    fn name(&self) -> &'static str {
        "datadog"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()> {
        debug!("DataDog: LLM request started - {}", event.request_id);

        let tags = [
            ("model", event.model.as_str()),
            ("provider", event.provider.as_deref().unwrap_or("unknown")),
        ];

        // Record request count metric
        self.record_metric("llm.requests", 1.0, 1, &tags, None)
            .await?;

        // Log the request start
        self.record_log(
            &format!(
                "LLM request started: request_id={}, model={}",
                event.request_id, event.model
            ),
            "info",
            &tags,
        )
        .await?;

        Ok(())
    }

    async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
        debug!("DataDog: LLM request completed - {}", event.request_id);

        let tags = [
            ("model", event.model.as_str()),
            ("provider", event.provider.as_deref().unwrap_or("unknown")),
            ("status", "success"),
        ];

        // Record latency metric
        self.record_metric(
            "llm.latency",
            event.latency_ms as f64,
            3, // Gauge
            &tags,
            Some("millisecond"),
        )
        .await?;

        // Record token metrics
        if let Some(input_tokens) = event.input_tokens {
            self.record_metric(
                "llm.tokens.prompt",
                input_tokens as f64,
                1, // Count
                &tags,
                None,
            )
            .await?;
        }

        if let Some(output_tokens) = event.output_tokens {
            self.record_metric(
                "llm.tokens.completion",
                output_tokens as f64,
                1,
                &tags,
                None,
            )
            .await?;
        }

        if let (Some(input), Some(output)) = (event.input_tokens, event.output_tokens) {
            self.record_metric("llm.tokens.total", (input + output) as f64, 1, &tags, None)
                .await?;
        }

        // Record cost metric
        if let Some(cost) = event.cost_usd {
            self.record_metric("llm.cost", cost, 1, &tags, Some("dollar"))
                .await?;
        }

        // Log completion
        self.record_log(
            &format!(
                "LLM request completed: request_id={}, model={}, latency={}ms",
                event.request_id, event.model, event.latency_ms
            ),
            "info",
            &tags,
        )
        .await?;

        Ok(())
    }

    async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
        debug!("DataDog: LLM request error - {}", event.request_id);

        let error_type = event.error_type.as_deref().unwrap_or("unknown");
        let tags = [
            ("model", event.model.as_str()),
            ("provider", event.provider.as_deref().unwrap_or("unknown")),
            ("error_type", error_type),
            ("status", "error"),
        ];

        // Record error count
        self.record_metric("llm.errors", 1.0, 1, &tags, None)
            .await?;

        // Log the error
        self.record_log(
            &format!(
                "LLM request error: request_id={}, model={}, error={}",
                event.request_id, event.model, event.error_message
            ),
            "error",
            &tags,
        )
        .await?;

        Ok(())
    }

    async fn on_llm_stream(&self, _event: &LlmStreamEvent) -> IntegrationResult<()> {
        // Record streaming chunk metric
        let tags: [(&str, &str); 0] = [];

        self.record_metric("llm.stream.chunks", 1.0, 1, &tags, None)
            .await?;

        Ok(())
    }

    async fn on_embedding_start(&self, event: &EmbeddingStartEvent) -> IntegrationResult<()> {
        let tags = [
            ("model", event.model.as_str()),
            ("provider", event.provider.as_deref().unwrap_or("unknown")),
        ];

        self.record_metric("embedding.requests", 1.0, 1, &tags, None)
            .await?;

        Ok(())
    }

    async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
        let tags = [
            ("model", event.model.as_str()),
            ("provider", event.provider.as_deref().unwrap_or("unknown")),
        ];

        self.record_metric(
            "embedding.latency",
            event.latency_ms as f64,
            3,
            &tags,
            Some("millisecond"),
        )
        .await?;

        if let Some(tokens) = event.total_tokens {
            self.record_metric("embedding.tokens", tokens as f64, 1, &tags, None)
                .await?;
        }

        Ok(())
    }

    async fn on_cache_hit(&self, event: &CacheHitEvent) -> IntegrationResult<()> {
        let tags = [("cache_backend", event.cache_backend.as_str())];

        self.record_metric("cache.hits", 1.0, 1, &tags, None)
            .await?;

        Ok(())
    }

    async fn flush(&self) -> IntegrationResult<()> {
        let events = {
            let mut buffer = self.buffer.write().await;
            std::mem::take(&mut *buffer)
        };

        if events.is_empty() {
            return Ok(());
        }

        debug!("DataDog: Flushing {} events", events.len());

        let mut metrics = Vec::new();
        let mut logs = Vec::new();

        for event in events {
            match event {
                BufferedEvent::Metric(m) => metrics.push(m),
                BufferedEvent::Log(l) => logs.push(l),
            }
        }

        let retry_metrics = metrics
            .iter()
            .cloned()
            .map(BufferedEvent::Metric)
            .collect::<Vec<_>>();
        let retry_logs = logs
            .iter()
            .cloned()
            .map(BufferedEvent::Log)
            .collect::<Vec<_>>();

        let (metrics_result, logs_result) =
            tokio::join!(self.send_metrics(metrics), self.send_logs(logs));

        let mut retry_events = Vec::new();
        let mut errors = Vec::new();
        if let Err(error) = metrics_result {
            retry_events.extend(retry_metrics);
            errors.push(error.to_string());
        }
        if let Err(error) = logs_result {
            retry_events.extend(retry_logs);
            errors.push(error.to_string());
        }
        if !retry_events.is_empty() {
            self.buffer.write().await.extend(retry_events);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(IntegrationError::other(errors.join("; ")))
        }
    }

    async fn shutdown(&self) -> IntegrationResult<()> {
        info!("DataDog integration shutting down");
        self.flush().await
    }
}

#[cfg(test)]
#[path = "datadog_tests.rs"]
mod tests;
