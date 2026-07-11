//! Router error types
//!
//! This module defines error types for the router system including
//! routing errors and cooldown triggers.

/// Cooldown trigger reason
///
/// Defines the reasons why a deployment enters cooldown state.
/// Different reasons may have different cooldown behaviors and durations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CooldownReason {
    /// Rate limit (429) - immediate cooldown
    RateLimit,
    /// Authentication error (401) - immediate cooldown
    AuthError,
    /// Not found (404) - immediate cooldown
    NotFound,
    /// Timeout (408) - immediate cooldown
    Timeout,
    /// Consecutive failures exceeded threshold
    ConsecutiveFailures,
    /// High failure rate (>50%)
    HighFailureRate,
    /// Manual cooldown
    Manual,
}

/// Router error types
///
/// Defines errors that can occur during routing operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RouterError {
    /// Gateway configuration cannot be represented safely by the runtime router.
    #[error("Invalid router configuration: {0}")]
    InvalidConfiguration(String),

    /// Model not found in router configuration
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// No available deployment for the requested model
    #[error("No available deployment for model: {0}")]
    NoAvailableDeployment(String),

    /// The model exists, but no deployment supports the requested capability
    #[error("Model '{model}' does not support capability: {capability}")]
    UnsupportedCapability { model: String, capability: String },

    /// Deployment not found by ID
    #[error("Deployment not found: {0}")]
    DeploymentNotFound(String),

    /// All deployments are in cooldown state
    #[error("All deployments in cooldown for model: {0}")]
    AllDeploymentsInCooldown(String),

    /// Rate limit exceeded for model
    #[error("Rate limit exceeded for model: {0}")]
    RateLimitExceeded(String),

    /// Circular alias detected
    #[error("Circular alias detected: {0}")]
    AliasCycle(String),

    /// Circular fallback chain detected
    #[error("Circular fallback chain detected: {0}")]
    FallbackCycle(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== CooldownReason Tests ====================

    #[test]
    fn test_cooldown_reason_rate_limit() {
        let reason = CooldownReason::RateLimit;
        assert_eq!(reason, CooldownReason::RateLimit);
    }

    #[test]
    fn test_cooldown_reason_auth_error() {
        let reason = CooldownReason::AuthError;
        assert_eq!(reason, CooldownReason::AuthError);
    }

    #[test]
    fn test_cooldown_reason_not_found() {
        let reason = CooldownReason::NotFound;
        assert_eq!(reason, CooldownReason::NotFound);
    }

    #[test]
    fn test_cooldown_reason_timeout() {
        let reason = CooldownReason::Timeout;
        assert_eq!(reason, CooldownReason::Timeout);
    }

    #[test]
    fn test_cooldown_reason_consecutive_failures() {
        let reason = CooldownReason::ConsecutiveFailures;
        assert_eq!(reason, CooldownReason::ConsecutiveFailures);
    }

    #[test]
    fn test_cooldown_reason_high_failure_rate() {
        let reason = CooldownReason::HighFailureRate;
        assert_eq!(reason, CooldownReason::HighFailureRate);
    }

    #[test]
    fn test_cooldown_reason_manual() {
        let reason = CooldownReason::Manual;
        assert_eq!(reason, CooldownReason::Manual);
    }

    #[test]
    fn test_cooldown_reason_clone() {
        let reason = CooldownReason::RateLimit;
        let cloned = reason;
        assert_eq!(reason, cloned);
    }

    #[test]
    fn test_cooldown_reason_copy() {
        let reason = CooldownReason::Timeout;
        let copied = reason;
        assert_eq!(reason, copied);
    }

    #[test]
    fn test_cooldown_reason_debug() {
        let reason = CooldownReason::ConsecutiveFailures;
        let debug_str = format!("{:?}", reason);
        assert_eq!(debug_str, "ConsecutiveFailures");
    }

    #[test]
    fn test_cooldown_reason_equality() {
        assert_eq!(CooldownReason::RateLimit, CooldownReason::RateLimit);
        assert_ne!(CooldownReason::RateLimit, CooldownReason::AuthError);
    }

    #[test]
    fn test_cooldown_reason_all_variants() {
        let reasons = [
            CooldownReason::RateLimit,
            CooldownReason::AuthError,
            CooldownReason::NotFound,
            CooldownReason::Timeout,
            CooldownReason::ConsecutiveFailures,
            CooldownReason::HighFailureRate,
            CooldownReason::Manual,
        ];

        assert_eq!(reasons.len(), 7);
        // Verify all are unique
        for (i, r1) in reasons.iter().enumerate() {
            for (j, r2) in reasons.iter().enumerate() {
                if i != j {
                    assert_ne!(r1, r2);
                }
            }
        }
    }

    // ==================== RouterError Tests ====================

    #[test]
    fn test_router_error_model_not_found() {
        let error = RouterError::ModelNotFound("gpt-5".to_string());
        assert_eq!(error.to_string(), "Model not found: gpt-5");
    }

    #[test]
    fn test_router_error_invalid_configuration() {
        let error = RouterError::InvalidConfiguration("bad health endpoint".to_string());
        assert_eq!(
            error.to_string(),
            "Invalid router configuration: bad health endpoint"
        );
    }

    #[test]
    fn test_router_error_no_available_deployment() {
        let error = RouterError::NoAvailableDeployment("gpt-4".to_string());
        assert_eq!(
            error.to_string(),
            "No available deployment for model: gpt-4"
        );
    }

    #[test]
    fn test_router_error_unsupported_capability() {
        let error = RouterError::UnsupportedCapability {
            model: "gpt-4".to_string(),
            capability: "TextToSpeech".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Model 'gpt-4' does not support capability: TextToSpeech"
        );
    }

    #[test]
    fn test_router_error_deployment_not_found() {
        let error = RouterError::DeploymentNotFound("dep-123".to_string());
        assert_eq!(error.to_string(), "Deployment not found: dep-123");
    }

    #[test]
    fn test_router_error_all_deployments_in_cooldown() {
        let error = RouterError::AllDeploymentsInCooldown("claude-3".to_string());
        assert_eq!(
            error.to_string(),
            "All deployments in cooldown for model: claude-3"
        );
    }

    #[test]
    fn test_router_error_rate_limit_exceeded() {
        let error = RouterError::RateLimitExceeded("gpt-4-turbo".to_string());
        assert_eq!(
            error.to_string(),
            "Rate limit exceeded for model: gpt-4-turbo"
        );
    }

    #[test]
    fn test_router_error_clone() {
        let error = RouterError::ModelNotFound("test".to_string());
        let cloned = error.clone();
        assert_eq!(error.to_string(), cloned.to_string());
    }

    #[test]
    fn test_router_error_debug() {
        let error = RouterError::RateLimitExceeded("model".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("RateLimitExceeded"));
        assert!(debug_str.contains("model"));
    }

    #[test]
    fn test_router_error_is_error_trait() {
        let error = RouterError::ModelNotFound("test".to_string());
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn test_router_error_empty_string() {
        let error = RouterError::ModelNotFound("".to_string());
        assert_eq!(error.to_string(), "Model not found: ");
    }

    #[test]
    fn test_router_error_special_characters() {
        let error = RouterError::ModelNotFound("model/with:special-chars".to_string());
        assert_eq!(
            error.to_string(),
            "Model not found: model/with:special-chars"
        );
    }

    #[test]
    fn test_router_error_unicode() {
        let error = RouterError::DeploymentNotFound("部署-123".to_string());
        assert_eq!(error.to_string(), "Deployment not found: 部署-123");
    }

    #[test]
    fn test_router_error_all_variants() {
        let errors = vec![
            RouterError::InvalidConfiguration("config".to_string()),
            RouterError::ModelNotFound("a".to_string()),
            RouterError::NoAvailableDeployment("b".to_string()),
            RouterError::UnsupportedCapability {
                model: "b".to_string(),
                capability: "c".to_string(),
            },
            RouterError::DeploymentNotFound("c".to_string()),
            RouterError::AllDeploymentsInCooldown("d".to_string()),
            RouterError::RateLimitExceeded("e".to_string()),
            RouterError::AliasCycle("f".to_string()),
            RouterError::FallbackCycle("g".to_string()),
        ];

        assert_eq!(errors.len(), 9);
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn test_router_error_alias_cycle() {
        let error = RouterError::AliasCycle("a -> b -> a".to_string());
        assert_eq!(error.to_string(), "Circular alias detected: a -> b -> a");
    }

    #[test]
    fn test_router_error_fallback_cycle() {
        let error = RouterError::FallbackCycle("x -> y -> x".to_string());
        assert_eq!(
            error.to_string(),
            "Circular fallback chain detected: x -> y -> x"
        );
    }
}
