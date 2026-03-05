//! Deployment selection logic
//!
//! This module contains the core routing logic for selecting
//! the best deployment for a given model.

use super::config::RoutingStrategy;
use super::deployment::{Deployment, DeploymentId};
use super::error::RouterError;
use super::strategy_impl;
use super::unified::Router;
use std::sync::atomic::Ordering::Relaxed;

impl Router {
    /// Check if deployment is within parallel request limit
    pub(crate) fn check_parallel_limit(&self, deployment: &Deployment) -> bool {
        match deployment.config.max_parallel_requests {
            Some(limit) => deployment.state.active_requests.load(Relaxed) < limit,
            None => true,
        }
    }

    /// Check if deployment is within rate limits (TPM/RPM)
    pub(crate) fn check_rate_limit(&self, deployment: &Deployment) -> bool {
        let rpm_ok = match deployment.config.rpm_limit {
            Some(limit) => deployment.state.rpm_current.load(Relaxed) < limit,
            None => true,
        };

        let tpm_ok = match deployment.config.tpm_limit {
            Some(limit) => deployment.state.tpm_current.load(Relaxed) < limit,
            None => true,
        };

        rpm_ok && tpm_ok
    }

    /// Select the best deployment for a given model (core routing method)
    ///
    /// # Flow
    ///
    /// 1. Resolve model_name (handle aliases)
    /// 2. Get all deployment IDs for this model
    /// 3. Filter: healthy + not in cooldown + not rate limited
    /// 4. Select based on routing strategy
    /// 5. Increment active_requests counter
    pub fn select_deployment(&self, model_name: &str) -> Result<DeploymentId, RouterError> {
        // 1. Resolve model name (handle aliases)
        let resolved_name = self.resolve_model_name(model_name);

        // 2. Get all deployment IDs for this model (hold DashMap guard, avoid Vec clone)
        let deployment_ids_ref = self
            .model_index
            .get(&resolved_name)
            .ok_or_else(|| RouterError::ModelNotFound(model_name.to_string()))?;

        if deployment_ids_ref.is_empty() {
            return Err(RouterError::ModelNotFound(model_name.to_string()));
        }

        // 3. Filter: healthy + not in cooldown + not rate limited
        let candidate_ids: Vec<DeploymentId> = deployment_ids_ref
            .iter()
            .filter(|id| {
                if let Some(deployment) = self.deployments.get(id.as_str()) {
                    if !deployment.is_healthy() || deployment.is_in_cooldown() {
                        return false;
                    }

                    if !self.check_parallel_limit(&deployment) {
                        return false;
                    }

                    if !self.check_rate_limit(&deployment) {
                        return false;
                    }

                    true
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        // Drop the model_index read guard before strategy selection
        drop(deployment_ids_ref);

        if candidate_ids.is_empty() {
            return Err(RouterError::NoAvailableDeployment(model_name.to_string()));
        }

        // 4. Build immutable routing context once, then let strategies read that snapshot.
        let routing_contexts =
            strategy_impl::build_routing_contexts(&candidate_ids, &self.deployments);

        // 5. Select based on routing strategy
        // Note: candidate_ids is guaranteed non-empty at this point (checked above)
        let selected_id = match self.config.routing_strategy {
            RoutingStrategy::SimpleShuffle => {
                strategy_impl::weighted_random_from_context(&routing_contexts)
            }
            RoutingStrategy::LeastBusy => strategy_impl::least_busy_from_context(&routing_contexts),
            RoutingStrategy::UsageBased => {
                strategy_impl::lowest_usage_from_context(&routing_contexts)
            }
            RoutingStrategy::LatencyBased => {
                strategy_impl::lowest_latency_from_context(&routing_contexts)
            }
            RoutingStrategy::CostBased => {
                strategy_impl::lowest_cost_from_context(&routing_contexts)
            }
            RoutingStrategy::RateLimitAware => {
                strategy_impl::rate_limit_aware_from_context(&routing_contexts)
            }
            RoutingStrategy::RoundRobin => strategy_impl::round_robin_from_context(
                &resolved_name,
                &routing_contexts,
                &self.round_robin_counters,
            ),
        }
        .cloned()
        .ok_or_else(|| RouterError::NoAvailableDeployment(model_name.to_string()))?;

        // 6. Increment active_requests counter
        if let Some(deployment) = self.deployments.get(&selected_id) {
            deployment.state.active_requests.fetch_add(1, Relaxed);
        }

        Ok(selected_id)
    }

    /// Release a deployment after request completion
    ///
    /// Decrements the active_requests counter for the deployment.
    pub fn release_deployment(&self, deployment_id: &str) {
        if let Some(deployment) = self.deployments.get(deployment_id) {
            deployment.state.active_requests.fetch_sub(1, Relaxed);
        }
    }
}
