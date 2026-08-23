//! Monitoring configuration validators
//!
//! This module provides validation implementations for monitoring-related
//! configuration structures including MonitoringConfig, MetricsConfig,
//! TracingConfig, and HealthConfig.

use super::trait_def::Validate;
use crate::config::models::monitoring::{
    CallbackBackendConfig, CallbackConfig, HealthConfig, MetricsConfig, MonitoringConfig,
    TracingConfig,
};
use std::collections::HashSet;
use tracing::debug;

impl Validate for MonitoringConfig {
    fn validate(&self) -> Result<(), String> {
        debug!("Validating monitoring configuration");

        self.metrics.validate()?;
        self.tracing.validate()?;
        self.health.validate()?;
        self.callbacks.validate()?;

        Ok(())
    }
}

impl Validate for CallbackConfig {
    fn validate(&self) -> Result<(), String> {
        if self.queue_capacity < 2 {
            return Err("Callback queue capacity must be at least 2".to_string());
        }
        if self.timeout_ms == 0 {
            return Err("Callback timeout must be greater than 0".to_string());
        }

        let mut kinds = HashSet::new();
        for backend in &self.backends {
            if !kinds.insert(backend.kind()) {
                return Err(format!("Duplicate callback backend: {}", backend.kind()));
            }
            validate_callback_backend(backend)?;
        }
        Ok(())
    }
}

fn validate_callback_backend(backend: &CallbackBackendConfig) -> Result<(), String> {
    match backend {
        CallbackBackendConfig::OpenTelemetry(config) => {
            if !config.enabled {
                return Err("Configured OpenTelemetry callback backend must be enabled".to_string());
            }
            validate_http_endpoint("OpenTelemetry callback endpoint", &config.endpoint)?;
            if config.timeout_ms == 0 {
                return Err("OpenTelemetry callback timeout must be greater than 0".to_string());
            }
            if config.max_batch_size == 0 {
                return Err(
                    "OpenTelemetry callback max_batch_size must be greater than 0".to_string(),
                );
            }
            if !config.sampling_ratio.is_finite() || !(0.0..=1.0).contains(&config.sampling_ratio) {
                return Err(
                    "OpenTelemetry callback sampling_ratio must be between 0.0 and 1.0".to_string(),
                );
            }
        }
        CallbackBackendConfig::Datadog(config) => {
            if config.api_key.trim().is_empty() {
                return Err("Datadog callback api_key cannot be empty".to_string());
            }
            if !crate::core::integrations::DataDogConfig::is_supported_site(&config.site) {
                return Err("Datadog callback site is not a supported Datadog site".to_string());
            }
            if config.batch_size == 0 {
                return Err("Datadog callback batch_size must be greater than 0".to_string());
            }
            if config.flush_interval_ms == 0 {
                return Err("Datadog callback flush_interval_ms must be greater than 0".to_string());
            }
        }
        CallbackBackendConfig::Langfuse(config) => {
            if !config.is_valid() {
                return Err(
                    "Langfuse callback requires enabled=true plus public_key and secret_key"
                        .to_string(),
                );
            }
            validate_http_endpoint("Langfuse callback host", &config.host)?;
            if config.batch_size == 0 {
                return Err("Langfuse callback batch_size must be greater than 0".to_string());
            }
            if config.flush_interval_ms == 0 {
                return Err(
                    "Langfuse callback flush_interval_ms must be greater than 0".to_string()
                );
            }
        }
        CallbackBackendConfig::Prometheus(config) => {
            if !config.enabled {
                return Err("Configured Prometheus callback backend must be enabled".to_string());
            }
            if config.prefix.trim().is_empty() {
                return Err("Prometheus callback prefix cannot be empty".to_string());
            }
        }
    }
    Ok(())
}

fn validate_http_endpoint(label: &str, value: &str) -> Result<(), String> {
    let parsed = url::Url::parse(value).map_err(|error| format!("{label} is invalid: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("{label} must use http or https"));
    }
    Ok(())
}

impl Validate for MetricsConfig {
    fn validate(&self) -> Result<(), String> {
        if self.enabled && self.port == 0 {
            return Err("Metrics port must be greater than 0 when metrics are enabled".to_string());
        }

        if self.path.is_empty() {
            return Err("Metrics path cannot be empty".to_string());
        }

        if !self.path.starts_with('/') {
            return Err("Metrics path must start with '/'".to_string());
        }

        Ok(())
    }
}

impl Validate for TracingConfig {
    fn validate(&self) -> Result<(), String> {
        if self.enabled && self.endpoint.is_none() {
            return Err("Tracing endpoint must be specified when tracing is enabled".to_string());
        }

        if self.service_name.is_empty() {
            return Err("Service name cannot be empty".to_string());
        }

        Ok(())
    }
}

impl Validate for HealthConfig {
    fn validate(&self) -> Result<(), String> {
        if self.path.is_empty() {
            return Err("Health check path cannot be empty".to_string());
        }

        if !self.path.starts_with('/') {
            return Err("Health check path must start with '/'".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::trait_def::Validate;
    use super::*;

    // Helper to call the Validate trait method explicitly
    fn validate_config<T: Validate>(config: &T) -> Result<(), String> {
        Validate::validate(config)
    }

    #[test]
    fn callback_config_rejects_duplicate_backends() {
        let config = CallbackConfig {
            backends: vec![
                CallbackBackendConfig::OpenTelemetry(
                    crate::core::integrations::OpenTelemetryConfig::default(),
                ),
                CallbackBackendConfig::OpenTelemetry(
                    crate::core::integrations::OpenTelemetryConfig::default(),
                ),
            ],
            ..CallbackConfig::default()
        };
        assert_eq!(
            validate_config(&config).unwrap_err(),
            "Duplicate callback backend: opentelemetry"
        );
    }

    #[test]
    fn callback_config_rejects_capacity_below_lifecycle_pair() {
        for queue_capacity in [0, 1] {
            let config = CallbackConfig {
                queue_capacity,
                ..CallbackConfig::default()
            };
            assert_eq!(
                validate_config(&config).unwrap_err(),
                "Callback queue capacity must be at least 2"
            );
        }
    }

    #[test]
    fn callback_config_rejects_datadog_site_host_confusion() {
        let config = CallbackConfig {
            backends: vec![CallbackBackendConfig::Datadog(
                crate::core::integrations::DataDogConfig::new("test-api-key")
                    .site("datadoghq.com@attacker.invalid"),
            )],
            ..CallbackConfig::default()
        };

        assert_eq!(
            validate_config(&config).unwrap_err(),
            "Datadog callback site is not a supported Datadog site"
        );
    }

    // ==================== MetricsConfig Validation Tests ====================

    #[test]
    fn test_metrics_config_valid() {
        let config = MetricsConfig {
            enabled: true,
            port: 9090,
            path: "/metrics".to_string(),
            ..MetricsConfig::default()
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_metrics_config_disabled_with_zero_port() {
        let config = MetricsConfig {
            enabled: false,
            port: 0,
            path: "/metrics".to_string(),
            ..MetricsConfig::default()
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_metrics_config_enabled_with_zero_port() {
        let config = MetricsConfig {
            enabled: true,
            port: 0,
            path: "/metrics".to_string(),
            ..MetricsConfig::default()
        };
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("port must be greater than 0"));
    }

    #[test]
    fn test_metrics_config_empty_path() {
        let config = MetricsConfig {
            enabled: true,
            port: 9090,
            path: "".to_string(),
            ..MetricsConfig::default()
        };
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("path cannot be empty"));
    }

    #[test]
    fn test_metrics_config_path_without_leading_slash() {
        let config = MetricsConfig {
            enabled: true,
            port: 9090,
            path: "metrics".to_string(),
            ..MetricsConfig::default()
        };
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must start with '/'"));
    }

    #[test]
    fn test_metrics_config_custom_path() {
        let config = MetricsConfig {
            enabled: true,
            port: 9090,
            path: "/custom/metrics/path".to_string(),
            ..MetricsConfig::default()
        };
        assert!(validate_config(&config).is_ok());
    }

    // ==================== TracingConfig Validation Tests ====================

    #[test]
    fn test_tracing_config_valid() {
        let config = TracingConfig {
            enabled: true,
            endpoint: Some("http://localhost:4317".to_string()),
            service_name: "gateway".to_string(),
            ..TracingConfig::default()
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_tracing_config_disabled_no_endpoint() {
        let config = TracingConfig {
            enabled: false,
            endpoint: None,
            service_name: "gateway".to_string(),
            ..TracingConfig::default()
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_tracing_config_enabled_no_endpoint() {
        let config = TracingConfig {
            enabled: true,
            endpoint: None,
            service_name: "gateway".to_string(),
            ..TracingConfig::default()
        };
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("endpoint must be specified"));
    }

    #[test]
    fn test_tracing_config_empty_service_name() {
        let config = TracingConfig {
            enabled: false,
            endpoint: None,
            service_name: "".to_string(),
            ..TracingConfig::default()
        };
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Service name cannot be empty"));
    }

    // ==================== HealthConfig Validation Tests ====================

    #[test]
    fn test_health_config_valid() {
        let config = HealthConfig {
            path: "/health".to_string(),
            ..Default::default()
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_health_config_empty_path() {
        let config = HealthConfig {
            path: "".to_string(),
            ..Default::default()
        };
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("path cannot be empty"));
    }

    #[test]
    fn test_health_config_path_without_leading_slash() {
        let config = HealthConfig {
            path: "health".to_string(),
            ..Default::default()
        };
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must start with '/'"));
    }

    #[test]
    fn test_health_config_custom_path() {
        let config = HealthConfig {
            path: "/api/v1/health".to_string(),
            ..Default::default()
        };
        assert!(validate_config(&config).is_ok());
    }

    // ==================== MonitoringConfig Validation Tests ====================

    #[test]
    fn test_monitoring_config_valid() {
        let config = MonitoringConfig::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_monitoring_config_with_invalid_metrics() {
        let mut config = MonitoringConfig::default();
        config.metrics.enabled = true;
        config.metrics.port = 0;

        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_monitoring_config_with_invalid_tracing() {
        let mut config = MonitoringConfig::default();
        config.tracing.enabled = true;
        config.tracing.endpoint = None;

        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_monitoring_config_with_invalid_health() {
        let mut config = MonitoringConfig::default();
        config.health.path = "".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
    }
}
