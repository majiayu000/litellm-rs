//! Integration tests for the Audit Logging system

use self::config::AuditConfig;
use self::events::{AuditEvent, EventType};
use self::logger::{AuditLogger, AuditLoggerBuilder};
use self::outputs::MemoryOutput;
use self::types::{LogLevel, RequestLog, ResponseLog, UserAction};
use super::*;
use std::sync::Arc;

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_audit_pipeline() {
    let config = AuditConfig::new().enable();
    let logger = AuditLogger::new(config).await.unwrap();

    // Log various events
    assert!(
        logger
            .log(AuditEvent::request_started(
                "req-1",
                r"/v1/chat/completions",
            ))
            .await
            .is_ok()
    );
    assert!(
        logger
            .log(AuditEvent::request_completed("req-1", 200, 150))
            .await
            .is_ok()
    );
    assert!(
        logger
            .log(AuditEvent::user_action("user-1", UserAction::Login))
            .await
            .is_ok()
    );
    assert!(
        logger
            .log(AuditEvent::security("Suspicious activity"))
            .await
            .is_ok()
    );

    // Flush
    logger.flush().await.unwrap();
}

#[tokio::test]
async fn test_request_response_logging() {
    let config = AuditConfig::new()
        .enable()
        .with_request_body(true)
        .with_response_body(true);

    let logger = AuditLogger::new(config).await.unwrap();

    // Create request log
    let request = RequestLog::new("req-1", "POST", r"/v1/chat/completions")
        .with_client_ip("192.168.1.1")
        .with_user_agent(r"test-client/1.0")
        .with_body(r#"{"model": "gpt-4"}"#, 18);

    let event = AuditEvent::request_started("req-1", r"/v1/chat/completions").with_request(request);

    assert!(logger.log(event).await.is_ok());

    // Create response log
    let response = ResponseLog::new("req-1", 200, 150).with_body(r#"{"choices": []}"#, 15);

    let event = AuditEvent::request_completed("req-1", 200, 150).with_response(response);

    assert!(logger.log(event).await.is_ok());

    logger.flush().await.unwrap();
}

#[tokio::test]
async fn test_user_action_logging() {
    let config = AuditConfig::new().enable();
    let logger = AuditLogger::new(config).await.unwrap();

    let actions = vec![
        UserAction::Login,
        UserAction::Logout,
        UserAction::ApiKeyCreated,
        UserAction::ApiKeyRevoked,
        UserAction::SettingsChanged,
        UserAction::BudgetUpdated,
    ];

    for action in actions {
        let event = AuditEvent::user_action("user-123", action);
        assert!(logger.log(event).await.is_ok());
    }

    logger.flush().await.unwrap();
}

#[tokio::test]
async fn test_log_level_filtering() {
    let config = AuditConfig::new().enable().with_min_level(LogLevel::Warn);

    let logger = AuditLogger::new(config).await.unwrap();

    // Debug and Info should be filtered
    assert!(
        logger
            .log(AuditEvent::new(EventType::System, "Debug").with_level(LogLevel::Debug))
            .await
            .is_ok()
    );
    assert!(
        logger
            .log(AuditEvent::new(EventType::System, "Info").with_level(LogLevel::Info))
            .await
            .is_ok()
    );

    // Warn and above should pass
    assert!(
        logger
            .log(AuditEvent::new(EventType::System, "Warn").with_level(LogLevel::Warn))
            .await
            .is_ok()
    );
    assert!(
        logger
            .log(AuditEvent::new(EventType::System, "Error").with_level(LogLevel::Error))
            .await
            .is_ok()
    );

    logger.flush().await.unwrap();
}

#[tokio::test]
async fn test_path_exclusion() {
    let config = AuditConfig::new().enable().exclude_path(r"/internal");

    let logger = AuditLogger::new(config).await.unwrap();

    assert!(!logger.should_log_path(r"/health"));
    assert!(!logger.should_log_path(r"/metrics"));
    assert!(!logger.should_log_path(r"/internal/status"));
    assert!(logger.should_log_path(r"/v1/chat/completions"));
}

#[tokio::test]
async fn test_memory_output_integration() {
    let output = Arc::new(MemoryOutput::new(100));

    // Write events directly to memory output
    for i in 0..10 {
        let event = AuditEvent::new(EventType::System, format!("Event {}", i));
        output.write(&event).await.unwrap();
    }

    assert_eq!(output.count().await, 10);

    let events = output.events().await;
    assert_eq!(events.len(), 10);
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = AuditConfig::new()
        .enable()
        .with_min_level(LogLevel::Debug)
        .with_file_output(r"./logs/audit.log")
        .with_request_body(true)
        .with_response_body(true)
        .with_retention_days(30);

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: AuditConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.enabled, deserialized.enabled);
    assert_eq!(config.min_level, deserialized.min_level);
    assert!(deserialized.file_output.is_some());
}

#[test]
fn test_yaml_config() {
    let yaml = r#"
enabled: true
min_level: info
log_requests: true
log_responses: true
log_request_body: false
log_response_body: false
max_body_size: 10240
exclude_paths:
  - /health
  - /metrics
  - /ready
file_output:
  path: ./logs/audit.log
  rotate: true
  max_file_size: 104857600
  max_backups: 10
buffer_size: 1000
flush_interval_ms: 1000
retention_days: 30
redact_sensitive: true
"#;

    let config: AuditConfig = serde_yml::from_str(yaml).unwrap();
    assert!(config.enabled);
    assert!(config.file_output.is_some());
    assert_eq!(config.retention_days, 30);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_disabled_logger() {
    let logger = AuditLogger::disabled();

    // Should not panic
    assert!(
        logger
            .log(AuditEvent::new(EventType::System, "Test"))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_high_volume_logging() {
    let config = AuditConfig::new().enable();
    let logger = AuditLoggerBuilder::new()
        .config(config)
        .add_output(Box::new(MemoryOutput::new(1000)))
        .build()
        .await
        .unwrap();

    // Log many events quickly
    for i in 0..1000 {
        let event = AuditEvent::new(EventType::System, format!("Event {}", i));
        assert!(logger.log(event).await.is_ok());
    }

    logger.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_logging() {
    let config = AuditConfig::new().enable();
    let logger = Arc::new(
        AuditLoggerBuilder::new()
            .config(config)
            .add_output(Box::new(MemoryOutput::new(1000)))
            .build()
            .await
            .unwrap(),
    );

    let mut handles = Vec::new();

    for i in 0..10 {
        let logger = logger.clone();
        let handle = tokio::spawn(async move {
            for j in 0..100 {
                let event = AuditEvent::new(EventType::System, format!("Thread {} Event {}", i, j));
                assert!(logger.log(event).await.is_ok());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    logger.shutdown().await.unwrap();
}

#[test]
fn test_event_serialization() {
    let request = RequestLog::new("req-1", "POST", r"/test")
        .with_client_ip("127.0.0.1")
        .with_body("{}", 2);

    let response = ResponseLog::new("req-1", 200, 100).with_body(r#"{"ok": true}"#, 12);

    let event = AuditEvent::request_completed("req-1", 200, 100)
        .with_request(request)
        .with_response(response)
        .with_user_id("user-1")
        .with_api_key_id("key-1")
        .with_metadata("custom", serde_json::json!({"key": "value"}));

    let json = event.to_json().unwrap();
    let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(event.request_id, deserialized.request_id);
    assert!(deserialized.request.is_some());
    assert!(deserialized.response.is_some());
}
