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
        let mut rng = rand::rng();
        let index = rng.random_range(0..contexts.len());
        return Some(contexts[index].deployment_id);
    }

    let mut rng = rand::rng();
    let mut point = rng.random_range(0..total_weight);

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
        let mut rng = rand::rng();
        let index = rng.random_range(0..tied.len());
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

