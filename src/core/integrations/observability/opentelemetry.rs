//! OpenTelemetry Integration
//!
//! Provides distributed tracing for LLM requests using OpenTelemetry.

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

use crate::core::traits::integration::{
    CacheHitEvent, EmbeddingEndEvent, EmbeddingStartEvent, Integration, IntegrationError,
    IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
};
use crate::utils::net::http::create_custom_client;

/// OpenTelemetry integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryConfig {
    /// Whether the integration is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// OTLP endpoint URL
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    /// Service name
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Service version
    pub service_version: Option<String>,

    /// Environment (e.g., "production", "staging")
    pub environment: Option<String>,

    /// Additional resource attributes
    #[serde(default)]
    pub resource_attributes: HashMap<String, String>,

    /// Whether to export traces (default: true)
    #[serde(default = "default_true")]
    pub export_traces: bool,

    /// Whether to export metrics (default: true)
    #[serde(default = "default_true")]
    pub export_metrics: bool,

    /// Batch export interval in milliseconds
    #[serde(default = "default_batch_interval")]
    pub batch_interval_ms: u64,

    /// Maximum batch size
    #[serde(default = "default_batch_size")]
    pub max_batch_size: usize,

    /// Export timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Sampling ratio (0.0 to 1.0)
    #[serde(default = "default_sampling_ratio")]
    pub sampling_ratio: f64,

    /// Headers to include in OTLP requests
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

fn default_enabled() -> bool {
    true
}

fn default_endpoint() -> String {
    "http://localhost:4317".to_string()
}

fn default_service_name() -> String {
    "litellm-gateway".to_string()
}

fn default_true() -> bool {
    true
}

fn default_batch_interval() -> u64 {
    5000
}

fn default_batch_size() -> usize {
    512
}

fn default_timeout() -> u64 {
    10000
}

fn default_sampling_ratio() -> f64 {
    1.0
}

impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            endpoint: default_endpoint(),
            service_name: default_service_name(),
            service_version: None,
            environment: None,
            resource_attributes: HashMap::new(),
            export_traces: true,
            export_metrics: true,
            batch_interval_ms: default_batch_interval(),
            max_batch_size: default_batch_size(),
            timeout_ms: default_timeout(),
            sampling_ratio: default_sampling_ratio(),
            headers: HashMap::new(),
        }
    }
}

/// Span status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStatus {
    Unset,
    Ok,
    Error,
}

/// Span kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Internal,
    Server,
    Client,
    Producer,
    Consumer,
}

/// A trace span
#[derive(Debug, Clone)]
pub struct Span {
    /// Trace ID (16 bytes as hex string)
    pub trace_id: String,
    /// Span ID (8 bytes as hex string)
    pub span_id: String,
    /// Parent span ID
    pub parent_span_id: Option<String>,
    /// Span name
    pub name: String,
    /// Span kind
    pub kind: SpanKind,
    /// Start time (Unix nanoseconds)
    pub start_time_ns: u64,
    /// End time (Unix nanoseconds)
    pub end_time_ns: Option<u64>,
    /// Span status
    pub status: SpanStatus,
    /// Status message (for errors)
    pub status_message: Option<String>,
    /// Span attributes
    pub attributes: HashMap<String, AttributeValue>,
    /// Events within the span
    pub events: Vec<SpanEvent>,
}

/// Span event
#[derive(Debug, Clone)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp_ns: u64,
    pub attributes: HashMap<String, AttributeValue>,
}

/// Attribute value types
#[derive(Debug, Clone)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    StringArray(Vec<String>),
    IntArray(Vec<i64>),
    FloatArray(Vec<f64>),
    BoolArray(Vec<bool>),
}

impl From<String> for AttributeValue {
    fn from(s: String) -> Self {
        AttributeValue::String(s)
    }
}

impl From<&str> for AttributeValue {
    fn from(s: &str) -> Self {
        AttributeValue::String(s.to_string())
    }
}

impl From<i64> for AttributeValue {
    fn from(i: i64) -> Self {
        AttributeValue::Int(i)
    }
}

impl From<f64> for AttributeValue {
    fn from(f: f64) -> Self {
        AttributeValue::Float(f)
    }
}

impl From<bool> for AttributeValue {
    fn from(b: bool) -> Self {
        AttributeValue::Bool(b)
    }
}

impl Span {
    /// Create a new span
    pub fn new(name: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            trace_id: generate_trace_id(),
            span_id: generate_span_id(),
            parent_span_id: None,
            name: name.into(),
            kind: SpanKind::Internal,
            start_time_ns: now,
            end_time_ns: None,
            status: SpanStatus::Unset,
            status_message: None,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Set the span kind
    pub fn kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set parent span
    pub fn parent(mut self, parent_span_id: impl Into<String>) -> Self {
        self.parent_span_id = Some(parent_span_id.into());
        self
    }

    /// Add an attribute
    pub fn attribute(mut self, key: impl Into<String>, value: impl Into<AttributeValue>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Add an event
    pub fn event(mut self, name: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        self.events.push(SpanEvent {
            name: name.into(),
            timestamp_ns: now,
            attributes: HashMap::new(),
        });
        self
    }

    /// End the span successfully
    pub fn end_ok(mut self) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        self.end_time_ns = Some(now);
        self.status = SpanStatus::Ok;
        self
    }

    /// End the span with an error
    pub fn end_error(mut self, message: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        self.end_time_ns = Some(now);
        self.status = SpanStatus::Error;
        self.status_message = Some(message.into());
        self
    }

    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> Option<u64> {
        self.end_time_ns
            .map(|end| (end - self.start_time_ns) / 1_000_000)
    }
}

/// Generate a random trace ID (16 bytes as hex)
fn generate_trace_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let random: u64 = (now as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    format!("{:016x}{:016x}", now as u64, random)
}

/// Generate a random span ID (8 bytes as hex)
fn generate_span_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let random: u64 = (now as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    format!("{:016x}", random)
}

/// Active span tracking
struct ActiveSpan {
    span: Span,
}

/// Span batch for export
struct SpanBatch {
    spans: Vec<Span>,
    created_at: SystemTime,
}

impl SpanBatch {
    fn new() -> Self {
        Self {
            spans: Vec::new(),
            created_at: SystemTime::now(),
        }
    }

    fn add(&mut self, span: Span) {
        self.spans.push(span);
    }

    fn len(&self) -> usize {
        self.spans.len()
    }

    fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.created_at)
            .unwrap_or_default()
    }

    fn take(&mut self) -> Vec<Span> {
        let spans = std::mem::take(&mut self.spans);
        self.created_at = SystemTime::now();
        spans
    }
}

/// OpenTelemetry integration for distributed tracing
pub struct OpenTelemetryIntegration {
    config: OpenTelemetryConfig,
    active_spans: RwLock<HashMap<String, ActiveSpan>>,
    pending_spans: RwLock<SpanBatch>,
    http_client: reqwest::Client,
}

impl OpenTelemetryIntegration {
    /// Create a new OpenTelemetry integration
    pub fn new(config: OpenTelemetryConfig) -> Self {
        let http_client =
            create_custom_client(Duration::from_millis(config.timeout_ms)).unwrap_or_default();

        Self {
            config,
            active_spans: RwLock::new(HashMap::new()),
            pending_spans: RwLock::new(SpanBatch::new()),
            http_client,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(OpenTelemetryConfig::default())
    }

    /// Check if request should be sampled
    fn should_sample(&self) -> bool {
        if self.config.sampling_ratio >= 1.0 {
            return true;
        }
        if self.config.sampling_ratio <= 0.0 {
            return false;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let random = (now as f64) % 1.0;
        random < self.config.sampling_ratio
    }

    /// Add a completed span to the batch
    fn add_span(&self, span: Span) {
        let mut batch = self.pending_spans.write();
        batch.add(span);

        // Check if we should flush
        let should_flush = batch.len() >= self.config.max_batch_size
            || batch.age() >= Duration::from_millis(self.config.batch_interval_ms);

        if should_flush {
            let spans = batch.take();
            drop(batch);

            // Spawn async export task
            let client = self.http_client.clone();
            let endpoint = self.config.endpoint.clone();
            let headers = self.config.headers.clone();
            let service_name = self.config.service_name.clone();

            tokio::spawn(async move {
                if let Err(e) =
                    export_spans(&client, &endpoint, &headers, &service_name, spans).await
                {
                    warn!("Failed to export spans to OTLP: {}", e);
                }
            });
        }
    }

    /// Get the number of active spans
    pub fn active_span_count(&self) -> usize {
        self.active_spans.read().len()
    }

    /// Get the number of pending spans
    pub fn pending_span_count(&self) -> usize {
        self.pending_spans.read().len()
    }
}

/// Export spans to OTLP endpoint
async fn export_spans(
    client: &reqwest::Client,
    endpoint: &str,
    headers: &HashMap<String, String>,
    service_name: &str,
    spans: Vec<Span>,
) -> Result<(), String> {
    if spans.is_empty() {
        return Ok(());
    }

    // Build OTLP JSON payload
    let payload = build_otlp_payload(service_name, &spans);

    let mut request = client
        .post(format!("{}/v1/traces", endpoint))
        .header("Content-Type", "application/json");

    for (key, value) in headers {
        request = request.header(key, value);
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "OTLP export failed with status: {}",
            response.status()
        ));
    }

    debug!("Exported {} spans to OTLP", spans.len());
    Ok(())
}

/// Build OTLP JSON payload
fn build_otlp_payload(service_name: &str, spans: &[Span]) -> serde_json::Value {
    let resource_spans = serde_json::json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [{
                    "key": "service.name",
                    "value": { "stringValue": service_name }
                }]
            },
            "scopeSpans": [{
                "scope": {
                    "name": "litellm-rs",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "spans": spans.iter().map(|span| {
                    let mut span_json = serde_json::json!({
                        "traceId": span.trace_id,
                        "spanId": span.span_id,
                        "name": span.name,
                        "kind": match span.kind {
                            SpanKind::Internal => 1,
                            SpanKind::Server => 2,
                            SpanKind::Client => 3,
                            SpanKind::Producer => 4,
                            SpanKind::Consumer => 5,
                        },
                        "startTimeUnixNano": span.start_time_ns.to_string(),
                        "endTimeUnixNano": span.end_time_ns.unwrap_or(span.start_time_ns).to_string(),
                        "status": {
                            "code": match span.status {
                                SpanStatus::Unset => 0,
                                SpanStatus::Ok => 1,
                                SpanStatus::Error => 2,
                            }
                        },
                        "attributes": span.attributes.iter().map(|(k, v)| {
                            serde_json::json!({
                                "key": k,
                                "value": match v {
                                    AttributeValue::String(s) => serde_json::json!({ "stringValue": s }),
                                    AttributeValue::Int(i) => serde_json::json!({ "intValue": i.to_string() }),
                                    AttributeValue::Float(f) => serde_json::json!({ "doubleValue": f }),
                                    AttributeValue::Bool(b) => serde_json::json!({ "boolValue": b }),
                                    _ => serde_json::json!({ "stringValue": "unsupported" }),
                                }
                            })
                        }).collect::<Vec<_>>()
                    });

                    if let Some(ref parent) = span.parent_span_id {
                        span_json["parentSpanId"] = serde_json::json!(parent);
                    }

                    if let Some(ref msg) = span.status_message {
                        span_json["status"]["message"] = serde_json::json!(msg);
                    }

                    span_json
                }).collect::<Vec<_>>()
            }]
        }]
    });

    resource_spans
}

#[async_trait]
impl Integration for OpenTelemetryIntegration {
    fn name(&self) -> &'static str {
        "opentelemetry"
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled && self.config.export_traces
    }

    async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()> {
        if !self.should_sample() {
            return Ok(());
        }

        let mut span = Span::new("llm.completion")
            .kind(SpanKind::Client)
            .attribute("llm.model", event.model.clone())
            .attribute("llm.request_id", event.request_id.clone());

        if let Some(ref provider) = event.provider {
            span = span.attribute("llm.provider", provider.clone());
        }

        if let Some(ref user_id) = event.user_id {
            span = span.attribute("user.id", user_id.clone());
        }

        if let Some(ref session_id) = event.session_id {
            span = span.attribute("session.id", session_id.clone());
        }

        // Store active span
        let active = ActiveSpan { span };

        self.active_spans
            .write()
            .insert(event.request_id.clone(), active);

        Ok(())
    }

    async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
        let active = self.active_spans.write().remove(&event.request_id);

        let Some(active) = active else {
            return Ok(());
        };

        let mut span = active
            .span
            .attribute("llm.latency_ms", event.latency_ms as i64)
            .end_ok();

        if let Some(input_tokens) = event.input_tokens {
            span = span.attribute("llm.input_tokens", input_tokens as i64);
        }

        if let Some(output_tokens) = event.output_tokens {
            span = span.attribute("llm.output_tokens", output_tokens as i64);
        }

        if let Some(cost) = event.cost_usd {
            span = span.attribute("llm.cost_usd", cost);
        }

        if let Some(ttft) = event.ttft_ms {
            span = span.attribute("llm.ttft_ms", ttft as i64);
        }

        self.add_span(span);

        Ok(())
    }

    async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
        let active = self.active_spans.write().remove(&event.request_id);

        let Some(active) = active else {
            return Ok(());
        };

        let mut span = active
            .span
            .attribute("error.message", event.error_message.clone())
            .end_error(&event.error_message);

        if let Some(ref error_type) = event.error_type {
            span = span.attribute("error.type", error_type.clone());
        }

        if let Some(status_code) = event.status_code {
            span = span.attribute("http.status_code", status_code as i64);
        }

        span = span.attribute("error.retryable", event.retryable);

        self.add_span(span);

        Ok(())
    }

    async fn on_llm_stream(&self, event: &LlmStreamEvent) -> IntegrationResult<()> {
        // Add stream events to the active span
        let mut active_spans = self.active_spans.write();
        if let Some(active) = active_spans.get_mut(&event.request_id) {
            if event.is_final {
                active.span = active.span.clone().event("stream.complete");
            }
        }
        Ok(())
    }

    async fn on_embedding_start(&self, event: &EmbeddingStartEvent) -> IntegrationResult<()> {
        if !self.should_sample() {
            return Ok(());
        }

        let mut span = Span::new("llm.embedding")
            .kind(SpanKind::Client)
            .attribute("llm.model", event.model.clone())
            .attribute("llm.request_id", event.request_id.clone())
            .attribute("llm.input_count", event.input_count as i64);

        if let Some(ref provider) = event.provider {
            span = span.attribute("llm.provider", provider.clone());
        }

        let active = ActiveSpan { span };

        self.active_spans
            .write()
            .insert(event.request_id.clone(), active);

        Ok(())
    }

    async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
        let active = self.active_spans.write().remove(&event.request_id);

        let Some(active) = active else {
            return Ok(());
        };

        let mut span = active
            .span
            .attribute("llm.latency_ms", event.latency_ms as i64)
            .end_ok();

        if let Some(tokens) = event.total_tokens {
            span = span.attribute("llm.total_tokens", tokens as i64);
        }

        if let Some(cost) = event.cost_usd {
            span = span.attribute("llm.cost_usd", cost);
        }

        self.add_span(span);

        Ok(())
    }

    async fn on_cache_hit(&self, event: &CacheHitEvent) -> IntegrationResult<()> {
        // Create a short span for cache hits
        let mut span = Span::new("cache.hit")
            .kind(SpanKind::Internal)
            .attribute("cache.key", event.cache_key.clone())
            .attribute("cache.backend", event.cache_backend.clone())
            .end_ok();

        if let Some(time_saved) = event.time_saved_ms {
            span = span.attribute("cache.time_saved_ms", time_saved as i64);
        }

        if let Some(cost_saved) = event.cost_saved_usd {
            span = span.attribute("cache.cost_saved_usd", cost_saved);
        }

        self.add_span(span);

        Ok(())
    }

    async fn flush(&self) -> IntegrationResult<()> {
        let spans = self.pending_spans.write().take();

        if spans.is_empty() {
            return Ok(());
        }

        export_spans(
            &self.http_client,
            &self.config.endpoint,
            &self.config.headers,
            &self.config.service_name,
            spans,
        )
        .await
        .map_err(IntegrationError::other)?;

        Ok(())
    }

    async fn shutdown(&self) -> IntegrationResult<()> {
        // Flush any remaining spans
        self.flush().await?;

        // Clear active spans (they won't be completed)
        let orphaned = self.active_spans.write().len();
        if orphaned > 0 {
            warn!("OpenTelemetry shutdown with {} orphaned spans", orphaned);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_opentelemetry_integration_creation() {
        let integration = OpenTelemetryIntegration::with_defaults();
        assert_eq!(integration.name(), "opentelemetry");
        assert!(integration.is_enabled());
    }

    #[tokio::test]
    async fn test_span_creation() {
        let span = Span::new("test-span")
            .kind(SpanKind::Client)
            .attribute("key", "value")
            .attribute("count", 42i64);

        assert_eq!(span.name, "test-span");
        assert_eq!(span.kind, SpanKind::Client);
        assert!(span.attributes.contains_key("key"));
        assert!(span.attributes.contains_key("count"));
    }

    #[tokio::test]
    async fn test_span_end_ok() {
        let span = Span::new("test-span").end_ok();

        assert_eq!(span.status, SpanStatus::Ok);
        assert!(span.end_time_ns.is_some());
    }

    #[tokio::test]
    async fn test_span_end_error() {
        let span = Span::new("test-span").end_error("Something went wrong");

        assert_eq!(span.status, SpanStatus::Error);
        assert_eq!(
            span.status_message,
            Some("Something went wrong".to_string())
        );
        assert!(span.end_time_ns.is_some());
    }

    #[tokio::test]
    async fn test_on_llm_start() {
        let integration = OpenTelemetryIntegration::with_defaults();

        let event = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
        integration.on_llm_start(&event).await.unwrap();

        assert_eq!(integration.active_span_count(), 1);
    }

    #[tokio::test]
    async fn test_on_llm_end() {
        let integration = OpenTelemetryIntegration::with_defaults();

        let start_event = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
        integration.on_llm_start(&start_event).await.unwrap();

        let end_event = LlmEndEvent::new("req-1", "gpt-4")
            .provider("openai")
            .tokens(100, 50)
            .latency(150);
        integration.on_llm_end(&end_event).await.unwrap();

        assert_eq!(integration.active_span_count(), 0);
        assert_eq!(integration.pending_span_count(), 1);
    }

    #[tokio::test]
    async fn test_on_llm_error() {
        let integration = OpenTelemetryIntegration::with_defaults();

        let start_event = LlmStartEvent::new("req-1", "gpt-4");
        integration.on_llm_start(&start_event).await.unwrap();

        let error_event = LlmErrorEvent::new("req-1", "gpt-4", "Rate limited")
            .error_type("RateLimitError")
            .status_code(429);
        integration.on_llm_error(&error_event).await.unwrap();

        assert_eq!(integration.active_span_count(), 0);
        assert_eq!(integration.pending_span_count(), 1);
    }

    #[tokio::test]
    async fn test_disabled_integration() {
        let config = OpenTelemetryConfig {
            enabled: false,
            ..Default::default()
        };
        let integration = OpenTelemetryIntegration::new(config);

        assert!(!integration.is_enabled());
    }

    #[tokio::test]
    async fn test_sampling() {
        let config = OpenTelemetryConfig {
            sampling_ratio: 0.0,
            ..Default::default()
        };
        let integration = OpenTelemetryIntegration::new(config);

        let event = LlmStartEvent::new("req-1", "gpt-4");
        integration.on_llm_start(&event).await.unwrap();

        // With 0% sampling, no spans should be created
        assert_eq!(integration.active_span_count(), 0);
    }

    #[test]
    fn test_generate_trace_id() {
        let id1 = generate_trace_id();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let id2 = generate_trace_id();

        assert_eq!(id1.len(), 32);
        assert_eq!(id2.len(), 32);
        // IDs should be different (with very high probability after sleep)
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_span_id() {
        let id1 = generate_span_id();
        let id2 = generate_span_id();

        assert_eq!(id1.len(), 16);
        assert_eq!(id2.len(), 16);
    }

    #[test]
    fn test_build_otlp_payload() {
        let spans = vec![Span::new("test-span").attribute("key", "value").end_ok()];

        let payload = build_otlp_payload("test-service", &spans);

        assert!(payload.get("resourceSpans").is_some());
    }
}
