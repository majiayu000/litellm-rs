//! Routing strategy implementations
//!
//! This module contains the implementation of 7 routing strategies
//! for selecting deployments.

use super::deployment::{Deployment, DeploymentId};
use dashmap::DashMap;
use rand::Rng;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

// Note: StrategySelector trait was removed as dead code — only free functions are used.

/// Immutable snapshot used by routing strategies.
///
/// This keeps strategy logic decoupled from deployment storage details.
#[derive(Debug, Clone, Copy)]
pub struct RoutingContext<'id> {
    pub deployment_id: &'id DeploymentId,
    pub weight: u32,
    pub priority: u32,
    pub active_requests: u32,
    pub tpm_current: u64,
    pub tpm_limit: Option<u64>,
    pub rpm_current: u64,
    pub rpm_limit: Option<u64>,
    pub avg_latency_us: u64,
}

/// Build immutable routing snapshots for all valid candidates.
pub fn build_routing_contexts<'id>(
    candidate_ids: &'id [DeploymentId],
    deployments: &DashMap<DeploymentId, Deployment>,
) -> Vec<RoutingContext<'id>> {
    candidate_ids
        .iter()
        .filter_map(|id| {
            deployments
                .get(id.as_str())
                .map(|deployment| RoutingContext {
                    deployment_id: id,
                    weight: deployment.config.weight,
                    priority: deployment.config.priority,
                    active_requests: deployment.state.active_requests.load(Relaxed),
                    tpm_current: deployment.state.tpm_current.load(Relaxed),
                    tpm_limit: deployment.config.tpm_limit,
                    rpm_current: deployment.state.rpm_current.load(Relaxed),
                    rpm_limit: deployment.config.rpm_limit,
                    avg_latency_us: deployment.state.avg_latency_us.load(Relaxed),
                })
        })
        .collect()
}

/// Weighted random selection (SimpleShuffle) using snapshot contexts.
pub fn weighted_random_from_context<'id>(
    contexts: &[RoutingContext<'id>],
) -> Option<&'id DeploymentId> {
    if contexts.is_empty() {
        return None;
    }

    if contexts.len() == 1 {
        return Some(contexts[0].deployment_id);
    }

    let total_weight: u32 = contexts.iter().map(|ctx| ctx.weight).sum();
    if total_weight == 0 {
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..contexts.len());
        return Some(contexts[index].deployment_id);
    }

    let mut rng = rand::thread_rng();
    let mut point = rng.gen_range(0..total_weight);

    for ctx in contexts {
        if point < ctx.weight {
            return Some(ctx.deployment_id);
        }
        point -= ctx.weight;
    }

    Some(contexts[0].deployment_id)
}

/// Select deployment with fewest active requests (LeastBusy) using snapshot contexts.
pub fn least_busy_from_context<'id>(contexts: &[RoutingContext<'id>]) -> Option<&'id DeploymentId> {
    if contexts.is_empty() {
        return None;
    }

    let min_active = contexts
        .iter()
        .map(|ctx| ctx.active_requests)
        .min()
        .unwrap_or(0);

    let tied: Vec<&DeploymentId> = contexts
        .iter()
        .filter(|ctx| ctx.active_requests == min_active)
        .map(|ctx| ctx.deployment_id)
        .collect();

    if tied.is_empty() {
        return Some(contexts[0].deployment_id);
    }

    if tied.len() == 1 {
        Some(tied[0])
    } else {
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..tied.len());
        Some(tied[index])
    }
}

/// Select deployment with lowest TPM usage rate (UsageBased) using snapshot contexts.
pub fn lowest_usage_from_context<'id>(
    contexts: &[RoutingContext<'id>],
) -> Option<&'id DeploymentId> {
    if contexts.is_empty() {
        return None;
    }

    let mut best_id = contexts[0].deployment_id;
    let mut best_usage_pct = u64::MAX;

    for ctx in contexts {
        let usage_pct = match ctx.tpm_limit {
            Some(limit) if limit > 0 => (ctx.tpm_current * 100) / limit,
            _ => 0, // No limit = 0% usage
        };

        if usage_pct < best_usage_pct {
            best_usage_pct = usage_pct;
            best_id = ctx.deployment_id;
        }
    }

    Some(best_id)
}

/// Select deployment with lowest average latency (LatencyBased) using snapshot contexts.
pub fn lowest_latency_from_context<'id>(
    contexts: &[RoutingContext<'id>],
) -> Option<&'id DeploymentId> {
    if contexts.is_empty() {
        return None;
    }

    let latencies: Vec<u64> = contexts
        .iter()
        .map(|ctx| ctx.avg_latency_us)
        .filter(|&lat| lat > 0)
        .collect();

    let avg_latency = if latencies.is_empty() {
        0
    } else {
        latencies.iter().sum::<u64>() / latencies.len() as u64
    };

    let mut best_id = contexts[0].deployment_id;
    let mut best_latency = u64::MAX;

    for ctx in contexts {
        let mut latency = ctx.avg_latency_us;
        if latency == 0 {
            latency = avg_latency;
        }

        if latency < best_latency {
            best_latency = latency;
            best_id = ctx.deployment_id;
        }
    }

    Some(best_id)
}

/// Select deployment with lowest cost (CostBased) using snapshot contexts.
pub fn lowest_cost_from_context<'id>(
    contexts: &[RoutingContext<'id>],
) -> Option<&'id DeploymentId> {
    if contexts.is_empty() {
        return None;
    }

    let mut best_id = contexts[0].deployment_id;
    let mut best_priority = u32::MAX;

    for ctx in contexts {
        if ctx.priority < best_priority {
            best_priority = ctx.priority;
            best_id = ctx.deployment_id;
        }
    }

    Some(best_id)
}

/// Select deployment furthest from rate limits (RateLimitAware) using snapshot contexts.
pub fn rate_limit_aware_from_context<'id>(
    contexts: &[RoutingContext<'id>],
) -> Option<&'id DeploymentId> {
    if contexts.is_empty() {
        return None;
    }

    let mut best_id = contexts[0].deployment_id;
    let mut best_distance: f64 = -1.0;

    for ctx in contexts {
        let tpm_distance = match ctx.tpm_limit {
            Some(limit) if limit > 0 => {
                let remaining = limit.saturating_sub(ctx.tpm_current);
                remaining as f64 / limit as f64
            }
            _ => 1.0, // No limit = maximum distance
        };

        let rpm_distance = match ctx.rpm_limit {
            Some(limit) if limit > 0 => {
                let remaining = limit.saturating_sub(ctx.rpm_current);
                remaining as f64 / limit as f64
            }
            _ => 1.0, // No limit = maximum distance
        };

        let distance = tpm_distance.min(rpm_distance);
        if distance > best_distance {
            best_distance = distance;
            best_id = ctx.deployment_id;
        }
    }

    Some(best_id)
}

/// Weighted random selection (SimpleShuffle)
///
/// Selects a deployment randomly based on weights.
/// Higher weight = higher probability of selection.
/// Returns None if candidate_ids is empty.
pub fn weighted_random<'a>(
    candidate_ids: &'a [DeploymentId],
    deployments: &DashMap<DeploymentId, Deployment>,
) -> Option<&'a DeploymentId> {
    let contexts = build_routing_contexts(candidate_ids, deployments);
    if contexts.is_empty() {
        return candidate_ids.first();
    }
    weighted_random_from_context(&contexts)
}

/// Select deployment with fewest active requests (LeastBusy)
///
/// Chooses the deployment with the lowest number of currently active requests.
/// In case of tie, selects randomly among tied deployments.
/// Returns None if candidate_ids is empty.
pub fn least_busy<'a>(
    candidate_ids: &'a [DeploymentId],
    deployments: &DashMap<DeploymentId, Deployment>,
) -> Option<&'a DeploymentId> {
    let contexts = build_routing_contexts(candidate_ids, deployments);
    if contexts.is_empty() {
        return candidate_ids.first();
    }
    least_busy_from_context(&contexts)
}

/// Select deployment with lowest TPM usage rate (UsageBased)
///
/// Calculates TPM usage as: (tpm_current / tpm_limit) * 100
/// Deployments without limits are considered at 0% usage.
/// Returns None if candidate_ids is empty.
pub fn lowest_usage<'a>(
    candidate_ids: &'a [DeploymentId],
    deployments: &DashMap<DeploymentId, Deployment>,
) -> Option<&'a DeploymentId> {
    let contexts = build_routing_contexts(candidate_ids, deployments);
    if contexts.is_empty() {
        return candidate_ids.first();
    }
    lowest_usage_from_context(&contexts)
}

/// Select deployment with lowest average latency (LatencyBased)
///
/// Selects the deployment with the lowest average latency.
/// New deployments (latency = 0) are given a chance by treating them
/// as having average latency.
/// Returns None if candidate_ids is empty.
pub fn lowest_latency<'a>(
    candidate_ids: &'a [DeploymentId],
    deployments: &DashMap<DeploymentId, Deployment>,
) -> Option<&'a DeploymentId> {
    let contexts = build_routing_contexts(candidate_ids, deployments);
    if contexts.is_empty() {
        return candidate_ids.first();
    }
    lowest_latency_from_context(&contexts)
}

/// Select deployment with lowest cost (CostBased)
///
/// Currently uses priority as a cost proxy (lower priority = lower cost).
/// Returns None if candidate_ids is empty.
pub fn lowest_cost<'a>(
    candidate_ids: &'a [DeploymentId],
    deployments: &DashMap<DeploymentId, Deployment>,
) -> Option<&'a DeploymentId> {
    let contexts = build_routing_contexts(candidate_ids, deployments);
    if contexts.is_empty() {
        return candidate_ids.first();
    }
    lowest_cost_from_context(&contexts)
}

/// Select deployment that is furthest from rate limits (RateLimitAware)
///
/// Calculates distance from rate limit as: (limit - current) / limit
/// Selects the deployment with maximum distance (most headroom).
/// Returns None if candidate_ids is empty.
pub fn rate_limit_aware<'a>(
    candidate_ids: &'a [DeploymentId],
    deployments: &DashMap<DeploymentId, Deployment>,
) -> Option<&'a DeploymentId> {
    let contexts = build_routing_contexts(candidate_ids, deployments);
    if contexts.is_empty() {
        return candidate_ids.first();
    }
    rate_limit_aware_from_context(&contexts)
}

/// Round-robin selection (RoundRobin) using snapshot contexts.
///
/// Cycles through deployment IDs in context order, using a per-model counter.
/// Returns None if contexts is empty.
pub fn round_robin_from_context<'id>(
    model_name: &str,
    contexts: &[RoutingContext<'id>],
    round_robin_counters: &DashMap<String, AtomicUsize>,
) -> Option<&'id DeploymentId> {
    if contexts.is_empty() {
        return None;
    }

    if contexts.len() == 1 {
        return Some(contexts[0].deployment_id);
    }

    // Get or create counter for this model
    let counter = round_robin_counters
        .entry(model_name.to_string())
        .or_insert_with(|| AtomicUsize::new(0));

    // Fetch and increment counter
    let index = counter.fetch_add(1, Relaxed) % contexts.len();

    Some(contexts[index].deployment_id)
}

/// Round-robin selection (RoundRobin)
///
/// Cycles through deployments in order, using a per-model counter.
/// Returns None if candidate_ids is empty.
pub fn round_robin<'a>(
    model_name: &str,
    candidate_ids: &'a [DeploymentId],
    round_robin_counters: &DashMap<String, AtomicUsize>,
) -> Option<&'a DeploymentId> {
    if candidate_ids.is_empty() {
        return None;
    }

    if candidate_ids.len() == 1 {
        return Some(&candidate_ids[0]);
    }

    // Get or create counter for this model
    let counter = round_robin_counters
        .entry(model_name.to_string())
        .or_insert_with(|| AtomicUsize::new(0));

    // Fetch and increment counter
    let index = counter.fetch_add(1, Relaxed) % candidate_ids.len();

    Some(&candidate_ids[index])
}

// ====================================================================================
// TESTS
// ====================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::Provider;
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::router::deployment::{DeploymentConfig, DeploymentState};
    use std::sync::atomic::Ordering::Relaxed;

    // Helper to create a test provider
    async fn create_test_provider() -> Provider {
        let openai = OpenAIProvider::with_api_key("sk-test-key-for-unit-testing-only")
            .await
            .expect("Failed to create OpenAI provider");
        Provider::OpenAI(openai)
    }

    // Helper to create a test deployment
    async fn create_test_deployment(id: &str, config: DeploymentConfig) -> Deployment {
        Deployment {
            id: id.to_string(),
            provider: create_test_provider().await,
            model: "gpt-4".to_string(),
            model_name: "gpt-4".to_string(),
            config,
            state: DeploymentState::new(),
            tags: vec![],
        }
    }

    #[tokio::test]
    async fn test_build_routing_contexts_skips_missing_deployments() {
        let deployments = DashMap::new();
        let config = DeploymentConfig {
            weight: 3,
            priority: 7,
            ..Default::default()
        };
        let deployment = create_test_deployment("d1", config).await;
        deployment.state.active_requests.store(2, Relaxed);
        deployment.state.tpm_current.store(120, Relaxed);
        deployment.state.rpm_current.store(12, Relaxed);
        deployment.state.avg_latency_us.store(55, Relaxed);
        deployments.insert("d1".to_string(), deployment);

        let candidates = vec!["d1".to_string(), "missing".to_string()];
        let contexts = build_routing_contexts(&candidates, &deployments);

        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].deployment_id, "d1");
        assert_eq!(contexts[0].weight, 3);
        assert_eq!(contexts[0].priority, 7);
        assert_eq!(contexts[0].active_requests, 2);
        assert_eq!(contexts[0].tpm_current, 120);
        assert_eq!(contexts[0].rpm_current, 12);
        assert_eq!(contexts[0].avg_latency_us, 55);
    }

    // ====================================================================================
    // weighted_random Tests
    // ====================================================================================

    #[tokio::test]
    async fn test_weighted_random_single_candidate() {
        let deployments = DashMap::new();
        let config = DeploymentConfig {
            weight: 1,
            ..Default::default()
        };
        deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

        let candidates = vec!["d1".to_string()];
        let selected = weighted_random(&candidates, &deployments).unwrap();
        assert_eq!(selected, "d1");
    }

    #[tokio::test]
    async fn test_weighted_random_returns_valid_candidate() {
        let deployments = DashMap::new();
        for i in 1..=3 {
            let config = DeploymentConfig {
                weight: 1,
                ..Default::default()
            };
            deployments.insert(
                format!("d{}", i),
                create_test_deployment(&format!("d{}", i), config).await,
            );
        }

        let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();

        // Run multiple times and verify result is always in candidates
        for _ in 0..100 {
            let selected = weighted_random(&candidates, &deployments).unwrap();
            assert!(candidates.contains(selected));
        }
    }

    #[tokio::test]
    async fn test_weighted_random_respects_weights() {
        let deployments = DashMap::new();

        // d1 has weight 10, d2 has weight 1
        let config1 = DeploymentConfig {
            weight: 10,
            ..Default::default()
        };
        let config2 = DeploymentConfig {
            weight: 1,
            ..Default::default()
        };
        deployments.insert(
            "d1".to_string(),
            create_test_deployment("d1", config1).await,
        );
        deployments.insert(
            "d2".to_string(),
            create_test_deployment("d2", config2).await,
        );

        let candidates = vec!["d1".to_string(), "d2".to_string()];

        let mut d1_count = 0;
        let mut d2_count = 0;

        for _ in 0..1000 {
            let selected = weighted_random(&candidates, &deployments).unwrap();
            if selected == "d1" {
                d1_count += 1;
            } else {
                d2_count += 1;
            }
        }

        // d1 should be selected significantly more often (roughly 10x)
        assert!(
            d1_count > d2_count * 5,
            "d1 should be selected much more often due to higher weight"
        );
    }

    #[tokio::test]
    async fn test_weighted_random_all_zero_weights() {
        let deployments = DashMap::new();
        for i in 1..=3 {
            let config = DeploymentConfig {
                weight: 0,
                ..Default::default()
            };
            deployments.insert(
                format!("d{}", i),
                create_test_deployment(&format!("d{}", i), config).await,
            );
        }

        let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();

        // Should fall back to uniform random
        for _ in 0..10 {
            let selected = weighted_random(&candidates, &deployments).unwrap();
            assert!(candidates.contains(selected));
        }
    }

    #[test]
    fn test_weighted_random_empty_candidates() {
        let deployments: DashMap<DeploymentId, Deployment> = DashMap::new();
        let candidates: Vec<String> = vec![];
        assert!(weighted_random(&candidates, &deployments).is_none());
    }

    // ====================================================================================
    // least_busy Tests
    // ====================================================================================

    #[tokio::test]
    async fn test_least_busy_single_candidate() {
        let deployments = DashMap::new();
        let config = DeploymentConfig::default();
        deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

        let candidates = vec!["d1".to_string()];
        let selected = least_busy(&candidates, &deployments).unwrap();
        assert_eq!(selected, "d1");
    }

    #[tokio::test]
    async fn test_least_busy_selects_lowest_active() {
        let deployments = DashMap::new();

        let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
        d1.state.active_requests.store(10, Relaxed);
        deployments.insert("d1".to_string(), d1);

        let d2 = create_test_deployment("d2", DeploymentConfig::default()).await;
        d2.state.active_requests.store(5, Relaxed);
        deployments.insert("d2".to_string(), d2);

        let d3 = create_test_deployment("d3", DeploymentConfig::default()).await;
        d3.state.active_requests.store(15, Relaxed);
        deployments.insert("d3".to_string(), d3);

        let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
        let selected = least_busy(&candidates, &deployments).unwrap();

        // d2 has the fewest active requests
        assert_eq!(selected, "d2");
    }

    #[tokio::test]
    async fn test_least_busy_with_ties() {
        let deployments = DashMap::new();

        let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
        d1.state.active_requests.store(5, Relaxed);
        deployments.insert("d1".to_string(), d1);

        let d2 = create_test_deployment("d2", DeploymentConfig::default()).await;
        d2.state.active_requests.store(5, Relaxed);
        deployments.insert("d2".to_string(), d2);

        let candidates = vec!["d1".to_string(), "d2".to_string()];

        // Result should be one of the tied deployments
        for _ in 0..10 {
            let selected = least_busy(&candidates, &deployments).unwrap();
            assert!(selected == "d1" || selected == "d2");
        }
    }

    #[tokio::test]
    async fn test_least_busy_all_zero() {
        let deployments = DashMap::new();
        for i in 1..=3 {
            let d = create_test_deployment(&format!("d{}", i), DeploymentConfig::default()).await;
            d.state.active_requests.store(0, Relaxed);
            deployments.insert(format!("d{}", i), d);
        }

        let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();
        let selected = least_busy(&candidates, &deployments).unwrap();
        assert!(candidates.contains(selected));
    }

    #[test]
    fn test_least_busy_empty_candidates() {
        let deployments: DashMap<DeploymentId, Deployment> = DashMap::new();
        let candidates: Vec<String> = vec![];
        assert!(least_busy(&candidates, &deployments).is_none());
    }

    // ====================================================================================
    // lowest_usage Tests
    // ====================================================================================

    #[tokio::test]
    async fn test_lowest_usage_single_candidate() {
        let deployments = DashMap::new();
        let config = DeploymentConfig {
            tpm_limit: Some(1000),
            ..Default::default()
        };
        deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

        let candidates = vec!["d1".to_string()];
        let selected = lowest_usage(&candidates, &deployments).unwrap();
        assert_eq!(selected, "d1");
    }

    #[tokio::test]
    async fn test_lowest_usage_selects_lowest_percentage() {
        let deployments = DashMap::new();

        // d1: 50% usage (500/1000)
        let config1 = DeploymentConfig {
            tpm_limit: Some(1000),
            ..Default::default()
        };
        let d1 = create_test_deployment("d1", config1).await;
        d1.state.tpm_current.store(500, Relaxed);
        deployments.insert("d1".to_string(), d1);

        // d2: 20% usage (200/1000)
        let config2 = DeploymentConfig {
            tpm_limit: Some(1000),
            ..Default::default()
        };
        let d2 = create_test_deployment("d2", config2).await;
        d2.state.tpm_current.store(200, Relaxed);
        deployments.insert("d2".to_string(), d2);

        // d3: 80% usage (800/1000)
        let config3 = DeploymentConfig {
            tpm_limit: Some(1000),
            ..Default::default()
        };
        let d3 = create_test_deployment("d3", config3).await;
        d3.state.tpm_current.store(800, Relaxed);
        deployments.insert("d3".to_string(), d3);

        let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
        let selected = lowest_usage(&candidates, &deployments).unwrap();

        // d2 has the lowest usage percentage
        assert_eq!(selected, "d2");
    }

    #[tokio::test]
    async fn test_lowest_usage_no_limit_treated_as_zero() {
        let deployments = DashMap::new();

        // d1 has no limit (0% usage)
        let config1 = DeploymentConfig {
            tpm_limit: None,
            ..Default::default()
        };
        let d1 = create_test_deployment("d1", config1).await;
        deployments.insert("d1".to_string(), d1);

        // d2 has 50% usage
        let config2 = DeploymentConfig {
            tpm_limit: Some(1000),
            ..Default::default()
        };
        let d2 = create_test_deployment("d2", config2).await;
        d2.state.tpm_current.store(500, Relaxed);
        deployments.insert("d2".to_string(), d2);

        let candidates = vec!["d1".to_string(), "d2".to_string()];
        let selected = lowest_usage(&candidates, &deployments).unwrap();

        // d1 has 0% usage (no limit)
        assert_eq!(selected, "d1");
    }

    #[test]
    fn test_lowest_usage_empty_candidates() {
        let deployments: DashMap<DeploymentId, Deployment> = DashMap::new();
        let candidates: Vec<String> = vec![];
        assert!(lowest_usage(&candidates, &deployments).is_none());
    }

    // ====================================================================================
    // lowest_latency Tests
    // ====================================================================================

    #[tokio::test]
    async fn test_lowest_latency_single_candidate() {
        let deployments = DashMap::new();
        let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
        d1.state.avg_latency_us.store(100, Relaxed);
        deployments.insert("d1".to_string(), d1);

        let candidates = vec!["d1".to_string()];
        let selected = lowest_latency(&candidates, &deployments).unwrap();
        assert_eq!(selected, "d1");
    }

    #[tokio::test]
    async fn test_lowest_latency_selects_fastest() {
        let deployments = DashMap::new();

        let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
        d1.state.avg_latency_us.store(500, Relaxed);
        deployments.insert("d1".to_string(), d1);

        let d2 = create_test_deployment("d2", DeploymentConfig::default()).await;
        d2.state.avg_latency_us.store(100, Relaxed);
        deployments.insert("d2".to_string(), d2);

        let d3 = create_test_deployment("d3", DeploymentConfig::default()).await;
        d3.state.avg_latency_us.store(300, Relaxed);
        deployments.insert("d3".to_string(), d3);

        let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
        let selected = lowest_latency(&candidates, &deployments).unwrap();

        // d2 has the lowest latency
        assert_eq!(selected, "d2");
    }

    #[tokio::test]
    async fn test_lowest_latency_new_deployment_uses_average() {
        let deployments = DashMap::new();

        // d1 has measured latency
        let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
        d1.state.avg_latency_us.store(1000, Relaxed);
        deployments.insert("d1".to_string(), d1);

        // d2 is new (latency = 0, will use average)
        let d2 = create_test_deployment("d2", DeploymentConfig::default()).await;
        d2.state.avg_latency_us.store(0, Relaxed);
        deployments.insert("d2".to_string(), d2);

        // d3 has high latency
        let d3 = create_test_deployment("d3", DeploymentConfig::default()).await;
        d3.state.avg_latency_us.store(2000, Relaxed);
        deployments.insert("d3".to_string(), d3);

        let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
        let selected = lowest_latency(&candidates, &deployments).unwrap();

        // d1 has the lowest actual latency (1000)
        // d2 gets average = (1000 + 2000) / 2 = 1500
        assert_eq!(selected, "d1");
    }

    #[tokio::test]
    async fn test_lowest_latency_all_zero() {
        let deployments = DashMap::new();
        for i in 1..=3 {
            let d = create_test_deployment(&format!("d{}", i), DeploymentConfig::default()).await;
            d.state.avg_latency_us.store(0, Relaxed);
            deployments.insert(format!("d{}", i), d);
        }

        let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();
        let selected = lowest_latency(&candidates, &deployments).unwrap();

        // Any candidate is valid when all have zero latency
        assert!(candidates.contains(selected));
    }

    #[test]
    fn test_lowest_latency_empty_candidates() {
        let deployments: DashMap<DeploymentId, Deployment> = DashMap::new();
        let candidates: Vec<String> = vec![];
        assert!(lowest_latency(&candidates, &deployments).is_none());
    }

    // ====================================================================================
    // lowest_cost Tests
    // ====================================================================================

    #[tokio::test]
    async fn test_lowest_cost_single_candidate() {
        let deployments = DashMap::new();
        let config = DeploymentConfig {
            priority: 5,
            ..Default::default()
        };
        deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

        let candidates = vec!["d1".to_string()];
        let selected = lowest_cost(&candidates, &deployments).unwrap();
        assert_eq!(selected, "d1");
    }

    #[tokio::test]
    async fn test_lowest_cost_selects_lowest_priority() {
        let deployments = DashMap::new();

        let config1 = DeploymentConfig {
            priority: 10,
            ..Default::default()
        };
        deployments.insert(
            "d1".to_string(),
            create_test_deployment("d1", config1).await,
        );

        let config2 = DeploymentConfig {
            priority: 1,
            ..Default::default()
        };
        deployments.insert(
            "d2".to_string(),
            create_test_deployment("d2", config2).await,
        );

        let config3 = DeploymentConfig {
            priority: 5,
            ..Default::default()
        };
        deployments.insert(
            "d3".to_string(),
            create_test_deployment("d3", config3).await,
        );

        let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
        let selected = lowest_cost(&candidates, &deployments).unwrap();

        // d2 has the lowest priority (cheapest)
        assert_eq!(selected, "d2");
    }

    #[tokio::test]
    async fn test_lowest_cost_all_same_priority() {
        let deployments = DashMap::new();
        for i in 1..=3 {
            let config = DeploymentConfig {
                priority: 5,
                ..Default::default()
            };
            deployments.insert(
                format!("d{}", i),
                create_test_deployment(&format!("d{}", i), config).await,
            );
        }

        let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();
        let selected = lowest_cost(&candidates, &deployments).unwrap();

        // First one wins when all have same priority
        assert_eq!(selected, "d1");
    }

    #[test]
    fn test_lowest_cost_empty_candidates() {
        let deployments: DashMap<DeploymentId, Deployment> = DashMap::new();
        let candidates: Vec<String> = vec![];
        assert!(lowest_cost(&candidates, &deployments).is_none());
    }

    // ====================================================================================
    // rate_limit_aware Tests
    // ====================================================================================

    #[tokio::test]
    async fn test_rate_limit_aware_single_candidate() {
        let deployments = DashMap::new();
        let config = DeploymentConfig {
            tpm_limit: Some(1000),
            rpm_limit: Some(100),
            ..Default::default()
        };
        deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

        let candidates = vec!["d1".to_string()];
        let selected = rate_limit_aware(&candidates, &deployments).unwrap();
        assert_eq!(selected, "d1");
    }

    #[tokio::test]
    async fn test_rate_limit_aware_selects_most_headroom() {
        let deployments = DashMap::new();

        // d1: 80% TPM usage (little headroom)
        let config1 = DeploymentConfig {
            tpm_limit: Some(1000),
            rpm_limit: Some(100),
            ..Default::default()
        };
        let d1 = create_test_deployment("d1", config1).await;
        d1.state.tpm_current.store(800, Relaxed);
        d1.state.rpm_current.store(20, Relaxed);
        deployments.insert("d1".to_string(), d1);

        // d2: 20% TPM usage (lots of headroom)
        let config2 = DeploymentConfig {
            tpm_limit: Some(1000),
            rpm_limit: Some(100),
            ..Default::default()
        };
        let d2 = create_test_deployment("d2", config2).await;
        d2.state.tpm_current.store(200, Relaxed);
        d2.state.rpm_current.store(20, Relaxed);
        deployments.insert("d2".to_string(), d2);

        let candidates = vec!["d1".to_string(), "d2".to_string()];
        let selected = rate_limit_aware(&candidates, &deployments).unwrap();

        // d2 has more headroom
        assert_eq!(selected, "d2");
    }

    #[tokio::test]
    async fn test_rate_limit_aware_considers_rpm() {
        let deployments = DashMap::new();

        // d1: Low TPM usage but high RPM usage
        let config1 = DeploymentConfig {
            tpm_limit: Some(1000),
            rpm_limit: Some(100),
            ..Default::default()
        };
        let d1 = create_test_deployment("d1", config1).await;
        d1.state.tpm_current.store(100, Relaxed);
        d1.state.rpm_current.store(90, Relaxed); // 90% RPM usage
        deployments.insert("d1".to_string(), d1);

        // d2: Moderate usage on both
        let config2 = DeploymentConfig {
            tpm_limit: Some(1000),
            rpm_limit: Some(100),
            ..Default::default()
        };
        let d2 = create_test_deployment("d2", config2).await;
        d2.state.tpm_current.store(400, Relaxed); // 40% TPM
        d2.state.rpm_current.store(40, Relaxed); // 40% RPM
        deployments.insert("d2".to_string(), d2);

        let candidates = vec!["d1".to_string(), "d2".to_string()];
        let selected = rate_limit_aware(&candidates, &deployments).unwrap();

        // d2 should win because d1 is constrained by RPM (10% headroom vs 60%)
        assert_eq!(selected, "d2");
    }

    #[tokio::test]
    async fn test_rate_limit_aware_no_limits() {
        let deployments = DashMap::new();

        // No limits = maximum distance (1.0)
        let config = DeploymentConfig {
            tpm_limit: None,
            rpm_limit: None,
            ..Default::default()
        };
        deployments.insert(
            "d1".to_string(),
            create_test_deployment("d1", config.clone()).await,
        );
        deployments.insert("d2".to_string(), create_test_deployment("d2", config).await);

        let candidates = vec!["d1".to_string(), "d2".to_string()];
        let selected = rate_limit_aware(&candidates, &deployments).unwrap();

        // Both have maximum distance, first one wins
        assert_eq!(selected, "d1");
    }

    #[test]
    fn test_rate_limit_aware_empty_candidates() {
        let deployments: DashMap<DeploymentId, Deployment> = DashMap::new();
        let candidates: Vec<String> = vec![];
        assert!(rate_limit_aware(&candidates, &deployments).is_none());
    }

    // ====================================================================================
    // round_robin Tests
    // ====================================================================================

    #[test]
    fn test_round_robin_single_candidate() {
        let counters: DashMap<String, AtomicUsize> = DashMap::new();
        let candidates = vec!["d1".to_string()];

        let selected = round_robin("gpt-4", &candidates, &counters).unwrap();
        assert_eq!(selected, "d1");
    }

    #[test]
    fn test_round_robin_cycles_through_candidates() {
        let counters: DashMap<String, AtomicUsize> = DashMap::new();
        let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];

        // First cycle
        assert_eq!(round_robin("gpt-4", &candidates, &counters).unwrap(), "d1");
        assert_eq!(round_robin("gpt-4", &candidates, &counters).unwrap(), "d2");
        assert_eq!(round_robin("gpt-4", &candidates, &counters).unwrap(), "d3");

        // Second cycle
        assert_eq!(round_robin("gpt-4", &candidates, &counters).unwrap(), "d1");
        assert_eq!(round_robin("gpt-4", &candidates, &counters).unwrap(), "d2");
    }

    #[test]
    fn test_round_robin_separate_counters_per_model() {
        let counters: DashMap<String, AtomicUsize> = DashMap::new();
        let candidates = vec!["d1".to_string(), "d2".to_string()];

        // gpt-4 model
        assert_eq!(round_robin("gpt-4", &candidates, &counters).unwrap(), "d1");
        assert_eq!(round_robin("gpt-4", &candidates, &counters).unwrap(), "d2");

        // claude model has its own counter
        assert_eq!(
            round_robin("claude-3", &candidates, &counters).unwrap(),
            "d1"
        );
        assert_eq!(
            round_robin("claude-3", &candidates, &counters).unwrap(),
            "d2"
        );

        // gpt-4 continues from where it left off
        assert_eq!(round_robin("gpt-4", &candidates, &counters).unwrap(), "d1");
    }

    #[test]
    fn test_round_robin_wraps_around() {
        let counters: DashMap<String, AtomicUsize> = DashMap::new();
        let candidates = vec!["d1".to_string(), "d2".to_string()];

        // Run many times and verify it keeps cycling
        for i in 0..100 {
            let selected = round_robin("gpt-4", &candidates, &counters).unwrap();
            if i % 2 == 0 {
                assert_eq!(selected, "d1");
            } else {
                assert_eq!(selected, "d2");
            }
        }
    }

    #[test]
    fn test_round_robin_empty_candidates() {
        let counters: DashMap<String, AtomicUsize> = DashMap::new();
        let candidates: Vec<String> = vec![];
        assert!(round_robin("gpt-4", &candidates, &counters).is_none());
    }

    #[test]
    fn test_round_robin_from_context_cycles_through_candidates() {
        let counters: DashMap<String, AtomicUsize> = DashMap::new();
        let candidate_ids = ["d1".to_string(), "d2".to_string(), "d3".to_string()];
        let contexts: Vec<RoutingContext<'_>> = candidate_ids
            .iter()
            .map(|id| RoutingContext {
                deployment_id: id,
                weight: 1,
                priority: 1,
                active_requests: 0,
                tpm_current: 0,
                tpm_limit: None,
                rpm_current: 0,
                rpm_limit: None,
                avg_latency_us: 0,
            })
            .collect();

        assert_eq!(
            round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
            "d1"
        );
        assert_eq!(
            round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
            "d2"
        );
        assert_eq!(
            round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
            "d3"
        );
    }

    // ====================================================================================
    // Integration Tests
    // ====================================================================================

    #[tokio::test]
    async fn test_strategy_consistency() {
        // Verify that with same input, deterministic strategies produce same output
        let deployments = DashMap::new();

        let config1 = DeploymentConfig {
            weight: 1,
            priority: 10,
            tpm_limit: Some(1000),
            ..Default::default()
        };
        let d1 = create_test_deployment("d1", config1).await;
        d1.state.tpm_current.store(500, Relaxed);
        d1.state.active_requests.store(5, Relaxed);
        d1.state.avg_latency_us.store(100, Relaxed);
        deployments.insert("d1".to_string(), d1);

        let config2 = DeploymentConfig {
            weight: 1,
            priority: 1,
            tpm_limit: Some(1000),
            ..Default::default()
        };
        let d2 = create_test_deployment("d2", config2).await;
        d2.state.tpm_current.store(100, Relaxed);
        d2.state.active_requests.store(2, Relaxed);
        d2.state.avg_latency_us.store(200, Relaxed);
        deployments.insert("d2".to_string(), d2);

        let candidates = vec!["d1".to_string(), "d2".to_string()];

        // Deterministic strategies should consistently return same result
        for _ in 0..10 {
            // least_busy always picks d2 (2 active vs 5)
            assert_eq!(least_busy(&candidates, &deployments).unwrap(), "d2");

            // lowest_usage always picks d2 (10% vs 50%)
            assert_eq!(lowest_usage(&candidates, &deployments).unwrap(), "d2");

            // lowest_latency always picks d1 (100us vs 200us)
            assert_eq!(lowest_latency(&candidates, &deployments).unwrap(), "d1");

            // lowest_cost always picks d2 (priority 1 vs 10)
            assert_eq!(lowest_cost(&candidates, &deployments).unwrap(), "d2");
        }
    }
}
