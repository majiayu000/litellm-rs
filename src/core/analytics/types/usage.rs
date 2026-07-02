use super::cost::CostBreakdown;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMetrics {
    /// User ID
    pub user_id: String,
    /// Request count
    pub request_count: u64,
    /// Token usage
    pub token_usage: TokenUsage,
    /// Cost breakdown
    pub cost_breakdown: CostBreakdown,
    /// Most used models
    pub top_models: Vec<ModelUsage>,
    /// Usage patterns
    pub usage_patterns: UsagePatterns,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens
    pub input_tokens: u64,
    /// Output tokens
    pub output_tokens: u64,
    /// Total tokens
    pub total_tokens: u64,
    /// Average tokens per request
    pub avg_tokens_per_request: f64,
}

/// Model usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    /// Model name
    pub model: String,
    /// Request count
    pub requests: u64,
    /// Token count
    pub tokens: u64,
    /// Cost
    pub cost: f64,
    /// Success rate
    pub success_rate: f64,
}

/// Usage patterns analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsagePatterns {
    /// Peak usage hours
    pub peak_hours: Vec<u8>,
    /// Usage by day of week
    pub usage_by_weekday: HashMap<String, u64>,
    /// Request size distribution
    pub request_size_distribution: RequestSizeDistribution,
    /// Seasonal trends
    pub seasonal_trends: Vec<SeasonalTrend>,
}

/// Request size distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSizeDistribution {
    /// Small requests (< 100 tokens)
    pub small: u64,
    /// Medium requests (100-1000 tokens)
    pub medium: u64,
    /// Large requests (1000-10000 tokens)
    pub large: u64,
    /// Extra large requests (> 10000 tokens)
    pub extra_large: u64,
}

/// Seasonal trend data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalTrend {
    /// Period (week, month, quarter)
    pub period: String,
    /// Start date
    pub start_date: DateTime<Utc>,
    /// End date
    pub end_date: DateTime<Utc>,
    /// Usage count
    pub usage: u64,
    /// Growth rate compared to previous period
    pub growth_rate: f64,
}
