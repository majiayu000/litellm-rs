//! Budget management implementation
//!
//! Provides CRUD operations for budget management, including creating,
//! updating, deleting, and listing budgets.

use super::tracker::{BudgetTracker, SpendResult};
use super::types::{Budget, BudgetCheckResult, BudgetConfig, BudgetScope, BudgetStatus};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Budget manager for CRUD operations
///
/// Provides a high-level interface for managing budgets, wrapping the
/// BudgetTracker with additional functionality like validation and
/// background reset tasks.
#[derive(Clone)]
pub struct BudgetManager {
    /// Internal budget tracker
    tracker: Arc<BudgetTracker>,
    /// Configuration
    config: Arc<RwLock<BudgetManagerConfig>>,
}

/// Configuration for the budget manager
#[derive(Debug, Clone)]
pub struct BudgetManagerConfig {
    /// Whether budget management is enabled
    pub enabled: bool,
    /// Default soft limit percentage (0.0 to 1.0)
    pub default_soft_limit_percentage: f64,
    /// Whether to block requests when budget is exceeded
    pub block_on_exceeded: bool,
    /// Enable automatic budget reset based on period
    pub auto_reset_enabled: bool,
    /// Reset check interval in seconds
    pub reset_check_interval_secs: u64,
}

impl Default for BudgetManagerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_soft_limit_percentage: 0.8,
            block_on_exceeded: true,
            auto_reset_enabled: true,
            reset_check_interval_secs: 60,
        }
    }
}

impl Default for BudgetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetManager {
    /// Create a new budget manager with default configuration
    pub fn new() -> Self {
        Self {
            tracker: Arc::new(BudgetTracker::new()),
            config: Arc::new(RwLock::new(BudgetManagerConfig::default())),
        }
    }

    /// Create a budget manager with custom configuration
    pub fn with_config(config: BudgetManagerConfig) -> Self {
        Self {
            tracker: Arc::new(BudgetTracker::new()),
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Create a budget manager with an existing tracker
    pub fn with_tracker(tracker: BudgetTracker) -> Self {
        Self {
            tracker: Arc::new(tracker),
            config: Arc::new(RwLock::new(BudgetManagerConfig::default())),
        }
    }

    /// Get a reference to the internal tracker
    pub fn tracker(&self) -> &BudgetTracker {
        &self.tracker
    }

    /// Create a new budget
    pub async fn create_budget(&self, scope: BudgetScope, config: BudgetConfig) -> Result<Budget> {
        // Validate configuration
        if config.max_budget <= 0.0 {
            return Err(GatewayError::Validation(
                "max_budget must be greater than 0".to_string(),
            ));
        }

        if config.name.trim().is_empty() {
            return Err(GatewayError::Validation(
                "Budget name cannot be empty".to_string(),
            ));
        }

        // Generate unique ID
        let id = uuid::Uuid::new_v4().to_string();

        // Calculate soft limit
        let manager_config = self.config.read().await;
        let soft_limit = config
            .soft_limit
            .unwrap_or(config.max_budget * manager_config.default_soft_limit_percentage);

        // Create budget
        let mut budget = Budget::new(&id, &config.name, scope.clone(), config.max_budget);
        budget.soft_limit = soft_limit;

        if let Some(period) = config.reset_period {
            budget.reset_period = period;
        }

        if let Some(currency) = config.currency {
            budget.currency = currency;
        }

        if let Some(enabled) = config.enabled {
            budget.enabled = enabled;
        }

        if let Some(metadata) = config.metadata {
            budget.metadata = metadata;
        }

        info!(
            "Creating budget '{}' for scope {} with max ${:.2}",
            budget.name, scope, budget.max_budget
        );

        // Atomically check-and-insert to avoid TOCTOU race between has_budget() and register_budget()
        if !self.tracker.try_register_budget(budget.clone()) {
            return Err(GatewayError::Conflict(format!(
                "Budget already exists for scope: {}",
                scope
            )));
        }

        Ok(budget)
    }

    /// Update an existing budget
    pub async fn update_budget(&self, scope: &BudgetScope, config: BudgetConfig) -> Result<Budget> {
        if !self.tracker.has_budget(scope) {
            return Err(GatewayError::NotFound(format!(
                "Budget not found for scope: {}",
                scope
            )));
        }

        // Validate configuration
        if config.max_budget <= 0.0 {
            return Err(GatewayError::Validation(
                "max_budget must be greater than 0".to_string(),
            ));
        }

        let manager_config = self.config.read().await;

        let updated = self.tracker.update_budget(scope, |budget| {
            budget.name = config.name.clone();
            budget.max_budget = config.max_budget;
            budget.soft_limit = config
                .soft_limit
                .unwrap_or(config.max_budget * manager_config.default_soft_limit_percentage);

            if let Some(period) = config.reset_period {
                budget.reset_period = period;
            }

            if let Some(currency) = config.currency {
                budget.currency = currency;
            }

            if let Some(enabled) = config.enabled {
                budget.enabled = enabled;
            }

            if let Some(metadata) = config.metadata.clone() {
                budget.metadata = metadata;
            }

            debug!(
                "Updated budget '{}' for scope {} with max ${:.2}",
                budget.name, scope, budget.max_budget
            );
        });

        if updated {
            self.tracker.get_budget(scope).ok_or_else(|| {
                GatewayError::Internal("Failed to retrieve updated budget".to_string())
            })
        } else {
            Err(GatewayError::Internal(
                "Failed to update budget".to_string(),
            ))
        }
    }

    /// Delete a budget
    pub async fn delete_budget(&self, scope: &BudgetScope) -> Result<()> {
        if !self.tracker.has_budget(scope) {
            return Err(GatewayError::NotFound(format!(
                "Budget not found for scope: {}",
                scope
            )));
        }

        info!("Deleting budget for scope: {}", scope);
        self.tracker.unregister_budget(scope);

        Ok(())
    }

    /// Get a budget by scope
    pub fn get_budget(&self, scope: &BudgetScope) -> Result<Budget> {
        self.tracker
            .get_budget(scope)
            .ok_or_else(|| GatewayError::NotFound(format!("Budget not found for scope: {}", scope)))
    }

    /// Get a budget by ID
    pub fn get_budget_by_id(&self, id: &str) -> Option<Budget> {
        self.tracker
            .get_all_budgets()
            .into_iter()
            .find(|b| b.id == id)
    }

    /// List all budgets
    pub fn list_budgets(&self) -> Vec<Budget> {
        self.tracker.get_all_budgets()
    }

    /// List budgets with optional filtering
    pub fn list_budgets_filtered(
        &self,
        scope_type: Option<&str>,
        status: Option<BudgetStatus>,
    ) -> Vec<Budget> {
        let mut budgets = match scope_type {
            Some(t) => self.tracker.get_budgets_by_type(t),
            None => self.tracker.get_all_budgets(),
        };

        if let Some(status_filter) = status {
            budgets.retain(|b| b.status() == status_filter);
        }

        budgets
    }

    /// Record spending against a scope
    pub async fn record_spend(&self, scope: &BudgetScope, amount: f64) -> Option<SpendResult> {
        if amount <= 0.0 {
            warn!("Attempted to record non-positive spend: {}", amount);
            return None;
        }

        self.tracker.record_spend(scope, amount)
    }

    /// Check if a spend would be allowed
    pub async fn check_spend(&self, scope: &BudgetScope, amount: f64) -> BudgetCheckResult {
        let config = self.config.read().await;

        if !config.enabled {
            return BudgetCheckResult::no_budget();
        }

        let result = self.tracker.check_spend(scope, amount);

        // If blocking is disabled, always allow
        if !config.block_on_exceeded && !result.allowed {
            return BudgetCheckResult {
                allowed: true,
                ..result
            };
        }

        result
    }

    /// Check budget status for a scope
    pub fn check_budget(&self, scope: &BudgetScope) -> BudgetCheckResult {
        self.tracker.check_budget(scope)
    }

    /// Get remaining budget for a scope
    pub fn get_remaining(&self, scope: &BudgetScope) -> f64 {
        self.tracker.get_remaining(scope)
    }

    /// Get current spend for a scope
    pub fn get_current_spend(&self, scope: &BudgetScope) -> f64 {
        self.tracker.get_current_spend(scope)
    }

    /// Reset a specific budget
    pub async fn reset_budget(&self, scope: &BudgetScope) -> Result<()> {
        if !self.tracker.has_budget(scope) {
            return Err(GatewayError::NotFound(format!(
                "Budget not found for scope: {}",
                scope
            )));
        }

        self.tracker.reset_budget(scope);
        info!("Reset budget for scope: {}", scope);

        Ok(())
    }

    /// Run automatic budget reset based on period
    pub fn run_periodic_reset(&self) -> Vec<String> {
        self.tracker.reset_budgets()
    }

    /// Start background reset task
    pub fn start_reset_task(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let interval = {
                    let config = self.config.read().await;
                    if !config.auto_reset_enabled {
                        tokio::time::Duration::from_secs(60)
                    } else {
                        tokio::time::Duration::from_secs(config.reset_check_interval_secs)
                    }
                };

                tokio::time::sleep(interval).await;

                let config = self.config.read().await;
                if config.auto_reset_enabled {
                    drop(config);
                    let reset_ids = self.run_periodic_reset();
                    if !reset_ids.is_empty() {
                        info!("Periodic reset: {} budgets reset", reset_ids.len());
                    }
                }
            }
        })
    }

    /// Get budgets that are in warning status
    pub fn get_warning_budgets(&self) -> Vec<Budget> {
        self.tracker.get_warning_budgets()
    }

    /// Get budgets that are exceeded
    pub fn get_exceeded_budgets(&self) -> Vec<Budget> {
        self.tracker.get_exceeded_budgets()
    }

    /// Get budget count
    pub fn budget_count(&self) -> usize {
        self.tracker.budget_count()
    }

    /// Update manager configuration
    pub async fn update_config(&self, new_config: BudgetManagerConfig) {
        let mut config = self.config.write().await;
        *config = new_config;
    }

    /// Get current configuration
    pub async fn get_config(&self) -> BudgetManagerConfig {
        self.config.read().await.clone()
    }

    /// Check if budget management is enabled
    pub async fn is_enabled(&self) -> bool {
        self.config.read().await.enabled
    }

    /// Enable or disable budget management
    pub async fn set_enabled(&self, enabled: bool) {
        let mut config = self.config.write().await;
        config.enabled = enabled;
    }

    /// Get budget summary statistics
    pub fn get_summary(&self) -> BudgetSummary {
        let budgets = self.tracker.get_all_budgets();

        let total_budgets = budgets.len();
        let mut total_allocated = 0.0;
        let mut total_spent = 0.0;
        let mut ok_count = 0;
        let mut warning_count = 0;
        let mut exceeded_count = 0;

        for budget in &budgets {
            total_allocated += budget.max_budget;
            total_spent += budget.current_spend;

            match budget.status() {
                BudgetStatus::Ok => ok_count += 1,
                BudgetStatus::Warning => warning_count += 1,
                BudgetStatus::Exceeded => exceeded_count += 1,
            }
        }

        BudgetSummary {
            total_budgets,
            total_allocated,
            total_spent,
            total_remaining: (total_allocated - total_spent).max(0.0),
            ok_count,
            warning_count,
            exceeded_count,
        }
    }
}

/// Summary statistics for all budgets
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetSummary {
    /// Total number of budgets
    pub total_budgets: usize,
    /// Total allocated budget across all budgets
    pub total_allocated: f64,
    /// Total spent across all budgets
    pub total_spent: f64,
    /// Total remaining across all budgets
    pub total_remaining: f64,
    /// Number of budgets in OK status
    pub ok_count: usize,
    /// Number of budgets in warning status
    pub warning_count: usize,
    /// Number of budgets in exceeded status
    pub exceeded_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::budget::types::ResetPeriod;

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = BudgetManager::new();
        assert_eq!(manager.budget_count(), 0);
    }

    #[tokio::test]
    async fn test_create_budget() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Test Budget", 100.0);
        let budget = manager
            .create_budget(BudgetScope::Global, config)
            .await
            .unwrap();

        assert_eq!(budget.name, "Test Budget");
        assert_eq!(budget.max_budget, 100.0);
        assert_eq!(budget.soft_limit, 80.0); // Default 80%
        assert_eq!(manager.budget_count(), 1);
    }

    #[tokio::test]
    async fn test_create_budget_with_custom_soft_limit() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Test Budget", 100.0).with_soft_limit(90.0);
        let budget = manager
            .create_budget(BudgetScope::Global, config)
            .await
            .unwrap();

        assert_eq!(budget.soft_limit, 90.0);
    }

    #[tokio::test]
    async fn test_create_budget_validation() {
        let manager = BudgetManager::new();

        // Test negative budget
        let config = BudgetConfig::new("Test", -10.0);
        let result = manager.create_budget(BudgetScope::Global, config).await;
        assert!(result.is_err());

        // Test empty name
        let config = BudgetConfig::new("", 100.0);
        let result = manager.create_budget(BudgetScope::Global, config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_duplicate_budget() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Test Budget", 100.0);
        manager
            .create_budget(BudgetScope::Global, config.clone())
            .await
            .unwrap();

        // Second create should fail
        let result = manager.create_budget(BudgetScope::Global, config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_budget() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Original", 100.0);
        manager
            .create_budget(BudgetScope::Global, config)
            .await
            .unwrap();

        let update_config = BudgetConfig::new("Updated", 200.0);
        let updated = manager
            .update_budget(&BudgetScope::Global, update_config)
            .await
            .unwrap();

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.max_budget, 200.0);
    }

    #[tokio::test]
    async fn test_update_nonexistent_budget() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Test", 100.0);
        let result = manager.update_budget(&BudgetScope::Global, config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_budget() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Test Budget", 100.0);
        manager
            .create_budget(BudgetScope::Global, config)
            .await
            .unwrap();

        assert_eq!(manager.budget_count(), 1);

        manager.delete_budget(&BudgetScope::Global).await.unwrap();
        assert_eq!(manager.budget_count(), 0);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_budget() {
        let manager = BudgetManager::new();

        let result = manager.delete_budget(&BudgetScope::Global).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_budget() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Test Budget", 100.0);
        let created = manager
            .create_budget(BudgetScope::Global, config)
            .await
            .unwrap();

        let retrieved = manager.get_budget(&BudgetScope::Global).unwrap();
        assert_eq!(retrieved.id, created.id);
    }

    #[tokio::test]
    async fn test_get_budget_by_id() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Test Budget", 100.0);
        let created = manager
            .create_budget(BudgetScope::Global, config)
            .await
            .unwrap();

        let retrieved = manager.get_budget_by_id(&created.id).unwrap();
        assert_eq!(retrieved.name, "Test Budget");
    }

    #[tokio::test]
    async fn test_list_budgets() {
        let manager = BudgetManager::new();

        manager
            .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
            .await
            .unwrap();
        manager
            .create_budget(
                BudgetScope::User("user-1".to_string()),
                BudgetConfig::new("User 1", 50.0),
            )
            .await
            .unwrap();

        let budgets = manager.list_budgets();
        assert_eq!(budgets.len(), 2);
    }

    #[tokio::test]
    async fn test_list_budgets_filtered() {
        let manager = BudgetManager::new();

        manager
            .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
            .await
            .unwrap();
        manager
            .create_budget(
                BudgetScope::User("user-1".to_string()),
                BudgetConfig::new("User 1", 50.0),
            )
            .await
            .unwrap();
        manager
            .create_budget(
                BudgetScope::User("user-2".to_string()),
                BudgetConfig::new("User 2", 50.0),
            )
            .await
            .unwrap();

        let user_budgets = manager.list_budgets_filtered(Some("user"), None);
        assert_eq!(user_budgets.len(), 2);

        let global_budgets = manager.list_budgets_filtered(Some("global"), None);
        assert_eq!(global_budgets.len(), 1);
    }

    #[tokio::test]
    async fn test_record_spend() {
        let manager = BudgetManager::new();

        manager
            .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
            .await
            .unwrap();

        let result = manager
            .record_spend(&BudgetScope::Global, 25.0)
            .await
            .unwrap();

        assert_eq!(result.current_spend, 25.0);
        assert_eq!(manager.get_current_spend(&BudgetScope::Global), 25.0);
    }

    #[tokio::test]
    async fn test_check_spend() {
        let manager = BudgetManager::new();

        manager
            .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
            .await
            .unwrap();

        // Record some spend first
        manager.record_spend(&BudgetScope::Global, 90.0).await;

        let result_ok = manager.check_spend(&BudgetScope::Global, 10.0).await;
        assert!(result_ok.allowed);

        let result_exceed = manager.check_spend(&BudgetScope::Global, 11.0).await;
        assert!(!result_exceed.allowed);
    }

    #[tokio::test]
    async fn test_check_spend_disabled_blocking() {
        let config = BudgetManagerConfig {
            block_on_exceeded: false,
            ..Default::default()
        };
        let manager = BudgetManager::with_config(config);

        manager
            .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
            .await
            .unwrap();

        manager.record_spend(&BudgetScope::Global, 100.0).await;

        // Should still be allowed even though exceeded
        let result = manager.check_spend(&BudgetScope::Global, 10.0).await;
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_reset_budget() {
        let manager = BudgetManager::new();

        manager
            .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
            .await
            .unwrap();

        manager.record_spend(&BudgetScope::Global, 50.0).await;
        assert_eq!(manager.get_current_spend(&BudgetScope::Global), 50.0);

        manager.reset_budget(&BudgetScope::Global).await.unwrap();
        assert_eq!(manager.get_current_spend(&BudgetScope::Global), 0.0);
    }

    #[tokio::test]
    async fn test_get_summary() {
        let manager = BudgetManager::new();

        manager
            .create_budget(BudgetScope::Global, BudgetConfig::new("Global", 100.0))
            .await
            .unwrap();
        manager
            .create_budget(
                BudgetScope::User("user-1".to_string()),
                BudgetConfig::new("User 1", 50.0),
            )
            .await
            .unwrap();

        manager.record_spend(&BudgetScope::Global, 85.0).await; // Warning
        manager
            .record_spend(&BudgetScope::User("user-1".to_string()), 10.0)
            .await; // OK

        let summary = manager.get_summary();

        assert_eq!(summary.total_budgets, 2);
        assert_eq!(summary.total_allocated, 150.0);
        assert_eq!(summary.total_spent, 95.0);
        assert_eq!(summary.total_remaining, 55.0);
        assert_eq!(summary.ok_count, 1);
        assert_eq!(summary.warning_count, 1);
        assert_eq!(summary.exceeded_count, 0);
    }

    #[tokio::test]
    async fn test_config_management() {
        let manager = BudgetManager::new();

        let config = manager.get_config().await;
        assert!(config.enabled);

        manager.set_enabled(false).await;
        assert!(!manager.is_enabled().await);

        let new_config = BudgetManagerConfig {
            enabled: true,
            default_soft_limit_percentage: 0.9,
            block_on_exceeded: false,
            auto_reset_enabled: false,
            reset_check_interval_secs: 120,
        };

        manager.update_config(new_config).await;

        let updated_config = manager.get_config().await;
        assert!(updated_config.enabled);
        assert!(!updated_config.block_on_exceeded);
        assert_eq!(updated_config.default_soft_limit_percentage, 0.9);
    }

    #[tokio::test]
    async fn test_get_warning_and_exceeded_budgets() {
        let manager = BudgetManager::new();

        // OK budget
        manager
            .create_budget(
                BudgetScope::User("user-1".to_string()),
                BudgetConfig::new("User 1", 100.0),
            )
            .await
            .unwrap();

        // Warning budget
        manager
            .create_budget(
                BudgetScope::User("user-2".to_string()),
                BudgetConfig::new("User 2", 100.0),
            )
            .await
            .unwrap();
        manager
            .record_spend(&BudgetScope::User("user-2".to_string()), 85.0)
            .await;

        // Exceeded budget
        manager
            .create_budget(
                BudgetScope::User("user-3".to_string()),
                BudgetConfig::new("User 3", 100.0),
            )
            .await
            .unwrap();
        manager
            .record_spend(&BudgetScope::User("user-3".to_string()), 110.0)
            .await;

        let warning_budgets = manager.get_warning_budgets();
        assert_eq!(warning_budgets.len(), 1);
        assert_eq!(warning_budgets[0].name, "User 2");

        let exceeded_budgets = manager.get_exceeded_budgets();
        assert_eq!(exceeded_budgets.len(), 1);
        assert_eq!(exceeded_budgets[0].name, "User 3");
    }

    #[tokio::test]
    async fn test_create_budget_with_reset_period() {
        let manager = BudgetManager::new();

        let config = BudgetConfig::new("Test", 100.0).with_reset_period(ResetPeriod::Weekly);

        let budget = manager
            .create_budget(BudgetScope::Global, config)
            .await
            .unwrap();

        assert_eq!(budget.reset_period, ResetPeriod::Weekly);
    }

    #[tokio::test]
    async fn test_create_budget_concurrent_no_toctou() {
        use std::sync::Arc;

        let manager = Arc::new(BudgetManager::new());
        let concurrency = 20;

        let handles: Vec<_> = (0..concurrency)
            .map(|_| {
                let m = Arc::clone(&manager);
                tokio::spawn(async move {
                    m.create_budget(BudgetScope::Global, BudgetConfig::new("Concurrent", 100.0))
                        .await
                })
            })
            .collect();

        let mut ok_count = 0usize;
        let mut conflict_count = 0usize;

        for handle in handles {
            match handle.await {
                Ok(Ok(_)) => ok_count += 1,
                Ok(Err(GatewayError::Conflict(_))) => conflict_count += 1,
                Ok(Err(e)) => panic!("unexpected error: {:?}", e),
                Err(e) => panic!("task panicked: {:?}", e),
            }
        }

        // Exactly one insertion must succeed; the rest must return Conflict
        assert_eq!(ok_count, 1, "exactly one create_budget should succeed");
        assert_eq!(
            conflict_count,
            concurrency - 1,
            "all other concurrent calls should return Conflict"
        );
        assert_eq!(manager.budget_count(), 1);
    }
}
