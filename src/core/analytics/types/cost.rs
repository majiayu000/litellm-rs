use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cost breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    /// Total cost
    pub total_cost: f64,
    /// Cost by provider
    pub by_provider: HashMap<String, f64>,
    /// Cost by model
    pub by_model: HashMap<String, f64>,
    /// Cost by operation type
    pub by_operation: HashMap<String, f64>,
    /// Daily costs
    pub daily_costs: Vec<DailyCost>,
}

/// Daily cost information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCost {
    /// Date
    pub date: DateTime<Utc>,
    /// Cost amount
    pub cost: f64,
    /// Request count
    pub requests: u64,
}

/// Overall cost metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostMetrics {
    /// Total cost across all users
    pub total_cost: f64,
    /// Cost by time period
    pub cost_by_period: HashMap<String, f64>,
    /// Cost trends
    pub cost_trends: Vec<CostTrend>,
    /// Budget utilization
    pub budget_utilization: HashMap<String, BudgetUtilization>,
}

/// Cost trend information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTrend {
    /// Period
    pub period: DateTime<Utc>,
    /// Cost amount
    pub cost: f64,
    /// Change from previous period
    pub change_percentage: f64,
    /// Projected cost for next period
    pub projected_cost: f64,
}

/// Budget utilization tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetUtilization {
    /// Budget limit
    pub budget_limit: f64,
    /// Current usage
    pub current_usage: f64,
    /// Utilization percentage
    pub utilization_percentage: f64,
    /// Projected end-of-period usage
    pub projected_usage: f64,
    /// Days remaining in period
    pub days_remaining: u32,
}
