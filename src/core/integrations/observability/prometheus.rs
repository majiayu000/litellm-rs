//! Prometheus Integration
//!
//! Exports LLM metrics to Prometheus for monitoring and alerting.

use crate::config::models::defaults::default_true;
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::traits::integration::{
    CacheHitEvent, EmbeddingEndEvent, EmbeddingStartEvent, Integration, IntegrationResult,
    LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
};

/// Prometheus integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusConfig {
    /// Whether the integration is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Metric prefix (default: "litellm")
    #[serde(default = "default_prefix")]
    pub prefix: String,

    /// Additional labels to add to all metrics
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Whether to track per-model metrics
    #[serde(default = "default_true")]
    pub per_model_metrics: bool,

    /// Whether to track per-provider metrics
    #[serde(default = "default_true")]
    pub per_provider_metrics: bool,

    /// Histogram buckets for latency (in milliseconds)
    #[serde(default = "default_latency_buckets")]
    pub latency_buckets: Vec<f64>,

    /// Histogram buckets for token counts
    #[serde(default = "default_token_buckets")]
    pub token_buckets: Vec<f64>,
}

fn default_enabled() -> bool {
    true
}

fn default_prefix() -> String {
    "litellm".to_string()
}

fn default_latency_buckets() -> Vec<f64> {
    vec![
        10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
    ]
}

fn default_token_buckets() -> Vec<f64> {
    vec![
        10.0, 50.0, 100.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
    ]
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            prefix: default_prefix(),
            labels: HashMap::new(),
            per_model_metrics: true,
            per_provider_metrics: true,
            latency_buckets: default_latency_buckets(),
            token_buckets: default_token_buckets(),
        }
    }
}

/// Counter metric
#[derive(Debug, Default)]
struct Counter {
    value: AtomicU64,
}

impl Counter {
    fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_by(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// Gauge metric
#[derive(Debug, Default)]
struct Gauge {
    value: AtomicU64,
}

impl Gauge {
    fn inc(&self) {
        let current = f64::from_bits(self.value.load(Ordering::Relaxed));
        self.value
            .store((current + 1.0).to_bits(), Ordering::Relaxed);
    }

    fn dec(&self) {
        let current = f64::from_bits(self.value.load(Ordering::Relaxed));
        self.value
            .store((current - 1.0).to_bits(), Ordering::Relaxed);
    }

    fn get(&self) -> f64 {
        f64::from_bits(self.value.load(Ordering::Relaxed))
    }
}

/// Histogram metric
#[derive(Debug)]
struct Histogram {
    buckets: Vec<f64>,
    counts: Vec<AtomicU64>,
    sum: AtomicU64,
    count: AtomicU64,
}

impl Histogram {
    fn new(buckets: Vec<f64>) -> Self {
        let counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            buckets,
            counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    fn observe(&self, value: f64) {
        // Update sum and count
        let sum_bits = self.sum.load(Ordering::Relaxed);
        let current_sum = f64::from_bits(sum_bits);
        self.sum
            .store((current_sum + value).to_bits(), Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        // Update bucket counts
        for (i, bucket) in self.buckets.iter().enumerate() {
            if value <= *bucket {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn get_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    fn get_sum(&self) -> f64 {
        f64::from_bits(self.sum.load(Ordering::Relaxed))
    }
}

/// Label set for metrics
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct Labels {
    model: Option<String>,
    provider: Option<String>,
}

impl Labels {
    fn new(model: Option<String>, provider: Option<String>) -> Self {
        Self { model, provider }
    }

    fn to_prometheus_string(&self, base_labels: &HashMap<String, String>) -> String {
        let mut parts = Vec::new();

        for (k, v) in base_labels {
            parts.push(format!("{}=\"{}\"", k, escape_prometheus_label_value(v)));
        }

        if let Some(ref model) = self.model {
            parts.push(format!(
                "model=\"{}\"",
                escape_prometheus_label_value(model)
            ));
        }

        if let Some(ref provider) = self.provider {
            parts.push(format!(
                "provider=\"{}\"",
                escape_prometheus_label_value(provider)
            ));
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!("{{{}}}", parts.join(","))
        }
    }
}

fn escape_prometheus_label_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Metrics storage
struct Metrics {
    // Request counters
    requests_total: RwLock<HashMap<Labels, Arc<Counter>>>,
    requests_success: RwLock<HashMap<Labels, Arc<Counter>>>,
    requests_error: RwLock<HashMap<Labels, Arc<Counter>>>,

    // Token counters
    input_tokens_total: RwLock<HashMap<Labels, Arc<Counter>>>,
    output_tokens_total: RwLock<HashMap<Labels, Arc<Counter>>>,

    // Cost tracking
    cost_total: RwLock<HashMap<Labels, AtomicU64>>,

    // Latency histograms
    request_latency: RwLock<HashMap<Labels, Arc<Histogram>>>,
    ttft_latency: RwLock<HashMap<Labels, Arc<Histogram>>>,

    // Active requests gauge
    active_requests: Gauge,

    // Cache metrics
    cache_hits: Counter,
    cache_misses: Counter,

    // Embedding metrics
    embedding_requests: Counter,
    embedding_tokens: Counter,

    // Configuration
    latency_buckets: Vec<f64>,
}

impl Metrics {
    fn new(config: &PrometheusConfig) -> Self {
        Self {
            requests_total: RwLock::new(HashMap::new()),
            requests_success: RwLock::new(HashMap::new()),
            requests_error: RwLock::new(HashMap::new()),
            input_tokens_total: RwLock::new(HashMap::new()),
            output_tokens_total: RwLock::new(HashMap::new()),
            cost_total: RwLock::new(HashMap::new()),
            request_latency: RwLock::new(HashMap::new()),
            ttft_latency: RwLock::new(HashMap::new()),
            active_requests: Gauge::default(),
            cache_hits: Counter::default(),
            cache_misses: Counter::default(),
            embedding_requests: Counter::default(),
            embedding_tokens: Counter::default(),
            latency_buckets: config.latency_buckets.clone(),
        }
    }

    fn get_or_create_counter(
        map: &RwLock<HashMap<Labels, Arc<Counter>>>,
        labels: &Labels,
    ) -> Arc<Counter> {
        if let Some(counter) = map.read().get(labels).cloned() {
            return counter;
        }

        let mut write = map.write();
        write
            .entry(labels.clone())
            .or_insert_with(|| Arc::new(Counter::default()))
            .clone()
    }

    fn get_or_create_histogram(
        map: &RwLock<HashMap<Labels, Arc<Histogram>>>,
        labels: &Labels,
        buckets: &[f64],
    ) -> Arc<Histogram> {
        if let Some(histogram) = map.read().get(labels).cloned() {
            return histogram;
        }

        let mut write = map.write();
        write
            .entry(labels.clone())
            .or_insert_with(|| Arc::new(Histogram::new(buckets.to_vec())))
            .clone()
    }
}

/// Prometheus integration for LLM metrics
pub struct PrometheusIntegration {
    config: PrometheusConfig,
    metrics: Arc<Metrics>,
}

impl PrometheusIntegration {
    /// Create a new Prometheus integration
    pub fn new(config: PrometheusConfig) -> Self {
        let metrics = Arc::new(Metrics::new(&config));
        Self { config, metrics }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(PrometheusConfig::default())
    }

    /// Get metrics in Prometheus text format
    pub fn render_metrics(&self) -> String {
        let mut output = String::new();
        let prefix = &self.config.prefix;

        // Helper to render counter
        let render_counter =
            |name: &str, help: &str, map: &RwLock<HashMap<Labels, Arc<Counter>>>| {
                let mut lines = Vec::new();
                lines.push(format!("# HELP {}_{} {}", prefix, name, help));
                lines.push(format!("# TYPE {}_{} counter", prefix, name));

                let read = map.read();
                for (labels, counter) in read.iter() {
                    let label_str = labels.to_prometheus_string(&self.config.labels);
                    lines.push(format!(
                        "{}_{}{} {}",
                        prefix,
                        name,
                        label_str,
                        counter.get()
                    ));
                }
                lines.join("\n")
            };

        // Render request counters
        output.push_str(&render_counter(
            "requests_total",
            "Total number of LLM requests",
            &self.metrics.requests_total,
        ));
        output.push('\n');

        output.push_str(&render_counter(
            "requests_success_total",
            "Total number of successful LLM requests",
            &self.metrics.requests_success,
        ));
        output.push('\n');

        output.push_str(&render_counter(
            "requests_error_total",
            "Total number of failed LLM requests",
            &self.metrics.requests_error,
        ));
        output.push('\n');

        // Render token counters
        output.push_str(&render_counter(
            "input_tokens_total",
            "Total number of input tokens",
            &self.metrics.input_tokens_total,
        ));
        output.push('\n');

        output.push_str(&render_counter(
            "output_tokens_total",
            "Total number of output tokens",
            &self.metrics.output_tokens_total,
        ));
        output.push('\n');

        // Render active requests gauge
        output.push_str(&format!(
            "# HELP {}_active_requests Current number of active requests\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_active_requests gauge\n", prefix));
        output.push_str(&format!(
            "{}_active_requests {}\n",
            prefix,
            self.metrics.active_requests.get()
        ));

        // Render cache metrics
        output.push_str(&format!(
            "# HELP {}_cache_hits_total Total number of cache hits\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_cache_hits_total counter\n", prefix));
        output.push_str(&format!(
            "{}_cache_hits_total {}\n",
            prefix,
            self.metrics.cache_hits.get()
        ));

        output.push_str(&format!(
            "# HELP {}_cache_misses_total Total number of cache misses\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_cache_misses_total counter\n", prefix));
        output.push_str(&format!(
            "{}_cache_misses_total {}\n",
            prefix,
            self.metrics.cache_misses.get()
        ));

        // Render latency histograms
        output.push_str(&format!(
            "# HELP {}_request_latency_seconds Request latency in seconds\n",
            prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_request_latency_seconds histogram\n",
            prefix
        ));

        let latency_read = self.metrics.request_latency.read();
        for (labels, histogram) in latency_read.iter() {
            let label_str = labels.to_prometheus_string(&self.config.labels);
            for (i, bucket) in histogram.buckets.iter().enumerate() {
                let bucket_label = if label_str.is_empty() {
                    format!("{{le=\"{}\"}}", bucket / 1000.0)
                } else {
                    let inner = &label_str[1..label_str.len() - 1];
                    format!("{{{},le=\"{}\"}}", inner, bucket / 1000.0)
                };
                output.push_str(&format!(
                    "{}_request_latency_seconds_bucket{} {}\n",
                    prefix,
                    bucket_label,
                    histogram.counts[i].load(Ordering::Relaxed)
                ));
            }
            output.push_str(&format!(
                "{}_request_latency_seconds_sum{} {}\n",
                prefix,
                label_str,
                histogram.get_sum() / 1000.0
            ));
            output.push_str(&format!(
                "{}_request_latency_seconds_count{} {}\n",
                prefix,
                label_str,
                histogram.get_count()
            ));
        }

        output
    }

    fn get_labels(&self, model: &str, provider: Option<&str>) -> Labels {
        let model = if self.config.per_model_metrics {
            Some(model.to_string())
        } else {
            None
        };

        let provider = if self.config.per_provider_metrics {
            provider.map(|p| p.to_string())
        } else {
            None
        };

        Labels::new(model, provider)
    }
}

#[async_trait]
impl Integration for PrometheusIntegration {
    fn name(&self) -> &'static str {
        "prometheus"
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()> {
        let labels = self.get_labels(&event.model, event.provider.as_deref());

        // Increment request counter
        let counter = Metrics::get_or_create_counter(&self.metrics.requests_total, &labels);
        counter.inc();

        // Increment active requests
        self.metrics.active_requests.inc();

        Ok(())
    }

    async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
        let labels = self.get_labels(&event.model, event.provider.as_deref());

        // Increment success counter
        let counter = Metrics::get_or_create_counter(&self.metrics.requests_success, &labels);
        counter.inc();

        // Decrement active requests
        self.metrics.active_requests.dec();

        // Record tokens
        if let Some(input_tokens) = event.input_tokens {
            let counter = Metrics::get_or_create_counter(&self.metrics.input_tokens_total, &labels);
            counter.inc_by(input_tokens as u64);
        }

        if let Some(output_tokens) = event.output_tokens {
            let counter =
                Metrics::get_or_create_counter(&self.metrics.output_tokens_total, &labels);
            counter.inc_by(output_tokens as u64);
        }

        // Record latency
        let histogram = Metrics::get_or_create_histogram(
            &self.metrics.request_latency,
            &labels,
            &self.metrics.latency_buckets,
        );
        histogram.observe(event.latency_ms as f64);

        // Record TTFT if available
        if let Some(ttft) = event.ttft_ms {
            let histogram = Metrics::get_or_create_histogram(
                &self.metrics.ttft_latency,
                &labels,
                &self.metrics.latency_buckets,
            );
            histogram.observe(ttft as f64);
        }

        // Record cost
        if let Some(cost) = event.cost_usd {
            let cost_map = self.metrics.cost_total.read();
            if let Some(cost_counter) = cost_map.get(&labels) {
                let current = f64::from_bits(cost_counter.load(Ordering::Relaxed));
                cost_counter.store((current + cost).to_bits(), Ordering::Relaxed);
            }
            drop(cost_map);

            // Create if not exists
            let mut cost_map = self.metrics.cost_total.write();
            cost_map.entry(labels).or_insert_with(|| {
                let counter = AtomicU64::new(0);
                counter.store(cost.to_bits(), Ordering::Relaxed);
                counter
            });
        }

        Ok(())
    }

    async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
        let labels = self.get_labels(&event.model, event.provider.as_deref());

        // Increment error counter
        let counter = Metrics::get_or_create_counter(&self.metrics.requests_error, &labels);
        counter.inc();

        // Decrement active requests
        self.metrics.active_requests.dec();

        Ok(())
    }

    async fn on_llm_stream(&self, _event: &LlmStreamEvent) -> IntegrationResult<()> {
        // Streaming events don't need special handling for Prometheus
        Ok(())
    }

    async fn on_embedding_start(&self, _event: &EmbeddingStartEvent) -> IntegrationResult<()> {
        self.metrics.embedding_requests.inc();
        Ok(())
    }

    async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
        if let Some(tokens) = event.total_tokens {
            self.metrics.embedding_tokens.inc_by(tokens as u64);
        }
        Ok(())
    }

    async fn on_cache_hit(&self, _event: &CacheHitEvent) -> IntegrationResult<()> {
        self.metrics.cache_hits.inc();
        Ok(())
    }

    async fn flush(&self) -> IntegrationResult<()> {
        // Prometheus metrics are always available, no flushing needed
        Ok(())
    }

    async fn shutdown(&self) -> IntegrationResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prometheus_integration_creation() {
        let integration = PrometheusIntegration::with_defaults();
        assert_eq!(integration.name(), "prometheus");
        assert!(integration.is_enabled());
    }

    #[tokio::test]
    async fn test_on_llm_start() {
        let integration = PrometheusIntegration::with_defaults();

        let event = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
        integration.on_llm_start(&event).await.unwrap();

        assert_eq!(integration.metrics.active_requests.get(), 1.0);
    }

    #[tokio::test]
    async fn test_on_llm_end() {
        let integration = PrometheusIntegration::with_defaults();

        let start_event = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
        integration.on_llm_start(&start_event).await.unwrap();

        let end_event = LlmEndEvent::new("req-1", "gpt-4")
            .provider("openai")
            .tokens(100, 50)
            .latency(150);
        integration.on_llm_end(&end_event).await.unwrap();

        assert_eq!(integration.metrics.active_requests.get(), 0.0);
    }

    #[tokio::test]
    async fn test_on_llm_error() {
        let integration = PrometheusIntegration::with_defaults();

        let start_event = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
        integration.on_llm_start(&start_event).await.unwrap();

        let error_event = LlmErrorEvent::new("req-1", "gpt-4", "Rate limited").provider("openai");
        integration.on_llm_error(&error_event).await.unwrap();

        assert_eq!(integration.metrics.active_requests.get(), 0.0);
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let integration = PrometheusIntegration::with_defaults();

        let event = CacheHitEvent {
            request_id: "req-1".to_string(),
            cache_key: "key-1".to_string(),
            cache_backend: "redis".to_string(),
            time_saved_ms: Some(100),
            cost_saved_usd: Some(0.01),
            timestamp_ms: 0,
        };
        integration.on_cache_hit(&event).await.unwrap();

        assert_eq!(integration.metrics.cache_hits.get(), 1);
    }

    #[tokio::test]
    async fn test_render_metrics() {
        let integration = PrometheusIntegration::with_defaults();

        let event = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
        integration.on_llm_start(&event).await.unwrap();

        let metrics = integration.render_metrics();
        assert!(metrics.contains("litellm_requests_total"));
        assert!(metrics.contains("litellm_active_requests"));
    }

    #[tokio::test]
    async fn test_disabled_integration() {
        let config = PrometheusConfig {
            enabled: false,
            ..Default::default()
        };
        let integration = PrometheusIntegration::new(config);

        assert!(!integration.is_enabled());
    }

    #[tokio::test]
    async fn test_custom_prefix() {
        let config = PrometheusConfig {
            prefix: "myapp".to_string(),
            ..Default::default()
        };
        let integration = PrometheusIntegration::new(config);

        let event = LlmStartEvent::new("req-1", "gpt-4");
        integration.on_llm_start(&event).await.unwrap();

        let metrics = integration.render_metrics();
        assert!(metrics.contains("myapp_requests_total"));
    }

    #[tokio::test]
    async fn label_values_are_escaped_in_rendered_metrics() {
        let mut config = PrometheusConfig::default();
        config.labels.insert(
            "tenant".to_string(),
            "team\\\"\nforged_config_metric 1".to_string(),
        );
        let integration = PrometheusIntegration::new(config);
        let event = LlmStartEvent::new("req-1", "model\\\"\nforged_model_metric 1")
            .provider("provider\\\"\nforged_provider_metric 1");

        integration.on_llm_start(&event).await.unwrap();
        let metrics = integration.render_metrics();

        assert!(metrics.contains(r#"tenant="team\\\"\nforged_config_metric 1""#));
        assert!(metrics.contains(r#"model="model\\\"\nforged_model_metric 1""#));
        assert!(metrics.contains(r#"provider="provider\\\"\nforged_provider_metric 1""#));
        assert!(!metrics.contains("\nforged_config_metric 1"));
        assert!(!metrics.contains("\nforged_model_metric 1"));
        assert!(!metrics.contains("\nforged_provider_metric 1"));
    }
}
