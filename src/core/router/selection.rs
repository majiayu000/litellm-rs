//! Deployment selection logic
//!
//! This module contains the core routing logic for selecting
//! the best deployment for a given model.

use super::config::RoutingStrategy;
use super::deployment::{Deployment, DeploymentId};
use super::error::RouterError;
use super::strategy_impl::{self, RoutingContext};
use super::unified::Router;
use crate::core::types::model::ProviderCapability;
use dashmap::DashMap;
use std::sync::atomic::{
    AtomicUsize,
    Ordering::{AcqRel, Acquire, Relaxed},
};

/// Active-request reservation for a selected deployment.
///
/// Dropping the lease releases the selected deployment unless ownership was
/// explicitly converted back into the legacy deployment-id API.
pub struct DeploymentLease<'router> {
    router: &'router Router,
    deployment_id: DeploymentId,
    release_on_drop: bool,
}

impl<'router> DeploymentLease<'router> {
    fn new(router: &'router Router, deployment_id: DeploymentId) -> Self {
        Self {
            router,
            deployment_id,
            release_on_drop: true,
        }
    }

    pub fn deployment_id(&self) -> &str {
        &self.deployment_id
    }

    pub fn clone_deployment_id(&self) -> DeploymentId {
        self.deployment_id().to_string()
    }

    pub fn into_deployment_id(mut self) -> DeploymentId {
        self.release_on_drop = false;
        std::mem::take(&mut self.deployment_id)
    }
}

impl Drop for DeploymentLease<'_> {
    fn drop(&mut self) {
        if self.release_on_drop {
            self.router.release_deployment(&self.deployment_id);
        }
    }
}

impl Router {
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
        self.select_deployment_lease(model_name)
            .map(DeploymentLease::into_deployment_id)
    }

    /// Select and reserve the best deployment for a given model.
    ///
    /// The returned lease releases the deployment on drop, which protects
    /// router-owned execution paths from cancellation leaks.
    pub fn select_deployment_lease(
        &self,
        model_name: &str,
    ) -> Result<DeploymentLease<'_>, RouterError> {
        self.select_deployment_matching(model_name, |_| true, None)
    }

    /// Select the best deployment for a model that supports `capability`.
    ///
    /// This uses the same health, cooldown, concurrency, rate-limit, and
    /// strategy filters as `select_deployment`, then reserves the selected
    /// deployment by incrementing its active request count.
    pub fn select_deployment_for_capability(
        &self,
        model_name: &str,
        capability: &ProviderCapability,
    ) -> Result<DeploymentId, RouterError> {
        self.select_deployment_lease_for_capability(model_name, capability)
            .map(DeploymentLease::into_deployment_id)
    }

    /// Select and reserve the best deployment for a model that supports
    /// `capability`.
    pub fn select_deployment_lease_for_capability(
        &self,
        model_name: &str,
        capability: &ProviderCapability,
    ) -> Result<DeploymentLease<'_>, RouterError> {
        let no_matching_candidate_error = RouterError::UnsupportedCapability {
            model: model_name.to_string(),
            capability: format!("{capability:?}"),
        };

        self.select_deployment_matching(
            model_name,
            |deployment| {
                deployment
                    .provider
                    .capabilities()
                    .iter()
                    .any(|cap| cap == capability)
            },
            Some(no_matching_candidate_error),
        )
    }

    /// Select a deployment ID from pre-built routing contexts.
    ///
    /// This is the shared routing-policy entry point used by the full
    /// `UnifiedRouter` and lightweight adapters such as the SDK client.
    pub fn select_from_routing_contexts<'id>(
        strategy: RoutingStrategy,
        model_name: &str,
        routing_contexts: &[RoutingContext<'id>],
        round_robin_counters: &DashMap<String, AtomicUsize>,
    ) -> Option<&'id DeploymentId> {
        match strategy {
            RoutingStrategy::SimpleShuffle => {
                strategy_impl::weighted_random_from_context(routing_contexts)
            }
            RoutingStrategy::LeastBusy => strategy_impl::least_busy_from_context(routing_contexts),
            RoutingStrategy::UsageBased => {
                strategy_impl::lowest_usage_from_context(routing_contexts)
            }
            RoutingStrategy::LatencyBased => {
                strategy_impl::lowest_latency_from_context(routing_contexts)
            }
            RoutingStrategy::PriorityBased => {
                strategy_impl::lowest_priority_from_context(routing_contexts)
            }
            RoutingStrategy::RateLimitAware => {
                strategy_impl::rate_limit_aware_from_context(routing_contexts)
            }
            RoutingStrategy::RoundRobin => strategy_impl::round_robin_from_context(
                model_name,
                routing_contexts,
                round_robin_counters,
            ),
        }
    }

    fn select_deployment_matching<F>(
        &self,
        model_name: &str,
        is_candidate: F,
        no_matching_candidate_error: Option<RouterError>,
    ) -> Result<DeploymentLease<'_>, RouterError>
    where
        F: Fn(&Deployment) -> bool,
    {
        // 1. Resolve model aliases consistently with the public resolver.
        let resolved_name = self.resolve_model_name(model_name);

        // 2. Get all deployment IDs for this model.
        let deployment_ids_ref = self
            .model_index
            .get(&resolved_name)
            .ok_or_else(|| RouterError::ModelNotFound(model_name.to_string()))?;

        if deployment_ids_ref.is_empty() {
            return Err(RouterError::ModelNotFound(model_name.to_string()));
        }

        // 3. Filter and build routing contexts in one pass to avoid cloning a
        // temporary candidate ID vector and then looking everything up again.
        let total_deployments = deployment_ids_ref.len();
        let mut existing_deployments = 0;
        let mut matching_deployments = 0;
        let mut routing_contexts = Vec::with_capacity(total_deployments);

        for id in deployment_ids_ref.iter() {
            let Some(deployment) = self.deployments.get(id.as_str()) else {
                continue;
            };

            if deployment.model_name != resolved_name {
                tracing::warn!(
                    deployment_id = id.as_str(),
                    requested_model = %model_name,
                    resolved_model = %resolved_name,
                    deployment_model = %deployment.model_name,
                    "deployment index entry points at a different model"
                );
                continue;
            }
            existing_deployments += 1;

            if !is_candidate(&deployment) {
                tracing::trace!(
                    deployment_id = id.as_str(),
                    model = %resolved_name,
                    reason = "capability_mismatch",
                    "deployment excluded from routing candidates"
                );
                continue;
            }
            matching_deployments += 1;

            // Check cooldown first: is_in_cooldown() resets health
            // from Cooldown to Degraded when the cooldown period expires.
            if deployment.is_in_cooldown() {
                tracing::trace!(
                    deployment_id = id.as_str(),
                    model = %resolved_name,
                    reason = "in_cooldown",
                    "deployment excluded from routing candidates"
                );
                continue;
            }

            if !deployment.is_healthy() {
                tracing::trace!(
                    deployment_id = id.as_str(),
                    model = %resolved_name,
                    reason = "unhealthy",
                    "deployment excluded from routing candidates"
                );
                continue;
            }

            let active_requests = deployment.state.active_requests.load(Relaxed);
            if let Some(limit) = deployment.config.max_parallel_requests
                && active_requests >= limit
            {
                tracing::trace!(
                    deployment_id = id.as_str(),
                    model = %resolved_name,
                    reason = "parallel_limit_reached",
                    "deployment excluded from routing candidates"
                );
                continue;
            }

            let rpm_current = deployment.state.rpm_current.load(Relaxed);
            if let Some(limit) = deployment.config.rpm_limit
                && rpm_current >= limit
            {
                tracing::trace!(
                    deployment_id = id.as_str(),
                    model = %resolved_name,
                    reason = "rate_limited",
                    "deployment excluded from routing candidates"
                );
                continue;
            }

            let tpm_current = deployment.state.tpm_current.load(Relaxed);
            if let Some(limit) = deployment.config.tpm_limit
                && tpm_current >= limit
            {
                tracing::trace!(
                    deployment_id = id.as_str(),
                    model = %resolved_name,
                    reason = "rate_limited",
                    "deployment excluded from routing candidates"
                );
                continue;
            }

            routing_contexts.push(strategy_impl::RoutingContext {
                deployment_id: id,
                weight: deployment.config.weight,
                priority: deployment.config.priority,
                active_requests,
                tpm_current,
                tpm_limit: deployment.config.tpm_limit,
                rpm_current,
                rpm_limit: deployment.config.rpm_limit,
                avg_latency_us: deployment.state.avg_latency_us.load(Relaxed),
            });
        }

        if routing_contexts.is_empty() {
            if existing_deployments > 0
                && matching_deployments == 0
                && let Some(err) = no_matching_candidate_error
            {
                return Err(err);
            }

            tracing::warn!(
                model = %model_name,
                total_deployments = total_deployments,
                "no available deployments after filtering"
            );
            return Err(RouterError::NoAvailableDeployment(model_name.to_string()));
        }

        // 4. Select based on immutable routing contexts, then atomically reserve
        // the selected deployment. If another caller wins the race for the last
        // slot, remove that candidate and retry within this selection attempt.
        while !routing_contexts.is_empty() {
            let selected_id = Self::select_from_routing_contexts(
                self.config.routing_strategy,
                &resolved_name,
                &routing_contexts,
                &self.round_robin_counters,
            )
            .ok_or_else(|| RouterError::NoAvailableDeployment(model_name.to_string()))?
            .clone();

            if self.try_reserve_deployment(&selected_id) {
                self.provider_selected_count.fetch_add(1, Relaxed);
                self.strategy_used_count.fetch_add(1, Relaxed);

                tracing::debug!(
                    model = %model_name,
                    strategy = ?self.config.routing_strategy,
                    candidate_count = routing_contexts.len(),
                    selected_id = %selected_id,
                    "deployment selected for routing"
                );

                return Ok(DeploymentLease::new(self, selected_id));
            }

            if let Some(pos) = routing_contexts
                .iter()
                .position(|ctx| ctx.deployment_id == &selected_id)
            {
                routing_contexts.swap_remove(pos);
            } else {
                break;
            }
        }

        tracing::warn!(
            model = %model_name,
            "no available deployments after reservation"
        );
        Err(RouterError::NoAvailableDeployment(model_name.to_string()))
    }

    fn try_reserve_deployment(&self, deployment_id: &str) -> bool {
        let Some(deployment) = self.deployments.get(deployment_id) else {
            return false;
        };

        match deployment.config.max_parallel_requests {
            Some(limit) => {
                let mut current = deployment.state.active_requests.load(Acquire);
                loop {
                    if current >= limit {
                        return false;
                    }

                    match deployment.state.active_requests.compare_exchange_weak(
                        current,
                        current + 1,
                        AcqRel,
                        Acquire,
                    ) {
                        Ok(_) => return true,
                        Err(next) => current = next,
                    }
                }
            }
            None => {
                deployment.state.active_requests.fetch_add(1, Relaxed);
                true
            }
        }
    }

    /// Release a deployment after request completion
    ///
    /// Decrements the active_requests counter for the deployment.
    pub fn release_deployment(&self, deployment_id: &str) {
        if let Some(deployment) = self.deployments.get(deployment_id) {
            let _ = deployment
                .state
                .active_requests
                .fetch_update(Relaxed, Relaxed, |v| Some(v.saturating_sub(1)));
        }
    }
}
