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
    /// Gateway API-key accounting uses SQL; the generic embedded tracker stays local.
    #[cfg(feature = "gateway")]
    api_key_database: Option<Arc<crate::storage::database::Database>>,
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
            #[cfg(feature = "gateway")]
            api_key_database: None,
            tracker: Arc::new(BudgetTracker::new()),
            config: Arc::new(RwLock::new(BudgetManagerConfig::default())),
        }
    }

    /// Create a budget manager with custom configuration
    pub fn with_config(config: BudgetManagerConfig) -> Self {
        Self {
            #[cfg(feature = "gateway")]
            api_key_database: None,
            tracker: Arc::new(BudgetTracker::new()),
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Create a budget manager with an existing tracker
    pub fn with_tracker(tracker: BudgetTracker) -> Self {
        Self {
            #[cfg(feature = "gateway")]
            api_key_database: None,
            tracker: Arc::new(tracker),
            config: Arc::new(RwLock::new(BudgetManagerConfig::default())),
        }
    }

    #[cfg(feature = "gateway")]
    pub(crate) fn with_api_key_database(
        mut self,
        database: Arc<crate::storage::database::Database>,
    ) -> Self {
        self.api_key_database = Some(database);
        self
    }

    #[cfg(feature = "gateway")]
    pub(crate) fn api_key_database(&self) -> Option<&Arc<crate::storage::database::Database>> {
        self.api_key_database.as_ref()
    }

    /// Get a reference to the internal tracker
    pub fn tracker(&self) -> &BudgetTracker {
        &self.tracker
    }

    /// Create a new budget
    pub async fn create_budget(&self, scope: BudgetScope, config: BudgetConfig) -> Result<Budget> {
        // Validate configuration
        if !config.max_budget.is_finite() || config.max_budget <= 0.0 {
            return Err(GatewayError::Validation(
                "max_budget must be finite and greater than 0".to_string(),
            ));
        }
        if let Some(soft_limit) = config.soft_limit
            && (!soft_limit.is_finite() || soft_limit < 0.0)
        {
            return Err(GatewayError::Validation(
                "soft_limit must be finite and non-negative".to_string(),
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
        if !config.max_budget.is_finite() || config.max_budget <= 0.0 {
            return Err(GatewayError::Validation(
                "max_budget must be finite and greater than 0".to_string(),
            ));
        }
        if let Some(soft_limit) = config.soft_limit
            && (!soft_limit.is_finite() || soft_limit < 0.0)
        {
            return Err(GatewayError::Validation(
                "soft_limit must be finite and non-negative".to_string(),
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
