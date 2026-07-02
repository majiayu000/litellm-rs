//! OpenTelemetry Integration
//!
//! Provides distributed tracing for LLM requests using OpenTelemetry.

mod config;
mod exporter;
mod integration_impl;
mod span;

#[cfg(test)]
mod tests;

pub use self::config::OpenTelemetryConfig;
pub use self::integration_impl::OpenTelemetryIntegration;
pub use self::span::{AttributeValue, Span, SpanEvent, SpanKind, SpanStatus};
