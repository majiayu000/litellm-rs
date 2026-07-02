use super::*;
use chrono::{TimeZone, Utc};

// ==================== RequestMetrics Tests ====================

#[test]
fn test_request_metrics_creation() {
    let now = Utc::now();
    let metrics = AnalyticsRequestMetrics {
        total_requests: 1000,
        successful_requests: 950,
        failed_requests: 50,
        avg_response_time_ms: 150.5,
        p95_response_time_ms: 300.0,
        p99_response_time_ms: 500.0,
        total_tokens: 500000,
        total_cost: 25.50,
        period_start: now,
        period_end: now + chrono::Duration::hours(24),
    };

    assert_eq!(metrics.total_requests, 1000);
    assert_eq!(metrics.successful_requests, 950);
    assert_eq!(metrics.failed_requests, 50);
    assert!((metrics.avg_response_time_ms - 150.5).abs() < f64::EPSILON);
}

#[test]
fn test_request_metrics_serialization() {
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let metrics = AnalyticsRequestMetrics {
        total_requests: 100,
        successful_requests: 95,
        failed_requests: 5,
        avg_response_time_ms: 100.0,
        p95_response_time_ms: 200.0,
        p99_response_time_ms: 350.0,
        total_tokens: 10000,
        total_cost: 5.0,
        period_start: now,
        period_end: now + chrono::Duration::hours(1),
    };

    let json = serde_json::to_string(&metrics).unwrap();
    let parsed: AnalyticsRequestMetrics = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.total_requests, metrics.total_requests);
    assert_eq!(parsed.successful_requests, metrics.successful_requests);
}

#[test]
fn test_request_metrics_success_rate_calculation() {
    let now = Utc::now();
    let metrics = AnalyticsRequestMetrics {
        total_requests: 1000,
        successful_requests: 950,
        failed_requests: 50,
        avg_response_time_ms: 100.0,
        p95_response_time_ms: 200.0,
        p99_response_time_ms: 300.0,
        total_tokens: 10000,
        total_cost: 5.0,
        period_start: now,
        period_end: now,
    };

    let success_rate = metrics.successful_requests as f64 / metrics.total_requests as f64;
    assert!((success_rate - 0.95).abs() < f64::EPSILON);
}

#[test]
fn test_request_metrics_clone() {
    let now = Utc::now();
    let metrics = AnalyticsRequestMetrics {
        total_requests: 500,
        successful_requests: 480,
        failed_requests: 20,
        avg_response_time_ms: 75.0,
        p95_response_time_ms: 150.0,
        p99_response_time_ms: 250.0,
        total_tokens: 25000,
        total_cost: 12.50,
        period_start: now,
        period_end: now,
    };

    let cloned = metrics.clone();
    assert_eq!(cloned.total_requests, metrics.total_requests);
    assert_eq!(cloned.total_cost, metrics.total_cost);
}

// ==================== ProviderMetrics Tests ====================

#[test]
fn test_provider_metrics_creation() {
    let metrics = ProviderMetrics {
        provider_name: "openai".to_string(),
        request_count: 5000,
        success_rate: 0.99,
        avg_latency_ms: 120.5,
        error_rate: 0.01,
        cost_efficiency: 1000.0,
        uptime_percentage: 99.95,
        rate_limit_hits: 15,
    };

    assert_eq!(metrics.provider_name, "openai");
    assert_eq!(metrics.request_count, 5000);
    assert!((metrics.success_rate - 0.99).abs() < f64::EPSILON);
}

#[test]
fn test_provider_metrics_serialization() {
    let metrics = ProviderMetrics {
        provider_name: "anthropic".to_string(),
        request_count: 3000,
        success_rate: 0.985,
        avg_latency_ms: 200.0,
        error_rate: 0.015,
        cost_efficiency: 800.0,
        uptime_percentage: 99.9,
        rate_limit_hits: 5,
    };

    let json = serde_json::to_string(&metrics).unwrap();
    assert!(json.contains("anthropic"));
    assert!(json.contains("3000"));

    let parsed: ProviderMetrics = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.provider_name, "anthropic");
}

#[test]
fn test_provider_metrics_high_performance() {
    let metrics = ProviderMetrics {
        provider_name: "fast-provider".to_string(),
        request_count: 100000,
        success_rate: 0.999,
        avg_latency_ms: 50.0,
        error_rate: 0.001,
        cost_efficiency: 2000.0,
        uptime_percentage: 99.99,
        rate_limit_hits: 0,
    };

    assert!(metrics.success_rate > 0.99);
    assert!(metrics.avg_latency_ms < 100.0);
    assert_eq!(metrics.rate_limit_hits, 0);
}
