use super::exporter::build_otlp_payload;
use super::integration_impl::sampling_fraction_from_nanos;
use super::span::{generate_span_id, generate_trace_id};
use super::*;
use crate::core::traits::integration::{
    EmbeddingErrorEvent, EmbeddingStartEvent, Integration, LlmEndEvent, LlmErrorEvent,
    LlmStartEvent,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;

async fn status_response_server(
    statuses: Vec<u16>,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test OTLP listener");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let completed = Arc::new(AtomicUsize::new(0));
    let server = tokio::spawn({
        let completed = Arc::clone(&completed);
        async move {
            for status in statuses {
                let (mut socket, _) = listener.accept().await.expect("OTLP request");
                let mut request = vec![0; 8192];
                let _ = socket.read(&mut request).await.expect("read OTLP request");
                let response = format!(
                    "HTTP/1.1 {status} test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("write OTLP response");
                socket.shutdown().await.expect("close OTLP response");
                completed.fetch_add(1, Ordering::SeqCst);
            }
        }
    });
    (endpoint, completed, server)
}

async fn wait_for_completed_requests(completed: &AtomicUsize, expected: usize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while completed.load(Ordering::SeqCst) < expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("OTLP requests should complete");
    tokio::time::sleep(Duration::from_millis(25)).await;
}

async fn export_one_span(integration: &OpenTelemetryIntegration, request_id: &str) {
    integration
        .on_llm_start(&LlmStartEvent::new(request_id, "gpt-test"))
        .await
        .unwrap();
    integration
        .on_llm_end(&LlmEndEvent::new(request_id, "gpt-test"))
        .await
        .unwrap();
}

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
async fn test_on_embedding_error_closes_and_exports_error_span() {
    let integration = OpenTelemetryIntegration::with_defaults();
    let start = EmbeddingStartEvent {
        request_id: "embedding-error".to_string(),
        model: "embedding-model".to_string(),
        provider: Some("provider".to_string()),
        input_count: 1,
        user_id: None,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    };
    integration.on_embedding_start(&start).await.unwrap();
    assert_eq!(integration.active_span_count(), 1);

    integration
        .on_embedding_error(&EmbeddingErrorEvent {
            request_id: start.request_id,
            model: start.model,
            provider: start.provider,
            error_message: "embedding failed".to_string(),
            error_type: Some("provider_error".to_string()),
            latency_ms: 25,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        })
        .await
        .unwrap();

    assert_eq!(integration.active_span_count(), 0);
    assert_eq!(integration.pending_span_count(), 1);
    assert_eq!(integration.pending_error_span_count(), 1);
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

#[tokio::test]
async fn test_shutdown_waits_for_in_flight_batch_export() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test OTLP listener");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let request_started = Arc::new(Notify::new());
    let release_response = Arc::new(Notify::new());
    let server = tokio::spawn({
        let request_started = Arc::clone(&request_started);
        let release_response = Arc::clone(&release_response);
        async move {
            let (mut socket, _) = listener.accept().await.expect("OTLP request");
            let mut request = vec![0; 8192];
            let _ = socket.read(&mut request).await.expect("read OTLP request");
            request_started.notify_one();
            release_response.notified().await;
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .expect("write OTLP response");
        }
    });

    let integration = OpenTelemetryIntegration::try_new(OpenTelemetryConfig {
        endpoint,
        max_batch_size: 1,
        ..Default::default()
    })
    .unwrap();
    integration
        .on_llm_start(&LlmStartEvent::new("req-shutdown", "gpt-test"))
        .await
        .unwrap();
    integration
        .on_llm_end(&LlmEndEvent::new("req-shutdown", "gpt-test"))
        .await
        .unwrap();
    request_started.notified().await;

    let mut shutdown = tokio::spawn(async move { integration.shutdown().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut shutdown)
            .await
            .is_err(),
        "shutdown returned before its in-flight export completed"
    );

    release_response.notify_one();
    tokio::time::timeout(Duration::from_secs(1), shutdown)
        .await
        .expect("shutdown should finish after export")
        .expect("shutdown task should join")
        .expect("shutdown should succeed");
    server.await.expect("OTLP server should finish");
}

#[tokio::test]
async fn test_completed_export_tasks_are_reaped_during_continuous_operation() {
    let (endpoint, completed, server) = status_response_server(vec![200; 9]).await;
    let integration = OpenTelemetryIntegration::try_new(OpenTelemetryConfig {
        endpoint,
        max_batch_size: 1,
        ..Default::default()
    })
    .unwrap();

    for index in 0..8 {
        export_one_span(&integration, &format!("req-reap-{index}")).await;
    }
    wait_for_completed_requests(&completed, 8).await;

    export_one_span(&integration, "req-reap-trigger").await;
    assert!(
        integration.export_task_count() <= 1,
        "completed export tasks should be reaped before tracking the next batch"
    );

    wait_for_completed_requests(&completed, 9).await;
    integration.flush().await.unwrap();
    assert_eq!(integration.export_task_count(), 0);
    server.await.expect("OTLP server should finish");
}

#[tokio::test]
async fn test_reaped_export_failure_is_reported_by_flush() {
    let (endpoint, completed, server) = status_response_server(vec![500, 200]).await;
    let integration = OpenTelemetryIntegration::try_new(OpenTelemetryConfig {
        endpoint,
        max_batch_size: 1,
        ..Default::default()
    })
    .unwrap();

    export_one_span(&integration, "req-failed-export").await;
    wait_for_completed_requests(&completed, 1).await;
    export_one_span(&integration, "req-reap-failure").await;
    assert_eq!(
        integration.export_failure_count(),
        1,
        "the completed failed task must be reaped before flush"
    );
    wait_for_completed_requests(&completed, 2).await;

    let error = integration
        .flush()
        .await
        .expect_err("reaped export failure must remain observable");
    assert!(error.to_string().contains("500"));
    server.await.expect("OTLP server should finish");
}

#[test]
fn test_sampling_fraction_from_nanos() {
    assert_eq!(sampling_fraction_from_nanos(0), 0.0);
    assert_eq!(sampling_fraction_from_nanos(500_000), 0.5);
    assert_eq!(sampling_fraction_from_nanos(1_250_000), 0.25);
    assert!(sampling_fraction_from_nanos(999_999) < 1.0);
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
