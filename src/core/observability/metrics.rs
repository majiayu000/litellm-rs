//! Metrics collection and export

use super::histogram::BoundedHistogram;
use super::types::TokenUsage;
use crate::utils::error::gateway_error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

/// Prometheus metrics structure
#[derive(Debug, Default)]
pub struct PrometheusMetrics {
    /// Request counters
    pub request_total: HashMap<String, u64>,
    /// Request duration histograms (bounded to prevent memory leaks)
    pub request_duration: HashMap<String, BoundedHistogram>,
    /// Error counters
    pub error_total: HashMap<String, u64>,
    /// Token usage counters
    pub token_usage: HashMap<String, u64>,
    /// Cost tracking
    pub cost_total: HashMap<String, f64>,
    /// Provider health status
    pub provider_health: HashMap<String, f64>,
    /// Cache hit/miss ratios
    pub cache_hits: u64,
    pub cache_misses: u64,
    /// Active connections
    pub active_connections: u64,
    /// Queue sizes
    pub queue_size: HashMap<String, u64>,
}

/// DataDog client for metrics
pub struct DataDogClient {
    /// API key
    pub api_key: String,
    /// Base URL
    pub base_url: String,
    /// HTTP client
    pub client: reqwest::Client,
    /// Tags to add to all metrics
    pub default_tags: Vec<String>,
}

/// OpenTelemetry exporter
pub struct OtelExporter {
    /// Endpoint URL
    pub endpoint: String,
    /// Headers
    pub headers: HashMap<String, String>,
    /// HTTP client
    pub client: reqwest::Client,
}

/// Metrics collector and exporter
pub struct MetricsCollector {
    /// Prometheus metrics
    pub prometheus_metrics: Arc<RwLock<PrometheusMetrics>>,
    /// DataDog metrics
    datadog_client: Option<DataDogClient>,
    /// OpenTelemetry exporter
    otel_exporter: Option<OtelExporter>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            prometheus_metrics: Arc::new(RwLock::new(PrometheusMetrics::default())),
            datadog_client: None,
            otel_exporter: None,
        }
    }

    /// Configure DataDog integration
    pub fn with_datadog(mut self, api_key: String, site: String) -> Self {
        self.datadog_client = Some(DataDogClient {
            api_key,
            base_url: format!("https://api.{}", site),
            client: reqwest::Client::new(),
            default_tags: vec![
                "service:litellm-gateway".to_string(),
                "env:production".to_string(),
            ],
        });
        self
    }

    /// Configure OpenTelemetry integration
    pub fn with_otel(mut self, endpoint: String, headers: HashMap<String, String>) -> Self {
        self.otel_exporter = Some(OtelExporter {
            endpoint,
            headers,
            client: reqwest::Client::new(),
        });
        self
    }

    /// Record request metrics
    pub async fn record_request(
        &self,
        provider: &str,
        model: &str,
        duration: Duration,
        tokens: Option<TokenUsage>,
        cost: Option<f64>,
        success: bool,
    ) {
        let key = format!("{}:{}", provider, model);
        let duration_secs = duration.as_secs_f64();

        {
            let mut metrics = self.prometheus_metrics.write().await;

            // Request counter
            *metrics.request_total.entry(key.clone()).or_insert(0) += 1;

            // Duration histogram (bounded to prevent memory leaks)
            metrics
                .request_duration
                .entry(key.clone())
                .or_insert_with(BoundedHistogram::default)
                .record(duration_secs);

            // Error counter
            if !success {
                *metrics.error_total.entry(key.clone()).or_insert(0) += 1;
            }
        }

        // Token usage
        if let Some(token_usage) = tokens {
            let prompt_key = format!("{}:prompt", key);
            let completion_key = format!("{}:completion", key);
            let prompt_tokens = token_usage.prompt_tokens as u64;
            let completion_tokens = token_usage.completion_tokens as u64;

            let mut metrics = self.prometheus_metrics.write().await;
            *metrics.token_usage.entry(prompt_key).or_insert(0) += prompt_tokens;
            *metrics.token_usage.entry(completion_key).or_insert(0) += completion_tokens;
        }

        // Cost tracking
        if let Some(request_cost) = cost {
            let mut metrics = self.prometheus_metrics.write().await;
            *metrics.cost_total.entry(key).or_insert(0.0) += request_cost;
        }
    }

    /// Record cache metrics
    pub async fn record_cache_hit(&self, hit: bool) {
        let mut metrics = self.prometheus_metrics.write().await;
        if hit {
            metrics.cache_hits += 1;
        } else {
            metrics.cache_misses += 1;
        }
    }

    /// Update provider health
    pub async fn update_provider_health(&self, provider: &str, health_score: f64) {
        let mut metrics = self.prometheus_metrics.write().await;
        metrics
            .provider_health
            .insert(provider.to_string(), health_score);
    }

    /// Export metrics to Prometheus format
    pub async fn export_prometheus(&self) -> String {
        let metrics = self.prometheus_metrics.read().await;
        let mut output = String::new();

        // Request total
        output.push_str("# HELP litellm_requests_total Total number of requests\n");
        output.push_str("# TYPE litellm_requests_total counter\n");
        for (key, value) in &metrics.request_total {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() == 2 {
                output.push_str(&format!(
                    "litellm_requests_total{{provider=\"{}\",model=\"{}\"}} {}\n",
                    parts[0], parts[1], value
                ));
            }
        }

        // Error total
        output.push_str("# HELP litellm_errors_total Total number of errors\n");
        output.push_str("# TYPE litellm_errors_total counter\n");
        for (key, value) in &metrics.error_total {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() == 2 {
                output.push_str(&format!(
                    "litellm_errors_total{{provider=\"{}\",model=\"{}\"}} {}\n",
                    parts[0], parts[1], value
                ));
            }
        }

        // Cache metrics
        output.push_str("# HELP litellm_cache_hits_total Total cache hits\n");
        output.push_str("# TYPE litellm_cache_hits_total counter\n");
        output.push_str(&format!(
            "litellm_cache_hits_total {}\n",
            metrics.cache_hits
        ));

        output.push_str("# HELP litellm_cache_misses_total Total cache misses\n");
        output.push_str("# TYPE litellm_cache_misses_total counter\n");
        output.push_str(&format!(
            "litellm_cache_misses_total {}\n",
            metrics.cache_misses
        ));

        // Provider health
        output.push_str("# HELP litellm_provider_health Provider health score\n");
        output.push_str("# TYPE litellm_provider_health gauge\n");
        for (provider, health) in &metrics.provider_health {
            output.push_str(&format!(
                "litellm_provider_health{{provider=\"{}\"}} {}\n",
                provider, health
            ));
        }

        output
    }

    /// Send metrics to DataDog
    pub async fn send_to_datadog(&self) -> Result<()> {
        if let Some(_client) = &self.datadog_client {
            let _metrics = self.prometheus_metrics.read().await;

            // Convert metrics to DataDog format and send
            // Implementation would depend on DataDog API format
            debug!("Sending metrics to DataDog");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== PrometheusMetrics Tests ====================

    #[test]
    fn test_prometheus_metrics_default() {
        let metrics = PrometheusMetrics::default();
        assert!(metrics.request_total.is_empty());
        assert!(metrics.request_duration.is_empty());
        assert!(metrics.error_total.is_empty());
        assert!(metrics.token_usage.is_empty());
        assert!(metrics.cost_total.is_empty());
        assert!(metrics.provider_health.is_empty());
        assert_eq!(metrics.cache_hits, 0);
        assert_eq!(metrics.cache_misses, 0);
        assert_eq!(metrics.active_connections, 0);
        assert!(metrics.queue_size.is_empty());
    }

    // ==================== MetricsCollector Creation Tests ====================

    #[test]
    fn test_metrics_collector_new() {
        let collector = MetricsCollector::new();
        // Should have empty metrics initially
        assert!(collector.datadog_client.is_none());
        assert!(collector.otel_exporter.is_none());
    }

    #[test]
    fn test_metrics_collector_default() {
        let collector = MetricsCollector::default();
        assert!(collector.datadog_client.is_none());
        assert!(collector.otel_exporter.is_none());
    }

    #[test]
    fn test_metrics_collector_with_datadog() {
        let collector = MetricsCollector::new()
            .with_datadog("api-key".to_string(), "datadoghq.com".to_string());
        assert!(collector.datadog_client.is_some());
        let client = collector.datadog_client.unwrap();
        assert_eq!(client.api_key, "api-key");
        assert_eq!(client.base_url, "https://api.datadoghq.com");
        assert!(!client.default_tags.is_empty());
    }

    #[test]
    fn test_metrics_collector_with_otel() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());

        let collector = MetricsCollector::new()
            .with_otel("https://otel.example.com".to_string(), headers.clone());
        assert!(collector.otel_exporter.is_some());
        let exporter = collector.otel_exporter.unwrap();
        assert_eq!(exporter.endpoint, "https://otel.example.com");
        assert!(exporter.headers.contains_key("Authorization"));
    }

    #[test]
    fn test_metrics_collector_chained_config() {
        let headers = HashMap::new();
        let collector = MetricsCollector::new()
            .with_datadog("api-key".to_string(), "datadoghq.com".to_string())
            .with_otel("https://otel.example.com".to_string(), headers);

        assert!(collector.datadog_client.is_some());
        assert!(collector.otel_exporter.is_some());
    }

    // ==================== Record Request Tests ====================

    #[tokio::test]
    async fn test_record_request_basic() {
        let collector = MetricsCollector::new();
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                None,
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.request_total.get("openai:gpt-4").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_record_request_multiple() {
        let collector = MetricsCollector::new();

        for _ in 0..5 {
            collector
                .record_request(
                    "openai",
                    "gpt-4",
                    Duration::from_millis(100),
                    None,
                    None,
                    true,
                )
                .await;
        }

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.request_total.get("openai:gpt-4").unwrap(), 5);
    }

    #[tokio::test]
    async fn test_record_request_with_error() {
        let collector = MetricsCollector::new();
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                None,
                false, // Error
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.request_total.get("openai:gpt-4").unwrap(), 1);
        assert_eq!(*metrics.error_total.get("openai:gpt-4").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_record_request_updates_all_primary_metrics() {
        let collector = MetricsCollector::new();
        let tokens = TokenUsage {
            prompt_tokens: 42,
            completion_tokens: 24,
            total_tokens: 66,
        };

        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                Some(tokens),
                Some(0.12),
                false,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.request_total.get("openai:gpt-4").unwrap(), 1);
        assert_eq!(*metrics.error_total.get("openai:gpt-4").unwrap(), 1);
        assert_eq!(*metrics.token_usage.get("openai:gpt-4:prompt").unwrap(), 42);
        assert_eq!(
            *metrics.token_usage.get("openai:gpt-4:completion").unwrap(),
            24
        );
        assert_eq!(*metrics.cost_total.get("openai:gpt-4").unwrap(), 0.12);
    }

    #[tokio::test]
    async fn test_record_request_with_tokens() {
        let collector = MetricsCollector::new();
        let tokens = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };

        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                Some(tokens),
                None,
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(
            *metrics.token_usage.get("openai:gpt-4:prompt").unwrap(),
            100
        );
        assert_eq!(
            *metrics.token_usage.get("openai:gpt-4:completion").unwrap(),
            50
        );
    }

    #[tokio::test]
    async fn test_record_request_with_cost() {
        let collector = MetricsCollector::new();
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                Some(0.05),
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.cost_total.get("openai:gpt-4").unwrap(), 0.05);
    }

    #[tokio::test]
    async fn test_record_request_cost_accumulates() {
        let collector = MetricsCollector::new();

        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                Some(0.05),
                true,
            )
            .await;
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                Some(0.03),
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert!((metrics.cost_total.get("openai:gpt-4").unwrap() - 0.08).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_record_request_different_providers() {
        let collector = MetricsCollector::new();

        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                None,
                true,
            )
            .await;
        collector
            .record_request(
                "anthropic",
                "claude-3",
                Duration::from_millis(150),
                None,
                None,
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.request_total.get("openai:gpt-4").unwrap(), 1);
        assert_eq!(*metrics.request_total.get("anthropic:claude-3").unwrap(), 1);
    }

    // ==================== Cache Metrics Tests ====================

    #[tokio::test]
    async fn test_record_cache_hit() {
        let collector = MetricsCollector::new();
        collector.record_cache_hit(true).await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 0);
    }

    #[tokio::test]
    async fn test_record_cache_miss() {
        let collector = MetricsCollector::new();
        collector.record_cache_hit(false).await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(metrics.cache_hits, 0);
        assert_eq!(metrics.cache_misses, 1);
    }

    #[tokio::test]
    async fn test_record_cache_mixed() {
        let collector = MetricsCollector::new();

        for _ in 0..5 {
            collector.record_cache_hit(true).await;
        }
        for _ in 0..3 {
            collector.record_cache_hit(false).await;
        }

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(metrics.cache_hits, 5);
        assert_eq!(metrics.cache_misses, 3);
    }

    // ==================== Provider Health Tests ====================

    #[tokio::test]
    async fn test_update_provider_health() {
        let collector = MetricsCollector::new();
        collector.update_provider_health("openai", 0.95).await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.provider_health.get("openai").unwrap(), 0.95);
    }

    #[tokio::test]
    async fn test_update_provider_health_multiple() {
        let collector = MetricsCollector::new();
        collector.update_provider_health("openai", 0.95).await;
        collector.update_provider_health("anthropic", 0.99).await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.provider_health.get("openai").unwrap(), 0.95);
        assert_eq!(*metrics.provider_health.get("anthropic").unwrap(), 0.99);
    }

    #[tokio::test]
    async fn test_update_provider_health_overwrite() {
        let collector = MetricsCollector::new();
        collector.update_provider_health("openai", 0.95).await;
        collector.update_provider_health("openai", 0.80).await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.provider_health.get("openai").unwrap(), 0.80);
    }

    // ==================== Prometheus Export Tests ====================

    #[tokio::test]
    async fn test_export_prometheus_empty() {
        let collector = MetricsCollector::new();
        let output = collector.export_prometheus().await;

        // Should contain headers even when empty
        assert!(output.contains("# HELP litellm_requests_total"));
        assert!(output.contains("# TYPE litellm_requests_total counter"));
        assert!(output.contains("litellm_cache_hits_total 0"));
        assert!(output.contains("litellm_cache_misses_total 0"));
    }

    #[tokio::test]
    async fn test_export_prometheus_with_requests() {
        let collector = MetricsCollector::new();
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                None,
                true,
            )
            .await;

        let output = collector.export_prometheus().await;

        assert!(output.contains("litellm_requests_total{provider=\"openai\",model=\"gpt-4\"} 1"));
    }

    #[tokio::test]
    async fn test_export_prometheus_with_errors() {
        let collector = MetricsCollector::new();
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                None,
                false,
            )
            .await;

        let output = collector.export_prometheus().await;

        assert!(output.contains("litellm_errors_total{provider=\"openai\",model=\"gpt-4\"} 1"));
    }

    #[tokio::test]
    async fn test_export_prometheus_with_cache() {
        let collector = MetricsCollector::new();
        collector.record_cache_hit(true).await;
        collector.record_cache_hit(true).await;
        collector.record_cache_hit(false).await;

        let output = collector.export_prometheus().await;

        assert!(output.contains("litellm_cache_hits_total 2"));
        assert!(output.contains("litellm_cache_misses_total 1"));
    }

    #[tokio::test]
    async fn test_export_prometheus_with_health() {
        let collector = MetricsCollector::new();
        collector.update_provider_health("openai", 0.95).await;

        let output = collector.export_prometheus().await;

        assert!(output.contains("litellm_provider_health{provider=\"openai\"} 0.95"));
    }

    #[tokio::test]
    async fn test_export_prometheus_format() {
        let collector = MetricsCollector::new();
        let output = collector.export_prometheus().await;

        // Check Prometheus format compliance
        for line in output.lines() {
            if !line.is_empty() {
                assert!(
                    line.starts_with('#') || line.starts_with("litellm_"),
                    "Invalid line format: {}",
                    line
                );
            }
        }
    }

    // ==================== DataDog Integration Tests ====================

    #[tokio::test]
    async fn test_send_to_datadog_no_client() {
        let collector = MetricsCollector::new();
        let result = collector.send_to_datadog().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_to_datadog_with_client() {
        let collector = MetricsCollector::new()
            .with_datadog("api-key".to_string(), "datadoghq.com".to_string());

        // This won't actually send (no network call in test)
        // but should not error
        let result = collector.send_to_datadog().await;
        assert!(result.is_ok());
    }

    // ==================== Duration Recording Tests ====================

    #[tokio::test]
    async fn test_record_request_duration() {
        let collector = MetricsCollector::new();

        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                None,
                true,
            )
            .await;
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(200),
                None,
                None,
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        let histogram = metrics.request_duration.get("openai:gpt-4").unwrap();
        assert_eq!(histogram.count(), 2);
    }

    // ==================== Edge Cases ====================

    #[tokio::test]
    async fn test_record_request_zero_duration() {
        let collector = MetricsCollector::new();
        collector
            .record_request("openai", "gpt-4", Duration::ZERO, None, None, true)
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.request_total.get("openai:gpt-4").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_record_request_very_long_duration() {
        let collector = MetricsCollector::new();
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_secs(3600),
                None,
                None,
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.request_total.get("openai:gpt-4").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_record_request_zero_cost() {
        let collector = MetricsCollector::new();
        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                None,
                Some(0.0),
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.cost_total.get("openai:gpt-4").unwrap(), 0.0);
    }

    #[tokio::test]
    async fn test_record_request_zero_tokens() {
        let collector = MetricsCollector::new();
        let tokens = TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };

        collector
            .record_request(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                Some(tokens),
                None,
                true,
            )
            .await;

        let metrics = collector.prometheus_metrics.read().await;
        assert_eq!(*metrics.token_usage.get("openai:gpt-4:prompt").unwrap(), 0);
    }
}
