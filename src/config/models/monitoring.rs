//! Monitoring and observability configuration
//!
//! Unified configuration for metrics, tracing, health checks, and logging.

use super::*;
use crate::core::integrations::{DataDogConfig, LangfuseConfig, OpenTelemetryConfig};
use serde::{Deserialize, Serialize};

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct MonitoringConfig {
    /// Metrics configuration
    #[serde(default)]
    pub metrics: MetricsConfig,
    /// Tracing configuration
    #[serde(default)]
    pub tracing: TracingConfig,
    /// Health check configuration
    #[serde(default)]
    pub health: HealthConfig,
    /// Logging configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingConfig>,
    /// External callback exporter configuration
    #[serde(default)]
    pub callbacks: CallbackConfig,
}

impl MonitoringConfig {
    /// Merge monitoring configurations
    pub fn merge(mut self, other: Self) -> Self {
        self.metrics = self.metrics.merge(other.metrics);
        self.tracing = self.tracing.merge(other.tracing);
        self.health = self.health.merge(other.health);
        if other.logging.is_some() {
            self.logging = other.logging;
        }
        self.callbacks = self.callbacks.merge(other.callbacks);
        self
    }
}

/// Non-blocking external callback dispatcher configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallbackConfig {
    /// Maximum lifecycle events waiting for exporter delivery.
    #[serde(default = "default_callback_queue_capacity")]
    pub queue_capacity: usize,
    /// Timeout for each integration callback in milliseconds.
    #[serde(default = "default_callback_timeout_ms")]
    pub timeout_ms: u64,
    /// Enabled callback backends.
    #[serde(default)]
    pub backends: Vec<CallbackBackendConfig>,
}

impl Default for CallbackConfig {
    fn default() -> Self {
        Self {
            queue_capacity: default_callback_queue_capacity(),
            timeout_ms: default_callback_timeout_ms(),
            backends: Vec::new(),
        }
    }
}

impl CallbackConfig {
    /// Merge callback settings, preferring explicitly non-default overlay values.
    pub fn merge(mut self, other: Self) -> Self {
        if other.queue_capacity != default_callback_queue_capacity() {
            self.queue_capacity = other.queue_capacity;
        }
        if other.timeout_ms != default_callback_timeout_ms() {
            self.timeout_ms = other.timeout_ms;
        }
        if !other.backends.is_empty() {
            self.backends = other.backends;
        }
        self
    }
}

/// A configured built-in callback backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
pub enum CallbackBackendConfig {
    /// OTLP exporter backed by the existing OpenTelemetry integration.
    #[serde(rename = "opentelemetry")]
    OpenTelemetry(OpenTelemetryConfig),
    /// Datadog metrics, trace, and log exporter.
    Datadog(DataDogConfig),
    /// Langfuse LLM observability exporter.
    Langfuse(LangfuseConfig),
}

impl CallbackBackendConfig {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::OpenTelemetry(_) => "opentelemetry",
            Self::Datadog(_) => "datadog",
            Self::Langfuse(_) => "langfuse",
        }
    }
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    /// Enable metrics
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Metrics port
    #[serde(default = "default_metrics_port")]
    pub port: u16,
    /// Metrics path / endpoint
    #[serde(default = "default_metrics_path")]
    pub path: String,
    /// Collection interval in seconds
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: default_metrics_port(),
            path: default_metrics_path(),
            interval_seconds: default_interval_seconds(),
        }
    }
}

impl MetricsConfig {
    /// Merge metrics configurations
    pub fn merge(mut self, other: Self) -> Self {
        if !other.enabled {
            self.enabled = other.enabled;
        }
        if other.port != default_metrics_port() {
            self.port = other.port;
        }
        if other.path != default_metrics_path() {
            self.path = other.path;
        }
        if other.interval_seconds != default_interval_seconds() {
            self.interval_seconds = other.interval_seconds;
        }
        self
    }
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TracingConfig {
    /// Enable tracing
    #[serde(default)]
    pub enabled: bool,
    /// Tracing endpoint (e.g. OpenTelemetry collector URL)
    pub endpoint: Option<String>,
    /// Service name
    #[serde(default = "default_service_name")]
    pub service_name: String,
    /// Sampling rate (0.0–1.0)
    #[serde(default = "default_sampling_rate")]
    pub sampling_rate: f64,
    /// Jaeger-specific configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jaeger: Option<JaegerConfig>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: default_service_name(),
            sampling_rate: default_sampling_rate(),
            jaeger: None,
        }
    }
}

impl TracingConfig {
    /// Merge tracing configurations
    pub fn merge(mut self, other: Self) -> Self {
        if other.enabled {
            self.enabled = other.enabled;
        }
        if other.endpoint.is_some() {
            self.endpoint = other.endpoint;
        }
        if other.service_name != default_service_name() {
            self.service_name = other.service_name;
        }
        if (other.sampling_rate - default_sampling_rate()).abs() > f64::EPSILON {
            self.sampling_rate = other.sampling_rate;
        }
        if other.jaeger.is_some() {
            self.jaeger = other.jaeger;
        }
        self
    }
}

/// Jaeger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JaegerConfig {
    /// Agent endpoint
    pub agent_endpoint: String,
    /// Service name
    pub service_name: String,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthConfig {
    /// Health check path
    #[serde(default = "default_health_path")]
    pub path: String,
    /// Enable detailed health checks
    #[serde(default = "default_true")]
    pub detailed: bool,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            path: default_health_path(),
            detailed: true,
        }
    }
}

impl HealthConfig {
    /// Merge health configurations
    pub fn merge(mut self, other: Self) -> Self {
        if other.path != default_health_path() {
            self.path = other.path;
        }
        if !other.detailed {
            self.detailed = other.detailed;
        }
        self
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Output format
    #[serde(default = "default_log_format")]
    pub format: LogFormat,
    /// Output targets
    #[serde(default)]
    pub outputs: Vec<LogOutput>,
}

/// Log format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Text,
    Json,
    Structured,
}

/// Log output target
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogOutput {
    #[serde(rename = "console")]
    Console,
    #[serde(rename = "file")]
    File { path: String },
    #[serde(rename = "syslog")]
    Syslog { facility: String },
}

fn default_interval_seconds() -> u64 {
    15
}

fn default_callback_queue_capacity() -> usize {
    1024
}

fn default_callback_timeout_ms() -> u64 {
    5000
}

fn default_sampling_rate() -> f64 {
    0.1
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> LogFormat {
    LogFormat::Json
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== MetricsConfig Tests ====================

    #[test]
    fn test_metrics_config_default() {
        let config = MetricsConfig::default();
        assert!(config.enabled);
        assert_eq!(config.port, 9090);
        assert_eq!(config.path, "/metrics");
        assert_eq!(config.interval_seconds, 15);
    }

    #[test]
    fn callback_config_defaults_to_disabled_backends() {
        let config = CallbackConfig::default();
        assert_eq!(config.queue_capacity, 1024);
        assert_eq!(config.timeout_ms, 5000);
        assert!(config.backends.is_empty());
    }

    #[test]
    fn callback_config_deserializes_tagged_existing_backend_configs() {
        let yaml = r#"
queue_capacity: 64
timeout_ms: 250
backends:
  - type: opentelemetry
    config:
      enabled: true
      endpoint: http://collector:4318
      service_name: gateway
      service_version: null
      environment: test
      resource_attributes: {}
      export_traces: true
      export_metrics: false
      batch_interval_ms: 100
      max_batch_size: 8
      timeout_ms: 200
      sampling_ratio: 1.0
      headers: {}
"#;
        let config: CallbackConfig = match serde_yml::from_str(yaml) {
            Ok(config) => config,
            Err(error) => panic!("callback config should deserialize: {error}"),
        };
        assert_eq!(config.queue_capacity, 64);
        assert_eq!(config.timeout_ms, 250);
        assert_eq!(config.backends.len(), 1);
        assert_eq!(config.backends[0].kind(), "opentelemetry");
    }

    #[test]
    fn test_metrics_config_structure() {
        let config = MetricsConfig {
            enabled: false,
            port: 8080,
            path: "/prometheus".to_string(),
            interval_seconds: 30,
        };
        assert!(!config.enabled);
        assert_eq!(config.port, 8080);
        assert_eq!(config.path, "/prometheus");
        assert_eq!(config.interval_seconds, 30);
    }

    #[test]
    fn test_metrics_config_serialization() {
        let config = MetricsConfig {
            enabled: true,
            port: 9100,
            path: "/stats".to_string(),
            interval_seconds: 60,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["port"], 9100);
        assert_eq!(json["path"], "/stats");
        assert_eq!(json["interval_seconds"], 60);
    }

    #[test]
    fn test_metrics_config_deserialization() {
        let json = r#"{"enabled": false, "port": 3000, "path": "/api/metrics"}"#;
        let config: MetricsConfig = serde_json::from_str(json).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.port, 3000);
        assert_eq!(config.interval_seconds, 15); // default
    }

    #[test]
    fn test_metrics_config_merge_disabled() {
        let base = MetricsConfig::default();
        let other = MetricsConfig {
            enabled: false,
            ..MetricsConfig::default()
        };
        let merged = base.merge(other);
        assert!(!merged.enabled);
    }

    #[test]
    fn test_metrics_config_merge_port() {
        let base = MetricsConfig::default();
        let other = MetricsConfig {
            port: 8888,
            ..MetricsConfig::default()
        };
        let merged = base.merge(other);
        assert_eq!(merged.port, 8888);
    }

    #[test]
    fn test_metrics_config_merge_interval() {
        let base = MetricsConfig::default();
        let other = MetricsConfig {
            interval_seconds: 60,
            ..MetricsConfig::default()
        };
        let merged = base.merge(other);
        assert_eq!(merged.interval_seconds, 60);
    }

    #[test]
    fn test_metrics_config_clone() {
        let config = MetricsConfig::default();
        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
        assert_eq!(config.port, cloned.port);
    }

    // ==================== TracingConfig Tests ====================

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert!(!config.enabled);
        assert!(config.endpoint.is_none());
        assert_eq!(config.service_name, "litellm-rs");
        assert!((config.sampling_rate - 0.1).abs() < f64::EPSILON);
        assert!(config.jaeger.is_none());
    }

    #[test]
    fn test_tracing_config_structure() {
        let config = TracingConfig {
            enabled: true,
            endpoint: Some("http://jaeger:14268".to_string()),
            service_name: "my-gateway".to_string(),
            sampling_rate: 0.5,
            jaeger: None,
        };
        assert!(config.enabled);
        assert_eq!(config.endpoint, Some("http://jaeger:14268".to_string()));
    }

    #[test]
    fn test_tracing_config_with_jaeger() {
        let config = TracingConfig {
            enabled: true,
            endpoint: None,
            service_name: "gw".to_string(),
            sampling_rate: 1.0,
            jaeger: Some(JaegerConfig {
                agent_endpoint: "localhost:6831".to_string(),
                service_name: "my-service".to_string(),
            }),
        };
        let jaeger = config.jaeger.unwrap();
        assert_eq!(jaeger.agent_endpoint, "localhost:6831");
    }

    #[test]
    fn test_tracing_config_serialization() {
        let config = TracingConfig {
            enabled: true,
            endpoint: Some("http://otel:4317".to_string()),
            service_name: "api-gateway".to_string(),
            sampling_rate: 0.25,
            jaeger: None,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["endpoint"], "http://otel:4317");
    }

    #[test]
    fn test_tracing_config_deserialization() {
        let json =
            r#"{"enabled": true, "endpoint": "http://zipkin:9411", "service_name": "tracer"}"#;
        let config: TracingConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.service_name, "tracer");
        // sampling_rate defaults to 0.1
        assert!((config.sampling_rate - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tracing_config_merge_enabled() {
        let base = TracingConfig::default();
        let other = TracingConfig {
            enabled: true,
            ..TracingConfig::default()
        };
        let merged = base.merge(other);
        assert!(merged.enabled);
    }

    #[test]
    fn test_tracing_config_merge_endpoint() {
        let base = TracingConfig::default();
        let other = TracingConfig {
            endpoint: Some("http://collector:4317".to_string()),
            ..TracingConfig::default()
        };
        let merged = base.merge(other);
        assert_eq!(merged.endpoint, Some("http://collector:4317".to_string()));
    }

    #[test]
    fn test_tracing_config_merge_sampling_rate() {
        let base = TracingConfig::default();
        let other = TracingConfig {
            sampling_rate: 0.5,
            ..TracingConfig::default()
        };
        let merged = base.merge(other);
        assert!((merged.sampling_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tracing_config_clone() {
        let config = TracingConfig::default();
        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
        assert_eq!(config.service_name, cloned.service_name);
    }

    // ==================== HealthConfig Tests ====================

    #[test]
    fn test_health_config_default() {
        let config = HealthConfig::default();
        assert_eq!(config.path, "/health");
        assert!(config.detailed);
    }

    #[test]
    fn test_health_config_structure() {
        let config = HealthConfig {
            path: "/api/health".to_string(),
            detailed: false,
        };
        assert_eq!(config.path, "/api/health");
        assert!(!config.detailed);
    }

    #[test]
    fn test_health_config_serialization() {
        let config = HealthConfig {
            path: "/status".to_string(),
            detailed: true,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["path"], "/status");
        assert_eq!(json["detailed"], true);
    }

    #[test]
    fn test_health_config_deserialization() {
        let json = r#"{"path": "/ready", "detailed": false}"#;
        let config: HealthConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.path, "/ready");
        assert!(!config.detailed);
    }

    #[test]
    fn test_health_config_merge_path() {
        let base = HealthConfig::default();
        let other = HealthConfig {
            path: "/healthz".to_string(),
            detailed: true,
        };
        let merged = base.merge(other);
        assert_eq!(merged.path, "/healthz");
    }

    #[test]
    fn test_health_config_merge_detailed() {
        let base = HealthConfig::default();
        let other = HealthConfig {
            path: "/health".to_string(),
            detailed: false,
        };
        let merged = base.merge(other);
        assert!(!merged.detailed);
    }

    #[test]
    fn test_health_config_clone() {
        let config = HealthConfig::default();
        let cloned = config.clone();
        assert_eq!(config.path, cloned.path);
        assert_eq!(config.detailed, cloned.detailed);
    }

    // ==================== LoggingConfig Tests ====================

    #[test]
    fn test_log_format_serialization() {
        assert_eq!(serde_json::to_string(&LogFormat::Text).unwrap(), "\"text\"");
        assert_eq!(serde_json::to_string(&LogFormat::Json).unwrap(), "\"json\"");
        assert_eq!(
            serde_json::to_string(&LogFormat::Structured).unwrap(),
            "\"structured\""
        );
    }

    #[test]
    fn test_log_output_console() {
        let json = r#"{"type": "console"}"#;
        let output: LogOutput = serde_json::from_str(json).unwrap();
        assert!(matches!(output, LogOutput::Console));
    }

    #[test]
    fn test_log_output_file() {
        let json = r#"{"type": "file", "path": "/tmp/test.log"}"#;
        let output: LogOutput = serde_json::from_str(json).unwrap();
        match output {
            LogOutput::File { path } => assert_eq!(path, "/tmp/test.log"),
            _ => panic!("Expected File"),
        }
    }

    // ==================== MonitoringConfig Tests ====================

    #[test]
    fn test_monitoring_config_default() {
        let config = MonitoringConfig::default();
        assert!(config.metrics.enabled);
        assert!(!config.tracing.enabled);
        assert!(config.health.detailed);
        assert!(config.logging.is_none());
    }

    #[test]
    fn test_monitoring_config_with_logging() {
        let config = MonitoringConfig {
            logging: Some(LoggingConfig {
                level: "debug".to_string(),
                format: LogFormat::Json,
                outputs: vec![LogOutput::Console],
            }),
            ..MonitoringConfig::default()
        };
        assert!(config.logging.is_some());
        assert_eq!(config.logging.unwrap().level, "debug");
    }

    #[test]
    fn test_monitoring_config_serialization() {
        let config = MonitoringConfig::default();
        let json = serde_json::to_value(&config).unwrap();
        assert!(json["metrics"].is_object());
        assert!(json["tracing"].is_object());
        assert!(json["health"].is_object());
    }

    #[test]
    fn test_monitoring_config_merge() {
        let base = MonitoringConfig::default();
        let other = MonitoringConfig {
            metrics: MetricsConfig {
                enabled: false,
                ..MetricsConfig::default()
            },
            ..MonitoringConfig::default()
        };
        let merged = base.merge(other);
        assert!(!merged.metrics.enabled);
    }

    #[test]
    fn test_monitoring_config_merge_logging() {
        let base = MonitoringConfig::default();
        let other = MonitoringConfig {
            logging: Some(LoggingConfig {
                level: "warn".to_string(),
                format: LogFormat::Text,
                outputs: vec![],
            }),
            ..MonitoringConfig::default()
        };
        let merged = base.merge(other);
        assert!(merged.logging.is_some());
    }

    #[test]
    fn test_monitoring_config_clone() {
        let config = MonitoringConfig::default();
        let cloned = config.clone();
        assert_eq!(config.metrics.enabled, cloned.metrics.enabled);
    }
}
