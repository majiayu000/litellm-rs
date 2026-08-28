//! Deployment selection logic
//!
//! This module contains the core routing logic for selecting
//! the best deployment for a given model.

use super::config::RoutingStrategy;
use super::deployment::{Deployment, DeploymentId, current_timestamp};
use super::error::RouterError;
use super::execution::router_error_to_provider_error;
use super::strategy_impl::{self, RoutingContext};
use super::unified::Router;
use super::{RoutingSnapshot, RuntimeHandle};
use crate::core::types::model::ProviderCapability;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering::{AcqRel, Acquire, Relaxed},
};

/// Active-request reservation for a selected deployment.
///
/// Dropping the lease releases the selected deployment unless ownership was
/// explicitly converted back into the legacy deployment-id API.
pub struct DeploymentLease {
    deployment: Arc<Deployment>,
    release_on_drop: bool,
}

impl DeploymentLease {
    fn new(deployment: Arc<Deployment>) -> Self {
        Self {
            deployment,
            release_on_drop: true,
        }
    }

    pub fn deployment_id(&self) -> &str {
        &self.deployment.id
    }

    pub fn deployment(&self) -> &Deployment {
        &self.deployment
    }

    pub fn clone_deployment_id(&self) -> DeploymentId {
        self.deployment_id().to_string()
    }

    pub fn clone_deployment(&self) -> Arc<Deployment> {
        self.deployment.clone()
    }

    /// Convert this lease into the legacy deployment-id API.
    ///
    /// Prefer keeping the lease alive so drop releases the exact snapshot
    /// deployment. This exists for deprecated ID-returning selectors.
    pub fn into_deployment_id(mut self) -> DeploymentId {
        self.release_on_drop = false;
        self.deployment_id().to_string()
    }
}

impl Drop for DeploymentLease {
    fn drop(&mut self) {
        if self.release_on_drop {
            Router::release_selected_deployment(self.deployment());
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
    #[deprecated(
        since = "0.5.0",
        note = "Use select_deployment_lease so the selected snapshot deployment is released by RAII"
    )]
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
    ) -> Result<DeploymentLease, RouterError> {
        let snapshot = self.load_routing_snapshot();
        self.select_deployment_matching(snapshot.as_ref(), model_name, |_| true, None)
    }

    pub(crate) fn select_deployment_lease_matching_in_snapshot<F>(
        &self,
        snapshot: &RoutingSnapshot,
        model_name: &str,
        is_candidate: F,
    ) -> Result<DeploymentLease, RouterError>
    where
        F: Fn(&Deployment) -> bool,
    {
        self.select_deployment_matching(snapshot, model_name, is_candidate, None)
    }

    /// Select the best deployment for a model that supports `capability`.
    ///
    /// This uses the same health, cooldown, concurrency, rate-limit, and
    /// strategy filters as `select_deployment`, then reserves the selected
    /// deployment by incrementing its active request count.
    #[deprecated(
        since = "0.5.0",
        note = "Use select_deployment_lease_for_capability so the selected snapshot deployment is released by RAII"
    )]
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
    ) -> Result<DeploymentLease, RouterError> {
        self.select_deployment_lease_for_capability_matching(model_name, capability, |_| true)
    }

    /// Select and reserve the best deployment for a model that supports
    /// `capability` and passes an additional route-level predicate.
    pub fn select_deployment_lease_for_capability_matching<F>(
        &self,
        model_name: &str,
        capability: &ProviderCapability,
        is_candidate: F,
    ) -> Result<DeploymentLease, RouterError>
    where
        F: Fn(&Deployment) -> bool,
    {
        let snapshot = self.load_routing_snapshot();
        self.select_deployment_lease_for_capability_matching_in_snapshot(
            snapshot.as_ref(),
            model_name,
            capability,
            is_candidate,
        )
    }

    pub(crate) fn select_deployment_lease_for_capability_matching_in_snapshot<F>(
        &self,
        snapshot: &RoutingSnapshot,
        model_name: &str,
        capability: &ProviderCapability,
        is_candidate: F,
    ) -> Result<DeploymentLease, RouterError>
    where
        F: Fn(&Deployment) -> bool,
    {
        let resolved_name = snapshot.resolve_model_name(model_name);
        let candidates = snapshot
            .model_index
            .get(&resolved_name)
            .into_iter()
            .flatten()
            .filter_map(|id| snapshot.deployments.get(id));
        let (candidate_count, missing_mapping_count) =
            candidates.fold((0usize, 0usize), |(count, missing), deployment| {
                (
                    count + 1,
                    missing + usize::from(deployment.requires_capability_mapping()),
                )
            });
        let no_matching_candidate_error = if candidate_count > 0
            && candidate_count == missing_mapping_count
        {
            RouterError::InvalidConfiguration(format!(
                "model '{model_name}' requires settings.model_identity_mappings.<deployment>.capability_catalog_model before capability routing"
            ))
        } else {
            RouterError::UnsupportedCapability {
                model: model_name.to_string(),
                capability: format!("{capability:?}"),
            }
        };

        self.select_deployment_matching(
            snapshot,
            model_name,
            |deployment| deployment.supports_capability(capability) && is_candidate(deployment),
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
        snapshot: &RoutingSnapshot,
        model_name: &str,
        is_candidate: F,
        no_matching_candidate_error: Option<RouterError>,
    ) -> Result<DeploymentLease, RouterError>
    where
        F: Fn(&Deployment) -> bool,
    {
        // 1. Resolve model aliases and deployment indexes from one immutable
        // routing generation.
        let resolved_name = snapshot.resolve_model_name(model_name);

        // 2. Get all deployment IDs for this model.
        let deployment_ids_ref = snapshot
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
        let now = current_timestamp();

        for id in deployment_ids_ref.iter() {
            let Some(deployment) = snapshot.deployments.get(id.as_str()) else {
                continue;
            };

            if deployment.model_name != resolved_name {
                tracing::debug!(
                    deployment_id = id.as_str(),
                    requested_model = %model_name,
                    resolved_model = %resolved_name,
                    deployment_model = %deployment.model_name,
                    "deployment index entry points at a different model"
                );
                continue;
            }
            existing_deployments += 1;

            if !is_candidate(deployment) {
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

            let minute = deployment.state.minute_counters(now);
            let rpm_current = minute.rpm;
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

            let tpm_current = minute.tpm;
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

            let Some(deployment) = snapshot.deployments.get(&selected_id).cloned() else {
                continue;
            };

            if self.try_reserve_deployment(&deployment, &resolved_name) {
                self.provider_selected_count.fetch_add(1, Relaxed);
                self.strategy_used_count.fetch_add(1, Relaxed);

                tracing::debug!(
                    model = %model_name,
                    strategy = ?self.config.routing_strategy,
                    candidate_count = routing_contexts.len(),
                    selected_id = %selected_id,
                    "deployment selected for routing"
                );

                return Ok(DeploymentLease::new(deployment));
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

    fn try_reserve_deployment(&self, deployment: &Deployment, expected_model: &str) -> bool {
        if deployment.model_name != expected_model {
            return false;
        }

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
    #[deprecated(
        since = "0.5.0",
        note = "Use select_deployment_lease so release targets the selected snapshot deployment"
    )]
    pub fn release_deployment(&self, deployment_id: &str) {
        let snapshot = self.load_routing_snapshot();
        if let Some(deployment) = snapshot.deployments.get(deployment_id) {
            Self::release_selected_deployment(deployment);
        }
    }

    pub(crate) fn release_selected_deployment(deployment: &Deployment) {
        let result = deployment
            .state
            .active_requests
            .fetch_update(Relaxed, Relaxed, |v| Some(v.saturating_sub(1)));
        debug_assert!(result.is_ok());
    }
}

impl RuntimeHandle {
    /// Select and reserve a deployment from this handle's pinned generation.
    pub(crate) fn select_deployment_lease_typed(
        &self,
        model_name: &str,
    ) -> Result<DeploymentLease, crate::core::providers::ProviderError> {
        self.binding
            .router
            .select_deployment_lease_matching_in_snapshot(
                self.snapshot.as_ref(),
                model_name,
                |_| true,
            )
            .map_err(router_error_to_provider_error)
    }
}

impl RuntimeHandle {
    pub fn select_deployment_lease(
        &self,
        model_name: &str,
    ) -> Result<DeploymentLease, RouterError> {
        self.binding.router.select_deployment_matching(
            self.snapshot.as_ref(),
            model_name,
            |_| true,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn select_deployment_lease_for_capability(
        &self,
        model_name: &str,
        capability: &ProviderCapability,
    ) -> Result<DeploymentLease, RouterError> {
        self.binding
            .router
            .select_deployment_lease_for_capability_matching_in_snapshot(
                self.snapshot.as_ref(),
                model_name,
                capability,
                |_| true,
            )
    }
}
