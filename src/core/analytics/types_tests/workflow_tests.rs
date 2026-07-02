use super::*;
use chrono::Utc;
use std::collections::HashMap;

// ==================== Integration Tests ====================

#[test]
fn test_full_analytics_workflow() {
    // Create a complete analytics snapshot
    let now = Utc::now();

    let request_metrics = AnalyticsRequestMetrics {
        total_requests: 10000,
        successful_requests: 9900,
        failed_requests: 100,
        avg_response_time_ms: 150.0,
        p95_response_time_ms: 300.0,
        p99_response_time_ms: 500.0,
        total_tokens: 5000000,
        total_cost: 250.0,
        period_start: now - chrono::Duration::days(7),
        period_end: now,
    };

    let provider_metrics = [
        ProviderMetrics {
            provider_name: "openai".to_string(),
            request_count: 6000,
            success_rate: 0.995,
            avg_latency_ms: 140.0,
            error_rate: 0.005,
            cost_efficiency: 900.0,
            uptime_percentage: 99.9,
            rate_limit_hits: 5,
        },
        ProviderMetrics {
            provider_name: "anthropic".to_string(),
            request_count: 4000,
            success_rate: 0.99,
            avg_latency_ms: 165.0,
            error_rate: 0.01,
            cost_efficiency: 850.0,
            uptime_percentage: 99.8,
            rate_limit_hits: 2,
        },
    ];

    // Verify aggregations
    let total_provider_requests: u64 = provider_metrics.iter().map(|p| p.request_count).sum();
    assert_eq!(total_provider_requests, request_metrics.total_requests);

    // Calculate weighted average latency
    let weighted_latency: f64 = provider_metrics
        .iter()
        .map(|p| p.avg_latency_ms * p.request_count as f64)
        .sum::<f64>()
        / total_provider_requests as f64;
    assert!(weighted_latency > 140.0 && weighted_latency < 165.0);
}

#[test]
fn test_cost_analysis_workflow() {
    let mut by_provider = HashMap::new();
    by_provider.insert("openai".to_string(), 150.0);
    by_provider.insert("anthropic".to_string(), 100.0);

    let mut by_model = HashMap::new();
    by_model.insert("gpt-4".to_string(), 100.0);
    by_model.insert("gpt-3.5-turbo".to_string(), 50.0);
    by_model.insert("claude-3-opus".to_string(), 75.0);
    by_model.insert("claude-3-sonnet".to_string(), 25.0);

    let cost_breakdown = CostBreakdown {
        total_cost: 250.0,
        by_provider: by_provider.clone(),
        by_model: by_model.clone(),
        by_operation: HashMap::new(),
        daily_costs: vec![],
    };

    // Verify provider totals match
    let provider_sum: f64 = by_provider.values().sum();
    assert!((provider_sum - cost_breakdown.total_cost).abs() < f64::EPSILON);

    // Verify model totals match
    let model_sum: f64 = by_model.values().sum();
    assert!((model_sum - cost_breakdown.total_cost).abs() < f64::EPSILON);
}
