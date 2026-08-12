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
#[cfg_attr(
    not(test),
    deprecated(
        since = "0.6.0",
        note = "legacy observability types are library-only and scheduled for removal in 0.7; use RuntimeObservability"
    )
)]
pub use destinations::{AlertChannel, AlertRule, LogDestination, TraceExporter};
#[cfg_attr(
    not(test),
    deprecated(
        since = "0.6.0",
        note = "legacy observability types are library-only and scheduled for removal in 0.7; use RuntimeObservability"
    )
)]
pub use histogram::{BoundedHistogram, HISTOGRAM_MAX_SAMPLES};
#[cfg_attr(
    not(test),
    deprecated(
        since = "0.6.0",
        note = "LogAggregator is not wired into the gateway and is scheduled for removal in 0.7; use RuntimeObservability"
    )
)]
pub use logging::LogAggregator;
#[cfg_attr(
    not(test),
    deprecated(
        since = "0.6.0",
        note = "legacy observability exporters are not wired and are scheduled for removal in 0.7; use configured callback integrations"
    )
)]
pub use metrics::{DataDogClient, MetricsCollector, OtelExporter, PrometheusMetrics};
#[cfg_attr(
    not(test),
    deprecated(
        since = "0.6.0",
        note = "legacy observability redaction helpers are library-only and scheduled for removal in 0.7"
    )
)]
pub use redaction::{RedactionConfig, redact_headers, redact_json_value, redact_value};
#[cfg_attr(
    not(test),
    deprecated(
        since = "0.6.0",
        note = "PerformanceTracer is not wired into the gateway and is scheduled for removal in 0.7; use RuntimeObservability"
    )
)]
pub use tracing::PerformanceTracer;
#[cfg_attr(
    not(test),
    deprecated(
        since = "0.6.0",
        note = "legacy observability records are library-only and scheduled for removal in 0.7; use callback lifecycle records"
    )
)]
pub use types::{
    AlertCondition, AlertSeverity, AlertState, ErrorDetails, LogEntry, LogLevel, MetricValue,
    ObservabilityLogRecord, SpanLog, TokenUsage, TraceSpan,
};

/// Canonical gateway observability handle.
///
/// The gateway stores this dispatcher in `AppState`; configured integrations
/// receive request start/success/failure events from the real LLM lifecycle.
pub type RuntimeObservability = crate::core::integrations::CallbackDispatcher;
