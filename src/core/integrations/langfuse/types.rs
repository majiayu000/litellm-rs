//! Langfuse Types
//!
//! Data types for Langfuse API integration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Generate a new unique ID
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

/// Langfuse trace - represents a complete request/response cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    /// Unique trace identifier
    pub id: String,

    /// Trace name (e.g., endpoint or operation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// User ID associated with the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Session ID for grouping related traces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Tags for categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Release version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,

    /// Version of the trace (for updates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Public visibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,

    /// Timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,

    /// Input data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,

    /// Output data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

impl Trace {
    /// Create a new trace with generated ID
    pub fn new() -> Self {
        Self {
            id: generate_id(),
            name: None,
            user_id: None,
            session_id: None,
            metadata: HashMap::new(),
            tags: Vec::new(),
            release: None,
            version: None,
            public: None,
            timestamp: Some(Utc::now()),
            input: None,
            output: None,
        }
    }

    /// Create a new trace with specific ID
    pub fn with_id(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::new()
        }
    }

    /// Set the trace name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the user ID
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set the session ID
    pub fn session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Add a tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set input data
    pub fn input(mut self, input: serde_json::Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Set output data
    pub fn output(mut self, output: serde_json::Value) -> Self {
        self.output = Some(output);
        self
    }
}

impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}

/// Log level for events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
    Debug,
    #[default]
    Default,
    Warning,
    Error,
}

/// Usage information for token counting
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Input tokens (prompt)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<u32>,

    /// Output tokens (completion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<u32>,

    /// Total tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,

    /// Unit of measurement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Input cost
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_cost: Option<f64>,

    /// Output cost
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_cost: Option<f64>,

    /// Total cost
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost: Option<f64>,
}

impl Usage {
    /// Create usage from token counts
    pub fn from_tokens(input: u32, output: u32) -> Self {
        Self {
            input: Some(input),
            output: Some(output),
            total: Some(input + output),
            unit: Some("TOKENS".to_string()),
            ..Default::default()
        }
    }

    /// Set costs
    pub fn with_costs(mut self, input_cost: f64, output_cost: f64) -> Self {
        self.input_cost = Some(input_cost);
        self.output_cost = Some(output_cost);
        self.total_cost = Some(input_cost + output_cost);
        self
    }
}

/// Generation - represents an LLM call
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Generation {
    /// Unique generation identifier
    pub id: String,

    /// Parent trace ID
    pub trace_id: String,

    /// Generation name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Start time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,

    /// End time (completion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,

    /// Completion start time (for TTFT)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_start_time: Option<DateTime<Utc>>,

    /// Model name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Model parameters (temperature, max_tokens, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_parameters: Option<HashMap<String, serde_json::Value>>,

    /// Input (prompt/messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,

    /// Output (completion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,

    /// Usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,

    /// Log level
    #[serde(default)]
    pub level: Level,

    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// Parent observation ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_observation_id: Option<String>,

    /// Version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Generation {
    /// Create a new generation for a trace
    pub fn new(trace_id: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            trace_id: trace_id.into(),
            name: None,
            start_time: Some(Utc::now()),
            end_time: None,
            completion_start_time: None,
            model: None,
            model_parameters: None,
            input: None,
            output: None,
            usage: None,
            level: Level::Default,
            status_message: None,
            parent_observation_id: None,
            version: None,
            metadata: HashMap::new(),
        }
    }

    /// Set generation name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set model name
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set input
    pub fn input(mut self, input: serde_json::Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Set output
    pub fn output(mut self, output: serde_json::Value) -> Self {
        self.output = Some(output);
        self
    }

    /// Set usage
    pub fn usage(mut self, usage: Usage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Set end time
    pub fn end(mut self) -> Self {
        self.end_time = Some(Utc::now());
        self
    }

    /// Set level
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Set error status
    pub fn error(mut self, message: impl Into<String>) -> Self {
        self.level = Level::Error;
        self.status_message = Some(message.into());
        self.end_time = Some(Utc::now());
        self
    }

    /// Add model parameter
    pub fn model_param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.model_parameters
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Span - represents a unit of work within a trace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    /// Unique span identifier
    pub id: String,

    /// Parent trace ID
    pub trace_id: String,

    /// Span name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Start time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,

    /// End time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,

    /// Input data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,

    /// Output data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,

    /// Log level
    #[serde(default)]
    pub level: Level,

    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// Parent observation ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_observation_id: Option<String>,

    /// Version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Span {
    /// Create a new span for a trace
    pub fn new(trace_id: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            trace_id: trace_id.into(),
            name: None,
            start_time: Some(Utc::now()),
            end_time: None,
            input: None,
            output: None,
            level: Level::Default,
            status_message: None,
            parent_observation_id: None,
            version: None,
            metadata: HashMap::new(),
        }
    }

    /// Set span name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set input
    pub fn input(mut self, input: serde_json::Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Set output
    pub fn output(mut self, output: serde_json::Value) -> Self {
        self.output = Some(output);
        self
    }

    /// Set end time
    pub fn end(mut self) -> Self {
        self.end_time = Some(Utc::now());
        self
    }

    /// Set error status
    pub fn error(mut self, message: impl Into<String>) -> Self {
        self.level = Level::Error;
        self.status_message = Some(message.into());
        self.end_time = Some(Utc::now());
        self
    }

    /// Set log level
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Event types for batch ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IngestionEvent {
    /// Create a new trace
    TraceCreate {
        id: String,
        timestamp: DateTime<Utc>,
        body: Trace,
    },
    /// Create a new generation
    GenerationCreate {
        id: String,
        timestamp: DateTime<Utc>,
        body: Generation,
    },
    /// Update an existing generation
    GenerationUpdate {
        id: String,
        timestamp: DateTime<Utc>,
        body: Generation,
    },
    /// Create a new span
    SpanCreate {
        id: String,
        timestamp: DateTime<Utc>,
        body: Span,
    },
    /// Update an existing span
    SpanUpdate {
        id: String,
        timestamp: DateTime<Utc>,
        body: Span,
    },
}

impl IngestionEvent {
    /// Create a trace creation event
    pub fn trace_create(trace: Trace) -> Self {
        Self::TraceCreate {
            id: generate_id(),
            timestamp: Utc::now(),
            body: trace,
        }
    }

    /// Create a generation creation event
    pub fn generation_create(generation: Generation) -> Self {
        Self::GenerationCreate {
            id: generate_id(),
            timestamp: Utc::now(),
            body: generation,
        }
    }

    /// Create a generation update event
    pub fn generation_update(generation: Generation) -> Self {
        Self::GenerationUpdate {
            id: generate_id(),
            timestamp: Utc::now(),
            body: generation,
        }
    }

    /// Create a span creation event
    pub fn span_create(span: Span) -> Self {
        Self::SpanCreate {
            id: generate_id(),
            timestamp: Utc::now(),
            body: span,
        }
    }

    /// Create a span update event
    pub fn span_update(span: Span) -> Self {
        Self::SpanUpdate {
            id: generate_id(),
            timestamp: Utc::now(),
            body: span,
        }
    }

    /// Get the event ID
    pub fn event_id(&self) -> &str {
        match self {
            Self::TraceCreate { id, .. } => id,
            Self::GenerationCreate { id, .. } => id,
            Self::GenerationUpdate { id, .. } => id,
            Self::SpanCreate { id, .. } => id,
            Self::SpanUpdate { id, .. } => id,
        }
    }
}

/// Batch ingestion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionBatch {
    /// Batch of events
    pub batch: Vec<IngestionEvent>,
}

impl IngestionBatch {
    /// Create a new batch
    pub fn new() -> Self {
        Self { batch: Vec::new() }
    }

    /// Add an event to the batch
    pub fn add(&mut self, event: IngestionEvent) {
        self.batch.push(event);
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Get batch size
    pub fn len(&self) -> usize {
        self.batch.len()
    }

    /// Take all events from the batch
    pub fn take(&mut self) -> Vec<IngestionEvent> {
        std::mem::take(&mut self.batch)
    }
}

impl Default for IngestionBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Ingestion API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionResponse {
    /// Number of successfully ingested events
    pub successes: Vec<IngestionSuccess>,
    /// Failed events with errors
    pub errors: Vec<IngestionError>,
}

/// Successful ingestion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionSuccess {
    /// Event ID
    pub id: String,
    /// Status code
    pub status: u16,
}

/// Failed ingestion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionError {
    /// Event ID
    pub id: String,
    /// Status code
    pub status: u16,
    /// Error message
    pub message: Option<String>,
    /// Error details
    pub error: Option<String>,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
