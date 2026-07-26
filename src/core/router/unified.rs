//! Unified Router core structure
//!
//! This module provides the unified Router infrastructure that manages deployments,
//! routing strategies, and intelligent request routing across multiple providers.

use super::config::RouterConfig;
use super::deployment::{Deployment, DeploymentId, LegacySelectorMetadata};
use super::error::CooldownReason;
use super::execution::infer_cooldown_reason;
use super::fallback::{FallbackConfig, FallbackType};
use crate::core::providers::Provider;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::model::ProviderCapability;
use crate::utils::auth::crypto::hmac::CredentialDigest;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{
    AtomicU64, AtomicUsize,
    Ordering::{Relaxed, SeqCst},
};
use std::time::Duration;

const MAX_ALIAS_HOPS: usize = 16;
static NEXT_ROUTING_GENERATION: AtomicU64 = AtomicU64::new(0);

fn next_routing_generation() -> u64 {
    NEXT_ROUTING_GENERATION
        .fetch_update(SeqCst, SeqCst, |current| current.checked_add(1))
        .expect("routing generation space exhausted")
        + 1
}

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
/// model indexes, model-group insertion order, and aliases. Router readers load
/// one snapshot and never walk split mutable maps from different generations.
#[derive(Debug, Clone)]
pub struct RoutingSnapshot {
    generation: u64,
    pub(crate) deployments: HashMap<DeploymentId, Arc<Deployment>>,
    pub(crate) model_index: HashMap<String, Vec<DeploymentId>>,
    pub(crate) model_order: Vec<String>,
    pub(crate) model_aliases: HashMap<String, String>,
    legacy_selector_metadata: HashMap<DeploymentId, LegacySelectorMetadata>,
}

impl RoutingSnapshot {
    fn empty() -> Self {
        Self {
            generation: next_routing_generation(),
            deployments: HashMap::new(),
            model_index: HashMap::new(),
            model_order: Vec::new(),
            model_aliases: HashMap::new(),
            legacy_selector_metadata: HashMap::new(),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    fn from_deployments_preserving_state(
        deployments: Vec<Deployment>,
        previous: &RoutingSnapshot,
    ) -> Self {
        let mut seen_models = HashSet::new();
        let mut input_model_order = Vec::new();
        for deployment in &deployments {
            if seen_models.insert(deployment.model_name.clone()) {
                input_model_order.push(deployment.model_name.clone());
            }
        }
        let mut snapshot = Self {
            generation: previous.generation,
            deployments: HashMap::new(),
            model_index: HashMap::new(),
            model_order: Vec::new(),
            model_aliases: previous.model_aliases.clone(),
            // A bulk replacement carries no fresh credential provenance. Runtime
            // state may follow a stable deployment ID, but selector metadata may
            // not: retaining it would accept an old credential after rotation.
            legacy_selector_metadata: HashMap::new(),
        };

        for mut deployment in deployments {
            if let Some(old) = previous.deployments.get(&deployment.id) {
                deployment.state = old.state.clone();
            }
            snapshot.insert_deployment(deployment);
        }
        // Duplicate deployment IDs retain their existing last-wins behavior.
        // Reapply input group order after indexing so a temporary empty group
        // cannot move behind groups that first appeared later in the input.
        snapshot.model_order = input_model_order
            .into_iter()
            .filter(|model_name| snapshot.model_index.contains_key(model_name))
            .collect();

        snapshot
    }

    pub(super) fn insert_deployment(&mut self, mut deployment: Deployment) {
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

        if !self.model_index.contains_key(&model_name) {
            self.model_order.push(model_name.clone());
        }

        let entry = self.model_index.entry(model_name).or_default();
        if !entry.iter().any(|id| id == &deployment_id) {
            entry.push(deployment_id);
        }
    }

    pub(super) fn insert_deployment_with_legacy_metadata(
        &mut self,
        deployment: Deployment,
        metadata: LegacySelectorMetadata,
    ) {
        let deployment_id = deployment.id.clone();
        self.insert_deployment(deployment);
        self.legacy_selector_metadata
            .insert(deployment_id, metadata);
    }

    fn remove_deployment(&mut self, id: &str) -> Option<Deployment> {
        let removed = self.deployments.remove(id);
        self.legacy_selector_metadata.remove(id);

        if let Some(ref deployment) = removed {
            self.remove_from_model_index(&deployment.model_name, id);
        }

        removed.map(|deployment| deployment.as_ref().clone())
    }

    #[allow(dead_code)] // Consumed by the staged D3 legacy-selector migration.
    pub(crate) fn resolve_legacy_credential(
        &self,
        model_name: &str,
        credential: &str,
    ) -> Result<DeploymentId, ProviderError> {
        self.resolve_legacy_credential_with(model_name, credential, |raw| {
            CredentialDigest::from_credential(raw)
        })
    }

    fn resolve_legacy_credential_with(
        &self,
        model_name: &str,
        credential: &str,
        digest: impl FnOnce(&str) -> CredentialDigest,
    ) -> Result<DeploymentId, ProviderError> {
        let request_digest = digest(credential);
        let resolved_model = self.resolve_model_name(model_name);
        let candidates = self.model_index.get(&resolved_model);
        let mut matched = None;
        let mut match_count = 0usize;

        for deployment_id in candidates.into_iter().flatten() {
            let Some(metadata) = self.legacy_selector_metadata.get(deployment_id) else {
                continue;
            };
            if metadata.credential_matches(&request_digest) {
                match_count += 1;
                matched.get_or_insert_with(|| deployment_id.clone());
            }
        }

        match (match_count, matched) {
            (1, Some(deployment_id)) => Ok(deployment_id),
            (0, _) => Err(ProviderError::model_not_found("router", resolved_model)),
            _ => Err(ProviderError::configuration(
                "router",
                "legacy credential selector matched multiple deployments",
            )),
        }
    }

    #[cfg(test)]
    pub(crate) fn resolve_legacy_credential_with_test_hasher(
        &self,
        model_name: &str,
        credential: &str,
        digest: impl FnOnce(&str) -> CredentialDigest,
    ) -> Result<DeploymentId, ProviderError> {
        self.resolve_legacy_credential_with(model_name, credential, digest)
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
            if let Some(position) = self
                .model_order
                .iter()
                .position(|existing| existing == model_name)
            {
                self.model_order.remove(position);
            }
        }
    }

    pub(super) fn add_model_alias(
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

    /// One active health probe task per gateway provider.
    pub(crate) health_probe_tasks: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
}

impl Router {
    /// Create a new router with the given configuration
    pub fn new(config: RouterConfig) -> Self {
        Self {
            routing_snapshot: ArcSwap::from_pointee(RoutingSnapshot::empty()),
            routing_snapshot_write_lock: Mutex::new(()),
            config,
            fallback_config: FallbackConfig::default(),
            round_robin_counters: Default::default(),
            provider_selected_count: AtomicU64::new(0),
            strategy_used_count: AtomicU64::new(0),
            fallback_triggered_count: AtomicU64::new(0),
            health_probe_tasks: Mutex::new(HashMap::new()),
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

    fn update_routing_snapshot<T>(&self, update: impl FnOnce(&mut RoutingSnapshot) -> T) -> T {
        let _guard = self.routing_snapshot_write_lock.lock();
        let mut next = self.routing_snapshot.load_full().as_ref().clone();
        let result = update(&mut next);
        next.generation = next_routing_generation();
        self.routing_snapshot.store(Arc::new(next));
        result
    }

    pub(super) fn try_update_routing_snapshot<T, E>(
        &self,
        update: impl FnOnce(&mut RoutingSnapshot) -> Result<T, E>,
    ) -> Result<T, E> {
        let _guard = self.routing_snapshot_write_lock.lock();
        let mut next = self.routing_snapshot.load_full().as_ref().clone();
        let result = update(&mut next)?;
        next.generation = next_routing_generation();
        self.routing_snapshot.store(Arc::new(next));
        Ok(result)
    }

    pub(super) fn load_routing_snapshot(&self) -> Arc<RoutingSnapshot> {
        self.routing_snapshot.load_full()
    }

    pub(super) fn publish_current_snapshot(&self) {
        self.update_routing_snapshot(|_| ());
    }

    /// Add a deployment to the router
    pub fn add_deployment(&self, deployment: Deployment) {
        self.update_routing_snapshot(|snapshot| {
            snapshot.legacy_selector_metadata.remove(&deployment.id);
            snapshot.insert_deployment(deployment)
        });
    }

    /// Remove a deployment from the router
    pub fn remove_deployment(&self, id: &str) -> Option<Deployment> {
        self.update_routing_snapshot(|snapshot| snapshot.remove_deployment(id))
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
        self.try_update_routing_snapshot(|snapshot| snapshot.add_model_alias(alias, model_name))
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
                .supports_capability_for_model(&deployment.model, capability)
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

    /// List model groups in first-insertion order for this immutable snapshot.
    pub fn list_models_in_insertion_order(&self) -> Vec<String> {
        self.routing_snapshot.load().model_order.clone()
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
                deployment.promote_to_healthy_if_degraded();
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
        let snapshot = self.load_routing_snapshot();
        self.get_models_with_fallbacks_for_snapshot(snapshot.as_ref(), model_name, fallback_type)
    }

    pub(super) fn get_models_with_fallbacks_for_snapshot(
        &self,
        snapshot: &RoutingSnapshot,
        model_name: &str,
        fallback_type: FallbackType,
    ) -> Vec<String> {
        let resolved = snapshot.resolve_model_name(model_name);
        let mut fallbacks = self
            .fallback_config
            .get_fallbacks_for_type(&resolved, fallback_type);
        if fallbacks.is_empty() && fallback_type != FallbackType::General {
            fallbacks = self
                .fallback_config
                .get_fallbacks_for_type(&resolved, FallbackType::General);
        }
        let mut models = vec![resolved];
        models.extend(fallbacks);
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

impl Drop for Router {
    fn drop(&mut self) {
        for (_, task) in self.health_probe_tasks.get_mut().drain() {
            task.abort();
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new(RouterConfig::default())
    }
}
