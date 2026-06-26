//! Unified Router core structure
//!
//! This module provides the unified Router infrastructure that manages deployments,
//! routing strategies, and intelligent request routing across multiple providers.

use super::config::RouterConfig;
use super::deployment::{Deployment, DeploymentId};
use super::error::CooldownReason;
use super::execution::infer_cooldown_reason;
use super::fallback::{FallbackConfig, FallbackType};
use crate::core::providers::Provider;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::model::ProviderCapability;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

const MAX_ALIAS_HOPS: usize = 16;

/// Snapshot of routing metrics counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingMetrics {
    /// Total number of deployments selected via `select_deployment`.
    pub provider_selected: u64,
    /// Total number of strategy evaluations (one per `select_deployment` call).
    pub strategy_used: u64,
    /// Total number of fallback model attempts in `execute`.
    pub fallback_triggered: u64,
}

/// Deployment snapshot for a capability-compatible model selection.
#[derive(Debug, Clone)]
pub struct CapabilityDeployment {
    pub deployment_id: DeploymentId,
    pub provider: Provider,
    pub model: String,
}

/// Immutable deployment routing generation.
///
/// Each snapshot owns a complete, internally consistent view of deployments,
/// model indexes, and aliases. Router readers load one snapshot and never walk
/// split mutable maps from different generations.
#[derive(Debug, Clone, Default)]
pub(crate) struct RoutingSnapshot {
    pub(crate) deployments: HashMap<DeploymentId, Arc<Deployment>>,
    pub(crate) model_index: HashMap<String, Vec<DeploymentId>>,
    pub(crate) model_aliases: HashMap<String, String>,
}

impl RoutingSnapshot {
    fn from_deployments_preserving_state(
        deployments: Vec<Deployment>,
        previous: &RoutingSnapshot,
    ) -> Self {
        let mut snapshot = Self {
            model_aliases: previous.model_aliases.clone(),
            ..Default::default()
        };

        for mut deployment in deployments {
            if let Some(old) = previous.deployments.get(&deployment.id) {
                deployment.state = old.state.clone();
            }
            snapshot.insert_deployment(deployment);
        }

        snapshot
    }

    fn insert_deployment(&mut self, mut deployment: Deployment) {
        let model_name = deployment.model_name.clone();
        let deployment_id = deployment.id.clone();
        if let Some(old) = self.deployments.get(&deployment_id) {
            deployment.state = old.state.clone();
        }

        if let Some(old) = self
            .deployments
            .insert(deployment_id.clone(), Arc::new(deployment))
            && old.model_name != model_name
        {
            self.remove_from_model_index(&old.model_name, &deployment_id);
        }

        let entry = self.model_index.entry(model_name).or_default();
        if !entry.iter().any(|id| id == &deployment_id) {
            entry.push(deployment_id);
        }
    }

    fn remove_deployment(&mut self, id: &str) -> Option<Deployment> {
        let removed = self.deployments.remove(id);

        if let Some(ref deployment) = removed {
            self.remove_from_model_index(&deployment.model_name, id);
        }

        removed.map(|deployment| deployment.as_ref().clone())
    }

    fn remove_from_model_index(&mut self, model_name: &str, deployment_id: &str) {
        let should_remove = if let Some(entry) = self.model_index.get_mut(model_name) {
            entry.retain(|did| did != deployment_id);
            entry.is_empty()
        } else {
            false
        };

        if should_remove {
            self.model_index.remove(model_name);
        }
    }

    fn add_model_alias(
        &mut self,
        alias: &str,
        model_name: &str,
    ) -> Result<(), super::error::RouterError> {
        if alias == model_name {
            return Err(super::error::RouterError::AliasCycle(format!(
                "'{alias}' -> '{model_name}' would create a cycle"
            )));
        }

        let mut current = model_name.to_string();
        let mut visited = HashSet::new();
        visited.insert(alias.to_string());

        while let Some(next) = self.model_aliases.get(&current) {
            let next_val = next.clone();
            if !visited.insert(next_val.clone()) {
                return Err(super::error::RouterError::AliasCycle(format!(
                    "'{alias}' -> '{model_name}' would create a cycle"
                )));
            }
            current = next_val;
        }

        self.model_aliases
            .insert(alias.to_string(), model_name.to_string());
        Ok(())
    }

    pub(crate) fn resolve_model_name(&self, name: &str) -> String {
        if self.model_aliases.is_empty() {
            return name.to_string();
        }

        let mut current = name.to_string();

        for _ in 0..MAX_ALIAS_HOPS {
            if let Some(next) = self.model_aliases.get(&current) {
                current = next.clone();
            } else {
                return current;
            }
        }

        tracing::debug!(
            requested_model = %name,
            resolved_model = %current,
            max_alias_hops = MAX_ALIAS_HOPS,
            "model alias resolution hit hop limit"
        );
        current
    }
}

/// Unified Router
///
/// The central orchestrator for deployment management and intelligent routing.
/// Uses lock-free data structures for high-performance concurrent access.
#[derive(Debug)]
pub struct Router {
    /// Atomically installed routing metadata generation.
    pub(crate) routing_snapshot: ArcSwap<RoutingSnapshot>,

    /// Serializes snapshot writers so concurrent updates cannot overwrite one
    /// another while readers keep using lock-free ArcSwap loads.
    pub(crate) routing_snapshot_write_lock: Mutex<()>,

    /// Router configuration
    pub(crate) config: RouterConfig,

    /// Fallback configuration
    pub(crate) fallback_config: FallbackConfig,

    /// Round-robin counters (per model, for RoundRobin strategy)
    pub(crate) round_robin_counters: DashMap<String, AtomicUsize>,

    /// Atomic counter: number of times a provider was selected.
    pub(crate) provider_selected_count: AtomicU64,

    /// Atomic counter: number of times a routing strategy was evaluated.
    pub(crate) strategy_used_count: AtomicU64,

    /// Atomic counter: number of fallback model attempts.
    pub(crate) fallback_triggered_count: AtomicU64,
}

impl Router {
    /// Create a new router with the given configuration
    pub fn new(config: RouterConfig) -> Self {
        Self {
            routing_snapshot: ArcSwap::from_pointee(RoutingSnapshot::default()),
            routing_snapshot_write_lock: Mutex::new(()),
            config,
            fallback_config: FallbackConfig::default(),
            round_robin_counters: Default::default(),
            provider_selected_count: AtomicU64::new(0),
            strategy_used_count: AtomicU64::new(0),
            fallback_triggered_count: AtomicU64::new(0),
        }
    }

    /// Set fallback configuration for the router (builder pattern)
    pub fn with_fallback_config(mut self, config: FallbackConfig) -> Self {
        self.fallback_config = config;
        self
    }

    /// Set fallback configuration (runtime method)
    pub fn set_fallback_config(&mut self, config: FallbackConfig) {
        self.fallback_config = config;
    }

    /// Get the router configuration
    pub fn config(&self) -> &RouterConfig {
        &self.config
    }

    /// Return a snapshot of the routing metrics counters.
    pub fn routing_metrics(&self) -> RoutingMetrics {
        RoutingMetrics {
            provider_selected: self.provider_selected_count.load(Relaxed),
            strategy_used: self.strategy_used_count.load(Relaxed),
            fallback_triggered: self.fallback_triggered_count.load(Relaxed),
        }
    }

    // ========== Deployment Management ==========

    fn update_routing_snapshot(&self, update: impl FnOnce(&mut RoutingSnapshot)) {
        let _guard = self.routing_snapshot_write_lock.lock();
        let mut next = self.routing_snapshot.load_full().as_ref().clone();
        update(&mut next);
        self.routing_snapshot.store(Arc::new(next));
    }

    /// Add a deployment to the router
    pub fn add_deployment(&self, deployment: Deployment) {
        self.update_routing_snapshot(|snapshot| snapshot.insert_deployment(deployment));
    }

    /// Remove a deployment from the router
    pub fn remove_deployment(&self, id: &str) -> Option<Deployment> {
        let mut removed = None;
        self.update_routing_snapshot(|snapshot| {
            removed = snapshot.remove_deployment(id);
        });
        removed
    }

    /// Get a deployment by ID
    pub fn get_deployment(&self, id: &str) -> Option<Arc<Deployment>> {
        self.routing_snapshot.load().deployments.get(id).cloned()
    }

    /// Set the complete list of deployments (batch operation)
    ///
    /// Builds the new generation locally first, then installs it with a single
    /// ArcSwap store so readers never observe mixed deployment/index state.
    pub fn set_model_list(&self, deployments: Vec<Deployment>) {
        self.update_routing_snapshot(|snapshot| {
            *snapshot = RoutingSnapshot::from_deployments_preserving_state(deployments, snapshot);
        });
    }

    // ========== Model Aliases ==========

    /// Add a model name alias
    ///
    /// Returns an error if the alias would create a circular reference
    /// (e.g., A -> B and then B -> A).
    pub fn add_model_alias(
        &self,
        alias: &str,
        model_name: &str,
    ) -> Result<(), super::error::RouterError> {
        let _guard = self.routing_snapshot_write_lock.lock();
        let mut next = self.routing_snapshot.load_full().as_ref().clone();
        next.add_model_alias(alias, model_name)?;
        self.routing_snapshot.store(Arc::new(next));
        Ok(())
    }

    /// Resolve a model name (handles aliases)
    pub fn resolve_model_name(&self, name: &str) -> String {
        self.routing_snapshot.load().resolve_model_name(name)
    }

    // ========== Query Methods ==========

    /// Get all deployment IDs for a given model
    pub fn get_deployments_for_model(&self, model_name: &str) -> Vec<DeploymentId> {
        let snapshot = self.routing_snapshot.load();
        let resolved_name = snapshot.resolve_model_name(model_name);

        snapshot
            .model_index
            .get(&resolved_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Get healthy deployment IDs for a given model
    pub fn get_healthy_deployments(&self, model_name: &str) -> Vec<DeploymentId> {
        let snapshot = self.routing_snapshot.load();
        let resolved_name = snapshot.resolve_model_name(model_name);

        let Some(deployment_ids) = snapshot.model_index.get(&resolved_name) else {
            return Vec::new();
        };

        let mut healthy = Vec::with_capacity(deployment_ids.len());

        for id in deployment_ids.iter() {
            if let Some(deployment) = snapshot.deployments.get(id.as_str())
                && deployment.is_healthy()
                && !deployment.is_in_cooldown()
            {
                healthy.push(id.clone());
            }
        }

        healthy
    }

    /// Select the first deployment for `model_name` that supports `capability`.
    ///
    /// This is a core, transport-agnostic primitive used by HTTP routes and any
    /// future lightweight AI executor that needs capability validation without
    /// re-implementing router scans at the gateway layer.
    pub fn select_capability_deployment(
        &self,
        model_name: &str,
        capability: &ProviderCapability,
    ) -> Option<CapabilityDeployment> {
        let snapshot = self.routing_snapshot.load();
        let resolved_name = snapshot.resolve_model_name(model_name);

        let deployment_ids = snapshot.model_index.get(&resolved_name)?;

        for id in deployment_ids.iter() {
            let Some(deployment) = snapshot.deployments.get(id.as_str()) else {
                continue;
            };

            if deployment.model_name != resolved_name {
                continue;
            }

            if deployment.is_in_cooldown() || !deployment.is_healthy() {
                continue;
            }

            let active_requests = deployment.state.active_requests.load(Relaxed);
            if let Some(limit) = deployment.config.max_parallel_requests
                && active_requests >= limit
            {
                continue;
            }

            let rpm_current = deployment.state.rpm_current.load(Relaxed);
            if let Some(limit) = deployment.config.rpm_limit
                && rpm_current >= limit
            {
                continue;
            }

            let tpm_current = deployment.state.tpm_current.load(Relaxed);
            if let Some(limit) = deployment.config.tpm_limit
                && tpm_current >= limit
            {
                continue;
            }

            if deployment
                .provider
                .capabilities()
                .iter()
                .any(|cap| cap == capability)
            {
                return Some(CapabilityDeployment {
                    deployment_id: id.clone(),
                    provider: deployment.provider.clone(),
                    model: deployment.model.clone(),
                });
            }
        }

        None
    }

    /// List all model names
    pub fn list_models(&self) -> Vec<String> {
        self.routing_snapshot
            .load()
            .model_index
            .keys()
            .cloned()
            .collect()
    }

    /// List all deployment IDs
    pub fn list_deployments(&self) -> Vec<DeploymentId> {
        self.routing_snapshot
            .load()
            .deployments
            .keys()
            .cloned()
            .collect()
    }

    // ========== Recording Methods ==========

    /// Record a successful request
    ///
    /// After recording, checks whether the deployment should be promoted from
    /// Degraded (half-open) back to Healthy based on `success_threshold`.
    pub fn record_success(&self, deployment_id: &str, tokens: u64, latency_us: u64) {
        let snapshot = self.routing_snapshot.load();
        if let Some(deployment) = snapshot.deployments.get(deployment_id) {
            self.record_success_for_deployment(deployment, tokens, latency_us);
        }
    }

    pub(crate) fn record_success_for_deployment(
        &self,
        deployment: &Deployment,
        tokens: u64,
        latency_us: u64,
    ) {
        deployment.record_success(tokens, latency_us);

        // Promote Degraded -> Healthy once enough consecutive successes
        let current_health = deployment.state.health.load(Relaxed);
        if current_health == super::deployment::HealthStatus::Degraded as u8 {
            let consec = deployment.state.consecutive_successes.load(Relaxed);
            if consec >= self.config.success_threshold {
                deployment
                    .state
                    .health
                    .store(super::deployment::HealthStatus::Healthy as u8, Relaxed);
            }
        }
    }

    /// Record a failed request
    ///
    /// Only trips the circuit breaker when both the per-minute failure count
    /// reaches `allowed_fails` **and** the total requests this minute meet the
    /// `min_requests` threshold.
    pub fn record_failure(&self, deployment_id: &str) {
        let snapshot = self.routing_snapshot.load();
        if let Some(deployment) = snapshot.deployments.get(deployment_id) {
            self.record_failure_for_deployment(deployment);
        }
    }

    pub(crate) fn record_failure_for_deployment(&self, deployment: &Deployment) {
        deployment.record_failure();

        let fails = deployment.state.fails_this_minute.load(Relaxed);
        let successes_this_minute = deployment.state.rpm_current.load(Relaxed);
        let total_this_minute = successes_this_minute + fails as u64;
        if fails >= self.config.allowed_fails
            && total_this_minute >= self.config.min_requests as u64
        {
            tracing::info!(
                deployment_id = %deployment.id,
                model = %deployment.model_name,
                reason = "consecutive_failures",
                cooldown_secs = self.config.cooldown_time_secs,
                fails_this_minute = fails,
                "deployment entering cooldown"
            );
            deployment.enter_cooldown(self.config.cooldown_time_secs);
        }
    }

    /// Record a failed request with a specific reason
    pub fn record_failure_with_reason(&self, deployment_id: &str, reason: CooldownReason) {
        let snapshot = self.routing_snapshot.load();
        if let Some(d) = snapshot.deployments.get(deployment_id) {
            self.record_failure_with_reason_for_deployment(d, reason);
        }
    }

    pub(crate) fn record_failure_with_reason_for_deployment(
        &self,
        deployment: &Deployment,
        reason: CooldownReason,
    ) {
        deployment.record_failure();

        let should_cooldown = match reason {
            CooldownReason::RateLimit
            | CooldownReason::AuthError
            | CooldownReason::NotFound
            | CooldownReason::Timeout
            | CooldownReason::Manual => true,

            CooldownReason::ConsecutiveFailures => {
                let fails = deployment.state.fails_this_minute.load(Relaxed);
                let successes_this_minute = deployment.state.rpm_current.load(Relaxed);
                let total_this_minute = successes_this_minute + fails as u64;
                fails >= self.config.allowed_fails
                    && total_this_minute >= self.config.min_requests as u64
            }

            CooldownReason::HighFailureRate => {
                let total = deployment.state.total_requests.load(Relaxed);
                let fails = deployment.state.fail_requests.load(Relaxed);
                total >= self.config.min_requests as u64 && (fails * 100 / total) > 50
            }
        };

        if should_cooldown {
            tracing::info!(
                deployment_id = %deployment.id,
                model = %deployment.model_name,
                reason = ?reason,
                cooldown_secs = self.config.cooldown_time_secs,
                "deployment entering cooldown"
            );
            deployment.enter_cooldown(self.config.cooldown_time_secs);
        }
    }

    // ========== Fallback Methods ==========

    /// Infer fallback type from a ProviderError
    pub fn infer_fallback_type(error: &ProviderError) -> FallbackType {
        super::execution::infer_fallback_type(error)
    }

    /// Get fallback models for a given model name and error type
    pub fn get_fallbacks(&self, model_name: &str, fallback_type: FallbackType) -> Vec<String> {
        let resolved_name = self.resolve_model_name(model_name);

        let mut fallbacks = self
            .fallback_config
            .get_fallbacks_for_type(&resolved_name, fallback_type);

        if fallbacks.is_empty() && fallback_type != FallbackType::General {
            fallbacks = self
                .fallback_config
                .get_fallbacks_for_type(&resolved_name, FallbackType::General);
        }

        fallbacks
    }

    /// Get all models to try (original model + fallbacks)
    pub fn get_models_with_fallbacks(
        &self,
        model_name: &str,
        fallback_type: FallbackType,
    ) -> Vec<String> {
        let mut models = vec![self.resolve_model_name(model_name)];
        models.extend(self.get_fallbacks(model_name, fallback_type));
        models
    }

    /// Infer cooldown reason from a ProviderError
    pub fn infer_cooldown_reason(error: &ProviderError) -> CooldownReason {
        infer_cooldown_reason(error)
    }

    // ========== Background Tasks ==========

    /// Reset per-minute counters for all deployments
    pub fn reset_minute_counters(&self) {
        let snapshot = self.routing_snapshot.load();
        for deployment in snapshot.deployments.values() {
            deployment.state.reset_minute();
        }
    }

    /// Start background task to reset minute counters
    pub fn start_minute_reset_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                self.reset_minute_counters();
            }
        })
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new(RouterConfig::default())
    }
}
