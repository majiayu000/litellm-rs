//! Aggregated metrics models

use super::super::Metadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Provider metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetrics {
    /// Metrics metadata
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Provider name
    pub provider: String,
    /// Time period start
    pub period_start: chrono::DateTime<chrono::Utc>,
    /// Time period end
    pub period_end: chrono::DateTime<chrono::Utc>,
    /// Total requests
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// P50 response time
    pub p50_response_time_ms: f64,
    /// P95 response time
    pub p95_response_time_ms: f64,
    /// P99 response time
    pub p99_response_time_ms: f64,
    /// Total tokens processed
    pub total_tokens: u64,
    /// Total cost
    pub total_cost: f64,
    /// Error breakdown
    pub error_breakdown: HashMap<String, u64>,
    /// Model breakdown
    pub model_breakdown: HashMap<String, ModelMetrics>,
}

/// Model-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    /// Model name
    pub model: String,
    /// Request count
    pub requests: u64,
    /// Success count
    pub successes: u64,
    /// Total tokens
    pub tokens: u64,
    /// Total cost
    pub cost: f64,
    /// Average response time
    pub avg_response_time_ms: f64,
}

/// System metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Metrics metadata
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Memory usage percentage
    pub memory_usage_percent: f64,
    /// Disk usage in bytes
    pub disk_usage: u64,
    /// Disk usage percentage
    pub disk_usage_percent: f64,
    /// Network I/O
    pub network_io: NetworkIO,
    /// Active connections
    pub active_connections: u32,
    /// Queue sizes
    pub queue_sizes: HashMap<String, u32>,
    /// Thread pool stats
    pub thread_pool: ThreadPoolStats,
}

/// Network I/O metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkIO {
    /// Bytes received
    pub bytes_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Packets received
    pub packets_received: u64,
    /// Packets sent
    pub packets_sent: u64,
}

/// Thread pool statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThreadPoolStats {
    /// Active threads
    pub active_threads: u32,
    /// Total threads
    pub total_threads: u32,
    /// Queued tasks
    pub queued_tasks: u32,
    /// Completed tasks
    pub completed_tasks: u64,
}

/// Usage analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageAnalytics {
    /// Analytics metadata
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Time period
    pub period: TimePeriod,
    /// User ID (if user-specific)
    pub user_id: Option<Uuid>,
    /// Team ID (if team-specific)
    pub team_id: Option<Uuid>,
    /// Total requests
    pub total_requests: u64,
    /// Total tokens
    pub total_tokens: u64,
    /// Total cost
    pub total_cost: f64,
    /// Model usage breakdown
    pub model_usage: HashMap<String, ModelUsage>,
    /// Provider usage breakdown
    pub provider_usage: HashMap<String, ProviderUsage>,
    /// Daily breakdown
    pub daily_breakdown: Vec<DailyUsage>,
    /// Top endpoints
    pub top_endpoints: Vec<EndpointUsage>,
}

/// Time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimePeriod {
    /// Period start
    pub start: chrono::DateTime<chrono::Utc>,
    /// Period end
    pub end: chrono::DateTime<chrono::Utc>,
    /// Period type
    pub period_type: PeriodType,
}

/// Period type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodType {
    /// Hourly period
    Hour,
    /// Daily period
    Day,
    /// Weekly period
    Week,
    /// Monthly period
    Month,
    /// Yearly period
    Year,
    /// Custom period
    Custom,
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
    /// Average response time
    pub avg_response_time_ms: f64,
}

/// Provider usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsage {
    /// Provider name
    pub provider: String,
    /// Request count
    pub requests: u64,
    /// Token count
    pub tokens: u64,
    /// Cost
    pub cost: f64,
    /// Success rate
    pub success_rate: f64,
    /// Average response time
    pub avg_response_time_ms: f64,
}

/// Daily usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    /// Date
    pub date: chrono::NaiveDate,
    /// Request count
    pub requests: u64,
    /// Token count
    pub tokens: u64,
    /// Cost
    pub cost: f64,
    /// Unique users
    pub unique_users: u32,
}

/// Endpoint usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointUsage {
    /// Endpoint path
    pub endpoint: String,
    /// Request count
    pub requests: u64,
    /// Success rate
    pub success_rate: f64,
    /// Average response time
    pub avg_response_time_ms: f64,
}

/// Alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Alert metadata
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Alert name
    pub name: String,
    /// Alert description
    pub description: Option<String>,
    /// Alert condition
    pub condition: AlertCondition,
    /// Alert threshold
    pub threshold: f64,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert channels
    pub channels: Vec<String>,
    /// Alert enabled
    pub enabled: bool,
    /// Cooldown period in seconds
    pub cooldown_seconds: u64,
}

/// Alert condition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertCondition {
    /// High error rate condition
    ErrorRateHigh,
    /// Slow response time condition
    ResponseTimeSlow,
    /// High request volume condition
    RequestVolumeHigh,
    /// High cost condition
    CostHigh,
    /// Provider down condition
    ProviderDown,
    /// Quota exceeded condition
    QuotaExceeded,
    /// Custom alert condition
    Custom(String),
}

/// Alert severity
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    /// Informational alert
    Info,
    /// Warning alert
    Warning,
    /// Error alert
    Error,
    /// Critical alert
    Critical,
}

#[cfg(test)]
#[path = "aggregates_tests.rs"]
mod tests;
