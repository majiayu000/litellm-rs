//! Router configuration types
//!
//! This module defines configuration types for the router including
//! routing strategies and router settings.

/// Routing strategy enumeration
///
/// Defines how the router selects which deployment to use when multiple deployments
/// are available for the same model.
///
/// ## Strategies
///
/// - **SimpleShuffle**: Weighted random selection (default, good for even distribution)
/// - **LeastBusy**: Select deployment with fewest active requests (good for balanced load)
/// - **UsageBased**: Select deployment with lowest TPM usage rate (good for rate limit optimization)
/// - **LatencyBased**: Select deployment with lowest average latency (good for performance)
/// - **PriorityBased**: Select deployment with lowest priority value (good for priority-based routing)
/// - **RateLimitAware**: Avoid deployments near rate limits (good for avoiding 429s)
/// - **RoundRobin**: Simple round-robin selection (good for predictable distribution)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    /// Weighted random selection (considers deployment weights)
    #[default]
    SimpleShuffle,
    /// Select deployment with fewest active requests
    LeastBusy,
    /// Select deployment with lowest TPM usage rate
    UsageBased,
    /// Select deployment with lowest average latency
    LatencyBased,
    /// Select deployment with lowest priority value
    #[serde(alias = "cost_based")]
    PriorityBased,
    /// Avoid deployments near rate limits
    RateLimitAware,
    /// Simple round-robin selection
    RoundRobin,
}

/// Router configuration
///
/// Contains global settings for router behavior including retry policies,
/// cooldown parameters, and feature flags.
///
/// ## Defaults
///
/// - `routing_strategy`: SimpleShuffle
/// - `num_retries`: 3
/// - `retry_after_secs`: 0 (no delay between retries)
/// - `allowed_fails`: 3 (failures before cooldown)
/// - `cooldown_time_secs`: 5
/// - `timeout_secs`: 60
/// - `max_fallbacks`: 5
/// - `enable_pre_call_checks`: true
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Routing strategy to use for deployment selection
    pub routing_strategy: RoutingStrategy,

    /// Number of retry attempts on failure (default: 3, so up to 4 attempts).
    ///
    /// Streaming calls retry only until a provider returns a stream. Set 0 for
    /// one attempt per call when the embedding application owns retries.
    pub num_retries: u32,

    /// Minimum seconds to wait between retries (default: 0)
    pub retry_after_secs: u64,

    /// Number of failures allowed before entering cooldown (default: 3)
    pub allowed_fails: u32,

    /// Minimum total requests before the circuit breaker can trip (default: 10)
    ///
    /// The breaker will not enter cooldown until the deployment has received at
    /// least this many requests in the current minute, preventing premature
    /// tripping on small sample sizes.
    pub min_requests: u32,

    /// Cooldown duration in seconds (default: 5)
    pub cooldown_time_secs: u64,

    /// Consecutive successes needed to promote from Degraded to Healthy (default: 3)
    ///
    /// After a deployment exits cooldown it enters the Degraded (half-open)
    /// state. This many consecutive successful requests must occur before the
    /// deployment is promoted back to Healthy.
    pub success_threshold: u32,

    /// Default timeout for requests in seconds (default: 60)
    pub timeout_secs: u64,

    /// Maximum number of fallback attempts (default: 5)
    pub max_fallbacks: u32,

    /// Enable pre-call validation checks (default: true)
    pub enable_pre_call_checks: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            routing_strategy: RoutingStrategy::SimpleShuffle,
            num_retries: 3,
            retry_after_secs: 0,
            allowed_fails: 3,
            min_requests: 10,
            cooldown_time_secs: 5,
            success_threshold: 3,
            timeout_secs: 60,
            max_fallbacks: 5,
            enable_pre_call_checks: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== RoutingStrategy Tests ====================

    #[test]
    fn test_routing_strategy_default() {
        let strategy = RoutingStrategy::default();
        assert_eq!(strategy, RoutingStrategy::SimpleShuffle);
    }

    #[test]
    fn test_routing_strategy_all_variants() {
        let strategies = [
            RoutingStrategy::SimpleShuffle,
            RoutingStrategy::LeastBusy,
            RoutingStrategy::UsageBased,
            RoutingStrategy::LatencyBased,
            RoutingStrategy::PriorityBased,
            RoutingStrategy::RateLimitAware,
            RoutingStrategy::RoundRobin,
        ];

        assert_eq!(strategies.len(), 7);
    }

    #[test]
    fn test_routing_strategy_equality() {
        let s1 = RoutingStrategy::LeastBusy;
        let s2 = RoutingStrategy::LeastBusy;
        let s3 = RoutingStrategy::LatencyBased;

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_routing_strategy_clone() {
        let original = RoutingStrategy::PriorityBased;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_routing_strategy_copy() {
        let s1 = RoutingStrategy::RoundRobin;
        let s2 = s1; // Copy
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_routing_strategy_debug() {
        let strategy = RoutingStrategy::LatencyBased;
        let debug_str = format!("{:?}", strategy);
        assert!(debug_str.contains("LatencyBased"));
    }

    // ==================== RouterConfig Tests ====================

    #[test]
    fn test_router_config_default() {
        let config = RouterConfig::default();

        assert_eq!(config.routing_strategy, RoutingStrategy::SimpleShuffle);
        assert_eq!(config.num_retries, 3);
        assert_eq!(config.retry_after_secs, 0);
        assert_eq!(config.allowed_fails, 3);
        assert_eq!(config.min_requests, 10);
        assert_eq!(config.cooldown_time_secs, 5);
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.max_fallbacks, 5);
        assert!(config.enable_pre_call_checks);
    }

    #[test]
    fn test_router_config_custom() {
        let config = RouterConfig {
            routing_strategy: RoutingStrategy::LatencyBased,
            num_retries: 5,
            retry_after_secs: 2,
            allowed_fails: 5,
            min_requests: 20,
            cooldown_time_secs: 10,
            success_threshold: 5,
            timeout_secs: 120,
            max_fallbacks: 10,
            enable_pre_call_checks: false,
        };

        assert_eq!(config.routing_strategy, RoutingStrategy::LatencyBased);
        assert_eq!(config.num_retries, 5);
        assert_eq!(config.retry_after_secs, 2);
        assert_eq!(config.min_requests, 20);
        assert_eq!(config.success_threshold, 5);
        assert!(!config.enable_pre_call_checks);
    }

    #[test]
    fn test_router_config_clone() {
        let config = RouterConfig::default();
        let cloned = config.clone();

        assert_eq!(cloned.routing_strategy, config.routing_strategy);
        assert_eq!(cloned.num_retries, config.num_retries);
        assert_eq!(cloned.timeout_secs, config.timeout_secs);
    }

    #[test]
    fn test_router_config_debug() {
        let config = RouterConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("RouterConfig"));
        assert!(debug_str.contains("SimpleShuffle"));
        assert!(debug_str.contains("num_retries"));
    }

    #[test]
    fn test_router_config_high_availability() {
        // Configuration for high availability scenarios
        let config = RouterConfig {
            routing_strategy: RoutingStrategy::LeastBusy,
            num_retries: 10,
            retry_after_secs: 1,
            allowed_fails: 10,
            min_requests: 20,
            cooldown_time_secs: 30,
            success_threshold: 5,
            timeout_secs: 30,
            max_fallbacks: 20,
            enable_pre_call_checks: true,
        };

        assert!(config.num_retries > 5);
        assert!(config.max_fallbacks > 10);
    }

    #[test]
    fn test_router_config_low_latency() {
        // Configuration for low latency scenarios
        let config = RouterConfig {
            routing_strategy: RoutingStrategy::LatencyBased,
            num_retries: 1,
            retry_after_secs: 0,
            allowed_fails: 1,
            min_requests: 1,
            cooldown_time_secs: 2,
            success_threshold: 1,
            timeout_secs: 10,
            max_fallbacks: 2,
            enable_pre_call_checks: false,
        };

        assert_eq!(config.routing_strategy, RoutingStrategy::LatencyBased);
        assert!(config.timeout_secs < 30);
        assert!(!config.enable_pre_call_checks);
    }

    #[test]
    fn test_router_config_priority_based() {
        // Configuration for priority-based routing
        let config = RouterConfig {
            routing_strategy: RoutingStrategy::PriorityBased,
            num_retries: 3,
            retry_after_secs: 5,
            allowed_fails: 5,
            cooldown_time_secs: 60,
            timeout_secs: 120,
            max_fallbacks: 3,
            ..RouterConfig::default()
        };

        assert_eq!(config.routing_strategy, RoutingStrategy::PriorityBased);
    }

    #[test]
    fn test_router_config_rate_limit_aware() {
        // Configuration to avoid rate limits
        let config = RouterConfig {
            routing_strategy: RoutingStrategy::RateLimitAware,
            num_retries: 5,
            retry_after_secs: 10,
            allowed_fails: 2,
            cooldown_time_secs: 60,
            timeout_secs: 60,
            max_fallbacks: 10,
            ..RouterConfig::default()
        };

        assert_eq!(config.routing_strategy, RoutingStrategy::RateLimitAware);
        assert!(config.retry_after_secs > 0);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_config_with_all_strategies() {
        let strategies = vec![
            RoutingStrategy::SimpleShuffle,
            RoutingStrategy::LeastBusy,
            RoutingStrategy::UsageBased,
            RoutingStrategy::LatencyBased,
            RoutingStrategy::PriorityBased,
            RoutingStrategy::RateLimitAware,
            RoutingStrategy::RoundRobin,
        ];

        for strategy in strategies {
            let config = RouterConfig {
                routing_strategy: strategy,
                ..RouterConfig::default()
            };
            assert_eq!(config.routing_strategy, strategy);
        }
    }

    #[test]
    fn test_config_retry_behavior() {
        // Test that retry settings are consistent
        let config = RouterConfig {
            num_retries: 5,
            retry_after_secs: 2,
            allowed_fails: 3,
            cooldown_time_secs: 10,
            ..RouterConfig::default()
        };

        // Max total retry wait time
        let max_retry_wait = config.num_retries as u64 * config.retry_after_secs;
        assert_eq!(max_retry_wait, 10);

        // Cooldown should be longer than retry wait
        assert!(config.cooldown_time_secs >= max_retry_wait);
    }

    #[test]
    fn test_config_timeout_vs_cooldown() {
        let config = RouterConfig::default();

        // Timeout should be longer than cooldown for typical scenarios
        assert!(config.timeout_secs >= config.cooldown_time_secs);
    }

    #[test]
    fn test_config_zero_retries() {
        let config = RouterConfig {
            num_retries: 0,
            retry_after_secs: 0,
            ..RouterConfig::default()
        };

        assert_eq!(config.num_retries, 0);
    }

    #[test]
    fn test_config_disabled_pre_call_checks() {
        let mut config = RouterConfig::default();
        assert!(config.enable_pre_call_checks);

        config.enable_pre_call_checks = false;
        assert!(!config.enable_pre_call_checks);
    }
}
