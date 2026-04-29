//! Deployment selection logic
//!
//! This module contains the core routing logic for selecting
//! the best deployment for a given model.

use super::config::RoutingStrategy;
use super::deployment::DeploymentId;
use super::error::RouterError;
use super::strategy_impl;
use super::unified::Router;
use std::sync::atomic::Ordering::Relaxed;

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
        // 1. Resolve model name with a borrowed fast path so the hot routing
        // path only allocates when we finally return the selected deployment ID.
        let alias_guard = self.maybe_model_alias(model_name);
        let resolved_name = alias_guard
            .as_ref()
            .map(|alias| alias.value().as_str())
            .unwrap_or(model_name);

        // 2. Get all deployment IDs for this model.
        let deployment_ids_ref = self
            .model_index
            .get(resolved_name)
            .ok_or_else(|| RouterError::ModelNotFound(model_name.to_string()))?;

        if deployment_ids_ref.is_empty() {
            return Err(RouterError::ModelNotFound(model_name.to_string()));
        }

        // 3. Filter and build routing contexts in one pass to avoid cloning a
        // temporary candidate ID vector and then looking everything up again.
        let total_deployments = deployment_ids_ref.len();
        let mut routing_contexts = Vec::with_capacity(total_deployments);

        for id in deployment_ids_ref.iter() {
            let Some(deployment) = self.deployments.get(id.as_str()) else {
                continue;
            };

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
            tracing::warn!(
                model = %model_name,
                total_deployments = total_deployments,
                "no available deployments after filtering"
            );
            return Err(RouterError::NoAvailableDeployment(model_name.to_string()));
        }

        // 4. Select based on the immutable routing contexts.
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
            RoutingStrategy::PriorityBased => {
                strategy_impl::lowest_priority_from_context(&routing_contexts)
            }
            RoutingStrategy::RateLimitAware => {
                strategy_impl::rate_limit_aware_from_context(&routing_contexts)
            }
            RoutingStrategy::RoundRobin => strategy_impl::round_robin_from_context(
                resolved_name,
                &routing_contexts,
                &self.round_robin_counters,
            ),
        }
        .ok_or_else(|| RouterError::NoAvailableDeployment(model_name.to_string()))?
        .clone();

        // 5. Increment active_requests counter and routing metrics.
        if let Some(deployment) = self.deployments.get(&selected_id) {
            deployment.state.active_requests.fetch_add(1, Relaxed);
        }
        self.provider_selected_count.fetch_add(1, Relaxed);
        self.strategy_used_count.fetch_add(1, Relaxed);

        tracing::debug!(
            model = %model_name,
            strategy = ?self.config.routing_strategy,
            candidate_count = routing_contexts.len(),
            selected_id = %selected_id,
            "deployment selected for routing"
        );

        Ok(selected_id)
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
