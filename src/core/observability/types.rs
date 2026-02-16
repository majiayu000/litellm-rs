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
        if let Some(tokens) = record.tokens {
            if let Ok(value) = serde_json::to_value(tokens) {
                metadata.insert("tokens".to_string(), value);
            }
        }
        if let Some(error) = record.error {
            if let Ok(value) = serde_json::to_value(error) {
                metadata.insert("error".to_string(), value);
            }
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
mod tests {
    use super::*;

    // ==================== MetricValue Tests ====================

    #[test]
    fn test_metric_value_counter() {
        let metric = MetricValue::Counter(42);
        if let MetricValue::Counter(v) = metric {
            assert_eq!(v, 42);
        } else {
            panic!("Expected Counter variant");
        }
    }

    #[test]
    fn test_metric_value_gauge() {
        let metric = MetricValue::Gauge(1.234);
        if let MetricValue::Gauge(v) = metric {
            assert!((v - 1.234).abs() < 0.001);
        } else {
            panic!("Expected Gauge variant");
        }
    }

    #[test]
    fn test_metric_value_histogram() {
        let metric = MetricValue::Histogram(vec![1.0, 2.0, 3.0]);
        if let MetricValue::Histogram(v) = metric {
            assert_eq!(v.len(), 3);
            assert_eq!(v[0], 1.0);
        } else {
            panic!("Expected Histogram variant");
        }
    }

    #[test]
    fn test_metric_value_summary() {
        let metric = MetricValue::Summary {
            sum: 100.0,
            count: 10,
        };
        if let MetricValue::Summary { sum, count } = metric {
            assert_eq!(sum, 100.0);
            assert_eq!(count, 10);
        } else {
            panic!("Expected Summary variant");
        }
    }

    #[test]
    fn test_metric_value_serialize() {
        let metric = MetricValue::Counter(100);
        let json = serde_json::to_string(&metric).unwrap();
        assert!(json.contains("Counter"));
        assert!(json.contains("100"));
    }

    #[test]
    fn test_metric_value_deserialize() {
        let json = r#"{"Counter": 50}"#;
        let metric: MetricValue = serde_json::from_str(json).unwrap();
        if let MetricValue::Counter(v) = metric {
            assert_eq!(v, 50);
        } else {
            panic!("Expected Counter variant");
        }
    }

    #[test]
    fn test_metric_value_clone() {
        let metric1 = MetricValue::Gauge(1.5);
        let metric2 = metric1.clone();
        if let (MetricValue::Gauge(v1), MetricValue::Gauge(v2)) = (metric1, metric2) {
            assert_eq!(v1, v2);
        }
    }

    // ==================== LogLevel Tests ====================

    #[test]
    fn test_log_level_variants() {
        let levels = [
            LogLevel::Error,
            LogLevel::Warn,
            LogLevel::Info,
            LogLevel::Debug,
            LogLevel::Trace,
        ];
        assert_eq!(levels.len(), 5);
    }

    #[test]
    fn test_log_level_serialize() {
        let level = LogLevel::Error;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"Error\"");
    }

    #[test]
    fn test_log_level_deserialize() {
        let json = "\"Warn\"";
        let level: LogLevel = serde_json::from_str(json).unwrap();
        matches!(level, LogLevel::Warn);
    }

    #[test]
    fn test_log_level_clone() {
        let level1 = LogLevel::Info;
        let level2 = level1.clone();
        assert!(matches!(level2, LogLevel::Info));
    }

    // ==================== TokenUsage Tests ====================

    #[test]
    fn test_token_usage_creation() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_token_usage_serialize() {
        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("prompt_tokens"));
        assert!(json.contains("10"));
    }

    #[test]
    fn test_token_usage_deserialize() {
        let json = r#"{"prompt_tokens": 5, "completion_tokens": 10, "total_tokens": 15}"#;
        let usage: TokenUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.prompt_tokens, 5);
        assert_eq!(usage.completion_tokens, 10);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn test_token_usage_clone() {
        let usage1 = TokenUsage {
            prompt_tokens: 1,
            completion_tokens: 2,
            total_tokens: 3,
        };
        let usage2 = usage1.clone();
        assert_eq!(usage1.prompt_tokens, usage2.prompt_tokens);
    }

    #[test]
    fn test_token_usage_zero() {
        let usage = TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };
        assert_eq!(usage.total_tokens, 0);
    }

    // ==================== ErrorDetails Tests ====================

    #[test]
    fn test_error_details_creation() {
        let error = ErrorDetails {
            error_type: "ValidationError".to_string(),
            error_message: "Invalid input".to_string(),
            error_code: Some("E001".to_string()),
            stack_trace: None,
        };
        assert_eq!(error.error_type, "ValidationError");
        assert_eq!(error.error_message, "Invalid input");
        assert_eq!(error.error_code, Some("E001".to_string()));
        assert!(error.stack_trace.is_none());
    }

    #[test]
    fn test_error_details_full() {
        let error = ErrorDetails {
            error_type: "RuntimeError".to_string(),
            error_message: "Something went wrong".to_string(),
            error_code: Some("E500".to_string()),
            stack_trace: Some("at main.rs:10\nat lib.rs:20".to_string()),
        };
        assert!(error.stack_trace.is_some());
    }

    #[test]
    fn test_error_details_serialize() {
        let error = ErrorDetails {
            error_type: "TestError".to_string(),
            error_message: "Test message".to_string(),
            error_code: None,
            stack_trace: None,
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("TestError"));
        assert!(json.contains("Test message"));
    }

    #[test]
    fn test_error_details_deserialize() {
        let json = r#"{
            "error_type": "ParseError",
            "error_message": "Failed to parse",
            "error_code": "P001",
            "stack_trace": null
        }"#;
        let error: ErrorDetails = serde_json::from_str(json).unwrap();
        assert_eq!(error.error_type, "ParseError");
        assert_eq!(error.error_code, Some("P001".to_string()));
    }

    // ==================== AlertCondition Tests ====================

    #[test]
    fn test_alert_condition_variants() {
        let conditions = [
            AlertCondition::GreaterThan,
            AlertCondition::LessThan,
            AlertCondition::Equal,
            AlertCondition::NotEqual,
            AlertCondition::GreaterThanOrEqual,
            AlertCondition::LessThanOrEqual,
        ];
        assert_eq!(conditions.len(), 6);
    }

    #[test]
    fn test_alert_condition_clone() {
        let cond1 = AlertCondition::GreaterThan;
        let cond2 = cond1.clone();
        assert!(matches!(cond2, AlertCondition::GreaterThan));
    }

    // ==================== AlertSeverity Tests ====================

    #[test]
    fn test_alert_severity_variants() {
        let severities = [
            AlertSeverity::Critical,
            AlertSeverity::High,
            AlertSeverity::Medium,
            AlertSeverity::Low,
            AlertSeverity::Info,
        ];
        assert_eq!(severities.len(), 5);
    }

    #[test]
    fn test_alert_severity_serialize() {
        let severity = AlertSeverity::Critical;
        let json = serde_json::to_string(&severity).unwrap();
        assert_eq!(json, "\"Critical\"");
    }

    #[test]
    fn test_alert_severity_deserialize() {
        let json = "\"High\"";
        let severity: AlertSeverity = serde_json::from_str(json).unwrap();
        assert!(matches!(severity, AlertSeverity::High));
    }

    // ==================== AlertState Tests ====================

    #[test]
    fn test_alert_state_not_firing() {
        let state = AlertState {
            firing: false,
            fired_at: None,
            last_notification: None,
            notification_count: 0,
        };
        assert!(!state.firing);
        assert!(state.fired_at.is_none());
    }

    #[test]
    fn test_alert_state_firing() {
        let now = Utc::now();
        let state = AlertState {
            firing: true,
            fired_at: Some(now),
            last_notification: Some(now),
            notification_count: 1,
        };
        assert!(state.firing);
        assert!(state.fired_at.is_some());
        assert_eq!(state.notification_count, 1);
    }

    #[test]
    fn test_alert_state_clone() {
        let state1 = AlertState {
            firing: true,
            fired_at: Some(Utc::now()),
            last_notification: None,
            notification_count: 5,
        };
        let state2 = state1.clone();
        assert_eq!(state1.firing, state2.firing);
        assert_eq!(state1.notification_count, state2.notification_count);
    }

    // ==================== ObservabilityLogRecord Tests ====================

    #[test]
    fn test_log_entry_minimal() {
        let entry = ObservabilityLogRecord {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            message: "Test log".to_string(),
            request_id: None,
            user_id: None,
            provider: None,
            model: None,
            duration_ms: None,
            tokens: None,
            cost: None,
            error: None,
            fields: HashMap::new(),
        };
        assert_eq!(entry.message, "Test log");
        assert!(matches!(entry.level, LogLevel::Info));
    }

    #[test]
    fn test_log_entry_full() {
        let mut fields = HashMap::new();
        fields.insert("custom_key".to_string(), serde_json::json!("custom_value"));

        let entry = ObservabilityLogRecord {
            timestamp: Utc::now(),
            level: LogLevel::Error,
            message: "Error occurred".to_string(),
            request_id: Some("req-123".to_string()),
            user_id: Some("user-456".to_string()),
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            duration_ms: Some(150),
            tokens: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            }),
            cost: Some(0.05),
            error: Some(ErrorDetails {
                error_type: "APIError".to_string(),
                error_message: "Rate limited".to_string(),
                error_code: Some("429".to_string()),
                stack_trace: None,
            }),
            fields,
        };

        assert_eq!(entry.request_id, Some("req-123".to_string()));
        assert_eq!(entry.duration_ms, Some(150));
        assert!(entry.tokens.is_some());
        assert!(entry.error.is_some());
    }

    #[test]
    fn test_log_entry_serialize() {
        let entry = ObservabilityLogRecord {
            timestamp: Utc::now(),
            level: LogLevel::Debug,
            message: "Debug message".to_string(),
            request_id: None,
            user_id: None,
            provider: None,
            model: None,
            duration_ms: None,
            tokens: None,
            cost: None,
            error: None,
            fields: HashMap::new(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("Debug message"));
        assert!(json.contains("Debug"));
    }

    #[test]
    fn test_log_entry_clone() {
        let entry1 = ObservabilityLogRecord {
            timestamp: Utc::now(),
            level: LogLevel::Warn,
            message: "Warning".to_string(),
            request_id: Some("req-1".to_string()),
            user_id: None,
            provider: None,
            model: None,
            duration_ms: None,
            tokens: None,
            cost: None,
            error: None,
            fields: HashMap::new(),
        };
        let entry2 = entry1.clone();
        assert_eq!(entry1.message, entry2.message);
        assert_eq!(entry1.request_id, entry2.request_id);
    }

    // ==================== TraceSpan Tests ====================

    #[test]
    fn test_trace_span_creation() {
        let span = TraceSpan {
            span_id: "span-123".to_string(),
            parent_id: None,
            trace_id: "trace-456".to_string(),
            operation: "http.request".to_string(),
            start_time: std::time::Instant::now(),
            end_time: None,
            tags: HashMap::new(),
            logs: Vec::new(),
        };
        assert_eq!(span.span_id, "span-123");
        assert_eq!(span.trace_id, "trace-456");
        assert!(span.parent_id.is_none());
        assert!(span.end_time.is_none());
    }

    #[test]
    fn test_trace_span_with_parent() {
        let span = TraceSpan {
            span_id: "child-span".to_string(),
            parent_id: Some("parent-span".to_string()),
            trace_id: "trace-1".to_string(),
            operation: "db.query".to_string(),
            start_time: std::time::Instant::now(),
            end_time: None,
            tags: HashMap::new(),
            logs: Vec::new(),
        };
        assert_eq!(span.parent_id, Some("parent-span".to_string()));
    }

    #[test]
    fn test_trace_span_with_tags() {
        let mut tags = HashMap::new();
        tags.insert("http.method".to_string(), "GET".to_string());
        tags.insert("http.url".to_string(), "/api/v1/users".to_string());

        let span = TraceSpan {
            span_id: "span-1".to_string(),
            parent_id: None,
            trace_id: "trace-1".to_string(),
            operation: "http.request".to_string(),
            start_time: std::time::Instant::now(),
            end_time: None,
            tags,
            logs: Vec::new(),
        };
        assert_eq!(span.tags.len(), 2);
        assert_eq!(span.tags.get("http.method"), Some(&"GET".to_string()));
    }

    #[test]
    fn test_trace_span_with_logs() {
        let log = SpanLog {
            timestamp: std::time::Instant::now(),
            message: "Request started".to_string(),
            fields: HashMap::new(),
        };

        let span = TraceSpan {
            span_id: "span-1".to_string(),
            parent_id: None,
            trace_id: "trace-1".to_string(),
            operation: "process".to_string(),
            start_time: std::time::Instant::now(),
            end_time: None,
            tags: HashMap::new(),
            logs: vec![log],
        };
        assert_eq!(span.logs.len(), 1);
        assert_eq!(span.logs[0].message, "Request started");
    }

    #[test]
    fn test_trace_span_clone() {
        let span1 = TraceSpan {
            span_id: "span-1".to_string(),
            parent_id: None,
            trace_id: "trace-1".to_string(),
            operation: "test".to_string(),
            start_time: std::time::Instant::now(),
            end_time: None,
            tags: HashMap::new(),
            logs: Vec::new(),
        };
        let span2 = span1.clone();
        assert_eq!(span1.span_id, span2.span_id);
        assert_eq!(span1.trace_id, span2.trace_id);
    }

    // ==================== SpanLog Tests ====================

    #[test]
    fn test_span_log_creation() {
        let log = SpanLog {
            timestamp: std::time::Instant::now(),
            message: "Test log message".to_string(),
            fields: HashMap::new(),
        };
        assert_eq!(log.message, "Test log message");
        assert!(log.fields.is_empty());
    }

    #[test]
    fn test_span_log_with_fields() {
        let mut fields = HashMap::new();
        fields.insert("key1".to_string(), "value1".to_string());
        fields.insert("key2".to_string(), "value2".to_string());

        let log = SpanLog {
            timestamp: std::time::Instant::now(),
            message: "Log with fields".to_string(),
            fields,
        };
        assert_eq!(log.fields.len(), 2);
        assert_eq!(log.fields.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_span_log_clone() {
        let mut fields = HashMap::new();
        fields.insert("test".to_string(), "data".to_string());

        let log1 = SpanLog {
            timestamp: std::time::Instant::now(),
            message: "Original".to_string(),
            fields,
        };
        let log2 = log1.clone();
        assert_eq!(log1.message, log2.message);
        assert_eq!(log1.fields.len(), log2.fields.len());
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_log_entry_with_all_components() {
        let mut fields = HashMap::new();
        fields.insert("environment".to_string(), serde_json::json!("production"));

        let entry = ObservabilityLogRecord {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            message: "Request completed".to_string(),
            request_id: Some("req-abc".to_string()),
            user_id: Some("user-xyz".to_string()),
            provider: Some("anthropic".to_string()),
            model: Some("claude-3".to_string()),
            duration_ms: Some(250),
            tokens: Some(TokenUsage {
                prompt_tokens: 200,
                completion_tokens: 100,
                total_tokens: 300,
            }),
            cost: Some(0.01),
            error: None,
            fields,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: ObservabilityLogRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.message, "Request completed");
        assert_eq!(parsed.provider, Some("anthropic".to_string()));
        assert!(parsed.tokens.is_some());
    }

    #[test]
    fn test_complete_trace_scenario() {
        let start = std::time::Instant::now();

        let log1 = SpanLog {
            timestamp: start,
            message: "Starting operation".to_string(),
            fields: HashMap::new(),
        };

        let mut tags = HashMap::new();
        tags.insert("service".to_string(), "gateway".to_string());

        let span = TraceSpan {
            span_id: "main-span".to_string(),
            parent_id: None,
            trace_id: "trace-complete".to_string(),
            operation: "handle_request".to_string(),
            start_time: start,
            end_time: Some(std::time::Instant::now()),
            tags,
            logs: vec![log1],
        };

        assert!(span.end_time.is_some());
        assert_eq!(span.logs.len(), 1);
        assert!(span.tags.contains_key("service"));
    }
}
