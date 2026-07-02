use super::*;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;

// ==================== TokenUsage Tests ====================

#[test]
fn test_token_usage_creation() {
    let usage = TokenUsage {
        input_tokens: 5000,
        output_tokens: 3000,
        total_tokens: 8000,
        avg_tokens_per_request: 80.0,
    };

    assert_eq!(usage.input_tokens, 5000);
    assert_eq!(usage.output_tokens, 3000);
    assert_eq!(usage.total_tokens, 8000);
}

#[test]
fn test_token_usage_total_matches_sum() {
    let usage = TokenUsage {
        input_tokens: 10000,
        output_tokens: 5000,
        total_tokens: 15000,
        avg_tokens_per_request: 150.0,
    };

    assert_eq!(usage.total_tokens, usage.input_tokens + usage.output_tokens);
}

#[test]
fn test_token_usage_serialization() {
    let usage = TokenUsage {
        input_tokens: 1000,
        output_tokens: 500,
        total_tokens: 1500,
        avg_tokens_per_request: 75.0,
    };

    let json = serde_json::to_string(&usage).unwrap();
    let parsed: TokenUsage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.input_tokens, usage.input_tokens);
    assert_eq!(parsed.total_tokens, usage.total_tokens);
}

// ==================== ModelUsage Tests ====================

#[test]
fn test_model_usage_creation() {
    let usage = ModelUsage {
        model: "gpt-4-turbo".to_string(),
        requests: 1000,
        tokens: 500000,
        cost: 50.0,
        success_rate: 0.995,
    };

    assert_eq!(usage.model, "gpt-4-turbo");
    assert_eq!(usage.requests, 1000);
    assert!(usage.success_rate > 0.99);
}

#[test]
fn test_model_usage_cost_per_token() {
    let usage = ModelUsage {
        model: "claude-3-opus".to_string(),
        requests: 500,
        tokens: 100000,
        cost: 75.0,
        success_rate: 0.99,
    };

    let cost_per_token = usage.cost / usage.tokens as f64;
    assert!(cost_per_token > 0.0);
    assert!((cost_per_token - 0.00075).abs() < 0.0001);
}

#[test]
fn test_model_usage_serialization() {
    let usage = ModelUsage {
        model: "gpt-3.5-turbo".to_string(),
        requests: 5000,
        tokens: 1000000,
        cost: 20.0,
        success_rate: 0.998,
    };

    let json = serde_json::to_string(&usage).unwrap();
    let parsed: ModelUsage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.model, "gpt-3.5-turbo");
    assert_eq!(parsed.tokens, 1000000);
}

// ==================== UsagePatterns Tests ====================

#[test]
fn test_usage_patterns_creation() {
    let mut usage_by_weekday = HashMap::new();
    usage_by_weekday.insert("Monday".to_string(), 1000);
    usage_by_weekday.insert("Tuesday".to_string(), 1200);

    let patterns = UsagePatterns {
        peak_hours: vec![9, 10, 11, 14, 15, 16],
        usage_by_weekday,
        request_size_distribution: RequestSizeDistribution {
            small: 500,
            medium: 300,
            large: 150,
            extra_large: 50,
        },
        seasonal_trends: vec![],
    };

    assert_eq!(patterns.peak_hours.len(), 6);
    assert!(patterns.peak_hours.contains(&9));
}

#[test]
fn test_usage_patterns_serialization() {
    let patterns = UsagePatterns {
        peak_hours: vec![10, 11, 12],
        usage_by_weekday: HashMap::new(),
        request_size_distribution: RequestSizeDistribution {
            small: 100,
            medium: 200,
            large: 50,
            extra_large: 10,
        },
        seasonal_trends: vec![],
    };

    let json = serde_json::to_string(&patterns).unwrap();
    let parsed: UsagePatterns = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.peak_hours.len(), 3);
}

// ==================== RequestSizeDistribution Tests ====================

#[test]
fn test_request_size_distribution_creation() {
    let dist = RequestSizeDistribution {
        small: 5000,
        medium: 3000,
        large: 1500,
        extra_large: 500,
    };

    assert_eq!(dist.small, 5000);
    assert_eq!(dist.medium, 3000);
    assert_eq!(dist.large, 1500);
    assert_eq!(dist.extra_large, 500);
}

#[test]
fn test_request_size_distribution_total() {
    let dist = RequestSizeDistribution {
        small: 1000,
        medium: 500,
        large: 300,
        extra_large: 200,
    };

    let total = dist.small + dist.medium + dist.large + dist.extra_large;
    assert_eq!(total, 2000);
}

#[test]
fn test_request_size_distribution_percentages() {
    let dist = RequestSizeDistribution {
        small: 500,
        medium: 300,
        large: 150,
        extra_large: 50,
    };

    let total = (dist.small + dist.medium + dist.large + dist.extra_large) as f64;
    let small_pct = dist.small as f64 / total;
    let extra_large_pct = dist.extra_large as f64 / total;

    assert!((small_pct - 0.5).abs() < f64::EPSILON);
    assert!((extra_large_pct - 0.05).abs() < f64::EPSILON);
}

// ==================== SeasonalTrend Tests ====================

#[test]
fn test_seasonal_trend_creation() {
    let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2024, 3, 31, 23, 59, 59).unwrap();

    let trend = SeasonalTrend {
        period: "Q1 2024".to_string(),
        start_date: start,
        end_date: end,
        usage: 100000,
        growth_rate: 15.5,
    };

    assert_eq!(trend.period, "Q1 2024");
    assert_eq!(trend.usage, 100000);
    assert!((trend.growth_rate - 15.5).abs() < f64::EPSILON);
}

#[test]
fn test_seasonal_trend_negative_growth() {
    let trend = SeasonalTrend {
        period: "Month".to_string(),
        start_date: Utc::now(),
        end_date: Utc::now(),
        usage: 8000,
        growth_rate: -10.0,
    };

    assert!(trend.growth_rate < 0.0);
}

#[test]
fn test_seasonal_trend_serialization() {
    let trend = SeasonalTrend {
        period: "Week".to_string(),
        start_date: Utc::now(),
        end_date: Utc::now(),
        usage: 5000,
        growth_rate: 5.0,
    };

    let json = serde_json::to_string(&trend).unwrap();
    assert!(json.contains("Week"));

    let parsed: SeasonalTrend = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.usage, 5000);
}

// ==================== UserMetrics Tests ====================

#[test]
fn test_user_metrics_creation() {
    let metrics = UserMetrics {
        user_id: "user_123".to_string(),
        request_count: 1000,
        token_usage: TokenUsage {
            input_tokens: 50000,
            output_tokens: 25000,
            total_tokens: 75000,
            avg_tokens_per_request: 75.0,
        },
        cost_breakdown: CostBreakdown {
            total_cost: 50.0,
            by_provider: HashMap::new(),
            by_model: HashMap::new(),
            by_operation: HashMap::new(),
            daily_costs: vec![],
        },
        top_models: vec![],
        usage_patterns: UsagePatterns {
            peak_hours: vec![10, 11],
            usage_by_weekday: HashMap::new(),
            request_size_distribution: RequestSizeDistribution {
                small: 100,
                medium: 50,
                large: 20,
                extra_large: 5,
            },
            seasonal_trends: vec![],
        },
    };

    assert_eq!(metrics.user_id, "user_123");
    assert_eq!(metrics.request_count, 1000);
}

#[test]
fn test_user_metrics_with_top_models() {
    let top_models = vec![
        ModelUsage {
            model: "gpt-4".to_string(),
            requests: 500,
            tokens: 200000,
            cost: 30.0,
            success_rate: 0.99,
        },
        ModelUsage {
            model: "gpt-3.5-turbo".to_string(),
            requests: 500,
            tokens: 100000,
            cost: 5.0,
            success_rate: 0.995,
        },
    ];

    let metrics = UserMetrics {
        user_id: "power_user".to_string(),
        request_count: 1000,
        token_usage: TokenUsage {
            input_tokens: 200000,
            output_tokens: 100000,
            total_tokens: 300000,
            avg_tokens_per_request: 300.0,
        },
        cost_breakdown: CostBreakdown {
            total_cost: 35.0,
            by_provider: HashMap::new(),
            by_model: HashMap::new(),
            by_operation: HashMap::new(),
            daily_costs: vec![],
        },
        top_models,
        usage_patterns: UsagePatterns {
            peak_hours: vec![],
            usage_by_weekday: HashMap::new(),
            request_size_distribution: RequestSizeDistribution {
                small: 0,
                medium: 0,
                large: 0,
                extra_large: 0,
            },
            seasonal_trends: vec![],
        },
    };

    assert_eq!(metrics.top_models.len(), 2);
    assert_eq!(metrics.top_models[0].model, "gpt-4");
}

#[test]
fn test_user_metrics_serialization() {
    let metrics = UserMetrics {
        user_id: "test_user".to_string(),
        request_count: 100,
        token_usage: TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            avg_tokens_per_request: 15.0,
        },
        cost_breakdown: CostBreakdown {
            total_cost: 1.0,
            by_provider: HashMap::new(),
            by_model: HashMap::new(),
            by_operation: HashMap::new(),
            daily_costs: vec![],
        },
        top_models: vec![],
        usage_patterns: UsagePatterns {
            peak_hours: vec![],
            usage_by_weekday: HashMap::new(),
            request_size_distribution: RequestSizeDistribution {
                small: 0,
                medium: 0,
                large: 0,
                extra_large: 0,
            },
            seasonal_trends: vec![],
        },
    };

    let json = serde_json::to_string(&metrics).unwrap();
    assert!(json.contains("test_user"));

    let parsed: UserMetrics = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.user_id, "test_user");
}
