//! Monitoring configuration validators
//!
//! This module provides validation implementations for monitoring-related
//! configuration structures including MonitoringConfig, MetricsConfig,
//! TracingConfig, and HealthConfig.

use super::trait_def::Validate;
use crate::config::models::monitoring::{
    HealthConfig, MetricsConfig, MonitoringConfig, TracingConfig,
};
use tracing::debug;

impl Validate for MonitoringConfig {
    fn validate(&self) -> Result<(), String> {
        debug!("Validating monitoring configuration");

        self.metrics.validate()?;
        self.tracing.validate()?;
        self.health.validate()?;

        Ok(())
    }
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
