use super::{
    BudgetLimitKind, BudgetLimitSnapshot, BudgetPersistenceSender, BudgetStatus,
    ModelBudgetManager, ProviderBudgetManager,
};

#[derive(Clone)]
pub struct UnifiedBudgetLimits {
    pub providers: ProviderBudgetManager,
    pub models: ModelBudgetManager,
}

impl Default for UnifiedBudgetLimits {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedBudgetLimits {
    pub fn new() -> Self {
        Self {
            providers: ProviderBudgetManager::new(),
            models: ModelBudgetManager::new(),
        }
    }
    pub fn with_persistence(persistence_tx: BudgetPersistenceSender) -> Self {
        Self {
            providers: ProviderBudgetManager::new().with_persistence(persistence_tx.clone()),
            models: ModelBudgetManager::new().with_persistence(persistence_tx),
        }
    }
    pub fn from_snapshots_with_persistence(
        snapshots: impl IntoIterator<Item = BudgetLimitSnapshot>,
        persistence_tx: BudgetPersistenceSender,
    ) -> Self {
        let limits = Self::with_persistence(persistence_tx);
        for snapshot in snapshots {
            match snapshot.kind {
                BudgetLimitKind::Provider => limits.providers.restore_snapshot(&snapshot),
                BudgetLimitKind::Model => limits.models.restore_snapshot(&snapshot),
            }
        }
        limits
    }
    #[cfg(feature = "gateway")]
    pub fn with_redis(self, redis: std::sync::Arc<crate::storage::redis::RedisPool>) -> Self {
        Self {
            providers: self.providers.with_redis(std::sync::Arc::clone(&redis)),
            models: self.models.with_redis(redis),
        }
    }
    #[cfg(all(test, feature = "gateway"))]
    pub fn with_redis_lease_ttl(
        self,
        redis: std::sync::Arc<crate::storage::redis::RedisPool>,
        lease_ttl_ms: i64,
    ) -> Self {
        Self {
            providers: self
                .providers
                .with_redis_lease_ttl(std::sync::Arc::clone(&redis), lease_ttl_ms),
            models: self.models.with_redis_lease_ttl(redis, lease_ttl_ms),
        }
    }
    #[cfg(test)]
    pub(crate) fn with_unavailable_backend() -> Self {
        let mut limits = Self::new();
        limits.providers.backend = super::distributed::BudgetLeaseBackend::Unavailable;
        limits.models.backend = super::distributed::BudgetLeaseBackend::Unavailable;
        limits
    }
    pub fn can_spend(&self, provider: &str, model: &str, amount: f64) -> bool {
        self.providers.can_provider_spend(provider, amount)
            && self.models.can_model_spend(model, amount)
    }
    pub fn record_spend(&self, provider: &str, model: &str, amount: f64) {
        self.providers.record_provider_spend(provider, amount);
        self.models.record_model_spend(model, amount);
    }
    pub fn filter_available_providers(&self, providers: Vec<String>) -> Vec<String> {
        let exceeded = self.providers.get_exceeded_providers();
        providers
            .into_iter()
            .filter(|provider| !exceeded.contains(provider))
            .collect()
    }
    pub fn is_provider_available(&self, provider: &str) -> bool {
        self.providers.check_provider_budget(provider) != BudgetStatus::Exceeded
    }
    pub fn is_model_available(&self, model: &str) -> bool {
        self.models.check_model_budget(model) != BudgetStatus::Exceeded
    }
    pub fn reset_due_budgets(&self) -> (Vec<String>, Vec<String>) {
        let providers = self.providers.reset_due_budgets();
        let models = self.models.reset_due_budgets();
        (providers, models)
    }
}
