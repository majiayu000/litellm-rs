//! Observability types and data structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Summary { sum: f64, count: u64 },
}

/// Structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityLogRecord {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Log level
    pub level: LogLevel,
    /// Message
    pub message: String,
    /// Request ID
    pub request_id: Option<String>,
    /// User ID
    pub user_id: Option<String>,
    /// Provider
    pub provider: Option<String>,
    /// Model
    pub model: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Token usage
    pub tokens: Option<TokenUsage>,
    /// Cost
    pub cost: Option<f64>,
    /// Error details
    pub error: Option<ErrorDetails>,
    /// Additional fields
    pub fields: HashMap<String, serde_json::Value>,
}

/// Canonical log entry type used across gateway utility logging surfaces.
pub type LogEntry = crate::utils::logging::utils::types::LogEntry;

/// Log levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

fn log_level_to_string(level: &LogLevel) -> String {
    match level {
        LogLevel::Error => "ERROR".to_string(),
        LogLevel::Warn => "WARN".to_string(),
        LogLevel::Info => "INFO".to_string(),
        LogLevel::Debug => "DEBUG".to_string(),
        LogLevel::Trace => "TRACE".to_string(),
    }
}

fn parse_log_level(level: &str) -> LogLevel {
    match level.to_uppercase().as_str() {
        "ERROR" => LogLevel::Error,
        "WARN" | "WARNING" => LogLevel::Warn,
        "INFO" => LogLevel::Info,
        "DEBUG" => LogLevel::Debug,
        "TRACE" => LogLevel::Trace,
        _ => LogLevel::Info,
    }
}

impl From<ObservabilityLogRecord> for LogEntry {
    fn from(record: ObservabilityLogRecord) -> Self {
        let mut metadata = record.fields;

        if let Some(user_id) = record.user_id {
            metadata.insert("user_id".to_string(), serde_json::json!(user_id));
        }
        if let Some(provider) = &record.provider {
            metadata.insert("provider".to_string(), serde_json::json!(provider));
        }
        if let Some(model) = record.model {
            metadata.insert("model".to_string(), serde_json::json!(model));
        }
        if let Some(duration_ms) = record.duration_ms {
            metadata.insert("duration_ms".to_string(), serde_json::json!(duration_ms));
        }
        if let Some(cost) = record.cost {
            metadata.insert("cost".to_string(), serde_json::json!(cost));
        }
        if let Some(tokens) = record.tokens
            && let Ok(value) = serde_json::to_value(tokens)
        {
            metadata.insert("tokens".to_string(), value);
        }
        if let Some(error) = record.error
            && let Ok(value) = serde_json::to_value(error)
        {
            metadata.insert("error".to_string(), value);
        }

        LogEntry {
            timestamp: record.timestamp,
            level: log_level_to_string(&record.level),
            message: record.message,
            module: record.provider,
            request_id: record.request_id,
            metadata,
        }
    }
}

impl From<LogEntry> for ObservabilityLogRecord {
    fn from(entry: LogEntry) -> Self {
        let mut fields = entry.metadata;
        let user_id = fields
            .remove("user_id")
            .and_then(|value| value.as_str().map(ToString::to_string));
        let mut provider = fields
            .remove("provider")
            .and_then(|value| value.as_str().map(ToString::to_string));
        let model = fields
            .remove("model")
            .and_then(|value| value.as_str().map(ToString::to_string));
        let duration_ms = fields
            .remove("duration_ms")
            .and_then(|value| value.as_u64());
        let cost = fields.remove("cost").and_then(|value| value.as_f64());
        let tokens = fields
            .remove("tokens")
            .and_then(|value| serde_json::from_value(value).ok());
        let error = fields
            .remove("error")
            .and_then(|value| serde_json::from_value(value).ok());

        if provider.is_none() {
            provider = entry.module.clone();
        }

        ObservabilityLogRecord {
            timestamp: entry.timestamp,
            level: parse_log_level(&entry.level),
            message: entry.message,
            request_id: entry.request_id,
            user_id,
            provider,
            model,
            duration_ms,
            tokens,
            cost,
            error,
            fields,
        }
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Error details for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    pub error_type: String,
    pub error_message: String,
    pub error_code: Option<String>,
    pub stack_trace: Option<String>,
}

/// Alert conditions
#[derive(Debug, Clone)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// Alert state tracking
#[derive(Debug, Clone)]
pub struct AlertState {
    /// Whether alert is currently firing
    pub firing: bool,
    /// When alert started firing
    pub fired_at: Option<DateTime<Utc>>,
    /// Last notification sent
    pub last_notification: Option<DateTime<Utc>>,
    /// Notification count
    pub notification_count: u32,
}

/// Trace span
#[derive(Debug, Clone)]
pub struct TraceSpan {
    /// Span ID
    pub span_id: String,
    /// Parent span ID
    pub parent_id: Option<String>,
    /// Trace ID
    pub trace_id: String,
    /// Operation name
    pub operation: String,
    /// Start time
    pub start_time: std::time::Instant,
    /// End time
    pub end_time: Option<std::time::Instant>,
    /// Tags
    pub tags: HashMap<String, String>,
    /// Logs
    pub logs: Vec<SpanLog>,
}

/// Span log entry
#[derive(Debug, Clone)]
pub struct SpanLog {
    pub timestamp: std::time::Instant,
    pub message: String,
    pub fields: HashMap<String, String>,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
