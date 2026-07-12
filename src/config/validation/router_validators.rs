//! Router configuration validators
//!
//! This module provides validation implementations for router-related configuration
//! structures including RouterConfig, CircuitBreakerConfig, and RetryConfig.

use super::trait_def::Validate;
use crate::config::models::router::{
    CircuitBreakerConfig, GatewayRouterConfig, LoadBalancerConfig,
};
use tracing::debug;

impl Validate for GatewayRouterConfig {
    fn validate(&self) -> Result<(), String> {
        debug!("Validating router configuration");

        self.circuit_breaker.validate()?;
        self.load_balancer.validate()?;

        Ok(())
    }
}

impl Validate for CircuitBreakerConfig {
    fn validate(&self) -> Result<(), String> {
        if self.failure_threshold == 0 {
            return Err("Circuit breaker failure threshold must be greater than 0".to_string());
        }

        if self.recovery_timeout == 0 {
            return Err("Circuit breaker recovery timeout must be greater than 0".to_string());
        }

        if self.min_requests == 0 {
            return Err("Circuit breaker min requests must be greater than 0".to_string());
        }

        Ok(())
    }
}

// Note: RetryConfig validation is implemented in config_validators.rs
// to avoid duplicate implementations and maintain consistency.

impl Validate for LoadBalancerConfig {
    fn validate(&self) -> Result<(), String> {
        if self.sticky_sessions {
            return Err(
                "router.load_balancer.sticky_sessions is not implemented by runtime router yet"
                    .to_string(),
            );
        }

        if self.session_timeout != LoadBalancerConfig::default().session_timeout {
            return Err(
                "router.load_balancer.session_timeout is not implemented by runtime router yet"
                    .to_string(),
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::trait_def::Validate;
    use super::*; // Import the trait explicitly
    use crate::config::models::provider::RetryConfig;

    // Helper to call the Validate trait method explicitly
    fn validate_config<T: Validate>(config: &T) -> Result<(), String> {
        Validate::validate(config)
    }

    fn create_valid_circuit_breaker_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 5,
            recovery_timeout: 60,
            min_requests: 10,
            ..Default::default()
        }
    }

    fn create_valid_retry_config() -> RetryConfig {
        RetryConfig {
            base_delay: 100,
            max_delay: 1000,
            backoff_multiplier: 2.0,
            ..Default::default()
        }
    }

    // ==================== CircuitBreakerConfig Validation Tests ====================

    #[test]
    fn test_circuit_breaker_config_valid() {
        let config = create_valid_circuit_breaker_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_circuit_breaker_zero_failure_threshold() {
        let mut config = create_valid_circuit_breaker_config();
        config.failure_threshold = 0;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failure threshold"));
    }

    #[test]
    fn test_circuit_breaker_high_failure_threshold() {
        let mut config = create_valid_circuit_breaker_config();
        config.failure_threshold = 100;

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_circuit_breaker_zero_recovery_timeout() {
        let mut config = create_valid_circuit_breaker_config();
        config.recovery_timeout = 0;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("recovery timeout"));
    }

    #[test]
    fn test_circuit_breaker_long_recovery_timeout() {
        let mut config = create_valid_circuit_breaker_config();
        config.recovery_timeout = 3600; // 1 hour

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_circuit_breaker_zero_min_requests() {
        let mut config = create_valid_circuit_breaker_config();
        config.min_requests = 0;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("min requests"));
    }

    #[test]
    fn test_circuit_breaker_high_min_requests() {
        let mut config = create_valid_circuit_breaker_config();
        config.min_requests = 1000;

        assert!(validate_config(&config).is_ok());
    }

    // ==================== RetryConfig Validation Tests ====================

    #[test]
    fn test_retry_config_valid() {
        let config = create_valid_retry_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_retry_zero_base_delay() {
        let mut config = create_valid_retry_config();
        config.base_delay = 0;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("base delay"));
    }

    #[test]
    fn test_retry_max_delay_equals_base_delay() {
        let mut config = create_valid_retry_config();
        config.base_delay = 100;
        config.max_delay = 100;

        // Equal base_delay and max_delay is now valid
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_retry_max_delay_less_than_base_delay() {
        let mut config = create_valid_retry_config();
        config.base_delay = 200;
        config.max_delay = 100;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("base delay cannot be greater than max delay")
        );
    }

    #[test]
    fn test_retry_valid_delay_range() {
        let mut config = create_valid_retry_config();
        config.base_delay = 100;
        config.max_delay = 10000;

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_retry_backoff_multiplier_exactly_one() {
        let mut config = create_valid_retry_config();
        config.backoff_multiplier = 1.0;

        // backoff_multiplier = 1.0 is now valid (just needs to be > 0.0)
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_retry_backoff_multiplier_zero() {
        let mut config = create_valid_retry_config();
        config.backoff_multiplier = 0.0;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("backoff multiplier must be greater than 0")
        );
    }

    #[test]
    fn test_retry_backoff_multiplier_negative() {
        let mut config = create_valid_retry_config();
        config.backoff_multiplier = -0.5;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("backoff multiplier must be greater than 0")
        );
    }

    #[test]
    fn test_retry_backoff_multiplier_valid() {
        let mut config = create_valid_retry_config();
        config.backoff_multiplier = 1.5;

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_retry_backoff_multiplier_high() {
        let mut config = create_valid_retry_config();
        config.backoff_multiplier = 10.0;

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_retry_backoff_multiplier_nan() {
        let mut config = create_valid_retry_config();
        config.backoff_multiplier = f64::NAN;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be finite"));
    }

    #[test]
    fn test_retry_backoff_multiplier_infinite() {
        let mut config = create_valid_retry_config();
        config.backoff_multiplier = f64::INFINITY;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be finite"));
    }

    #[test]
    fn test_retry_jitter_nan() {
        let mut config = create_valid_retry_config();
        config.jitter = f64::NAN;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be finite"));
    }

    #[test]
    fn test_retry_jitter_out_of_range() {
        let mut config = create_valid_retry_config();
        config.jitter = 1.1;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("between 0.0 and 1.0"));
    }

    // ==================== RouterConfig Validation Tests ====================

    #[test]
    fn test_router_config_valid() {
        let config = GatewayRouterConfig::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_router_config_with_invalid_circuit_breaker() {
        let mut config = GatewayRouterConfig::default();
        config.circuit_breaker.failure_threshold = 0;

        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_router_config_accepts_runtime_mapped_circuit_breaker_fields() {
        let mut config = GatewayRouterConfig::default();
        config.circuit_breaker.min_requests = 25;
        config.circuit_breaker.success_threshold = 4;

        assert!(validate_config(&config).is_ok());
    }

    // ==================== LoadBalancerConfig Validation Tests ====================

    #[test]
    fn test_load_balancer_config_valid() {
        let config = LoadBalancerConfig::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_load_balancer_rejects_sticky_sessions() {
        let config = LoadBalancerConfig {
            sticky_sessions: true,
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sticky_sessions"));
    }

    #[test]
    fn test_load_balancer_rejects_session_timeout() {
        let config = LoadBalancerConfig {
            session_timeout: 900,
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("session_timeout"));
    }
}
