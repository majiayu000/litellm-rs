use crate::config::models::defaults::default_true;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OpenTelemetry integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryConfig {
    /// Whether the integration is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// OTLP endpoint URL
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    /// Service name
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Service version
    pub service_version: Option<String>,

    /// Environment (e.g., "production", "staging")
    pub environment: Option<String>,

    /// Additional resource attributes
    #[serde(default)]
    pub resource_attributes: HashMap<String, String>,

    /// Whether to export traces (default: true)
    #[serde(default = "default_true")]
    pub export_traces: bool,

    /// Whether to export metrics (default: true)
    #[serde(default = "default_true")]
    pub export_metrics: bool,

    /// Batch export interval in milliseconds
    #[serde(default = "default_batch_interval")]
    pub batch_interval_ms: u64,

    /// Maximum batch size
    #[serde(default = "default_batch_size")]
    pub max_batch_size: usize,

    /// Export timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Sampling ratio (0.0 to 1.0)
    #[serde(default = "default_sampling_ratio")]
    pub sampling_ratio: f64,

    /// Headers to include in OTLP requests
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

fn default_enabled() -> bool {
    true
}

fn default_endpoint() -> String {
    "http://localhost:4317".to_string()
}

fn default_service_name() -> String {
    "litellm-gateway".to_string()
}

fn default_batch_interval() -> u64 {
    5000
}

fn default_batch_size() -> usize {
    512
}

fn default_timeout() -> u64 {
    10000
}

fn default_sampling_ratio() -> f64 {
    1.0
}

impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            endpoint: default_endpoint(),
            service_name: default_service_name(),
            service_version: None,
            environment: None,
            resource_attributes: HashMap::new(),
            export_traces: true,
            export_metrics: true,
            batch_interval_ms: default_batch_interval(),
            max_batch_size: default_batch_size(),
            timeout_ms: default_timeout(),
            sampling_ratio: default_sampling_ratio(),
            headers: HashMap::new(),
        }
    }
}
