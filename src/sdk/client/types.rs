//! Type definitions for the LLM client

use std::sync::atomic::AtomicUsize;
use std::time::SystemTime;

/// Provider statistics
#[derive(Debug, Clone, Default)]
pub struct ProviderStats {
    pub requests: u64,
    pub errors: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub avg_latency_ms: f64,
    pub last_used: Option<SystemTime>,
    pub health_score: f64,
}

/// Load balancer
#[derive(Debug)]
pub struct LoadBalancer {
    pub(crate) strategy: LoadBalancingStrategy,
    pub(crate) round_robin_counter: AtomicUsize,
}

/// Load balancing strategy
#[derive(Debug, Clone)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastLatency,
    WeightedRandom,
    HealthBased,
}

impl LoadBalancer {
    pub(crate) fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            round_robin_counter: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ProviderStats Tests ====================

    #[test]
    fn test_provider_stats_default() {
        let stats = ProviderStats::default();

        assert_eq!(stats.requests, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.total_tokens, 0);
        assert!((stats.total_cost - 0.0).abs() < f64::EPSILON);
        assert!((stats.avg_latency_ms - 0.0).abs() < f64::EPSILON);
        assert!(stats.last_used.is_none());
        assert!((stats.health_score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_stats_with_values() {
        let stats = ProviderStats {
            requests: 1000,
            errors: 10,
            total_tokens: 500000,
            total_cost: 25.50,
            avg_latency_ms: 150.0,
            last_used: Some(SystemTime::now()),
            health_score: 0.95,
        };

        assert_eq!(stats.requests, 1000);
        assert_eq!(stats.errors, 10);
        assert_eq!(stats.total_tokens, 500000);
        assert!((stats.total_cost - 25.50).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_stats_clone() {
        let stats = ProviderStats {
            requests: 500,
            errors: 5,
            total_tokens: 250000,
            total_cost: 12.75,
            avg_latency_ms: 100.0,
            last_used: None,
            health_score: 0.90,
        };

        let cloned = stats.clone();
        assert_eq!(cloned.requests, stats.requests);
        assert_eq!(cloned.errors, stats.errors);
        assert_eq!(cloned.health_score, stats.health_score);
    }

    #[test]
    fn test_provider_stats_debug() {
        let stats = ProviderStats::default();
        let debug_str = format!("{:?}", stats);

        assert!(debug_str.contains("ProviderStats"));
        assert!(debug_str.contains("requests"));
    }

    #[test]
    fn test_provider_stats_error_rate() {
        let stats = ProviderStats {
            requests: 100,
            errors: 5,
            total_tokens: 0,
            total_cost: 0.0,
            avg_latency_ms: 0.0,
            last_used: None,
            health_score: 0.0,
        };

        let error_rate = if stats.requests > 0 {
            stats.errors as f64 / stats.requests as f64
        } else {
            0.0
        };

        assert!((error_rate - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_stats_tokens_per_request() {
        let stats = ProviderStats {
            requests: 100,
            errors: 0,
            total_tokens: 50000,
            total_cost: 0.0,
            avg_latency_ms: 0.0,
            last_used: None,
            health_score: 0.0,
        };

        let avg_tokens = if stats.requests > 0 {
            stats.total_tokens as f64 / stats.requests as f64
        } else {
            0.0
        };

        assert!((avg_tokens - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_stats_cost_per_request() {
        let stats = ProviderStats {
            requests: 1000,
            errors: 0,
            total_tokens: 0,
            total_cost: 10.0,
            avg_latency_ms: 0.0,
            last_used: None,
            health_score: 0.0,
        };

        let cost_per_request = if stats.requests > 0 {
            stats.total_cost / stats.requests as f64
        } else {
            0.0
        };

        assert!((cost_per_request - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_stats_high_load() {
        let stats = ProviderStats {
            requests: 1_000_000,
            errors: 1000,
            total_tokens: 500_000_000,
            total_cost: 5000.0,
            avg_latency_ms: 200.0,
            last_used: Some(SystemTime::now()),
            health_score: 0.85,
        };

        assert!(stats.requests > 0);
        assert!(stats.health_score > 0.8);
    }

    #[test]
    fn test_provider_stats_unhealthy() {
        let stats = ProviderStats {
            requests: 100,
            errors: 50, // 50% error rate
            total_tokens: 10000,
            total_cost: 1.0,
            avg_latency_ms: 5000.0, // Very slow
            last_used: Some(SystemTime::now()),
            health_score: 0.2,
        };

        let error_rate = stats.errors as f64 / stats.requests as f64;
        assert!(error_rate > 0.3);
        assert!(stats.health_score < 0.5);
    }

    // ==================== LoadBalancingStrategy Tests ====================

    #[test]
    fn test_load_balancing_strategy_round_robin() {
        let strategy = LoadBalancingStrategy::RoundRobin;
        let debug_str = format!("{:?}", strategy);
        assert!(debug_str.contains("RoundRobin"));
    }

    #[test]
    fn test_load_balancing_strategy_least_latency() {
        let strategy = LoadBalancingStrategy::LeastLatency;
        let debug_str = format!("{:?}", strategy);
        assert!(debug_str.contains("LeastLatency"));
    }

    #[test]
    fn test_load_balancing_strategy_weighted_random() {
        let strategy = LoadBalancingStrategy::WeightedRandom;
        let debug_str = format!("{:?}", strategy);
        assert!(debug_str.contains("WeightedRandom"));
    }

    #[test]
    fn test_load_balancing_strategy_health_based() {
        let strategy = LoadBalancingStrategy::HealthBased;
        let debug_str = format!("{:?}", strategy);
        assert!(debug_str.contains("HealthBased"));
    }

    #[test]
    fn test_load_balancing_strategy_clone() {
        let strategy = LoadBalancingStrategy::LeastLatency;
        let cloned = strategy.clone();

        let original_str = format!("{:?}", strategy);
        let cloned_str = format!("{:?}", cloned);
        assert_eq!(original_str, cloned_str);
    }

    // ==================== LoadBalancer Tests ====================

    #[test]
    fn test_load_balancer_new_round_robin() {
        let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let debug_str = format!("{:?}", balancer);
        assert!(debug_str.contains("RoundRobin"));
    }

    #[test]
    fn test_load_balancer_new_least_latency() {
        let balancer = LoadBalancer::new(LoadBalancingStrategy::LeastLatency);
        let debug_str = format!("{:?}", balancer);
        assert!(debug_str.contains("LeastLatency"));
    }

    #[test]
    fn test_load_balancer_new_weighted_random() {
        let balancer = LoadBalancer::new(LoadBalancingStrategy::WeightedRandom);
        let debug_str = format!("{:?}", balancer);
        assert!(debug_str.contains("WeightedRandom"));
    }

    #[test]
    fn test_load_balancer_new_health_based() {
        let balancer = LoadBalancer::new(LoadBalancingStrategy::HealthBased);
        let debug_str = format!("{:?}", balancer);
        assert!(debug_str.contains("HealthBased"));
    }

    #[test]
    fn test_load_balancer_debug() {
        let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let debug_str = format!("{:?}", balancer);
        assert!(debug_str.contains("LoadBalancer"));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_provider_selection_simulation() {
        let providers = [
            ProviderStats {
                requests: 100,
                errors: 1,
                health_score: 0.99,
                avg_latency_ms: 50.0,
                ..ProviderStats::default()
            },
            ProviderStats {
                requests: 200,
                errors: 20,
                health_score: 0.80,
                avg_latency_ms: 100.0,
                ..ProviderStats::default()
            },
            ProviderStats {
                requests: 50,
                errors: 0,
                health_score: 1.0,
                avg_latency_ms: 30.0,
                ..ProviderStats::default()
            },
        ];

        // Simulate health-based selection - choose highest health score
        let best_provider = providers
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.health_score
                    .partial_cmp(&b.health_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx);

        assert_eq!(best_provider, Some(2)); // Third provider has 1.0 health score
    }

    #[test]
    fn test_latency_based_selection_simulation() {
        let providers = [
            ProviderStats {
                avg_latency_ms: 150.0,
                ..ProviderStats::default()
            },
            ProviderStats {
                avg_latency_ms: 50.0,
                ..ProviderStats::default()
            },
            ProviderStats {
                avg_latency_ms: 100.0,
                ..ProviderStats::default()
            },
        ];

        // Simulate least-latency selection
        let fastest = providers
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.avg_latency_ms
                    .partial_cmp(&b.avg_latency_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx);

        assert_eq!(fastest, Some(1)); // Second provider has lowest latency
    }

    #[test]
    fn test_stats_aggregation() {
        let providers = [
            ProviderStats {
                requests: 100,
                errors: 5,
                total_tokens: 50000,
                total_cost: 5.0,
                ..ProviderStats::default()
            },
            ProviderStats {
                requests: 200,
                errors: 10,
                total_tokens: 100000,
                total_cost: 10.0,
                ..ProviderStats::default()
            },
        ];

        let total_requests: u64 = providers.iter().map(|p| p.requests).sum();
        let total_errors: u64 = providers.iter().map(|p| p.errors).sum();
        let total_tokens: u64 = providers.iter().map(|p| p.total_tokens).sum();
        let total_cost: f64 = providers.iter().map(|p| p.total_cost).sum();

        assert_eq!(total_requests, 300);
        assert_eq!(total_errors, 15);
        assert_eq!(total_tokens, 150000);
        assert!((total_cost - 15.0).abs() < f64::EPSILON);
    }
}
