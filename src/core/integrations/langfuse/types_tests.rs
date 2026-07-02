use super::*;

#[test]
fn test_trace_creation() {
    let trace = Trace::new()
        .name("test-trace")
        .user_id("user-123")
        .session_id("session-456")
        .tag("production")
        .metadata("key", serde_json::json!("value"));

    assert!(!trace.id.is_empty());
    assert_eq!(trace.name, Some("test-trace".to_string()));
    assert_eq!(trace.user_id, Some("user-123".to_string()));
    assert_eq!(trace.session_id, Some("session-456".to_string()));
    assert_eq!(trace.tags, vec!["production"]);
    assert!(trace.metadata.contains_key("key"));
}

#[test]
fn test_trace_with_id() {
    let trace = Trace::with_id("custom-id");
    assert_eq!(trace.id, "custom-id");
}

#[test]
fn test_generation_creation() {
    let generation = Generation::new("trace-123")
        .name("chat-completion")
        .model("gpt-4")
        .input(serde_json::json!({"messages": []}))
        .model_param("temperature", serde_json::json!(0.7));

    assert!(!generation.id.is_empty());
    assert_eq!(generation.trace_id, "trace-123");
    assert_eq!(generation.name, Some("chat-completion".to_string()));
    assert_eq!(generation.model, Some("gpt-4".to_string()));
    assert!(generation.input.is_some());
    assert!(generation.model_parameters.is_some());
}

#[test]
fn test_generation_error() {
    let generation = Generation::new("trace-123").error("API rate limited");

    assert_eq!(generation.level, Level::Error);
    assert_eq!(
        generation.status_message,
        Some("API rate limited".to_string())
    );
    assert!(generation.end_time.is_some());
}

#[test]
fn test_span_creation() {
    let span = Span::new("trace-123")
        .name("process-request")
        .input(serde_json::json!({"data": "test"}));

    assert!(!span.id.is_empty());
    assert_eq!(span.trace_id, "trace-123");
    assert_eq!(span.name, Some("process-request".to_string()));
}

#[test]
fn test_span_error() {
    let span = Span::new("trace-123").error("Processing failed");

    assert_eq!(span.level, Level::Error);
    assert!(span.end_time.is_some());
}

#[test]
fn test_usage_from_tokens() {
    let usage = Usage::from_tokens(100, 50);

    assert_eq!(usage.input, Some(100));
    assert_eq!(usage.output, Some(50));
    assert_eq!(usage.total, Some(150));
    assert_eq!(usage.unit, Some("TOKENS".to_string()));
}

#[test]
fn test_usage_with_costs() {
    let usage = Usage::from_tokens(100, 50).with_costs(0.01, 0.02);

    assert_eq!(usage.input_cost, Some(0.01));
    assert_eq!(usage.output_cost, Some(0.02));
    assert_eq!(usage.total_cost, Some(0.03));
}

#[test]
fn test_ingestion_event_trace() {
    let trace = Trace::new().name("test");
    let event = IngestionEvent::trace_create(trace);

    assert!(!event.event_id().is_empty());
    if let IngestionEvent::TraceCreate { body, .. } = event {
        assert_eq!(body.name, Some("test".to_string()));
    } else {
        panic!("Expected TraceCreate");
    }
}

#[test]
fn test_ingestion_event_generation() {
    let generation = Generation::new("trace-123");
    let event = IngestionEvent::generation_create(generation);

    if let IngestionEvent::GenerationCreate { body, .. } = event {
        assert_eq!(body.trace_id, "trace-123");
    } else {
        panic!("Expected GenerationCreate");
    }
}

#[test]
fn test_ingestion_batch() {
    let mut batch = IngestionBatch::new();
    assert!(batch.is_empty());

    batch.add(IngestionEvent::trace_create(Trace::new()));
    batch.add(IngestionEvent::generation_create(Generation::new("trace")));

    assert_eq!(batch.len(), 2);
    assert!(!batch.is_empty());

    let events = batch.take();
    assert_eq!(events.len(), 2);
    assert!(batch.is_empty());
}

#[test]
fn test_level_serialization() {
    let level = Level::Error;
    let json = serde_json::to_string(&level).unwrap();
    assert_eq!(json, "\"ERROR\"");

    let deserialized: Level = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, Level::Error);
}

#[test]
fn test_trace_serialization() {
    let trace = Trace::new().name("test").user_id("user").tag("prod");

    let json = serde_json::to_value(&trace).unwrap();
    assert!(json.get("id").is_some());
    assert_eq!(json.get("name").unwrap(), "test");
    assert_eq!(json.get("userId").unwrap(), "user");
}

#[test]
fn test_generation_serialization() {
    let generation = Generation::new("trace-123")
        .model("gpt-4")
        .usage(Usage::from_tokens(100, 50));

    let json = serde_json::to_value(&generation).unwrap();
    assert_eq!(json.get("traceId").unwrap(), "trace-123");
    assert_eq!(json.get("model").unwrap(), "gpt-4");
    assert!(json.get("usage").is_some());
}

#[test]
fn test_ingestion_event_serialization() {
    let trace = Trace::new().name("test");
    let event = IngestionEvent::trace_create(trace);

    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json.get("type").unwrap(), "trace-create");
    assert!(json.get("body").is_some());
}
