//! Provider and model budget limits management.

use super::config::{
    BudgetLimitKind, BudgetLimitSnapshot, BudgetPersistenceEvent, BudgetPersistenceSender,
    ModelLimitConfig, ProviderLimitConfig,
};
use super::distributed::{
    BudgetLeaseBackend, BudgetLeaseScope, LeaseSnapshot, budget_period_epoch,
};
use super::types::{
    BudgetStatus, ModelBudget, ModelUsageStats, ProviderBudget, ProviderUsageStats,
};
use super::{BudgetAmount, release_budget_spend};
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, info, warn};

pub(crate) struct RequestCounter {
    pub(crate) count: AtomicU64,
}

impl Default for RequestCounter {
    fn default() -> Self {
        Self {
            count: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
pub struct ProviderBudgetManager {
    pub(crate) budgets: Arc<DashMap<String, ProviderBudget>>,
    pub(crate) request_counts: Arc<DashMap<String, RequestCounter>>,
    pub(crate) reserved_spend: Arc<DashMap<String, BudgetAmount>>,
    pub(crate) enabled: Arc<std::sync::atomic::AtomicBool>,
    persistence_tx: Option<BudgetPersistenceSender>,
    pub(crate) backend: BudgetLeaseBackend,
}

impl Default for ProviderBudgetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderBudgetManager {
    pub fn new() -> Self {
        Self {
            budgets: Arc::new(DashMap::new()),
            request_counts: Arc::new(DashMap::new()),
            reserved_spend: Arc::new(DashMap::new()),
            enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            persistence_tx: None,
            backend: BudgetLeaseBackend::InProcess,
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            budgets: Arc::new(DashMap::with_capacity(capacity)),
            request_counts: Arc::new(DashMap::with_capacity(capacity)),
            reserved_spend: Arc::new(DashMap::with_capacity(capacity)),
            enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            persistence_tx: None,
            backend: BudgetLeaseBackend::InProcess,
        }
    }
    pub fn with_persistence(mut self, persistence_tx: BudgetPersistenceSender) -> Self {
        self.persistence_tx = Some(persistence_tx);
        self
    }
    #[cfg(feature = "gateway")]
    pub fn with_redis(mut self, redis: Arc<crate::storage::redis::RedisPool>) -> Self {
        self.backend = BudgetLeaseBackend::redis(redis);
        self
    }
    #[cfg(all(test, feature = "gateway"))]
    pub fn with_redis_lease_ttl(
        mut self,
        redis: Arc<crate::storage::redis::RedisPool>,
        lease_ttl_ms: i64,
    ) -> Self {
        self.backend = BudgetLeaseBackend::redis_with_ttl(redis, lease_ttl_ms);
        self
    }

    pub(crate) fn sync_lease_snapshot(&self, name: &str, snapshot: LeaseSnapshot) {
        if let Some(mut budget) = self.budgets.get_mut(name) {
            budget.current_spend = snapshot
                .committed
                .checked_add(snapshot.outstanding)
                .unwrap_or(snapshot.committed)
                .as_f64();
            budget.updated_at = chrono::Utc::now();
        }
        if snapshot.outstanding == BudgetAmount::zero() {
            self.reserved_spend.remove(name);
        } else {
            self.reserved_spend
                .insert(name.to_string(), snapshot.outstanding);
        }
    }

    fn reset_distributed_lease(&self, name: &str, reset_period: super::types::ResetPeriod) {
        if !self.backend.is_distributed() {
            return;
        }
        let epoch = budget_period_epoch(reset_period, chrono::Utc::now());
        match self.backend.reset(BudgetLeaseScope::Provider, name, epoch) {
            Ok(snapshot) => self.sync_lease_snapshot(name, snapshot),
            Err(err) => warn!(
                provider = name,
                error = ?err,
                "failed to reset distributed provider budget lease; failing closed on later reserves"
            ),
        }
    }

    pub(crate) fn send_persistence_event(&self, event: BudgetPersistenceEvent) {
        if let Some(tx) = &self.persistence_tx
            && tx.send(event).is_err()
        {
            warn!("Provider budget persistence worker is unavailable");
        }
    }

    pub(crate) fn snapshot_for(&self, budget: &ProviderBudget) -> BudgetLimitSnapshot {
        let request_count = self
            .request_counts
            .get(&budget.provider_name)
            .map(|c| c.count.load(Ordering::Relaxed))
            .unwrap_or(0);
        let current_spend = committed_spend(
            budget.current_spend,
            self.reserved_spend
                .get(&budget.provider_name)
                .map(|reserved| *reserved)
                .unwrap_or_else(BudgetAmount::zero),
        );

        BudgetLimitSnapshot {
            kind: BudgetLimitKind::Provider,
            name: budget.provider_name.clone(),
            max_budget: budget.max_budget,
            current_spend,
            soft_limit: budget.soft_limit,
            reset_period: budget.reset_period,
            currency: budget.currency,
            enabled: budget.enabled,
            last_reset_at: budget.last_reset_at,
            request_count,
        }
    }
    pub fn restore_snapshot(&self, snapshot: &BudgetLimitSnapshot) {
        if snapshot.kind != BudgetLimitKind::Provider {
            return;
        }

        let mut budget = ProviderBudget::new(&snapshot.name, snapshot.max_budget);
        budget.current_spend = snapshot.current_spend;
        budget.soft_limit = snapshot.soft_limit;
        budget.reset_period = snapshot.reset_period;
        budget.currency = snapshot.currency;
        budget.enabled = snapshot.enabled;
        budget.last_reset_at = snapshot.last_reset_at;

        self.budgets.insert(snapshot.name.clone(), budget);
        self.request_counts.insert(
            snapshot.name.clone(),
            RequestCounter {
                count: AtomicU64::new(snapshot.request_count),
            },
        );
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
    pub fn set_provider_limit(&self, provider: &str, config: ProviderLimitConfig) {
        let percentage = config.soft_limit_percentage;
        self.set_provider_limit_optional(provider, config, Some(percentage));
    }
    pub fn set_provider_limit_optional(
        &self,
        provider: &str,
        mut config: ProviderLimitConfig,
        percentage: Option<f64>,
    ) {
        if let Some(percentage) = percentage {
            config.soft_limit_percentage = percentage;
        }
        if !config.max_budget.is_finite() || config.max_budget <= 0.0 {
            warn!(
                "Rejected invalid provider budget limit for '{}': {}",
                provider, config.max_budget
            );
            return;
        }
        if !config.soft_limit_percentage.is_finite()
            || !(0.0..=1.0).contains(&config.soft_limit_percentage)
        {
            warn!(
                "Rejected invalid provider budget soft limit for '{}': {}",
                provider, config.soft_limit_percentage
            );
            return;
        }
        info!(
            "Setting provider budget limit for '{}': ${:.2} ({})",
            provider, config.max_budget, config.reset_period
        );
        let snapshot = match self.budgets.entry(provider.to_string()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let budget = entry.get_mut();
                config.soft_limit_percentage =
                    percentage.unwrap_or_else(|| budget.soft_limit / budget.max_budget);
                budget.max_budget = config.max_budget;
                budget.soft_limit = config.max_budget * config.soft_limit_percentage;
                budget.reset_period = config.reset_period;
                budget.currency = config.currency;
                budget.enabled = config.enabled;
                budget.updated_at = chrono::Utc::now();
                self.snapshot_for(budget)
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let mut budget = ProviderBudget::new(provider, config.max_budget);
                budget.soft_limit = config.max_budget * config.soft_limit_percentage;
                budget.reset_period = config.reset_period;
                budget.currency = config.currency;
                budget.enabled = config.enabled;
                let snapshot = self.snapshot_for(&budget);
                entry.insert(budget);
                snapshot
            }
        };
        self.request_counts.entry(provider.to_string()).or_default();
        self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
    }
    pub fn remove_provider_limit(&self, provider: &str) -> bool {
        let removed = self.budgets.remove(provider).is_some();
        self.request_counts.remove(provider);
        self.reserved_spend.remove(provider);

        if removed {
            info!("Removed provider budget limit for '{}'", provider);
            self.send_persistence_event(BudgetPersistenceEvent::Delete {
                kind: BudgetLimitKind::Provider,
                name: provider.to_string(),
            });
        }

        removed
    }
    pub fn has_provider_limit(&self, provider: &str) -> bool {
        self.budgets.contains_key(provider)
    }
    pub fn get_provider_soft_limit_percentage(&self, provider: &str) -> Option<f64> {
        self.budgets
            .get(provider)
            .map(|budget| budget.soft_limit / budget.max_budget)
    }
    pub fn check_provider_budget(&self, provider: &str) -> BudgetStatus {
        if !self.is_enabled() {
            return BudgetStatus::Ok;
        }

        match self.budgets.get(provider) {
            Some(budget) => {
                if !budget.enabled {
                    return BudgetStatus::Ok;
                }
                budget.status()
            }
            None => BudgetStatus::Ok,
        }
    }
    pub fn can_provider_spend(&self, provider: &str, amount: f64) -> bool {
        if super::BudgetAmount::from_f64(amount).is_err() {
            return false;
        }
        if !self.is_enabled() {
            return true;
        }

        match self.budgets.get(provider) {
            Some(budget) => budget.can_spend(amount),
            None => true, // No budget configured = unlimited
        }
    }
    pub fn record_provider_spend(&self, provider: &str, amount: f64) -> Option<BudgetStatus> {
        if amount <= 0.0 || super::BudgetAmount::from_f64(amount).is_err() {
            return None;
        }
        if let Some(counter) = self.request_counts.get(provider) {
            counter.count.fetch_add(1, Ordering::Relaxed);
        }
        self.budgets.get_mut(provider).map(|mut budget| {
            let previous_status = budget.status();
            budget.record_spend(amount);
            let new_status = budget.status();

            debug!(
                "Recorded spend ${:.4} for provider '{}': ${:.2} / ${:.2} ({})",
                amount, provider, budget.current_spend, budget.max_budget, new_status
            );
            if new_status != previous_status {
                match new_status {
                    BudgetStatus::Warning => {
                        warn!(
                            "Provider '{}' approaching budget limit: ${:.2} / ${:.2} ({:.1}%)",
                            provider,
                            budget.current_spend,
                            budget.max_budget,
                            budget.usage_percentage()
                        );
                    }
                    BudgetStatus::Exceeded => {
                        warn!(
                            "Provider '{}' exceeded budget limit: ${:.2} / ${:.2}",
                            provider, budget.current_spend, budget.max_budget
                        );
                    }
                    BudgetStatus::Ok => {}
                }
            }

            let snapshot = self.snapshot_for(&budget);
            drop(budget);
            self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));

            new_status
        })
    }
    pub fn get_provider_usage(&self, provider: &str) -> Option<ProviderUsageStats> {
        let budget = self.budgets.get(provider)?;
        let request_count = self
            .request_counts
            .get(provider)
            .map(|c| c.count.load(Ordering::Relaxed))
            .unwrap_or(0);

        Some(ProviderUsageStats {
            provider_name: provider.to_string(),
            current_spend: budget.current_spend,
            max_budget: budget.max_budget,
            remaining: budget.remaining(),
            usage_percentage: budget.usage_percentage(),
            status: budget.status(),
            reset_period: budget.reset_period,
            last_reset_at: budget.last_reset_at,
            request_count,
        })
    }
    pub fn list_provider_budgets(&self) -> Vec<ProviderBudget> {
        self.budgets.iter().map(|r| r.value().clone()).collect()
    }
    pub fn list_provider_usage(&self) -> Vec<ProviderUsageStats> {
        self.budgets
            .iter()
            .map(|r| {
                let provider = r.key();
                let budget = r.value();
                let request_count = self
                    .request_counts
                    .get(provider)
                    .map(|c| c.count.load(Ordering::Relaxed))
                    .unwrap_or(0);

                ProviderUsageStats {
                    provider_name: provider.clone(),
                    current_spend: budget.current_spend,
                    max_budget: budget.max_budget,
                    remaining: budget.remaining(),
                    usage_percentage: budget.usage_percentage(),
                    status: budget.status(),
                    reset_period: budget.reset_period,
                    last_reset_at: budget.last_reset_at,
                    request_count,
                }
            })
            .collect()
    }
    pub fn get_available_providers(&self) -> Vec<String> {
        if !self.is_enabled() {
            return self.budgets.iter().map(|r| r.key().clone()).collect();
        }

        self.budgets
            .iter()
            .filter(|r| {
                let budget = r.value();
                !budget.enabled || budget.status() != BudgetStatus::Exceeded
            })
            .map(|r| r.key().clone())
            .collect()
    }
    pub fn get_exceeded_providers(&self) -> Vec<String> {
        self.budgets
            .iter()
            .filter(|r| {
                let budget = r.value();
                budget.enabled && budget.status() == BudgetStatus::Exceeded
            })
            .map(|r| r.key().clone())
            .collect()
    }
    pub fn reset_provider_budget(&self, provider: &str) -> bool {
        if let Some(mut budget) = self.budgets.get_mut(provider) {
            info!(
                "Resetting provider '{}' budget (was ${:.2})",
                provider, budget.current_spend
            );
            let reset_period = budget.reset_period;
            budget.reset();
            if let Some(counter) = self.request_counts.get(provider) {
                counter.count.store(0, Ordering::Relaxed);
            }
            self.reserved_spend.remove(provider);

            let snapshot = self.snapshot_for(&budget);
            drop(budget);
            self.reset_distributed_lease(provider, reset_period);
            self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
            true
        } else {
            false
        }
    }
    pub fn reset_due_budgets(&self) -> Vec<String> {
        let mut reset_providers = Vec::new();

        for mut entry in self.budgets.iter_mut() {
            if entry.value().should_reset() {
                let provider = entry.key().clone();
                info!(
                    "Auto-resetting provider '{}' budget (was ${:.2})",
                    provider,
                    entry.value().current_spend
                );
                let reset_period = entry.value().reset_period;
                entry.value_mut().reset();
                if let Some(counter) = self.request_counts.get(&provider) {
                    counter.count.store(0, Ordering::Relaxed);
                }
                self.reserved_spend.remove(&provider);

                let snapshot = self.snapshot_for(entry.value());
                self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
                reset_providers.push((provider, reset_period));
            }
        }

        for (provider, reset_period) in &reset_providers {
            self.reset_distributed_lease(provider, *reset_period);
        }

        reset_providers.into_iter().map(|(name, _)| name).collect()
    }
    pub fn provider_count(&self) -> usize {
        self.budgets.len()
    }
}

#[derive(Clone)]
pub struct ModelBudgetManager {
    pub(crate) budgets: Arc<DashMap<String, ModelBudget>>,
    pub(crate) request_counts: Arc<DashMap<String, RequestCounter>>,
    pub(crate) reserved_spend: Arc<DashMap<String, BudgetAmount>>,
    pub(crate) enabled: Arc<std::sync::atomic::AtomicBool>,
    persistence_tx: Option<BudgetPersistenceSender>,
    pub(crate) backend: BudgetLeaseBackend,
}

impl Default for ModelBudgetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelBudgetManager {
    pub fn new() -> Self {
        Self {
            budgets: Arc::new(DashMap::new()),
            request_counts: Arc::new(DashMap::new()),
            reserved_spend: Arc::new(DashMap::new()),
            enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            persistence_tx: None,
            backend: BudgetLeaseBackend::InProcess,
        }
    }
    pub fn with_persistence(mut self, persistence_tx: BudgetPersistenceSender) -> Self {
        self.persistence_tx = Some(persistence_tx);
        self
    }
    #[cfg(feature = "gateway")]
    pub fn with_redis(mut self, redis: Arc<crate::storage::redis::RedisPool>) -> Self {
        self.backend = BudgetLeaseBackend::redis(redis);
        self
    }
    #[cfg(all(test, feature = "gateway"))]
    pub fn with_redis_lease_ttl(
        mut self,
        redis: Arc<crate::storage::redis::RedisPool>,
        lease_ttl_ms: i64,
    ) -> Self {
        self.backend = BudgetLeaseBackend::redis_with_ttl(redis, lease_ttl_ms);
        self
    }

    pub(crate) fn sync_lease_snapshot(&self, name: &str, snapshot: LeaseSnapshot) {
        if let Some(mut budget) = self.budgets.get_mut(name) {
            budget.current_spend = snapshot
                .committed
                .checked_add(snapshot.outstanding)
                .unwrap_or(snapshot.committed)
                .as_f64();
            budget.updated_at = chrono::Utc::now();
        }
        if snapshot.outstanding == BudgetAmount::zero() {
            self.reserved_spend.remove(name);
        } else {
            self.reserved_spend
                .insert(name.to_string(), snapshot.outstanding);
        }
    }

    fn reset_distributed_lease(&self, name: &str, reset_period: super::types::ResetPeriod) {
        if !self.backend.is_distributed() {
            return;
        }
        let epoch = budget_period_epoch(reset_period, chrono::Utc::now());
        match self.backend.reset(BudgetLeaseScope::Model, name, epoch) {
            Ok(snapshot) => self.sync_lease_snapshot(name, snapshot),
            Err(err) => warn!(
                model = name,
                error = ?err,
                "failed to reset distributed model budget lease; failing closed on later reserves"
            ),
        }
    }

    pub(crate) fn send_persistence_event(&self, event: BudgetPersistenceEvent) {
        if let Some(tx) = &self.persistence_tx
            && tx.send(event).is_err()
        {
            warn!("Model budget persistence worker is unavailable");
        }
    }

    pub(crate) fn snapshot_for(&self, budget: &ModelBudget) -> BudgetLimitSnapshot {
        let request_count = self
            .request_counts
            .get(&budget.model_name)
            .map(|c| c.count.load(Ordering::Relaxed))
            .unwrap_or(0);
        let current_spend = committed_spend(
            budget.current_spend,
            self.reserved_spend
                .get(&budget.model_name)
                .map(|reserved| *reserved)
                .unwrap_or_else(BudgetAmount::zero),
        );

        BudgetLimitSnapshot {
            kind: BudgetLimitKind::Model,
            name: budget.model_name.clone(),
            max_budget: budget.max_budget,
            current_spend,
            soft_limit: budget.soft_limit,
            reset_period: budget.reset_period,
            currency: budget.currency,
            enabled: budget.enabled,
            last_reset_at: budget.last_reset_at,
            request_count,
        }
    }
    pub fn restore_snapshot(&self, snapshot: &BudgetLimitSnapshot) {
        if snapshot.kind != BudgetLimitKind::Model {
            return;
        }

        let mut budget = ModelBudget::new(&snapshot.name, snapshot.max_budget);
        budget.current_spend = snapshot.current_spend;
        budget.soft_limit = snapshot.soft_limit;
        budget.reset_period = snapshot.reset_period;
        budget.currency = snapshot.currency;
        budget.enabled = snapshot.enabled;
        budget.last_reset_at = snapshot.last_reset_at;

        self.budgets.insert(snapshot.name.clone(), budget);
        self.request_counts.insert(
            snapshot.name.clone(),
            RequestCounter {
                count: AtomicU64::new(snapshot.request_count),
            },
        );
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
    pub fn set_model_limit(&self, model: &str, config: ModelLimitConfig) {
        let percentage = config.soft_limit_percentage;
        self.set_model_limit_optional(model, config, Some(percentage));
    }
    pub fn set_model_limit_optional(
        &self,
        model: &str,
        mut config: ModelLimitConfig,
        percentage: Option<f64>,
    ) {
        if let Some(percentage) = percentage {
            config.soft_limit_percentage = percentage;
        }
        if !config.max_budget.is_finite() || config.max_budget <= 0.0 {
            warn!(
                "Rejected invalid model budget limit for '{}': {}",
                model, config.max_budget
            );
            return;
        }
        if !config.soft_limit_percentage.is_finite()
            || !(0.0..=1.0).contains(&config.soft_limit_percentage)
        {
            warn!(
                "Rejected invalid model budget soft limit for '{}': {}",
                model, config.soft_limit_percentage
            );
            return;
        }
        info!(
            "Setting model budget limit for '{}': ${:.2} ({})",
            model, config.max_budget, config.reset_period
        );
        let snapshot = match self.budgets.entry(model.to_string()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let budget = entry.get_mut();
                config.soft_limit_percentage =
                    percentage.unwrap_or_else(|| budget.soft_limit / budget.max_budget);
                budget.max_budget = config.max_budget;
                budget.soft_limit = config.max_budget * config.soft_limit_percentage;
                budget.reset_period = config.reset_period;
                budget.currency = config.currency;
                budget.enabled = config.enabled;
                budget.updated_at = chrono::Utc::now();
                self.snapshot_for(budget)
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let mut budget = ModelBudget::new(model, config.max_budget);
                budget.soft_limit = config.max_budget * config.soft_limit_percentage;
                budget.reset_period = config.reset_period;
                budget.currency = config.currency;
                budget.enabled = config.enabled;
                let snapshot = self.snapshot_for(&budget);
                entry.insert(budget);
                snapshot
            }
        };
        self.request_counts.entry(model.to_string()).or_default();
        self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
    }
    pub fn remove_model_limit(&self, model: &str) -> bool {
        let removed = self.budgets.remove(model).is_some();
        self.request_counts.remove(model);
        self.reserved_spend.remove(model);

        if removed {
            info!("Removed model budget limit for '{}'", model);
            self.send_persistence_event(BudgetPersistenceEvent::Delete {
                kind: BudgetLimitKind::Model,
                name: model.to_string(),
            });
        }

        removed
    }
    pub fn has_model_limit(&self, model: &str) -> bool {
        self.budgets.contains_key(model)
    }
    pub fn get_model_soft_limit_percentage(&self, model: &str) -> Option<f64> {
        self.budgets
            .get(model)
            .map(|budget| budget.soft_limit / budget.max_budget)
    }
    pub fn check_model_budget(&self, model: &str) -> BudgetStatus {
        if !self.is_enabled() {
            return BudgetStatus::Ok;
        }

        match self.budgets.get(model) {
            Some(budget) => {
                if !budget.enabled {
                    return BudgetStatus::Ok;
                }
                budget.status()
            }
            None => BudgetStatus::Ok,
        }
    }
    pub fn can_model_spend(&self, model: &str, amount: f64) -> bool {
        if super::BudgetAmount::from_f64(amount).is_err() {
            return false;
        }
        if !self.is_enabled() {
            return true;
        }

        match self.budgets.get(model) {
            Some(budget) => budget.can_spend(amount),
            None => true,
        }
    }
    pub fn record_model_spend(&self, model: &str, amount: f64) -> Option<BudgetStatus> {
        if amount <= 0.0 || super::BudgetAmount::from_f64(amount).is_err() {
            return None;
        }
        if let Some(counter) = self.request_counts.get(model) {
            counter.count.fetch_add(1, Ordering::Relaxed);
        }
        self.budgets.get_mut(model).map(|mut budget| {
            let previous_status = budget.status();
            budget.record_spend(amount);
            let new_status = budget.status();

            debug!(
                "Recorded spend ${:.4} for model '{}': ${:.2} / ${:.2} ({})",
                amount, model, budget.current_spend, budget.max_budget, new_status
            );

            if new_status != previous_status && new_status == BudgetStatus::Exceeded {
                warn!(
                    "Model '{}' exceeded budget limit: ${:.2} / ${:.2}",
                    model, budget.current_spend, budget.max_budget
                );
            }

            let snapshot = self.snapshot_for(&budget);
            drop(budget);
            self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));

            new_status
        })
    }
    pub fn get_model_usage(&self, model: &str) -> Option<ModelUsageStats> {
        let budget = self.budgets.get(model)?;
        let request_count = self
            .request_counts
            .get(model)
            .map(|c| c.count.load(Ordering::Relaxed))
            .unwrap_or(0);

        Some(ModelUsageStats {
            model_name: model.to_string(),
            current_spend: budget.current_spend,
            max_budget: budget.max_budget,
            remaining: budget.remaining(),
            usage_percentage: budget.usage_percentage(),
            status: budget.status(),
            reset_period: budget.reset_period,
            last_reset_at: budget.last_reset_at,
            request_count,
        })
    }
    pub fn list_model_budgets(&self) -> Vec<ModelBudget> {
        self.budgets.iter().map(|r| r.value().clone()).collect()
    }
    pub fn list_model_usage(&self) -> Vec<ModelUsageStats> {
        self.budgets
            .iter()
            .map(|r| {
                let model = r.key();
                let budget = r.value();
                let request_count = self
                    .request_counts
                    .get(model)
                    .map(|c| c.count.load(Ordering::Relaxed))
                    .unwrap_or(0);

                ModelUsageStats {
                    model_name: model.clone(),
                    current_spend: budget.current_spend,
                    max_budget: budget.max_budget,
                    remaining: budget.remaining(),
                    usage_percentage: budget.usage_percentage(),
                    status: budget.status(),
                    reset_period: budget.reset_period,
                    last_reset_at: budget.last_reset_at,
                    request_count,
                }
            })
            .collect()
    }
    pub fn reset_model_budget(&self, model: &str) -> bool {
        if let Some(mut budget) = self.budgets.get_mut(model) {
            info!(
                "Resetting model '{}' budget (was ${:.2})",
                model, budget.current_spend
            );
            let reset_period = budget.reset_period;
            budget.reset();

            if let Some(counter) = self.request_counts.get(model) {
                counter.count.store(0, Ordering::Relaxed);
            }
            self.reserved_spend.remove(model);

            let snapshot = self.snapshot_for(&budget);
            drop(budget);
            self.reset_distributed_lease(model, reset_period);
            self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
            true
        } else {
            false
        }
    }
    pub fn reset_due_budgets(&self) -> Vec<String> {
        let mut reset_models = Vec::new();

        for mut entry in self.budgets.iter_mut() {
            if entry.value().should_reset() {
                let model = entry.key().clone();
                info!(
                    "Auto-resetting model '{}' budget (was ${:.2})",
                    model,
                    entry.value().current_spend
                );
                let reset_period = entry.value().reset_period;
                entry.value_mut().reset();

                if let Some(counter) = self.request_counts.get(&model) {
                    counter.count.store(0, Ordering::Relaxed);
                }
                self.reserved_spend.remove(&model);

                let snapshot = self.snapshot_for(entry.value());
                self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
                reset_models.push((model, reset_period));
            }
        }

        for (model, reset_period) in &reset_models {
            self.reset_distributed_lease(model, *reset_period);
        }

        reset_models.into_iter().map(|(name, _)| name).collect()
    }
    pub fn model_count(&self) -> usize {
        self.budgets.len()
    }
}

fn committed_spend(current_spend: f64, reserved: BudgetAmount) -> f64 {
    release_budget_spend(current_spend, reserved, true)
        .map(|amount| amount.as_f64())
        .unwrap_or(current_spend)
}
