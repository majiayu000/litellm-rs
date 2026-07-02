use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Request metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRequestMetrics {
    /// Total requests
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Average response time
    pub avg_response_time_ms: f64,
    /// P95 response time
    pub p95_response_time_ms: f64,
    /// P99 response time
    pub p99_response_time_ms: f64,
    /// Total tokens processed
    pub total_tokens: u64,
    /// Total cost
    pub total_cost: f64,
    /// Time period
    pub period_start: DateTime<Utc>,
    /// End of analysis period
    pub period_end: DateTime<Utc>,
}

/// Provider-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetrics {
    /// Provider name
    pub provider_name: String,
    /// Request count
    pub request_count: u64,
    /// Success rate
    pub success_rate: f64,
    /// Average latency
    pub avg_latency_ms: f64,
    /// Error rate
    pub error_rate: f64,
    /// Cost efficiency (tokens per dollar)
    pub cost_efficiency: f64,
    /// Uptime percentage
    pub uptime_percentage: f64,
    /// Rate limit hits
    pub rate_limit_hits: u64,
}
