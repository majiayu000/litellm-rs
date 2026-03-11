//! Unified Router core structure
//!
//! This module provides the unified Router infrastructure that manages deployments,
//! routing strategies, and intelligent request routing across multiple providers.

use super::config::RouterConfig;
use super::deployment::{Deployment, DeploymentId};
use super::error::CooldownReason;
use super::execution::infer_cooldown_reason;
use super::fallback::{FallbackConfig, FallbackType};
use crate::core::providers::unified_provider::ProviderError;
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

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

    /// Router configuration
    pub(crate) config: RouterConfig,

    /// Fallback configuration
    pub(crate) fallback_config: FallbackConfig,

    /// Round-robin counters (per model, for RoundRobin strategy)
    pub(crate) round_robin_counters: DashMap<String, AtomicUsize>,
}

impl Router {
    /// Create a new router with the given configuration
    pub fn new(config: RouterConfig) -> Self {
        Self {
            deployments: DashMap::new(),
            model_index: DashMap::new(),
            model_aliases: DashMap::new(),
            config,
            fallback_config: FallbackConfig::default(),
            round_robin_counters: DashMap::new(),
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
    pub fn add_model_alias(&self, alias: &str, model_name: &str) {
        self.model_aliases
            .insert(alias.to_string(), model_name.to_string());
    }

    /// Resolve a model name (handles aliases)
    pub fn resolve_model_name(&self, name: &str) -> String {
        self.model_aliases
            .get(name)
            .map(|v| v.clone())
            .unwrap_or_else(|| name.to_string())
    }

    // ========== Query Methods ==========

    /// Get all deployment IDs for a given model
    pub fn get_deployments_for_model(&self, model_name: &str) -> Vec<DeploymentId> {
        let resolved_name = self.resolve_model_name(model_name);

        self.model_index
            .get(&resolved_name)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Get healthy deployment IDs for a given model
    pub fn get_healthy_deployments(&self, model_name: &str) -> Vec<DeploymentId> {
        let resolved_name = self.resolve_model_name(model_name);

        let Some(deployment_ids) = self.model_index.get(&resolved_name) else {
            return Vec::new();
        };

        deployment_ids
            .iter()
            .filter(|id| {
                if let Some(deployment) = self.deployments.get(id.as_str()) {
                    deployment.is_healthy() && !deployment.is_in_cooldown()
                } else {
                    false
                }
            })
            .cloned()
            .collect()
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
    pub fn record_success(&self, deployment_id: &str, tokens: u64, latency_us: u64) {
        if let Some(deployment) = self.deployments.get(deployment_id) {
            deployment.record_success(tokens, latency_us);
        }
    }

    /// Record a failed request
    pub fn record_failure(&self, deployment_id: &str) {
        if let Some(deployment) = self.deployments.get(deployment_id) {
            deployment.record_failure();

            let fails_this_minute = deployment.state.fails_this_minute.load(Relaxed);
            if fails_this_minute >= self.config.allowed_fails {
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
                    d.state.fails_this_minute.load(Relaxed) >= self.config.allowed_fails
                }

                CooldownReason::HighFailureRate => {
                    let total = d.state.total_requests.load(Relaxed);
                    let fails = d.state.fail_requests.load(Relaxed);
                    total >= 10 && (fails * 100 / total) > 50
                }
            };

            if should_cooldown {
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
