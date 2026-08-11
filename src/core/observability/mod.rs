//! Advanced observability and monitoring system
//!
//! This module provides comprehensive monitoring, logging, and alerting capabilities.

mod destinations;
mod histogram;
mod logging;
mod metrics;
mod redaction;
mod tracing;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use destinations::{AlertChannel, AlertRule, LogDestination, TraceExporter};
pub use histogram::{BoundedHistogram, HISTOGRAM_MAX_SAMPLES};
pub use logging::LogAggregator;
pub use metrics::{DataDogClient, MetricsCollector, OtelExporter, PrometheusMetrics};
pub use redaction::{RedactionConfig, redact_headers, redact_json_value, redact_value};
pub use tracing::PerformanceTracer;
pub use types::{
    AlertCondition, AlertSeverity, AlertState, ErrorDetails, LogEntry, LogLevel, MetricValue,
    ObservabilityLogRecord, SpanLog, TokenUsage, TraceSpan,
};

/// Canonical gateway observability handle.
///
/// The gateway stores this dispatcher in `AppState`; configured integrations
/// receive request start/success/failure events from the real LLM lifecycle.
pub type RuntimeObservability = crate::core::integrations::CallbackDispatcher;
