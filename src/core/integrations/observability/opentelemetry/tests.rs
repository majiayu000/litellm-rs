use super::exporter::build_otlp_payload;
use super::span::{generate_span_id, generate_trace_id};
use super::*;
use crate::core::traits::integration::{Integration, LlmEndEvent, LlmErrorEvent, LlmStartEvent};

#[tokio::test]
async fn test_opentelemetry_integration_creation() {
    let integration = OpenTelemetryIntegration::with_defaults();
    assert_eq!(integration.name(), "opentelemetry");
    assert!(integration.is_enabled());
}

#[tokio::test]
async fn test_span_creation() {
    let span = Span::new("test-span")
        .kind(SpanKind::Client)
        .attribute("key", "value")
        .attribute("count", 42i64);

    assert_eq!(span.name, "test-span");
    assert_eq!(span.kind, SpanKind::Client);
    assert!(span.attributes.contains_key("key"));
    assert!(span.attributes.contains_key("count"));
}

#[tokio::test]
async fn test_span_end_ok() {
    let span = Span::new("test-span").end_ok();

    assert_eq!(span.status, SpanStatus::Ok);
    assert!(span.end_time_ns.is_some());
}

#[tokio::test]
async fn test_span_end_error() {
    let span = Span::new("test-span").end_error("Something went wrong");

    assert_eq!(span.status, SpanStatus::Error);
    assert_eq!(
        span.status_message,
        Some("Something went wrong".to_string())
    );
    assert!(span.end_time_ns.is_some());
}

#[tokio::test]
async fn test_on_llm_start() {
    let integration = OpenTelemetryIntegration::with_defaults();

    let event = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
    integration.on_llm_start(&event).await.unwrap();

    assert_eq!(integration.active_span_count(), 1);
}

#[tokio::test]
async fn test_on_llm_end() {
    let integration = OpenTelemetryIntegration::with_defaults();

    let start_event = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
    integration.on_llm_start(&start_event).await.unwrap();

    let end_event = LlmEndEvent::new("req-1", "gpt-4")
        .provider("openai")
        .tokens(100, 50)
        .latency(150);
    integration.on_llm_end(&end_event).await.unwrap();

    assert_eq!(integration.active_span_count(), 0);
    assert_eq!(integration.pending_span_count(), 1);
}

#[tokio::test]
async fn test_on_llm_error() {
    let integration = OpenTelemetryIntegration::with_defaults();

    let start_event = LlmStartEvent::new("req-1", "gpt-4");
    integration.on_llm_start(&start_event).await.unwrap();

    let error_event = LlmErrorEvent::new("req-1", "gpt-4", "Rate limited")
        .error_type("RateLimitError")
        .status_code(429);
    integration.on_llm_error(&error_event).await.unwrap();

    assert_eq!(integration.active_span_count(), 0);
    assert_eq!(integration.pending_span_count(), 1);
}

#[tokio::test]
async fn test_disabled_integration() {
    let config = OpenTelemetryConfig {
        enabled: false,
        ..Default::default()
    };
    let integration = OpenTelemetryIntegration::new(config);

    assert!(!integration.is_enabled());
}

#[tokio::test]
async fn test_sampling() {
    let config = OpenTelemetryConfig {
        sampling_ratio: 0.0,
        ..Default::default()
    };
    let integration = OpenTelemetryIntegration::new(config);

    let event = LlmStartEvent::new("req-1", "gpt-4");
    integration.on_llm_start(&event).await.unwrap();

    // With 0% sampling, no spans should be created
    assert_eq!(integration.active_span_count(), 0);
}

#[test]
fn test_generate_trace_id() {
    let id1 = generate_trace_id();
    std::thread::sleep(std::time::Duration::from_millis(1));
    let id2 = generate_trace_id();

    assert_eq!(id1.len(), 32);
    assert_eq!(id2.len(), 32);
    // IDs should be different (with very high probability after sleep)
    assert_ne!(id1, id2);
}

#[test]
fn test_generate_span_id() {
    let id1 = generate_span_id();
    let id2 = generate_span_id();

    assert_eq!(id1.len(), 16);
    assert_eq!(id2.len(), 16);
}

#[test]
fn test_build_otlp_payload() {
    let spans = vec![Span::new("test-span").attribute("key", "value").end_ok()];

    let payload = build_otlp_payload("test-service", &spans);

    assert!(payload.get("resourceSpans").is_some());
}
