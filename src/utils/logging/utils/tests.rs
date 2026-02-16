#[cfg(test)]
use super::core::LoggingUtils;
use super::file_logging::FileLogging;
use super::logger::Logger;
use super::sanitization::Sanitization;
use super::types::{LogEntry, LogLevel};
use std::collections::HashMap;
use tempfile::NamedTempFile;

// ==================== LogLevel Tests ====================

#[test]
fn test_log_level_from_string() {
    assert_eq!("DEBUG".parse::<LogLevel>().unwrap(), LogLevel::Debug);
    assert_eq!("INFO".parse::<LogLevel>().unwrap(), LogLevel::Info);
    assert_eq!("WARN".parse::<LogLevel>().unwrap(), LogLevel::Warn);
    assert_eq!("ERROR".parse::<LogLevel>().unwrap(), LogLevel::Error);
    assert!("INVALID".parse::<LogLevel>().is_err());
}

#[test]
fn test_log_level_case_insensitive() {
    assert_eq!("debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
    assert_eq!("Debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
    assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
    assert_eq!("Info".parse::<LogLevel>().unwrap(), LogLevel::Info);
}

#[test]
fn test_log_level_warning_alias() {
    assert_eq!("WARNING".parse::<LogLevel>().unwrap(), LogLevel::Warn);
    assert_eq!("warning".parse::<LogLevel>().unwrap(), LogLevel::Warn);
}

#[test]
fn test_log_level_to_tracing_level() {
    let debug: tracing::Level = LogLevel::Debug.into();
    assert_eq!(debug, tracing::Level::DEBUG);

    let info: tracing::Level = LogLevel::Info.into();
    assert_eq!(info, tracing::Level::INFO);

    let warn: tracing::Level = LogLevel::Warn.into();
    assert_eq!(warn, tracing::Level::WARN);

    let error: tracing::Level = LogLevel::Error.into();
    assert_eq!(error, tracing::Level::ERROR);
}

#[test]
fn test_log_level_equality() {
    assert_eq!(LogLevel::Debug, LogLevel::Debug);
    assert_ne!(LogLevel::Debug, LogLevel::Info);
    assert_eq!(LogLevel::Error, LogLevel::Error);
}

#[test]
fn test_log_level_clone() {
    let level = LogLevel::Warn;
    let cloned = level.clone();
    assert_eq!(level, cloned);
}

// ==================== LogEntry Tests ====================

#[test]
fn test_log_entry_creation() {
    let entry = LogEntry::new(LogLevel::Info, "Test message".to_string())
        .with_module("test_module".to_string())
        .with_request_id("req-123".to_string())
        .with_metadata(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );

    assert_eq!(entry.level, "INFO");
    assert_eq!(entry.message, "Test message");
    assert_eq!(entry.module, Some("test_module".to_string()));
    assert_eq!(entry.request_id, Some("req-123".to_string()));
    assert!(entry.metadata.contains_key("key"));
}

#[test]
fn test_log_entry_minimal() {
    let entry = LogEntry::new(LogLevel::Debug, "Minimal".to_string());

    assert_eq!(entry.level, "DEBUG");
    assert_eq!(entry.message, "Minimal");
    assert!(entry.module.is_none());
    assert!(entry.request_id.is_none());
    assert!(entry.metadata.is_empty());
}

#[test]
fn test_log_entry_multiple_metadata() {
    let entry = LogEntry::new(LogLevel::Info, "Test".to_string())
        .with_metadata("key1".to_string(), serde_json::json!("value1"))
        .with_metadata("key2".to_string(), serde_json::json!(42))
        .with_metadata("key3".to_string(), serde_json::json!(true));

    assert_eq!(entry.metadata.len(), 3);
    assert_eq!(
        entry.metadata.get("key1").unwrap(),
        &serde_json::json!("value1")
    );
    assert_eq!(entry.metadata.get("key2").unwrap(), &serde_json::json!(42));
    assert_eq!(
        entry.metadata.get("key3").unwrap(),
        &serde_json::json!(true)
    );
}

#[test]
fn test_log_entry_serialization() {
    let entry =
        LogEntry::new(LogLevel::Error, "Error occurred".to_string()).with_module("api".to_string());

    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("ERROR"));
    assert!(json.contains("Error occurred"));
    assert!(json.contains("api"));
}

#[test]
fn test_log_entry_timestamp_exists() {
    let before = chrono::Utc::now();
    let entry = LogEntry::new(LogLevel::Info, "Test".to_string());
    let after = chrono::Utc::now();

    assert!(entry.timestamp >= before);
    assert!(entry.timestamp <= after);
}

// ==================== LoggingUtils Tests ====================

#[test]
fn test_should_log_at_level() {
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Debug,
        &LogLevel::Error
    ));
    assert!(!LoggingUtils::should_log_at_level(
        &LogLevel::Error,
        &LogLevel::Debug
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Info,
        &LogLevel::Info
    ));
}

#[test]
fn test_should_log_at_level_all_combinations() {
    // Debug level allows all
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Debug,
        &LogLevel::Debug
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Debug,
        &LogLevel::Info
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Debug,
        &LogLevel::Warn
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Debug,
        &LogLevel::Error
    ));

    // Info level allows info and above
    assert!(!LoggingUtils::should_log_at_level(
        &LogLevel::Info,
        &LogLevel::Debug
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Info,
        &LogLevel::Info
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Info,
        &LogLevel::Warn
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Info,
        &LogLevel::Error
    ));

    // Warn level allows warn and above
    assert!(!LoggingUtils::should_log_at_level(
        &LogLevel::Warn,
        &LogLevel::Debug
    ));
    assert!(!LoggingUtils::should_log_at_level(
        &LogLevel::Warn,
        &LogLevel::Info
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Warn,
        &LogLevel::Warn
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Warn,
        &LogLevel::Error
    ));

    // Error level only allows error
    assert!(!LoggingUtils::should_log_at_level(
        &LogLevel::Error,
        &LogLevel::Debug
    ));
    assert!(!LoggingUtils::should_log_at_level(
        &LogLevel::Error,
        &LogLevel::Info
    ));
    assert!(!LoggingUtils::should_log_at_level(
        &LogLevel::Error,
        &LogLevel::Warn
    ));
    assert!(LoggingUtils::should_log_at_level(
        &LogLevel::Error,
        &LogLevel::Error
    ));
}

#[test]
fn test_format_duration() {
    assert_eq!(
        LoggingUtils::format_duration(std::time::Duration::from_millis(500)),
        "500ms"
    );
    assert_eq!(
        LoggingUtils::format_duration(std::time::Duration::from_secs(2)),
        "2.00s"
    );
    assert!(LoggingUtils::format_duration(std::time::Duration::from_secs(65)).contains("1m"));
}

#[test]
fn test_format_duration_edge_cases() {
    assert_eq!(
        LoggingUtils::format_duration(std::time::Duration::from_millis(0)),
        "0ms"
    );
    assert_eq!(
        LoggingUtils::format_duration(std::time::Duration::from_millis(999)),
        "999ms"
    );
    assert_eq!(
        LoggingUtils::format_duration(std::time::Duration::from_millis(1000)),
        "1.00s"
    );
    assert_eq!(
        LoggingUtils::format_duration(std::time::Duration::from_secs(59)),
        "59.00s"
    );
}

#[test]
fn test_format_duration_minutes() {
    let duration = std::time::Duration::from_secs(120);
    let formatted = LoggingUtils::format_duration(duration);
    assert!(formatted.contains("2m"));
}

#[test]
fn test_logging_id_generation() {
    let id1 = LoggingUtils::get_logging_id();
    let id2 = LoggingUtils::get_logging_id();
    assert_ne!(id1, id2);
    assert!(uuid::Uuid::parse_str(&id1).is_ok());
}

#[test]
fn test_logging_id_uniqueness() {
    let mut ids = std::collections::HashSet::new();
    for _ in 0..100 {
        let id = LoggingUtils::get_logging_id();
        assert!(ids.insert(id), "Logging IDs should be unique");
    }
}

#[test]
fn test_logging_id_with_timestamp() {
    let start_time = chrono::Utc::now();
    let id = LoggingUtils::get_logging_id_with_timestamp(start_time);

    // Should contain timestamp and UUID separated by hyphen
    assert!(id.contains('-'));
    let parts: Vec<&str> = id.splitn(2, '-').collect();
    assert_eq!(parts.len(), 2);

    // First part should be parseable as timestamp
    let timestamp_str = parts[0];
    assert!(timestamp_str.parse::<i64>().is_ok());
}

#[test]
fn test_create_structured_log() {
    let entry = LoggingUtils::create_structured_log(
        LogLevel::Info,
        "Test message",
        Some("test_module"),
        None,
    );

    assert_eq!(entry.level, "INFO");
    assert_eq!(entry.message, "Test message");
    assert_eq!(entry.module, Some("test_module".to_string()));
}

#[test]
fn test_create_structured_log_with_metadata() {
    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), serde_json::json!("user123"));
    metadata.insert("action".to_string(), serde_json::json!("login"));

    let entry = LoggingUtils::create_structured_log(
        LogLevel::Warn,
        "Warning message",
        None,
        Some(metadata),
    );

    assert_eq!(entry.level, "WARN");
    assert!(entry.metadata.contains_key("user_id"));
    assert!(entry.metadata.contains_key("action"));
}

#[test]
fn test_create_structured_log_minimal() {
    let entry = LoggingUtils::create_structured_log(LogLevel::Debug, "Debug", None, None);

    assert_eq!(entry.level, "DEBUG");
    assert!(entry.module.is_none());
    assert!(entry.metadata.is_empty());
}

// ==================== Sanitization Tests ====================

#[test]
fn test_mask_sensitive_data() {
    let input = r#"{"api_key": "sk-1234567890", "model": "gpt-4"}"#;
    let masked = Sanitization::mask_sensitive_data(input);
    assert!(!masked.contains("sk-1234567890"));
    assert!(masked.contains("sk***90") || masked.contains("***"));
}

#[test]
fn test_sanitize_log_data() {
    let input = "API_KEY=sk-1234567890 model=gpt-4";
    let sanitized = Sanitization::sanitize_log_data(input);
    assert!(!sanitized.contains("sk-1234567890"));
}

#[test]
fn test_mask_sensitive_data_multiple_keys() {
    let input = r#"{"api_key": "sk-abc123", "token": "tok-xyz789", "password": "secret123"}"#;
    let masked = Sanitization::mask_sensitive_data(input);

    assert!(!masked.contains("sk-abc123"));
    assert!(!masked.contains("tok-xyz789"));
    assert!(!masked.contains("secret123"));
}

#[test]
fn test_mask_sensitive_data_no_sensitive() {
    let input = r#"{"model": "gpt-4", "temperature": 0.7}"#;
    let masked = Sanitization::mask_sensitive_data(input);
    assert!(masked.contains("gpt-4"));
    assert!(masked.contains("0.7"));
}

#[test]
fn test_mask_sensitive_data_short_value() {
    let input = r#"{"api_key": "short"}"#;
    let masked = Sanitization::mask_sensitive_data(input);
    assert!(!masked.contains("short") || masked.contains("***"));
}

#[test]
fn test_sanitize_log_data_token() {
    let input = "token=my-secret-token other=value";
    let sanitized = Sanitization::sanitize_log_data(input);
    assert!(!sanitized.contains("my-secret-token"));
}

#[test]
fn test_sanitize_log_data_password() {
    let input = "password=super_secret123";
    let sanitized = Sanitization::sanitize_log_data(input);
    assert!(!sanitized.contains("super_secret123"));
}

#[test]
fn test_sanitize_log_data_secret() {
    let input = "SECRET=my_secret_value";
    let sanitized = Sanitization::sanitize_log_data(input);
    assert!(!sanitized.contains("my_secret_value"));
}

#[test]
fn test_sanitize_log_data_empty() {
    let input = "";
    let sanitized = Sanitization::sanitize_log_data(input);
    assert_eq!(sanitized, "");
}

#[test]
fn test_mask_sensitive_data_empty() {
    let input = "";
    let masked = Sanitization::mask_sensitive_data(input);
    assert_eq!(masked, "");
}

// ==================== FileLogging Tests ====================

#[test]
fn test_file_logging() {
    let temp_file = NamedTempFile::new().unwrap();
    let writer = FileLogging::setup_file_logging(temp_file.path().to_str().unwrap()).unwrap();

    let entry = LogEntry::new(LogLevel::Info, "Test log entry".to_string());
    assert!(FileLogging::log_to_file(&writer, &entry).is_ok());
}

#[test]
fn test_file_logging_multiple_entries() {
    let temp_file = NamedTempFile::new().unwrap();
    let writer = FileLogging::setup_file_logging(temp_file.path().to_str().unwrap()).unwrap();

    for i in 0..10 {
        let entry = LogEntry::new(LogLevel::Info, format!("Log entry {}", i));
        assert!(FileLogging::log_to_file(&writer, &entry).is_ok());
    }
}

#[test]
fn test_file_logging_all_levels() {
    let temp_file = NamedTempFile::new().unwrap();
    let writer = FileLogging::setup_file_logging(temp_file.path().to_str().unwrap()).unwrap();

    let levels = vec![
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
    ];
    for level in levels {
        let entry = LogEntry::new(level, "Test message".to_string());
        assert!(FileLogging::log_to_file(&writer, &entry).is_ok());
    }
}

#[test]
fn test_file_logging_with_metadata() {
    let temp_file = NamedTempFile::new().unwrap();
    let writer = FileLogging::setup_file_logging(temp_file.path().to_str().unwrap()).unwrap();

    let entry = LogEntry::new(LogLevel::Info, "With metadata".to_string())
        .with_metadata("key".to_string(), serde_json::json!("value"))
        .with_request_id("req-456".to_string());

    assert!(FileLogging::log_to_file(&writer, &entry).is_ok());
}

// ==================== Logger Tests ====================

#[test]
fn test_logger_creation() {
    let logger = Logger::new(LogLevel::Info);
    logger.info("Test info message");
    logger.debug("Test debug message"); // Should not be logged due to level
}

#[test]
fn test_logger_all_log_methods() {
    let logger = Logger::new(LogLevel::Debug);
    logger.debug("Debug message");
    logger.info("Info message");
    logger.warn("Warn message");
    logger.error("Error message");
}

#[test]
fn test_logger_with_context() {
    let logger = Logger::new(LogLevel::Debug);
    let mut context = HashMap::new();
    context.insert("request_id".to_string(), "req-123".to_string());
    context.insert("user_id".to_string(), "user-456".to_string());

    logger.log(LogLevel::Info, "Message with context", Some(context));
}

#[test]
fn test_logger_level_filtering() {
    let logger = Logger::new(LogLevel::Warn);

    // These should be filtered out
    logger.debug("Should not log");
    logger.info("Should not log");

    // These should log
    logger.warn("Should log");
    logger.error("Should log");
}

#[test]
fn test_logger_error_level_only() {
    let logger = Logger::new(LogLevel::Error);

    logger.debug("Filtered");
    logger.info("Filtered");
    logger.warn("Filtered");
    logger.error("Only this should log");
}
