use super::*;
use chrono::Utc;
use std::collections::HashMap;

// ==================== RequestMetrics Tests ====================

#[test]
fn test_request_metrics_creation() {
    let metrics = MonitoringRequestMetrics {
        total_requests: 1000,
        requests_per_second: 50.5,
        avg_response_time_ms: 120.0,
        p95_response_time_ms: 250.0,
        p99_response_time_ms: 500.0,
        success_rate: 98.5,
        status_codes: HashMap::new(),
        endpoints: HashMap::new(),
    };

    assert_eq!(metrics.total_requests, 1000);
    assert_eq!(metrics.requests_per_second, 50.5);
    assert_eq!(metrics.success_rate, 98.5);
}

#[test]
fn test_request_metrics_with_status_codes() {
    let mut status_codes = HashMap::new();
    status_codes.insert(200, 950u64);
    status_codes.insert(400, 30u64);
    status_codes.insert(500, 20u64);

    let metrics = MonitoringRequestMetrics {
        total_requests: 1000,
        requests_per_second: 10.0,
        avg_response_time_ms: 100.0,
        p95_response_time_ms: 200.0,
        p99_response_time_ms: 300.0,
        success_rate: 95.0,
        status_codes,
        endpoints: HashMap::new(),
    };

    assert_eq!(metrics.status_codes.get(&200), Some(&950));
    assert_eq!(metrics.status_codes.get(&500), Some(&20));
}

#[test]
fn test_request_metrics_clone() {
    let metrics = MonitoringRequestMetrics {
        total_requests: 500,
        requests_per_second: 25.0,
        avg_response_time_ms: 80.0,
        p95_response_time_ms: 150.0,
        p99_response_time_ms: 200.0,
        success_rate: 99.0,
        status_codes: HashMap::new(),
        endpoints: HashMap::new(),
    };

    let cloned = metrics.clone();
    assert_eq!(metrics.total_requests, cloned.total_requests);
    assert_eq!(metrics.success_rate, cloned.success_rate);
}

#[test]
fn test_request_metrics_serialization() {
    let metrics = MonitoringRequestMetrics {
        total_requests: 100,
        requests_per_second: 5.0,
        avg_response_time_ms: 50.0,
        p95_response_time_ms: 100.0,
        p99_response_time_ms: 150.0,
        success_rate: 100.0,
        status_codes: HashMap::new(),
        endpoints: HashMap::new(),
    };

    let json = serde_json::to_value(&metrics).unwrap();
    assert_eq!(json["total_requests"], 100);
    assert_eq!(json["requests_per_second"], 5.0);
}

// ==================== ProviderMetrics Tests ====================

#[test]
fn test_provider_metrics_creation() {
    let metrics = ProviderMetrics {
        total_provider_requests: 5000,
        provider_success_rates: HashMap::new(),
        provider_response_times: HashMap::new(),
        provider_errors: HashMap::new(),
        provider_usage: HashMap::new(),
        token_usage: HashMap::new(),
        costs: HashMap::new(),
    };

    assert_eq!(metrics.total_provider_requests, 5000);
}

#[test]
fn test_provider_metrics_with_data() {
    let mut success_rates = HashMap::new();
    success_rates.insert("openai".to_string(), 99.5);
    success_rates.insert("anthropic".to_string(), 98.0);

    let mut costs = HashMap::new();
    costs.insert("openai".to_string(), 150.25);
    costs.insert("anthropic".to_string(), 75.50);

    let metrics = ProviderMetrics {
        total_provider_requests: 10000,
        provider_success_rates: success_rates,
        provider_response_times: HashMap::new(),
        provider_errors: HashMap::new(),
        provider_usage: HashMap::new(),
        token_usage: HashMap::new(),
        costs,
    };

    assert_eq!(metrics.provider_success_rates.get("openai"), Some(&99.5));
    assert_eq!(metrics.costs.get("anthropic"), Some(&75.50));
}

#[test]
fn test_provider_metrics_serialization() {
    let mut usage = HashMap::new();
    usage.insert("openai".to_string(), 3000u64);

    let metrics = ProviderMetrics {
        total_provider_requests: 3000,
        provider_success_rates: HashMap::new(),
        provider_response_times: HashMap::new(),
        provider_errors: HashMap::new(),
        provider_usage: usage,
        token_usage: HashMap::new(),
        costs: HashMap::new(),
    };

    let json = serde_json::to_value(&metrics).unwrap();
    assert_eq!(json["total_provider_requests"], 3000);
    assert_eq!(json["provider_usage"]["openai"], 3000);
}

// ==================== SystemResourceMetrics Tests ====================

#[test]
fn test_system_resource_metrics_creation() {
    let metrics = SystemResourceMetrics {
        cpu_usage: 45.5,
        memory_usage: 4_000_000_000,
        memory_usage_percent: 50.0,
        disk_usage: 100_000_000_000,
        disk_usage_percent: 25.0,
        network_bytes_in: 1_000_000,
        network_bytes_out: 500_000,
        active_connections: 150,
        database_connections: 20,
        redis_connections: 10,
    };

    assert_eq!(metrics.cpu_usage, 45.5);
    assert_eq!(metrics.memory_usage, 4_000_000_000);
    assert_eq!(metrics.active_connections, 150);
}

#[test]
fn test_system_resource_metrics_high_usage() {
    let metrics = SystemResourceMetrics {
        cpu_usage: 95.0,
        memory_usage: 15_000_000_000,
        memory_usage_percent: 95.0,
        disk_usage: 900_000_000_000,
        disk_usage_percent: 90.0,
        network_bytes_in: 100_000_000,
        network_bytes_out: 50_000_000,
        active_connections: 1000,
        database_connections: 100,
        redis_connections: 50,
    };

    assert!(metrics.cpu_usage > 90.0);
    assert!(metrics.memory_usage_percent > 90.0);
}

#[test]
fn test_system_resource_metrics_serialization() {
    let metrics = SystemResourceMetrics {
        cpu_usage: 30.0,
        memory_usage: 2_000_000_000,
        memory_usage_percent: 25.0,
        disk_usage: 50_000_000_000,
        disk_usage_percent: 10.0,
        network_bytes_in: 500_000,
        network_bytes_out: 250_000,
        active_connections: 50,
        database_connections: 10,
        redis_connections: 5,
    };

    let json = serde_json::to_value(&metrics).unwrap();
    assert_eq!(json["cpu_usage"], 30.0);
    assert_eq!(json["active_connections"], 50);
}

// ==================== ErrorMetrics Tests ====================

#[test]
fn test_error_metrics_creation() {
    let metrics = ErrorMetrics {
        total_errors: 50,
        error_rate: 0.5,
        error_types: HashMap::new(),
        error_endpoints: HashMap::new(),
        critical_errors: 5,
        warnings: 20,
    };

    assert_eq!(metrics.total_errors, 50);
    assert_eq!(metrics.critical_errors, 5);
    assert_eq!(metrics.warnings, 20);
}

#[test]
fn test_error_metrics_with_types() {
    let mut error_types = HashMap::new();
    error_types.insert("timeout".to_string(), 10u64);
    error_types.insert("rate_limit".to_string(), 25u64);
    error_types.insert("validation".to_string(), 15u64);

    let metrics = ErrorMetrics {
        total_errors: 50,
        error_rate: 0.5,
        error_types,
        error_endpoints: HashMap::new(),
        critical_errors: 2,
        warnings: 10,
    };

    assert_eq!(metrics.error_types.get("timeout"), Some(&10));
    assert_eq!(metrics.error_types.get("rate_limit"), Some(&25));
}

#[test]
fn test_error_metrics_serialization() {
    let metrics = ErrorMetrics {
        total_errors: 100,
        error_rate: 1.0,
        error_types: HashMap::new(),
        error_endpoints: HashMap::new(),
        critical_errors: 10,
        warnings: 30,
    };

    let json = serde_json::to_value(&metrics).unwrap();
    assert_eq!(json["total_errors"], 100);
    assert_eq!(json["critical_errors"], 10);
}

// ==================== PerformanceMetrics Tests ====================

#[test]
fn test_performance_metrics_creation() {
    let metrics = PerformanceMetrics {
        cache_hit_rate: 85.0,
        cache_miss_rate: 15.0,
        avg_db_query_time_ms: 5.5,
        queue_depth: 100,
        throughput: 1000.0,
        latency_percentiles: LatencyPercentiles {
            p50: 10.0,
            p90: 50.0,
            p95: 100.0,
            p99: 200.0,
            p999: 500.0,
        },
    };

    assert_eq!(metrics.cache_hit_rate, 85.0);
    assert_eq!(metrics.throughput, 1000.0);
}

#[test]
fn test_performance_metrics_serialization() {
    let metrics = PerformanceMetrics {
        cache_hit_rate: 90.0,
        cache_miss_rate: 10.0,
        avg_db_query_time_ms: 3.0,
        queue_depth: 50,
        throughput: 500.0,
        latency_percentiles: LatencyPercentiles {
            p50: 5.0,
            p90: 25.0,
            p95: 50.0,
            p99: 100.0,
            p999: 250.0,
        },
    };

    let json = serde_json::to_value(&metrics).unwrap();
    assert_eq!(json["cache_hit_rate"], 90.0);
    assert_eq!(json["latency_percentiles"]["p50"], 5.0);
    assert_eq!(json["latency_percentiles"]["p99"], 100.0);
}

// ==================== LatencyPercentiles Tests ====================

#[test]
fn test_latency_percentiles_creation() {
    let percentiles = LatencyPercentiles {
        p50: 10.0,
        p90: 50.0,
        p95: 100.0,
        p99: 200.0,
        p999: 500.0,
    };

    assert_eq!(percentiles.p50, 10.0);
    assert_eq!(percentiles.p99, 200.0);
}

#[test]
fn test_latency_percentiles_ordering() {
    let percentiles = LatencyPercentiles {
        p50: 10.0,
        p90: 50.0,
        p95: 100.0,
        p99: 200.0,
        p999: 500.0,
    };

    // Higher percentiles should have higher values
    assert!(percentiles.p50 <= percentiles.p90);
    assert!(percentiles.p90 <= percentiles.p95);
    assert!(percentiles.p95 <= percentiles.p99);
    assert!(percentiles.p99 <= percentiles.p999);
}

#[test]
fn test_latency_percentiles_clone() {
    let percentiles = LatencyPercentiles {
        p50: 5.0,
        p90: 25.0,
        p95: 50.0,
        p99: 100.0,
        p999: 200.0,
    };

    let cloned = percentiles.clone();
    assert_eq!(percentiles.p50, cloned.p50);
    assert_eq!(percentiles.p999, cloned.p999);
}

#[test]
fn test_latency_percentiles_serialization() {
    let percentiles = LatencyPercentiles {
        p50: 15.0,
        p90: 75.0,
        p95: 150.0,
        p99: 300.0,
        p999: 750.0,
    };

    let json = serde_json::to_value(&percentiles).unwrap();
    assert_eq!(json["p50"], 15.0);
    assert_eq!(json["p90"], 75.0);
    assert_eq!(json["p95"], 150.0);
}

// ==================== AlertSeverity Tests ====================

#[test]
fn test_alert_severity_variants() {
    assert_eq!(AlertSeverity::Info.to_string(), "INFO");
    assert_eq!(AlertSeverity::Warning.to_string(), "WARNING");
    assert_eq!(AlertSeverity::Critical.to_string(), "CRITICAL");
    assert_eq!(AlertSeverity::Emergency.to_string(), "EMERGENCY");
}

#[test]
fn test_alert_severity_equality() {
    assert_eq!(AlertSeverity::Info, AlertSeverity::Info);
    assert_ne!(AlertSeverity::Info, AlertSeverity::Warning);
    assert_ne!(AlertSeverity::Critical, AlertSeverity::Emergency);
}

#[test]
fn test_alert_severity_clone() {
    let severity = AlertSeverity::Critical;
    let cloned = severity;
    assert_eq!(severity, cloned);
}

#[test]
fn test_alert_severity_serialization() {
    let severity = AlertSeverity::Warning;
    let json = serde_json::to_value(severity).unwrap();
    assert_eq!(json, "Warning");
}

#[test]
fn test_alert_severity_deserialization() {
    let json = "\"Critical\"";
    let severity: AlertSeverity = serde_json::from_str(json).unwrap();
    assert_eq!(severity, AlertSeverity::Critical);
}

#[test]
fn test_alert_severity_all_variants_serialization() {
    let variants = vec![
        (AlertSeverity::Info, "Info"),
        (AlertSeverity::Warning, "Warning"),
        (AlertSeverity::Critical, "Critical"),
        (AlertSeverity::Emergency, "Emergency"),
    ];

    for (severity, expected) in variants {
        let json = serde_json::to_value(severity).unwrap();
        assert_eq!(json.as_str().unwrap(), expected);
    }
}

// ==================== Alert Tests ====================

#[test]
fn test_alert_creation() {
    let alert = Alert {
        id: "alert-123".to_string(),
        severity: AlertSeverity::Critical,
        title: "High CPU Usage".to_string(),
        description: "CPU usage exceeds 90%".to_string(),
        timestamp: Utc::now(),
        source: "monitoring".to_string(),
        metadata: serde_json::json!({"cpu": 95.0}),
        resolved: false,
    };

    assert_eq!(alert.id, "alert-123");
    assert_eq!(alert.severity, AlertSeverity::Critical);
    assert!(!alert.resolved);
}

#[test]
fn test_alert_resolved() {
    let alert = Alert {
        id: "alert-456".to_string(),
        severity: AlertSeverity::Warning,
        title: "Memory Warning".to_string(),
        description: "Memory usage above threshold".to_string(),
        timestamp: Utc::now(),
        source: "system".to_string(),
        metadata: serde_json::json!({}),
        resolved: true,
    };

    assert!(alert.resolved);
}

#[test]
fn test_alert_clone() {
    let alert = Alert {
        id: "clone-test".to_string(),
        severity: AlertSeverity::Info,
        title: "Test Alert".to_string(),
        description: "Test description".to_string(),
        timestamp: Utc::now(),
        source: "test".to_string(),
        metadata: serde_json::json!(null),
        resolved: false,
    };

    let cloned = alert.clone();
    assert_eq!(alert.id, cloned.id);
    assert_eq!(alert.severity, cloned.severity);
    assert_eq!(alert.title, cloned.title);
}

#[test]
fn test_alert_serialization() {
    let alert = Alert {
        id: "ser-test".to_string(),
        severity: AlertSeverity::Emergency,
        title: "Emergency Alert".to_string(),
        description: "System down".to_string(),
        timestamp: Utc::now(),
        source: "heartbeat".to_string(),
        metadata: serde_json::json!({"service": "api"}),
        resolved: false,
    };

    let json = serde_json::to_value(&alert).unwrap();
    assert_eq!(json["id"], "ser-test");
    assert_eq!(json["severity"], "Emergency");
    assert_eq!(json["title"], "Emergency Alert");
    assert_eq!(json["resolved"], false);
}

// ==================== SystemMetrics Tests ====================

#[test]
fn test_system_metrics_creation() {
    let metrics = SystemMetrics {
        timestamp: Utc::now(),
        requests: MonitoringRequestMetrics {
            total_requests: 1000,
            requests_per_second: 10.0,
            avg_response_time_ms: 50.0,
            p95_response_time_ms: 100.0,
            p99_response_time_ms: 150.0,
            success_rate: 99.0,
            status_codes: HashMap::new(),
            endpoints: HashMap::new(),
        },
        providers: ProviderMetrics {
            total_provider_requests: 1000,
            provider_success_rates: HashMap::new(),
            provider_response_times: HashMap::new(),
            provider_errors: HashMap::new(),
            provider_usage: HashMap::new(),
            token_usage: HashMap::new(),
            costs: HashMap::new(),
        },
        system: SystemResourceMetrics {
            cpu_usage: 50.0,
            memory_usage: 4_000_000_000,
            memory_usage_percent: 50.0,
            disk_usage: 100_000_000_000,
            disk_usage_percent: 25.0,
            network_bytes_in: 1_000_000,
            network_bytes_out: 500_000,
            active_connections: 100,
            database_connections: 20,
            redis_connections: 10,
        },
        errors: ErrorMetrics {
            total_errors: 10,
            error_rate: 0.01,
            error_types: HashMap::new(),
            error_endpoints: HashMap::new(),
            critical_errors: 0,
            warnings: 5,
        },
        performance: PerformanceMetrics {
            cache_hit_rate: 90.0,
            cache_miss_rate: 10.0,
            avg_db_query_time_ms: 5.0,
            queue_depth: 50,
            throughput: 100.0,
            latency_percentiles: LatencyPercentiles {
                p50: 10.0,
                p90: 40.0,
                p95: 80.0,
                p99: 150.0,
                p999: 300.0,
            },
        },
    };

    assert_eq!(metrics.requests.total_requests, 1000);
    assert_eq!(metrics.system.cpu_usage, 50.0);
    assert_eq!(metrics.errors.total_errors, 10);
}

#[test]
fn test_system_metrics_clone() {
    let metrics = SystemMetrics {
        timestamp: Utc::now(),
        requests: MonitoringRequestMetrics {
            total_requests: 500,
            requests_per_second: 5.0,
            avg_response_time_ms: 30.0,
            p95_response_time_ms: 60.0,
            p99_response_time_ms: 90.0,
            success_rate: 100.0,
            status_codes: HashMap::new(),
            endpoints: HashMap::new(),
        },
        providers: ProviderMetrics {
            total_provider_requests: 500,
            provider_success_rates: HashMap::new(),
            provider_response_times: HashMap::new(),
            provider_errors: HashMap::new(),
            provider_usage: HashMap::new(),
            token_usage: HashMap::new(),
            costs: HashMap::new(),
        },
        system: SystemResourceMetrics {
            cpu_usage: 25.0,
            memory_usage: 2_000_000_000,
            memory_usage_percent: 25.0,
            disk_usage: 50_000_000_000,
            disk_usage_percent: 12.5,
            network_bytes_in: 500_000,
            network_bytes_out: 250_000,
            active_connections: 50,
            database_connections: 10,
            redis_connections: 5,
        },
        errors: ErrorMetrics {
            total_errors: 0,
            error_rate: 0.0,
            error_types: HashMap::new(),
            error_endpoints: HashMap::new(),
            critical_errors: 0,
            warnings: 0,
        },
        performance: PerformanceMetrics {
            cache_hit_rate: 95.0,
            cache_miss_rate: 5.0,
            avg_db_query_time_ms: 2.0,
            queue_depth: 10,
            throughput: 50.0,
            latency_percentiles: LatencyPercentiles {
                p50: 5.0,
                p90: 20.0,
                p95: 40.0,
                p99: 75.0,
                p999: 150.0,
            },
        },
    };

    let cloned = metrics.clone();
    assert_eq!(
        metrics.requests.total_requests,
        cloned.requests.total_requests
    );
    assert_eq!(metrics.system.cpu_usage, cloned.system.cpu_usage);
}

#[test]
fn test_system_metrics_serialization() {
    let metrics = SystemMetrics {
        timestamp: Utc::now(),
        requests: MonitoringRequestMetrics {
            total_requests: 100,
            requests_per_second: 1.0,
            avg_response_time_ms: 10.0,
            p95_response_time_ms: 20.0,
            p99_response_time_ms: 30.0,
            success_rate: 100.0,
            status_codes: HashMap::new(),
            endpoints: HashMap::new(),
        },
        providers: ProviderMetrics {
            total_provider_requests: 100,
            provider_success_rates: HashMap::new(),
            provider_response_times: HashMap::new(),
            provider_errors: HashMap::new(),
            provider_usage: HashMap::new(),
            token_usage: HashMap::new(),
            costs: HashMap::new(),
        },
        system: SystemResourceMetrics {
            cpu_usage: 10.0,
            memory_usage: 1_000_000_000,
            memory_usage_percent: 12.5,
            disk_usage: 25_000_000_000,
            disk_usage_percent: 5.0,
            network_bytes_in: 100_000,
            network_bytes_out: 50_000,
            active_connections: 25,
            database_connections: 5,
            redis_connections: 2,
        },
        errors: ErrorMetrics {
            total_errors: 1,
            error_rate: 0.01,
            error_types: HashMap::new(),
            error_endpoints: HashMap::new(),
            critical_errors: 0,
            warnings: 1,
        },
        performance: PerformanceMetrics {
            cache_hit_rate: 98.0,
            cache_miss_rate: 2.0,
            avg_db_query_time_ms: 1.0,
            queue_depth: 5,
            throughput: 10.0,
            latency_percentiles: LatencyPercentiles {
                p50: 2.0,
                p90: 8.0,
                p95: 15.0,
                p99: 25.0,
                p999: 50.0,
            },
        },
    };

    let json = serde_json::to_value(&metrics).unwrap();
    assert!(json["timestamp"].is_string());
    assert_eq!(json["requests"]["total_requests"], 100);
    assert_eq!(json["system"]["cpu_usage"], 10.0);
    assert_eq!(json["errors"]["total_errors"], 1);
    assert_eq!(json["performance"]["cache_hit_rate"], 98.0);
}
