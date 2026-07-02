use super::*;
use chrono::{NaiveDate, Utc};

fn create_test_metadata() -> Metadata {
    Metadata {
        id: Uuid::new_v4(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        extra: HashMap::new(),
    }
}

// ==================== ModelMetrics Tests ====================

#[test]
fn test_model_metrics_structure() {
    let metrics = ModelMetrics {
        model: "gpt-4".to_string(),
        requests: 100,
        successes: 95,
        tokens: 50000,
        cost: 5.0,
        avg_response_time_ms: 150.0,
    };

    assert_eq!(metrics.model, "gpt-4");
    assert_eq!(metrics.requests, 100);
    assert_eq!(metrics.successes, 95);
}

#[test]
fn test_model_metrics_serialization() {
    let metrics = ModelMetrics {
        model: "claude-3".to_string(),
        requests: 50,
        successes: 48,
        tokens: 25000,
        cost: 2.5,
        avg_response_time_ms: 200.0,
    };

    let json = serde_json::to_value(&metrics).unwrap();
    assert_eq!(json["model"], "claude-3");
    assert_eq!(json["requests"], 50);
    assert_eq!(json["cost"], 2.5);
}

#[test]
fn test_model_metrics_clone() {
    let metrics = ModelMetrics {
        model: "test".to_string(),
        requests: 10,
        successes: 10,
        tokens: 1000,
        cost: 0.5,
        avg_response_time_ms: 100.0,
    };

    let cloned = metrics.clone();
    assert_eq!(metrics.model, cloned.model);
    assert_eq!(metrics.requests, cloned.requests);
}

// ==================== NetworkIO Tests ====================

#[test]
fn test_network_io_default() {
    let io = NetworkIO::default();
    assert_eq!(io.bytes_received, 0);
    assert_eq!(io.bytes_sent, 0);
    assert_eq!(io.packets_received, 0);
    assert_eq!(io.packets_sent, 0);
}

#[test]
fn test_network_io_serialization() {
    let io = NetworkIO {
        bytes_received: 1024,
        bytes_sent: 2048,
        packets_received: 100,
        packets_sent: 200,
    };

    let json = serde_json::to_value(&io).unwrap();
    assert_eq!(json["bytes_received"], 1024);
    assert_eq!(json["bytes_sent"], 2048);
}

// ==================== ThreadPoolStats Tests ====================

#[test]
fn test_thread_pool_stats_default() {
    let stats = ThreadPoolStats::default();
    assert_eq!(stats.active_threads, 0);
    assert_eq!(stats.total_threads, 0);
    assert_eq!(stats.queued_tasks, 0);
    assert_eq!(stats.completed_tasks, 0);
}

#[test]
fn test_thread_pool_stats_serialization() {
    let stats = ThreadPoolStats {
        active_threads: 4,
        total_threads: 8,
        queued_tasks: 10,
        completed_tasks: 1000,
    };

    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["active_threads"], 4);
    assert_eq!(json["total_threads"], 8);
}

// ==================== PeriodType Tests ====================

#[test]
fn test_period_type_variants() {
    let types = vec![
        PeriodType::Hour,
        PeriodType::Day,
        PeriodType::Week,
        PeriodType::Month,
        PeriodType::Year,
        PeriodType::Custom,
    ];

    for period_type in types {
        let json = serde_json::to_string(&period_type).unwrap();
        assert!(!json.is_empty());
    }
}

#[test]
fn test_period_type_serialization() {
    assert!(
        serde_json::to_string(&PeriodType::Hour)
            .unwrap()
            .contains("hour")
    );
    assert!(
        serde_json::to_string(&PeriodType::Day)
            .unwrap()
            .contains("day")
    );
    assert!(
        serde_json::to_string(&PeriodType::Week)
            .unwrap()
            .contains("week")
    );
    assert!(
        serde_json::to_string(&PeriodType::Month)
            .unwrap()
            .contains("month")
    );
}

// ==================== TimePeriod Tests ====================

#[test]
fn test_time_period_structure() {
    let period = TimePeriod {
        start: Utc::now(),
        end: Utc::now(),
        period_type: PeriodType::Day,
    };

    assert!(matches!(period.period_type, PeriodType::Day));
}

#[test]
fn test_time_period_serialization() {
    let period = TimePeriod {
        start: Utc::now(),
        end: Utc::now(),
        period_type: PeriodType::Week,
    };

    let json = serde_json::to_value(&period).unwrap();
    assert!(json["start"].is_string());
    assert!(json["end"].is_string());
    assert!(json["period_type"].is_string());
}

// ==================== ModelUsage Tests ====================

#[test]
fn test_model_usage_structure() {
    let usage = ModelUsage {
        model: "gpt-4".to_string(),
        requests: 100,
        tokens: 50000,
        cost: 5.0,
        success_rate: 0.95,
        avg_response_time_ms: 150.0,
    };

    assert_eq!(usage.model, "gpt-4");
    assert!((usage.success_rate - 0.95).abs() < f64::EPSILON);
}

// ==================== ProviderUsage Tests ====================

#[test]
fn test_provider_usage_structure() {
    let usage = ProviderUsage {
        provider: "openai".to_string(),
        requests: 500,
        tokens: 100000,
        cost: 10.0,
        success_rate: 0.98,
        avg_response_time_ms: 120.0,
    };

    assert_eq!(usage.provider, "openai");
    assert_eq!(usage.requests, 500);
}

// ==================== DailyUsage Tests ====================

#[test]
fn test_daily_usage_structure() {
    let usage = DailyUsage {
        date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        requests: 1000,
        tokens: 500000,
        cost: 50.0,
        unique_users: 25,
    };

    assert_eq!(usage.requests, 1000);
    assert_eq!(usage.unique_users, 25);
}

#[test]
fn test_daily_usage_serialization() {
    let usage = DailyUsage {
        date: NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
        requests: 100,
        tokens: 10000,
        cost: 1.0,
        unique_users: 5,
    };

    let json = serde_json::to_value(&usage).unwrap();
    assert!(json["date"].is_string());
    assert_eq!(json["requests"], 100);
}

// ==================== EndpointUsage Tests ====================

#[test]
fn test_endpoint_usage_structure() {
    let usage = EndpointUsage {
        endpoint: "/v1/chat/completions".to_string(),
        requests: 5000,
        success_rate: 0.99,
        avg_response_time_ms: 100.0,
    };

    assert_eq!(usage.endpoint, "/v1/chat/completions");
    assert_eq!(usage.requests, 5000);
}

// ==================== AlertCondition Tests ====================

#[test]
fn test_alert_condition_variants() {
    let conditions = vec![
        AlertCondition::ErrorRateHigh,
        AlertCondition::ResponseTimeSlow,
        AlertCondition::RequestVolumeHigh,
        AlertCondition::CostHigh,
        AlertCondition::ProviderDown,
        AlertCondition::QuotaExceeded,
        AlertCondition::Custom("custom_condition".to_string()),
    ];

    for condition in conditions {
        let json = serde_json::to_string(&condition).unwrap();
        assert!(!json.is_empty());
    }
}

#[test]
fn test_alert_condition_serialization() {
    let condition = AlertCondition::ErrorRateHigh;
    let json = serde_json::to_string(&condition).unwrap();
    assert!(json.contains("error_rate_high"));
}

#[test]
fn test_alert_condition_custom() {
    let condition = AlertCondition::Custom("my_custom_alert".to_string());
    let json = serde_json::to_string(&condition).unwrap();
    assert!(json.contains("my_custom_alert"));
}

// ==================== AlertSeverity Tests ====================

#[test]
fn test_alert_severity_variants() {
    let severities = vec![
        AlertSeverity::Info,
        AlertSeverity::Warning,
        AlertSeverity::Error,
        AlertSeverity::Critical,
    ];

    for severity in severities {
        let json = serde_json::to_string(&severity).unwrap();
        assert!(!json.is_empty());
    }
}

#[test]
fn test_alert_severity_serialization() {
    assert!(
        serde_json::to_string(&AlertSeverity::Info)
            .unwrap()
            .contains("info")
    );
    assert!(
        serde_json::to_string(&AlertSeverity::Warning)
            .unwrap()
            .contains("warning")
    );
    assert!(
        serde_json::to_string(&AlertSeverity::Error)
            .unwrap()
            .contains("error")
    );
    assert!(
        serde_json::to_string(&AlertSeverity::Critical)
            .unwrap()
            .contains("critical")
    );
}

// ==================== AlertConfig Tests ====================

#[test]
fn test_alert_config_structure() {
    let config = AlertConfig {
        metadata: create_test_metadata(),
        name: "High Error Rate".to_string(),
        description: Some("Alert when error rate exceeds threshold".to_string()),
        condition: AlertCondition::ErrorRateHigh,
        threshold: 0.05,
        severity: AlertSeverity::Warning,
        channels: vec!["slack".to_string(), "email".to_string()],
        enabled: true,
        cooldown_seconds: 300,
    };

    assert_eq!(config.name, "High Error Rate");
    assert!(config.enabled);
    assert_eq!(config.channels.len(), 2);
}

#[test]
fn test_alert_config_no_description() {
    let config = AlertConfig {
        metadata: create_test_metadata(),
        name: "Simple Alert".to_string(),
        description: None,
        condition: AlertCondition::CostHigh,
        threshold: 100.0,
        severity: AlertSeverity::Info,
        channels: vec![],
        enabled: false,
        cooldown_seconds: 0,
    };

    assert!(config.description.is_none());
    assert!(!config.enabled);
}

// ==================== ProviderMetrics Tests ====================

#[test]
fn test_provider_metrics_structure() {
    let mut error_breakdown = HashMap::new();
    error_breakdown.insert("rate_limit".to_string(), 10u64);
    error_breakdown.insert("timeout".to_string(), 5u64);

    let metrics = ProviderMetrics {
        metadata: create_test_metadata(),
        provider: "openai".to_string(),
        period_start: Utc::now(),
        period_end: Utc::now(),
        total_requests: 1000,
        successful_requests: 950,
        failed_requests: 50,
        success_rate: 0.95,
        avg_response_time_ms: 150.0,
        p50_response_time_ms: 100.0,
        p95_response_time_ms: 300.0,
        p99_response_time_ms: 500.0,
        total_tokens: 500000,
        total_cost: 50.0,
        error_breakdown,
        model_breakdown: HashMap::new(),
    };

    assert_eq!(metrics.provider, "openai");
    assert_eq!(metrics.total_requests, 1000);
    assert_eq!(metrics.error_breakdown.len(), 2);
}

#[test]
fn test_provider_metrics_with_model_breakdown() {
    let mut model_breakdown = HashMap::new();
    model_breakdown.insert(
        "gpt-4".to_string(),
        ModelMetrics {
            model: "gpt-4".to_string(),
            requests: 500,
            successes: 480,
            tokens: 250000,
            cost: 30.0,
            avg_response_time_ms: 200.0,
        },
    );

    let metrics = ProviderMetrics {
        metadata: create_test_metadata(),
        provider: "openai".to_string(),
        period_start: Utc::now(),
        period_end: Utc::now(),
        total_requests: 500,
        successful_requests: 480,
        failed_requests: 20,
        success_rate: 0.96,
        avg_response_time_ms: 200.0,
        p50_response_time_ms: 150.0,
        p95_response_time_ms: 400.0,
        p99_response_time_ms: 600.0,
        total_tokens: 250000,
        total_cost: 30.0,
        error_breakdown: HashMap::new(),
        model_breakdown,
    };

    assert!(metrics.model_breakdown.contains_key("gpt-4"));
}

// ==================== SystemMetrics Tests ====================

#[test]
fn test_system_metrics_structure() {
    let metrics = SystemMetrics {
        metadata: create_test_metadata(),
        timestamp: Utc::now(),
        cpu_usage: 45.5,
        memory_usage: 8_000_000_000,
        memory_usage_percent: 50.0,
        disk_usage: 100_000_000_000,
        disk_usage_percent: 40.0,
        network_io: NetworkIO::default(),
        active_connections: 100,
        queue_sizes: HashMap::new(),
        thread_pool: ThreadPoolStats::default(),
    };

    assert!((metrics.cpu_usage - 45.5).abs() < f64::EPSILON);
    assert_eq!(metrics.active_connections, 100);
}

// ==================== UsageAnalytics Tests ====================

#[test]
fn test_usage_analytics_structure() {
    let analytics = UsageAnalytics {
        metadata: create_test_metadata(),
        period: TimePeriod {
            start: Utc::now(),
            end: Utc::now(),
            period_type: PeriodType::Month,
        },
        user_id: Some(Uuid::new_v4()),
        team_id: None,
        total_requests: 10000,
        total_tokens: 5000000,
        total_cost: 500.0,
        model_usage: HashMap::new(),
        provider_usage: HashMap::new(),
        daily_breakdown: vec![],
        top_endpoints: vec![],
    };

    assert!(analytics.user_id.is_some());
    assert!(analytics.team_id.is_none());
    assert_eq!(analytics.total_requests, 10000);
}

#[test]
fn test_usage_analytics_with_breakdowns() {
    let mut model_usage = HashMap::new();
    model_usage.insert(
        "gpt-4".to_string(),
        ModelUsage {
            model: "gpt-4".to_string(),
            requests: 100,
            tokens: 50000,
            cost: 5.0,
            success_rate: 0.95,
            avg_response_time_ms: 150.0,
        },
    );

    let analytics = UsageAnalytics {
        metadata: create_test_metadata(),
        period: TimePeriod {
            start: Utc::now(),
            end: Utc::now(),
            period_type: PeriodType::Day,
        },
        user_id: None,
        team_id: None,
        total_requests: 100,
        total_tokens: 50000,
        total_cost: 5.0,
        model_usage,
        provider_usage: HashMap::new(),
        daily_breakdown: vec![],
        top_endpoints: vec![],
    };

    assert!(analytics.model_usage.contains_key("gpt-4"));
}

// ==================== Deserialization Tests ====================

#[test]
fn test_period_type_deserialization() {
    let json = r#""day""#;
    let period_type: PeriodType = serde_json::from_str(json).unwrap();
    assert!(matches!(period_type, PeriodType::Day));
}

#[test]
fn test_alert_severity_deserialization() {
    let json = r#""critical""#;
    let severity: AlertSeverity = serde_json::from_str(json).unwrap();
    assert!(matches!(severity, AlertSeverity::Critical));
}

#[test]
fn test_alert_condition_deserialization() {
    let json = r#""provider_down""#;
    let condition: AlertCondition = serde_json::from_str(json).unwrap();
    assert!(matches!(condition, AlertCondition::ProviderDown));
}
