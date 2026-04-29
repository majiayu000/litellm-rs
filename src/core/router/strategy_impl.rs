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
    let mut contexts = Vec::with_capacity(candidate_ids.len());

    for id in candidate_ids {
        if let Some(deployment) = deployments.get(id.as_str()) {
            contexts.push(RoutingContext {
                deployment_id: id,
                weight: deployment.config.weight,
                priority: deployment.config.priority,
                active_requests: deployment.state.active_requests.load(Relaxed),
                tpm_current: deployment.state.tpm_current.load(Relaxed),
                tpm_limit: deployment.config.tpm_limit,
                rpm_current: deployment.state.rpm_current.load(Relaxed),
                rpm_limit: deployment.config.rpm_limit,
                avg_latency_us: deployment.state.avg_latency_us.load(Relaxed),
            });
        }
    }

    contexts
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

    if contexts.len() == 1 {
        return Some(contexts[0].deployment_id);
    }

    let mut rng = rand::rng();
    let mut best_id = contexts[0].deployment_id;
    let mut min_active = contexts[0].active_requests;
    let mut tied_count = 1usize;

    for ctx in &contexts[1..] {
        if ctx.active_requests < min_active {
            min_active = ctx.active_requests;
            best_id = ctx.deployment_id;
            tied_count = 1;
        } else if ctx.active_requests == min_active {
            tied_count += 1;
            if rng.random_range(0..tied_count) == 0 {
                best_id = ctx.deployment_id;
            }
        }
    }

    Some(best_id)
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

    // New deployments report 0 latency until first success. Reuse the average
    // observed latency as a fallback without allocating a temporary vector.
    let mut non_zero_latency_sum = 0u64;
    let mut non_zero_latency_count = 0u64;
    for ctx in contexts {
        if ctx.avg_latency_us > 0 {
            non_zero_latency_sum = non_zero_latency_sum.saturating_add(ctx.avg_latency_us);
            non_zero_latency_count += 1;
        }
    }
    let avg_latency = non_zero_latency_sum
        .checked_div(non_zero_latency_count)
        .unwrap_or(0);

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

/// Select deployment with lowest priority value (PriorityBased) using snapshot contexts.
pub fn lowest_priority_from_context<'id>(
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

    // Fast path avoids allocating `model_name.to_string()` when the counter
    // already exists (the common case in steady-state routing).
    let candidate_count = contexts.len();
    let index = if let Some(counter) = round_robin_counters.get(model_name) {
        counter.fetch_add(1, Relaxed) % candidate_count
    } else {
        round_robin_counters
            .entry(model_name.to_string())
            .or_insert_with(|| AtomicUsize::new(0))
            .fetch_add(1, Relaxed)
            % candidate_count
    };

    Some(contexts[index].deployment_id)
}
