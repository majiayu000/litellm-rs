use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::warn;

use crate::core::traits::integration::{
    CacheHitEvent, EmbeddingEndEvent, EmbeddingStartEvent, Integration, IntegrationError,
    IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
};
use crate::utils::net::http::create_custom_client;

use super::config::OpenTelemetryConfig;
use super::exporter::export_spans;
use super::span::{Span, SpanKind};

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
    /// Create a new OpenTelemetry integration and surface client construction errors.
    pub fn try_new(config: OpenTelemetryConfig) -> IntegrationResult<Self> {
        let http_client =
            create_custom_client(Duration::from_millis(config.timeout_ms)).map_err(|error| {
                IntegrationError::connection(format!(
                    "Failed to create OpenTelemetry HTTP client: {error}"
                ))
            })?;

        Ok(Self {
            config,
            active_spans: RwLock::new(HashMap::new()),
            pending_spans: RwLock::new(SpanBatch::new()),
            http_client,
        })
    }

    /// Create a new OpenTelemetry integration
    pub fn new(config: OpenTelemetryConfig) -> Self {
        match Self::try_new(config.clone()) {
            Ok(integration) => integration,
            Err(error) => {
                warn!(
                    "OpenTelemetry custom HTTP client initialization failed; \
                     using the legacy default client fallback: {}",
                    error
                );
                Self {
                    config,
                    active_spans: RwLock::new(HashMap::new()),
                    pending_spans: RwLock::new(SpanBatch::new()),
                    http_client: reqwest::Client::new(),
                }
            }
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
        let random = sampling_fraction_from_nanos(now);
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

pub(super) fn sampling_fraction_from_nanos(nanos: u128) -> f64 {
    ((nanos % 1_000_000) as f64) / 1_000_000.0
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
        if let Some(active) = active_spans.get_mut(&event.request_id)
            && event.is_final
        {
            active.span.add_event("stream.complete");
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
