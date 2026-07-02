use super::*;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;

// ==================== CostBreakdown Tests ====================

#[test]
fn test_cost_breakdown_creation() {
    let mut by_provider = HashMap::new();
    by_provider.insert("openai".to_string(), 50.0);
    by_provider.insert("anthropic".to_string(), 30.0);

    let breakdown = CostBreakdown {
        total_cost: 80.0,
        by_provider,
        by_model: HashMap::new(),
        by_operation: HashMap::new(),
        daily_costs: vec![],
    };

    assert_eq!(breakdown.total_cost, 80.0);
    assert_eq!(breakdown.by_provider.len(), 2);
}

#[test]
fn test_cost_breakdown_with_daily_costs() {
    let now = Utc::now();
    let daily_costs = vec![
        DailyCost {
            date: now,
            cost: 10.0,
            requests: 100,
        },
        DailyCost {
            date: now + chrono::Duration::days(1),
            cost: 15.0,
            requests: 150,
        },
    ];

    let breakdown = CostBreakdown {
        total_cost: 25.0,
        by_provider: HashMap::new(),
        by_model: HashMap::new(),
        by_operation: HashMap::new(),
        daily_costs,
    };

    assert_eq!(breakdown.daily_costs.len(), 2);
    let sum: f64 = breakdown.daily_costs.iter().map(|d| d.cost).sum();
    assert!((sum - 25.0).abs() < f64::EPSILON);
}

#[test]
fn test_cost_breakdown_serialization() {
    let mut by_model = HashMap::new();
    by_model.insert("gpt-4".to_string(), 40.0);
    by_model.insert("gpt-3.5-turbo".to_string(), 10.0);

    let breakdown = CostBreakdown {
        total_cost: 50.0,
        by_provider: HashMap::new(),
        by_model,
        by_operation: HashMap::new(),
        daily_costs: vec![],
    };

    let json = serde_json::to_string(&breakdown).unwrap();
    assert!(json.contains("gpt-4"));

    let parsed: CostBreakdown = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total_cost, 50.0);
}

// ==================== DailyCost Tests ====================

#[test]
fn test_daily_cost_creation() {
    let date = Utc.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).unwrap();
    let daily = DailyCost {
        date,
        cost: 25.50,
        requests: 500,
    };

    assert_eq!(daily.cost, 25.50);
    assert_eq!(daily.requests, 500);
}

#[test]
fn test_daily_cost_average_cost_per_request() {
    let daily = DailyCost {
        date: Utc::now(),
        cost: 100.0,
        requests: 1000,
    };

    let avg_cost = daily.cost / daily.requests as f64;
    assert!((avg_cost - 0.1).abs() < f64::EPSILON);
}

// ==================== CostMetrics Tests ====================

#[test]
fn test_cost_metrics_creation() {
    let mut cost_by_period = HashMap::new();
    cost_by_period.insert("2024-01".to_string(), 100.0);
    cost_by_period.insert("2024-02".to_string(), 120.0);

    let metrics = CostMetrics {
        total_cost: 220.0,
        cost_by_period,
        cost_trends: vec![],
        budget_utilization: HashMap::new(),
    };

    assert_eq!(metrics.total_cost, 220.0);
    assert_eq!(metrics.cost_by_period.len(), 2);
}

#[test]
fn test_cost_metrics_with_trends() {
    let now = Utc::now();
    let trends = vec![
        CostTrend {
            period: now,
            cost: 100.0,
            change_percentage: 0.0,
            projected_cost: 100.0,
        },
        CostTrend {
            period: now + chrono::Duration::days(30),
            cost: 110.0,
            change_percentage: 10.0,
            projected_cost: 120.0,
        },
    ];

    let metrics = CostMetrics {
        total_cost: 210.0,
        cost_by_period: HashMap::new(),
        cost_trends: trends,
        budget_utilization: HashMap::new(),
    };

    assert_eq!(metrics.cost_trends.len(), 2);
    assert!((metrics.cost_trends[1].change_percentage - 10.0).abs() < f64::EPSILON);
}

#[test]
fn test_cost_metrics_serialization() {
    let metrics = CostMetrics {
        total_cost: 500.0,
        cost_by_period: HashMap::new(),
        cost_trends: vec![],
        budget_utilization: HashMap::new(),
    };

    let json = serde_json::to_string(&metrics).unwrap();
    let parsed: CostMetrics = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.total_cost, 500.0);
}

// ==================== CostTrend Tests ====================

#[test]
fn test_cost_trend_creation() {
    let trend = CostTrend {
        period: Utc::now(),
        cost: 150.0,
        change_percentage: 5.5,
        projected_cost: 160.0,
    };

    assert_eq!(trend.cost, 150.0);
    assert!((trend.change_percentage - 5.5).abs() < f64::EPSILON);
}

#[test]
fn test_cost_trend_decrease() {
    let trend = CostTrend {
        period: Utc::now(),
        cost: 90.0,
        change_percentage: -10.0,
        projected_cost: 85.0,
    };

    assert!(trend.change_percentage < 0.0);
    assert!(trend.projected_cost < trend.cost);
}

#[test]
fn test_cost_trend_serialization() {
    let trend = CostTrend {
        period: Utc::now(),
        cost: 200.0,
        change_percentage: 15.0,
        projected_cost: 230.0,
    };

    let json = serde_json::to_string(&trend).unwrap();
    let parsed: CostTrend = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.cost, 200.0);
}

// ==================== BudgetUtilization Tests ====================

#[test]
fn test_budget_utilization_creation() {
    let util = BudgetUtilization {
        budget_limit: 1000.0,
        current_usage: 750.0,
        utilization_percentage: 75.0,
        projected_usage: 900.0,
        days_remaining: 10,
    };

    assert_eq!(util.budget_limit, 1000.0);
    assert_eq!(util.current_usage, 750.0);
    assert!((util.utilization_percentage - 75.0).abs() < f64::EPSILON);
}

#[test]
fn test_budget_utilization_under_budget() {
    let util = BudgetUtilization {
        budget_limit: 500.0,
        current_usage: 200.0,
        utilization_percentage: 40.0,
        projected_usage: 350.0,
        days_remaining: 15,
    };

    assert!(util.current_usage < util.budget_limit);
    assert!(util.projected_usage < util.budget_limit);
    assert!(util.utilization_percentage < 100.0);
}

#[test]
fn test_budget_utilization_over_budget_projected() {
    let util = BudgetUtilization {
        budget_limit: 500.0,
        current_usage: 450.0,
        utilization_percentage: 90.0,
        projected_usage: 600.0,
        days_remaining: 5,
    };

    assert!(util.projected_usage > util.budget_limit);
}

#[test]
fn test_budget_utilization_serialization() {
    let util = BudgetUtilization {
        budget_limit: 1000.0,
        current_usage: 500.0,
        utilization_percentage: 50.0,
        projected_usage: 750.0,
        days_remaining: 15,
    };

    let json = serde_json::to_string(&util).unwrap();
    let parsed: BudgetUtilization = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.budget_limit, 1000.0);
    assert_eq!(parsed.days_remaining, 15);
}
