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
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

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

/// Unified Router
///
/// The central orchestrator for deployment management and intelligent routing.
/// Uses lock-free data structures for high-performance concurrent access.
#[derive(Debug)]
pub struct Router {
    /// All deployments (DashMap for lock-free concurrent access)
    pub(crate) deployments: DashMap<DeploymentId, Deployment>,

    /// Model name to deployment IDs index (for fast lookup)
    pub(crate) model_index: DashMap<String, Vec<DeploymentId>>,

    /// Model name aliases: "gpt4" -> "gpt-4"
    pub(crate) model_aliases: DashMap<String, String>,

    /// Fast-path gate for alias lookups in hot routing paths.
    ///
    /// Most deployments do not use aliases; this avoids an unnecessary DashMap
    /// lookup when `model_aliases` is known to be empty.
    pub(crate) has_model_aliases: AtomicBool,

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
            deployments: DashMap::new(),
            model_index: DashMap::new(),
            model_aliases: DashMap::new(),
            has_model_aliases: AtomicBool::new(false),
            config,
            fallback_config: FallbackConfig::default(),
            round_robin_counters: DashMap::new(),
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

    /// Add a deployment to the router
    pub fn add_deployment(&self, deployment: Deployment) {
        let model_name = deployment.model_name.clone();
        let deployment_id = deployment.id.clone();

        self.deployments.insert(deployment_id.clone(), deployment);

        self.model_index
            .entry(model_name)
            .or_default()
            .push(deployment_id);
    }

    /// Remove a deployment from the router
    pub fn remove_deployment(&self, id: &str) -> Option<Deployment> {
        let removed = self.deployments.remove(id).map(|(_, v)| v);

        if let Some(ref deployment) = removed
            && let Some(mut entry) = self.model_index.get_mut(&deployment.model_name)
        {
            entry.retain(|did| did != id);
        }

        removed
    }

    /// Get a deployment by ID
    pub fn get_deployment(&self, id: &str) -> Option<Ref<'_, DeploymentId, Deployment>> {
        self.deployments.get(id)
    }

    /// Set the complete list of deployments (batch operation)
    ///
    /// Builds the new maps locally first, then swaps entry-by-entry so
    /// concurrent readers never observe an empty deployment window.
    pub fn set_model_list(&self, deployments: Vec<Deployment>) {
        let new_deployments: DashMap<DeploymentId, Deployment> = DashMap::new();
        let new_index: DashMap<String, Vec<DeploymentId>> = DashMap::new();

        for deployment in deployments {
            let model_name = deployment.model_name.clone();
            let id = deployment.id.clone();
            new_deployments.insert(id.clone(), deployment);
            new_index.entry(model_name).or_default().push(id);
        }

        self.deployments
            .retain(|k, _| new_deployments.contains_key(k));
        for (k, v) in new_deployments {
            self.deployments.insert(k, v);
        }

        self.model_index.retain(|k, _| new_index.contains_key(k));
        for (k, v) in new_index {
            self.model_index.insert(k, v);
        }
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
        // Self-alias is always a cycle
        if alias == model_name {
            return Err(super::error::RouterError::AliasCycle(format!(
                "'{alias}' -> '{model_name}' would create a cycle"
            )));
        }

        // Walk the alias chain starting from model_name to detect cycles
        let mut current = model_name.to_string();
        let mut visited = std::collections::HashSet::new();
        visited.insert(alias.to_string());

        while let Some(next) = self.model_aliases.get(&current) {
            let next_val = next.value().clone();
            if !visited.insert(next_val.clone()) {
                return Err(super::error::RouterError::AliasCycle(format!(
                    "'{alias}' -> '{model_name}' would create a cycle"
                )));
            }
            current = next_val;
        }

        self.model_aliases
            .insert(alias.to_string(), model_name.to_string());
        self.has_model_aliases.store(true, Relaxed);
        Ok(())
    }

    #[inline]
    pub(crate) fn maybe_model_alias<'a>(&'a self, name: &str) -> Option<Ref<'a, String, String>> {
        if self.has_model_aliases.load(Relaxed) {
            self.model_aliases.get(name)
        } else {
            None
        }
    }

    /// Resolve a model name (handles aliases)
    pub fn resolve_model_name(&self, name: &str) -> String {
        self.maybe_model_alias(name)
            .map(|v| v.clone())
            .unwrap_or_else(|| name.to_string())
    }

    // ========== Query Methods ==========

    /// Get all deployment IDs for a given model
    pub fn get_deployments_for_model(&self, model_name: &str) -> Vec<DeploymentId> {
        let alias_guard = self.maybe_model_alias(model_name);
        let resolved_name = alias_guard
            .as_ref()
            .map(|alias| alias.value().as_str())
            .unwrap_or(model_name);

        self.model_index
            .get(resolved_name)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Get healthy deployment IDs for a given model
    pub fn get_healthy_deployments(&self, model_name: &str) -> Vec<DeploymentId> {
        let alias_guard = self.maybe_model_alias(model_name);
        let resolved_name = alias_guard
            .as_ref()
            .map(|alias| alias.value().as_str())
            .unwrap_or(model_name);

        let Some(deployment_ids) = self.model_index.get(resolved_name) else {
            return Vec::new();
        };

        let mut healthy = Vec::with_capacity(deployment_ids.len());

        for id in deployment_ids.iter() {
            if let Some(deployment) = self.deployments.get(id.as_str())
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
        let alias_guard = self.maybe_model_alias(model_name);
        let resolved_name = alias_guard
            .as_ref()
            .map(|alias| alias.value().as_str())
            .unwrap_or(model_name);

        let deployment_ids = self.model_index.get(resolved_name)?;

        for id in deployment_ids.iter() {
            let Some(deployment) = self.deployments.get(id.as_str()) else {
                continue;
            };

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
        self.model_index
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// List all deployment IDs
    pub fn list_deployments(&self) -> Vec<DeploymentId> {
        self.deployments
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    // ========== Recording Methods ==========

    /// Record a successful request
    ///
    /// After recording, checks whether the deployment should be promoted from
    /// Degraded (half-open) back to Healthy based on `success_threshold`.
    pub fn record_success(&self, deployment_id: &str, tokens: u64, latency_us: u64) {
        if let Some(deployment) = self.deployments.get(deployment_id) {
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
    }

    /// Record a failed request
    ///
    /// Only trips the circuit breaker when both the per-minute failure count
    /// reaches `allowed_fails` **and** the total requests this minute meet the
    /// `min_requests` threshold.
    pub fn record_failure(&self, deployment_id: &str) {
        if let Some(deployment) = self.deployments.get(deployment_id) {
            deployment.record_failure();

            let fails = deployment.state.fails_this_minute.load(Relaxed);
            let successes_this_minute = deployment.state.rpm_current.load(Relaxed);
            let total_this_minute = successes_this_minute + fails as u64;
            if fails >= self.config.allowed_fails
                && total_this_minute >= self.config.min_requests as u64
            {
                tracing::info!(
                    deployment_id = %deployment_id,
                    model = %deployment.model_name,
                    reason = "consecutive_failures",
                    cooldown_secs = self.config.cooldown_time_secs,
                    fails_this_minute = fails,
                    "deployment entering cooldown"
                );
                deployment.enter_cooldown(self.config.cooldown_time_secs);
            }
        }
    }

    /// Record a failed request with a specific reason
    pub fn record_failure_with_reason(&self, deployment_id: &str, reason: CooldownReason) {
        if let Some(d) = self.deployments.get(deployment_id) {
            d.record_failure();

            let should_cooldown = match reason {
                CooldownReason::RateLimit
                | CooldownReason::AuthError
                | CooldownReason::NotFound
                | CooldownReason::Timeout
                | CooldownReason::Manual => true,

                CooldownReason::ConsecutiveFailures => {
                    let fails = d.state.fails_this_minute.load(Relaxed);
                    let successes_this_minute = d.state.rpm_current.load(Relaxed);
                    let total_this_minute = successes_this_minute + fails as u64;
                    fails >= self.config.allowed_fails
                        && total_this_minute >= self.config.min_requests as u64
                }

                CooldownReason::HighFailureRate => {
                    let total = d.state.total_requests.load(Relaxed);
                    let fails = d.state.fail_requests.load(Relaxed);
                    total >= self.config.min_requests as u64 && (fails * 100 / total) > 50
                }
            };

            if should_cooldown {
                tracing::info!(
                    deployment_id = %deployment_id,
                    model = %d.model_name,
                    reason = ?reason,
                    cooldown_secs = self.config.cooldown_time_secs,
                    "deployment entering cooldown"
                );
                d.enter_cooldown(self.config.cooldown_time_secs);
            }
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
        for entry in self.deployments.iter() {
            entry.value().state.reset_minute();
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
