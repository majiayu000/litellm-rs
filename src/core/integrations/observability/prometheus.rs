//! Prometheus Integration
//!
//! Exports LLM metrics to Prometheus for monitoring and alerting.

#[path = "prometheus_render.rs"]
mod render;

use crate::config::models::defaults::default_true;
use async_trait::async_trait;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::warn;

use crate::core::integrations::callback_runtime::CallbackMetrics;
use crate::core::traits::integration::{
    CacheHitEvent, EmbeddingEndEvent, EmbeddingStartEvent, Integration, IntegrationError,
    IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
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

impl PrometheusConfig {
    /// Validate values that are interpolated into Prometheus exposition text.
    pub fn validate(&self) -> IntegrationResult<()> {
        if !is_prometheus_identifier(&self.prefix) {
            return Err(IntegrationError::config(
                "Prometheus metric prefix must match [A-Za-z_][A-Za-z0-9_]*",
            ));
        }
        for key in self.labels.keys() {
            if !is_prometheus_identifier(key) || key.starts_with("__") {
                return Err(IntegrationError::config(format!(
                    "Prometheus label key '{key}' is invalid"
                )));
            }
            if matches!(key.as_str(), "model" | "provider" | "le") {
                return Err(IntegrationError::config(format!(
                    "Prometheus label key '{key}' is reserved"
                )));
            }
        }
        validate_buckets("latency_buckets", &self.latency_buckets)?;
        validate_buckets("token_buckets", &self.token_buckets)?;
        Ok(())
    }
}

fn is_prometheus_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn validate_buckets(name: &str, buckets: &[f64]) -> IntegrationResult<()> {
    if buckets.is_empty() {
        return Err(IntegrationError::config(format!(
            "Prometheus {name} must not be empty"
        )));
    }
    if buckets
        .iter()
        .any(|bucket| !bucket.is_finite() || *bucket <= 0.0)
    {
        return Err(IntegrationError::config(format!(
            "Prometheus {name} must contain only finite positive values"
        )));
    }
    if buckets.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(IntegrationError::config(format!(
            "Prometheus {name} must be strictly increasing without duplicates"
        )));
    }
    Ok(())
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
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    fn dec(&self) {
        let mut current = self.value.load(Ordering::Relaxed);
        while current != 0 {
            match self.value.compare_exchange_weak(
                current,
                current - 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// Histogram metric
#[derive(Debug)]
struct Histogram {
    buckets: Vec<f64>,
    state: Mutex<HistogramState>,
}

#[derive(Debug, Clone)]
struct HistogramState {
    counts: Vec<u64>,
    sum: f64,
    count: u64,
}

impl Histogram {
    fn new(buckets: Vec<f64>) -> Self {
        let counts = vec![0; buckets.len()];
        Self {
            buckets,
            state: Mutex::new(HistogramState {
                counts,
                sum: 0.0,
                count: 0,
            }),
        }
    }

    fn observe(&self, value: f64) {
        let mut state = self.state.lock();
        state.sum += value;
        state.count += 1;
        for (i, bucket) in self.buckets.iter().enumerate() {
            if value <= *bucket {
                state.counts[i] += 1;
            }
        }
    }

    fn snapshot(&self) -> HistogramState {
        self.state.lock().clone()
    }
}

fn atomic_add_f64(value: &AtomicU64, delta: f64) -> bool {
    let mut current = value.load(Ordering::Relaxed);
    loop {
        let next = f64::from_bits(current) + delta;
        if !next.is_finite() {
            return false;
        }
        match value.compare_exchange_weak(
            current,
            next.to_bits(),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
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
    // Active requests gauge
    active_requests: Gauge,

    // Cache metrics
    cache_hits: Counter,

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
            active_requests: Gauge::default(),
            cache_hits: Counter::default(),
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
    /// Create a new Prometheus integration.
    ///
    /// # Panics
    ///
    /// Panics when `config` could produce invalid Prometheus exposition text.
    /// Use [`Self::try_new`] to handle invalid programmatic configuration.
    #[track_caller]
    pub fn new(config: PrometheusConfig) -> Self {
        Self::try_new(config)
            .unwrap_or_else(|error| panic!("invalid Prometheus configuration: {error}"))
    }

    /// Create a validated Prometheus integration.
    pub fn try_new(config: PrometheusConfig) -> IntegrationResult<Self> {
        config.validate()?;
        let metrics = Arc::new(Metrics::new(&config));
        Ok(Self { config, metrics })
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(PrometheusConfig::default())
    }

    /// Get metrics in Prometheus text format.
    pub fn render_metrics(&self) -> String {
        render::render_metrics(self)
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

    fn record_llm_start(&self, event: &LlmStartEvent) {
        let labels = self.get_labels(&event.model, event.provider.as_deref());
        Metrics::get_or_create_counter(&self.metrics.requests_total, &labels).inc();
        self.metrics.active_requests.inc();
    }

    fn record_llm_end(&self, event: &LlmEndEvent) {
        let labels = self.get_labels(&event.model, event.provider.as_deref());
        Metrics::get_or_create_counter(&self.metrics.requests_success, &labels).inc();
        self.metrics.active_requests.dec();

        if let Some(input_tokens) = event.input_tokens {
            Metrics::get_or_create_counter(&self.metrics.input_tokens_total, &labels)
                .inc_by(input_tokens as u64);
        }
        if let Some(output_tokens) = event.output_tokens {
            Metrics::get_or_create_counter(&self.metrics.output_tokens_total, &labels)
                .inc_by(output_tokens as u64);
        }

        Metrics::get_or_create_histogram(
            &self.metrics.request_latency,
            &labels,
            &self.metrics.latency_buckets,
        )
        .observe(event.latency_ms as f64);

        if let Some(cost) = event.cost_usd {
            if !cost.is_finite() || cost < 0.0 {
                warn!(
                    cost,
                    "Ignoring invalid negative or non-finite callback cost"
                );
                return;
            }
            let overflowed = {
                let mut costs = self.metrics.cost_total.write();
                let counter = costs
                    .entry(labels)
                    .or_insert_with(|| AtomicU64::new(0.0_f64.to_bits()));
                !atomic_add_f64(counter, cost)
            };
            if overflowed {
                warn!(
                    cost,
                    "Ignoring callback cost because the accumulated Prometheus cost total would become non-finite"
                );
            }
        }
    }

    fn record_llm_error(&self, event: &LlmErrorEvent) {
        let labels = self.get_labels(&event.model, event.provider.as_deref());
        Metrics::get_or_create_counter(&self.metrics.requests_error, &labels).inc();
        self.metrics.active_requests.dec();
    }

    fn record_llm_cancelled(&self) {
        self.metrics.active_requests.dec();
    }

    fn record_embedding_start(&self, _event: &EmbeddingStartEvent) {
        self.metrics.embedding_requests.inc();
    }

    fn record_embedding_end(&self, event: &EmbeddingEndEvent) {
        if let Some(tokens) = event.total_tokens {
            self.metrics.embedding_tokens.inc_by(tokens as u64);
        }
    }
}

impl CallbackMetrics for PrometheusIntegration {
    fn begin_llm_lifecycle(&self, _event: &LlmStartEvent) {
        self.metrics.active_requests.inc();
    }

    fn finish_llm_lifecycle(&self, event: &LlmEndEvent) {
        let labels = self.get_labels(&event.model, event.provider.as_deref());
        Metrics::get_or_create_counter(&self.metrics.requests_total, &labels).inc();
        PrometheusIntegration::record_llm_end(self, event);
    }

    fn fail_llm_lifecycle(&self, event: &LlmErrorEvent) {
        let labels = self.get_labels(&event.model, event.provider.as_deref());
        Metrics::get_or_create_counter(&self.metrics.requests_total, &labels).inc();
        PrometheusIntegration::record_llm_error(self, event);
    }

    fn cancel_llm_lifecycle(&self, event: &LlmStartEvent) {
        let labels = self.get_labels(&event.model, event.provider.as_deref());
        Metrics::get_or_create_counter(&self.metrics.requests_total, &labels).inc();
        PrometheusIntegration::record_llm_cancelled(self);
    }

    fn record_embedding_start(&self, event: &EmbeddingStartEvent) {
        PrometheusIntegration::record_embedding_start(self, event);
    }

    fn record_embedding_end(&self, event: &EmbeddingEndEvent) {
        PrometheusIntegration::record_embedding_end(self, event);
    }

    fn render(&self) -> String {
        self.render_metrics()
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
        self.record_llm_start(event);
        Ok(())
    }

    async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
        self.record_llm_end(event);
        Ok(())
    }

    async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
        self.record_llm_error(event);
        Ok(())
    }

    async fn on_llm_stream(&self, _event: &LlmStreamEvent) -> IntegrationResult<()> {
        // Streaming events don't need special handling for Prometheus
        Ok(())
    }

    async fn on_embedding_start(&self, event: &EmbeddingStartEvent) -> IntegrationResult<()> {
        self.record_embedding_start(event);
        Ok(())
    }

    async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
        self.record_embedding_end(event);
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
#[path = "prometheus_tests.rs"]
mod tests;
