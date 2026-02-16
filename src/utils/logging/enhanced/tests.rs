//! Tests for logging module

#[cfg(test)]
use crate::utils::logging::enhanced::async_logger::AsyncLogger;
use crate::utils::logging::enhanced::sampler::LogSampler;
use crate::utils::logging::enhanced::types::{
    AsyncLogRecord, AsyncLoggerConfig, HttpRequestMetrics,
};
use std::collections::HashMap;
use tracing::Level;
use uuid::Uuid;

// ==================== LogSampler Tests ====================

#[test]
fn test_log_sampler() {
    let mut sampler = LogSampler::new();
    sampler.set_sample_rate("test", 0.5);

    // Should sample approximately half the logs
    let mut sampled_count = 0;
    for _ in 0..1000 {
        if sampler.should_log("test") {
            sampled_count += 1;
        }
    }

    // Allow some variance due to sampling
    assert!(sampled_count > 400 && sampled_count < 600);
}

#[test]
fn test_log_sampler_edge_cases() {
    let mut sampler = LogSampler::new();

    // Test 100% sampling
    sampler.set_sample_rate("full", 1.0);
    let mut count = 0;
    for _ in 0..100 {
        if sampler.should_log("full") {
            count += 1;
        }
    }
    assert_eq!(count, 100);

    // Test 0% sampling
    sampler.set_sample_rate("none", 0.0);
    count = 0;
    for _ in 0..100 {
        if sampler.should_log("none") {
            count += 1;
        }
    }
    assert_eq!(count, 0);

    // Test 10% sampling
    sampler.set_sample_rate("ten_percent", 0.1);
    count = 0;
    for _ in 0..1000 {
        if sampler.should_log("ten_percent") {
            count += 1;
        }
    }
    // Should be exactly 100 (every 10th log)
    assert_eq!(count, 100);
}

#[test]
fn test_log_sampler_default() {
    let sampler = LogSampler::default();
    // Unknown category should always log
    assert!(sampler.should_log("unknown_category"));
}

#[test]
fn test_log_sampler_rate_clamping() {
    let mut sampler = LogSampler::new();

    // Rate > 1.0 should be clamped to 1.0
    sampler.set_sample_rate("high", 2.0);
    let mut count = 0;
    for _ in 0..100 {
        if sampler.should_log("high") {
            count += 1;
        }
    }
    assert_eq!(count, 100);

    // Rate < 0.0 should be clamped to 0.0
    sampler.set_sample_rate("low", -1.0);
    count = 0;
    for _ in 0..100 {
        if sampler.should_log("low") {
            count += 1;
        }
    }
    assert_eq!(count, 0);
}

#[test]
fn test_log_sampler_multiple_categories() {
    let mut sampler = LogSampler::new();
    sampler.set_sample_rate("cat_a", 1.0);
    sampler.set_sample_rate("cat_b", 0.0);

    assert!(sampler.should_log("cat_a"));
    assert!(!sampler.should_log("cat_b"));
}

// ==================== AsyncLoggerConfig Tests ====================

#[test]
fn test_async_logger_config() {
    let config = AsyncLoggerConfig {
        buffer_size: 5000,
        drop_on_overflow: true,
        sample_rate: 0.8,
        max_message_length: 512,
    };

    assert_eq!(config.buffer_size, 5000);
    assert!(config.drop_on_overflow);
    assert_eq!(config.sample_rate, 0.8);
    assert_eq!(config.max_message_length, 512);
}

#[test]
fn test_async_logger_config_default() {
    let config = AsyncLoggerConfig::default();

    assert_eq!(config.buffer_size, 10000);
    assert!(!config.drop_on_overflow);
    assert_eq!(config.sample_rate, 1.0);
    assert_eq!(config.max_message_length, 1024);
}

#[test]
fn test_async_logger_config_clone() {
    let config = AsyncLoggerConfig {
        buffer_size: 100,
        drop_on_overflow: true,
        sample_rate: 0.5,
        max_message_length: 256,
    };

    let cloned = config.clone();
    assert_eq!(cloned.buffer_size, config.buffer_size);
    assert_eq!(cloned.drop_on_overflow, config.drop_on_overflow);
    assert_eq!(cloned.sample_rate, config.sample_rate);
    assert_eq!(cloned.max_message_length, config.max_message_length);
}

// ==================== AsyncLogRecord Tests ====================

#[test]
fn test_log_entry_creation() {
    let mut fields = HashMap::new();
    fields.insert(
        "key".to_string(),
        serde_json::Value::String("value".to_string()),
    );

    let entry = AsyncLogRecord {
        timestamp: chrono::Utc::now(),
        level: "INFO".to_string(),
        logger: "test_logger".to_string(),
        message: "Test message".to_string(),
        fields,
        request_id: Some("req-123".to_string()),
        user_id: Some(Uuid::new_v4()),
        trace_id: Some("trace-456".to_string()),
    };

    assert_eq!(entry.level, "INFO");
    assert_eq!(entry.logger, "test_logger");
    assert_eq!(entry.message, "Test message");
    assert!(entry.fields.contains_key("key"));
    assert_eq!(entry.request_id, Some("req-123".to_string()));
    assert!(entry.user_id.is_some());
    assert_eq!(entry.trace_id, Some("trace-456".to_string()));
}

#[test]
fn test_log_entry_serialization() {
    let entry = AsyncLogRecord {
        timestamp: chrono::Utc::now(),
        level: "DEBUG".to_string(),
        logger: "test".to_string(),
        message: "Serializable".to_string(),
        fields: HashMap::new(),
        request_id: None,
        user_id: None,
        trace_id: None,
    };

    let json = serde_json::to_string(&entry);
    assert!(json.is_ok());
    assert!(json.unwrap().contains("Serializable"));
}

#[test]
fn test_log_entry_clone() {
    let entry = AsyncLogRecord {
        timestamp: chrono::Utc::now(),
        level: "WARN".to_string(),
        logger: "test".to_string(),
        message: "Clone me".to_string(),
        fields: HashMap::new(),
        request_id: Some("req-999".to_string()),
        user_id: None,
        trace_id: None,
    };

    let cloned = entry.clone();
    assert_eq!(cloned.level, entry.level);
    assert_eq!(cloned.message, entry.message);
    assert_eq!(cloned.request_id, entry.request_id);
}

// ==================== HttpRequestMetrics Tests ====================

#[test]
fn test_request_metrics_creation() {
    let metrics = HttpRequestMetrics {
        method: "POST".to_string(),
        path: "/api/v1/chat".to_string(),
        status_code: 200,
        duration_ms: 150,
        request_size: 1024,
        response_size: 2048,
        user_id: Some(Uuid::new_v4()),
        request_id: Some("req-abc".to_string()),
    };

    assert_eq!(metrics.method, "POST");
    assert_eq!(metrics.path, "/api/v1/chat");
    assert_eq!(metrics.status_code, 200);
    assert_eq!(metrics.duration_ms, 150);
    assert_eq!(metrics.request_size, 1024);
    assert_eq!(metrics.response_size, 2048);
    assert!(metrics.user_id.is_some());
    assert!(metrics.request_id.is_some());
}

#[test]
fn test_request_metrics_without_optional_fields() {
    let metrics = HttpRequestMetrics {
        method: "GET".to_string(),
        path: "/health".to_string(),
        status_code: 200,
        duration_ms: 5,
        request_size: 0,
        response_size: 50,
        user_id: None,
        request_id: None,
    };

    assert!(metrics.user_id.is_none());
    assert!(metrics.request_id.is_none());
}

#[test]
fn test_request_metrics_debug() {
    let metrics = HttpRequestMetrics {
        method: "DELETE".to_string(),
        path: "/api/resource/123".to_string(),
        status_code: 404,
        duration_ms: 25,
        request_size: 0,
        response_size: 100,
        user_id: None,
        request_id: None,
    };

    let debug_str = format!("{:?}", metrics);
    assert!(debug_str.contains("DELETE"));
    assert!(debug_str.contains("404"));
}

// ==================== AsyncLogger Tests ====================

#[tokio::test]
async fn test_async_logger_creation() {
    let config = AsyncLoggerConfig::default();
    let logger = AsyncLogger::new(config);

    // Test basic logging
    logger.log(Level::INFO, "test", "test message");

    // Give background task time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
}

#[tokio::test]
async fn test_async_logger_bounded_channel() {
    // Create logger with small buffer to test backpressure
    let config = AsyncLoggerConfig {
        buffer_size: 10,
        drop_on_overflow: true,
        sample_rate: 1.0,
        max_message_length: 100,
    };
    let logger = AsyncLogger::new(config);

    // Send more messages than buffer can hold
    for i in 0..100 {
        logger.log(Level::INFO, "test", &format!("message {}", i));
    }

    // Should not panic or hang - messages are dropped when buffer full
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_async_logger_sampling() {
    let config = AsyncLoggerConfig {
        buffer_size: 1000,
        drop_on_overflow: false,
        sample_rate: 0.5, // 50% sampling
        max_message_length: 100,
    };
    let logger = AsyncLogger::new(config);

    // The sampling counter is internal, so we just verify no panic
    for i in 0..100 {
        logger.log(Level::INFO, "test", &format!("sampled message {}", i));
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_async_logger_all_levels() {
    let config = AsyncLoggerConfig::default();
    let logger = AsyncLogger::new(config);

    logger.log(Level::DEBUG, "test", "debug message");
    logger.log(Level::INFO, "test", "info message");
    logger.log(Level::WARN, "test", "warn message");
    logger.log(Level::ERROR, "test", "error message");
    logger.log(Level::TRACE, "test", "trace message");

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
}

#[tokio::test]
async fn test_async_logger_long_message() {
    let config = AsyncLoggerConfig {
        buffer_size: 100,
        drop_on_overflow: true,
        sample_rate: 1.0,
        max_message_length: 50,
    };
    let logger = AsyncLogger::new(config);

    let long_message = "A".repeat(1000);
    logger.log(Level::INFO, "test", &long_message);

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
}

#[tokio::test]
async fn test_async_logger_empty_message() {
    let config = AsyncLoggerConfig::default();
    let logger = AsyncLogger::new(config);

    logger.log(Level::INFO, "test", "");

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
}

#[tokio::test]
async fn test_async_logger_unicode_message() {
    let config = AsyncLoggerConfig::default();
    let logger = AsyncLogger::new(config);

    logger.log(Level::INFO, "test", "Hello 你好 🌍 مرحبا");

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
}
