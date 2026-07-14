//! Execution flow for router operations
//!
//! This module contains the execution logic for running operations
//! with retry and fallback support.

use super::config::RouterConfig;
use super::deployment::{DeploymentId, RetrySchedule};
use super::error::{CooldownReason, RouterError};
use super::fallback::{ExecutionResult, FallbackType};
use crate::core::providers::unified_provider::ProviderError;
use rand::Rng;
use std::time::Duration;

/// Check if an error is retryable
///
/// Determines whether a request should be retried based on the error type.
pub fn is_retryable_error(error: &ProviderError) -> bool {
    match error {
        ProviderError::RateLimit { .. }
        | ProviderError::Timeout { .. }
        | ProviderError::ProviderUnavailable { .. }
        | ProviderError::Network { .. } => true,
        ProviderError::QuotaExceeded { .. } => retryable_budget_scope(error).is_some(),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetRetryScope {
    Provider,
    Model,
}

pub(crate) fn retryable_budget_scope(error: &ProviderError) -> Option<BudgetRetryScope> {
    match error {
        ProviderError::QuotaExceeded {
            provider: "budget",
            message,
        } if message.starts_with("provider ") => Some(BudgetRetryScope::Provider),
        ProviderError::QuotaExceeded {
            provider: "budget",
            message,
        } if message.starts_with("model ") => Some(BudgetRetryScope::Model),
        _ => None,
    }
}

/// Calculate retry delay using exponential backoff
///
/// Implements exponential backoff with a maximum delay cap.
/// The formula is: `base * 2^(attempt - 1)`, capped at 30 seconds.
pub fn calculate_retry_delay(config: &RouterConfig, attempt: u32) -> Duration {
    let base = config.retry_after_secs.max(1);
    let delay = base * (2_u64.pow(attempt.saturating_sub(1)));
    Duration::from_secs(delay.min(30)) // Cap at 30 seconds
}

/// Calculate retry delay from a deployment-specific schedule.
pub(crate) fn calculate_retry_delay_for_schedule(
    schedule: &RetrySchedule,
    attempt: u32,
) -> Duration {
    let jitter_sample = if schedule.jitter_ratio == 0.0 {
        0.0
    } else {
        rand::rng().random_range(-1.0..=1.0)
    };

    calculate_retry_delay_for_schedule_with_sample(schedule, attempt, jitter_sample)
}

fn calculate_retry_delay_for_schedule_with_sample(
    schedule: &RetrySchedule,
    attempt: u32,
    jitter_sample: f64,
) -> Duration {
    let jitter_multiplier = 1.0 + schedule.jitter_ratio * jitter_sample.clamp(-1.0, 1.0);
    if jitter_multiplier <= 0.0 {
        return Duration::ZERO;
    }

    let exponent = f64::from(attempt.saturating_sub(1));
    let base_delay = schedule.base_delay_ms as f64 * schedule.backoff_multiplier.powf(exponent);
    let delay_ms = base_delay * jitter_multiplier;
    let max_delay = Duration::from_millis(schedule.max_delay_ms);

    if !delay_ms.is_finite() || delay_ms >= schedule.max_delay_ms as f64 {
        return max_delay;
    }
    if delay_ms <= 0.0 {
        return Duration::ZERO;
    }

    Duration::from_secs_f64(delay_ms / 1_000.0).min(max_delay)
}

/// Infer fallback type from a ProviderError
///
/// Analyzes the error to determine which type of fallback should be used.
pub fn infer_fallback_type(error: &ProviderError) -> FallbackType {
    match error {
        // Context length exceeded -> use context window fallback
        ProviderError::ContextLengthExceeded { .. } => FallbackType::ContextWindow,

        // Content filtered -> use content policy fallback
        ProviderError::ContentFiltered { .. } => FallbackType::ContentPolicy,

        // Rate limit -> use rate limit fallback
        ProviderError::RateLimit { .. } => FallbackType::RateLimit,

        // All other errors -> use general fallback
        _ => FallbackType::General,
    }
}

/// Infer cooldown reason from a ProviderError
///
/// Maps provider error types to cooldown reasons based on the error characteristics.
pub fn infer_cooldown_reason(error: &ProviderError) -> CooldownReason {
    match error {
        // Rate limit errors
        ProviderError::RateLimit { .. } => CooldownReason::RateLimit,

        // Authentication errors
        ProviderError::Authentication { .. } => CooldownReason::AuthError,

        // Model/deployment not found
        ProviderError::ModelNotFound { .. } | ProviderError::DeploymentError { .. } => {
            CooldownReason::NotFound
        }

        // Timeout errors
        ProviderError::Timeout { .. } => CooldownReason::Timeout,

        // Bedrock permission failures retain HTTP 403 while cooling down the deployment.
        ProviderError::ApiError {
            provider: "bedrock",
            status: 403,
            ..
        } => CooldownReason::AuthError,

        // API errors - map based on status code
        ProviderError::ApiError { status, .. } => match *status {
            401 => CooldownReason::AuthError,
            404 => CooldownReason::NotFound,
            408 => CooldownReason::Timeout,
            429 => CooldownReason::RateLimit,
            _ => CooldownReason::ConsecutiveFailures,
        },

        // All other errors are treated as consecutive failures
        _ => CooldownReason::ConsecutiveFailures,
    }
}

/// Convert RouterError to ProviderError for consistency
pub fn router_error_to_provider_error(err: RouterError) -> ProviderError {
    match err {
        RouterError::InvalidConfiguration(msg) => ProviderError::Configuration {
            provider: "router",
            message: msg,
        },
        RouterError::ModelNotFound(msg) => ProviderError::model_not_found("router", msg),
        RouterError::NoAvailableDeployment(msg) => ProviderError::ProviderUnavailable {
            provider: "router",
            message: format!("No available deployment: {}", msg),
        },
        RouterError::UnsupportedCapability { model, capability } => ProviderError::invalid_request(
            "router",
            format!("Model '{model}' does not support capability {capability}"),
        ),
        RouterError::AllDeploymentsInCooldown(msg) => ProviderError::ProviderUnavailable {
            provider: "router",
            message: format!("All deployments in cooldown: {}", msg),
        },
        RouterError::DeploymentNotFound(msg) => ProviderError::DeploymentError {
            provider: "router",
            deployment: msg.clone(),
            message: "Deployment not found".to_string(),
        },
        RouterError::RateLimitExceeded(_msg) => ProviderError::rate_limit("router", Some(60)),
        RouterError::AliasCycle(msg) => ProviderError::Other {
            provider: "router",
            message: format!("Circular alias detected: {}", msg),
        },
        RouterError::FallbackCycle(msg) => ProviderError::Other {
            provider: "router",
            message: format!("Circular fallback chain detected: {}", msg),
        },
    }
}

/// Convert final ProviderError back to RouterError
pub fn provider_error_to_router_error(err: ProviderError, model_name: &str) -> RouterError {
    match err {
        ProviderError::ModelNotFound { model, .. } => RouterError::ModelNotFound(model),
        ProviderError::RateLimit { .. } => RouterError::RateLimitExceeded(model_name.to_string()),
        _ => RouterError::NoAvailableDeployment(format!("{}: {}", model_name, err)),
    }
}

/// Build execution result from successful execution
pub fn build_execution_result<T>(
    result: T,
    deployment_id: DeploymentId,
    attempts: u32,
    model_used: String,
    used_fallback: bool,
    latency_us: u64,
) -> ExecutionResult<T> {
    ExecutionResult {
        result,
        deployment_id,
        attempts,
        model_used,
        used_fallback,
        latency_us,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(jitter_ratio: f64) -> RetrySchedule {
        RetrySchedule {
            base_delay_ms: 250,
            max_delay_ms: 900,
            backoff_multiplier: 2.0,
            jitter_ratio,
        }
    }

    #[test]
    fn deployment_retry_schedule_applies_backoff_and_cap() {
        let schedule = schedule(0.0);

        assert_eq!(
            calculate_retry_delay_for_schedule(&schedule, 1),
            Duration::from_millis(250)
        );
        assert_eq!(
            calculate_retry_delay_for_schedule(&schedule, 2),
            Duration::from_millis(500)
        );
        assert_eq!(
            calculate_retry_delay_for_schedule(&schedule, 3),
            Duration::from_millis(900)
        );
    }

    #[test]
    fn deployment_retry_schedule_bounds_jitter_before_hard_cap() {
        let schedule = RetrySchedule {
            base_delay_ms: 1_000,
            max_delay_ms: 1_100,
            backoff_multiplier: 1.0,
            jitter_ratio: 0.2,
        };

        assert_eq!(
            calculate_retry_delay_for_schedule_with_sample(&schedule, 1, -1.0),
            Duration::from_millis(800)
        );
        assert_eq!(
            calculate_retry_delay_for_schedule_with_sample(&schedule, 1, 1.0),
            Duration::from_millis(1_100)
        );
    }

    #[test]
    fn deployment_retry_schedule_saturates_large_attempts() {
        let schedule = schedule(0.0);

        assert_eq!(
            calculate_retry_delay_for_schedule(&schedule, u32::MAX),
            Duration::from_millis(900)
        );
    }

    #[test]
    fn deployment_retry_schedule_keeps_exact_integer_cap_after_float_conversion() {
        let max_delay_ms = 9_007_199_254_740_995;
        let schedule = RetrySchedule {
            base_delay_ms: max_delay_ms,
            max_delay_ms,
            backoff_multiplier: 1.0,
            jitter_ratio: 0.0,
        };

        assert_eq!(
            calculate_retry_delay_for_schedule(&schedule, 1),
            Duration::from_millis(max_delay_ms)
        );
    }

    #[test]
    fn deployment_retry_schedule_preserves_fractional_backoff() {
        let schedule = RetrySchedule {
            base_delay_ms: 1,
            max_delay_ms: 10,
            backoff_multiplier: 0.5,
            jitter_ratio: 0.0,
        };

        assert_eq!(
            calculate_retry_delay_for_schedule(&schedule, 2),
            Duration::from_micros(500)
        );
        assert_eq!(
            calculate_retry_delay_for_schedule(&schedule, 3),
            Duration::from_micros(250)
        );
    }

    #[test]
    fn deployment_retry_schedule_preserves_fractional_jitter_bounds() {
        let schedule = RetrySchedule {
            base_delay_ms: 3,
            max_delay_ms: 10,
            backoff_multiplier: 1.0,
            jitter_ratio: 0.2,
        };

        assert_eq!(
            calculate_retry_delay_for_schedule_with_sample(&schedule, 1, -1.0),
            Duration::from_micros(2_400)
        );
        assert_eq!(
            calculate_retry_delay_for_schedule_with_sample(&schedule, 1, 1.0),
            Duration::from_micros(3_600)
        );
    }

    #[test]
    fn deployment_retry_schedule_zero_jitter_endpoint_wins_before_overflow() {
        let schedule = RetrySchedule {
            base_delay_ms: 250,
            max_delay_ms: 900,
            backoff_multiplier: 2.0,
            jitter_ratio: 1.0,
        };

        assert_eq!(
            calculate_retry_delay_for_schedule_with_sample(&schedule, u32::MAX, -1.0),
            Duration::ZERO
        );
    }
}
