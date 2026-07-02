use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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
pub(super) fn generate_trace_id() -> String {
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
pub(super) fn generate_span_id() -> String {
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
