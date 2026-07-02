//! OpenTelemetry Integration
//!
//! Provides distributed tracing for LLM requests using OpenTelemetry.

mod config;
mod exporter;
mod integration_impl;
mod span;

#[cfg(test)]
mod tests;

pub use config::OpenTelemetryConfig;
pub use integration_impl::OpenTelemetryIntegration;
pub use span::{AttributeValue, Span, SpanEvent, SpanKind, SpanStatus};
