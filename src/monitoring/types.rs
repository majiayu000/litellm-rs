//! Type definitions for monitoring metrics and alerts

/// System metrics snapshot
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemMetrics {
    /// Timestamp of the snapshot
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Request metrics
    pub requests: MonitoringRequestMetrics,
    /// Provider metrics
    pub providers: ProviderMetrics,
    /// System resource metrics
    pub system: SystemResourceMetrics,
    /// Error metrics
    pub errors: ErrorMetrics,
    /// Performance metrics
    pub performance: PerformanceMetrics,
}

/// Request-related metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct MonitoringRequestMetrics {
    /// Total requests processed
    pub total_requests: u64,
    /// Requests per second (current)
    pub requests_per_second: f64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// 95th percentile response time
    pub p95_response_time_ms: f64,
    /// 99th percentile response time
    pub p99_response_time_ms: f64,
    /// Success rate (percentage)
    pub success_rate: f64,
    /// Requests by status code
    pub status_codes: std::collections::HashMap<u16, u64>,
    /// Requests by endpoint
    pub endpoints: std::collections::HashMap<String, u64>,
}

/// Provider-related metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderMetrics {
    /// Total provider requests
    pub total_provider_requests: u64,
    /// Provider success rates
    pub provider_success_rates: std::collections::HashMap<String, f64>,
    /// Provider response times
    pub provider_response_times: std::collections::HashMap<String, f64>,
    /// Provider error counts
    pub provider_errors: std::collections::HashMap<String, u64>,
    /// Provider usage distribution
    pub provider_usage: std::collections::HashMap<String, u64>,
    /// Token usage by provider
    pub token_usage: std::collections::HashMap<String, u64>,
    /// Cost by provider
    pub costs: std::collections::HashMap<String, f64>,
}

/// System resource metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemResourceMetrics {
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
    /// Network bytes received
    pub network_bytes_in: u64,
    /// Network bytes sent
    pub network_bytes_out: u64,
    /// Active connections
    pub active_connections: u32,
    /// Database connections
    pub database_connections: u32,
    /// Redis connections
    pub redis_connections: u32,
}

/// Error-related metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorMetrics {
    /// Total errors
    pub total_errors: u64,
    /// Error rate (errors per second)
    pub error_rate: f64,
    /// Errors by type
    pub error_types: std::collections::HashMap<String, u64>,
    /// Errors by endpoint
    pub error_endpoints: std::collections::HashMap<String, u64>,
    /// Critical errors
    pub critical_errors: u64,
    /// Warning count
    pub warnings: u64,
}

/// Performance-related metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct PerformanceMetrics {
    /// Cache hit rate
    pub cache_hit_rate: f64,
    /// Cache miss rate
    pub cache_miss_rate: f64,
    /// Database query time (average)
    pub avg_db_query_time_ms: f64,
    /// Queue depth
    pub queue_depth: u32,
    /// Throughput (requests per second)
    pub throughput: f64,
    /// Latency percentiles
    pub latency_percentiles: LatencyPercentiles,
}

/// Latency percentile metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct LatencyPercentiles {
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub p999: f64,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Info => write!(f, "INFO"),
            AlertSeverity::Warning => write!(f, "WARNING"),
            AlertSeverity::Critical => write!(f, "CRITICAL"),
            AlertSeverity::Emergency => write!(f, "EMERGENCY"),
        }
    }
}

/// Alert information
#[derive(Debug, Clone, serde::Serialize)]
pub struct Alert {
    /// Alert ID
    pub id: String,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert title
    pub title: String,
    /// Alert description
    pub description: String,
    /// Alert timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Alert source
    pub source: String,
    /// Alert metadata
    pub metadata: serde_json::Value,
    /// Whether the alert is resolved
    pub resolved: bool,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
