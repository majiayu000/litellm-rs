//! Budget tracking implementation
//!
//! Provides lock-free concurrent budget tracking using DashMap for high-performance
//! budget monitoring in multi-threaded environments.

use super::types::{Budget, BudgetCheckResult, BudgetScope, BudgetStatus};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Budget tracker for concurrent budget monitoring
///
/// Uses DashMap for lock-free concurrent access, allowing multiple threads
/// to record spend and check budgets simultaneously without contention.
#[derive(Clone)]
pub struct BudgetTracker {
    /// Budget storage keyed by scope
    budgets: Arc<DashMap<String, Budget>>,
    /// Track which scopes have alerts already sent to avoid duplicates
    alert_states: Arc<DashMap<String, AlertState>>,
}

/// Internal state for tracking alert status per budget
#[derive(Debug, Clone, Default)]
struct AlertState {
    /// Whether soft limit alert has been sent
    soft_limit_alerted: bool,
    /// Whether exceeded alert has been sent
    exceeded_alerted: bool,
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetTracker {
    /// Create a new budget tracker
    pub fn new() -> Self {
        Self {
            budgets: Arc::new(DashMap::new()),
            alert_states: Arc::new(DashMap::new()),
        }
    }

    /// Create a budget tracker with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            budgets: Arc::new(DashMap::with_capacity(capacity)),
            alert_states: Arc::new(DashMap::with_capacity(capacity)),
        }
    }

    /// Register a budget for tracking
    pub fn register_budget(&self, budget: Budget) {
        let key = budget.scope.to_key();
        debug!("Registering budget: {} ({})", budget.name, key);
        self.budgets.insert(key.clone(), budget);
        self.alert_states.insert(key, AlertState::default());
    }

    /// Atomically register a budget only if no budget exists for the scope.
    ///
    /// Uses the DashMap Entry API to eliminate the TOCTOU race between
    /// `has_budget()` and `register_budget()`. Returns `true` if the budget
    /// was inserted, or `false` if a budget already existed for the scope.
    pub fn try_register_budget(&self, budget: Budget) -> bool {
        let key = budget.scope.to_key();
        match self.budgets.entry(key.clone()) {
            dashmap::mapref::entry::Entry::Occupied(_) => false,
            dashmap::mapref::entry::Entry::Vacant(e) => {
                debug!("Registering budget: {} ({})", budget.name, key);
                e.insert(budget);
                self.alert_states.insert(key, AlertState::default());
                true
            }
        }
    }

    /// Unregister a budget
    pub fn unregister_budget(&self, scope: &BudgetScope) {
        let key = scope.to_key();
        debug!("Unregistering budget: {}", key);
        self.budgets.remove(&key);
        self.alert_states.remove(&key);
    }

    /// Record spending against a scope
    ///
    /// Returns the updated budget status and whether alerts should be triggered.
    pub fn record_spend(&self, scope: &BudgetScope, amount: f64) -> Option<SpendResult> {
        let key = scope.to_key();

        self.budgets.get_mut(&key).map(|mut budget| {
            let previous_status = budget.status();
            budget.record_spend(amount);
            let new_status = budget.status();

            debug!(
                "Recorded spend ${:.4} for {}: ${:.2} / ${:.2} ({})",
                amount, key, budget.current_spend, budget.max_budget, new_status
            );

            // Determine if we need to trigger alerts
            let should_alert_soft_limit = new_status == BudgetStatus::Warning
                && previous_status == BudgetStatus::Ok
                && !self.has_soft_limit_alert(&key);

            let should_alert_exceeded = new_status == BudgetStatus::Exceeded
                && previous_status != BudgetStatus::Exceeded
                && !self.has_exceeded_alert(&key);

            // Update alert state
            if should_alert_soft_limit {
                self.mark_soft_limit_alerted(&key);
            }
            if should_alert_exceeded {
                self.mark_exceeded_alerted(&key);
            }

            SpendResult {
                budget_id: budget.id.clone(),
                scope: budget.scope.clone(),
                previous_status,
                new_status,
                current_spend: budget.current_spend,
                max_budget: budget.max_budget,
                remaining: budget.remaining(),
                should_alert_soft_limit,
                should_alert_exceeded,
            }
        })
    }

    /// Check budget status for a scope
    pub fn check_budget(&self, scope: &BudgetScope) -> BudgetCheckResult {
        let key = scope.to_key();

        match self.budgets.get(&key) {
            Some(budget) => BudgetCheckResult::from_budget(&budget, 0.0),
            None => BudgetCheckResult::no_budget(),
        }
    }

    /// Check if a spend amount would be allowed
    pub fn check_spend(&self, scope: &BudgetScope, amount: f64) -> BudgetCheckResult {
        let key = scope.to_key();

        match self.budgets.get(&key) {
            Some(budget) => BudgetCheckResult::from_budget(&budget, amount),
            None => BudgetCheckResult::no_budget(),
        }
    }

    /// Get remaining budget for a scope
    pub fn get_remaining(&self, scope: &BudgetScope) -> f64 {
        let key = scope.to_key();

        match self.budgets.get(&key) {
            Some(budget) => budget.remaining(),
            None => f64::INFINITY,
        }
    }

    /// Get current spend for a scope
    pub fn get_current_spend(&self, scope: &BudgetScope) -> f64 {
        let key = scope.to_key();

        match self.budgets.get(&key) {
            Some(budget) => budget.current_spend,
            None => 0.0,
        }
    }

    /// Get a budget by scope
    pub fn get_budget(&self, scope: &BudgetScope) -> Option<Budget> {
        let key = scope.to_key();
        self.budgets.get(&key).map(|b| b.clone())
    }

    /// Get all budgets
    pub fn get_all_budgets(&self) -> Vec<Budget> {
        self.budgets
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Reset budgets based on their reset period
    ///
    /// Returns a list of budget IDs that were reset.
    pub fn reset_budgets(&self) -> Vec<String> {
        let mut reset_ids = Vec::new();

        for mut entry in self.budgets.iter_mut() {
            let budget = entry.value_mut();
            if budget.should_reset() {
                info!(
                    "Resetting budget '{}' ({}) - previous spend: ${:.2}",
                    budget.name,
                    budget.scope.to_key(),
                    budget.current_spend
                );
                budget.reset();
                reset_ids.push(budget.id.clone());

                // Reset alert state for this budget
                let key = budget.scope.to_key();
                if let Some(mut state) = self.alert_states.get_mut(&key) {
                    *state = AlertState {
                        soft_limit_alerted: false,
                        exceeded_alerted: false,
                    };
                }
            }
        }

        reset_ids
    }

    /// Force reset a specific budget
    pub fn reset_budget(&self, scope: &BudgetScope) -> bool {
        let key = scope.to_key();

        if let Some(mut budget) = self.budgets.get_mut(&key) {
            info!(
                "Force resetting budget '{}' ({}) - previous spend: ${:.2}",
                budget.name, key, budget.current_spend
            );
            budget.reset();

            // Reset alert state
            if let Some(mut state) = self.alert_states.get_mut(&key) {
                *state = AlertState {
                    soft_limit_alerted: false,
                    exceeded_alerted: false,
                };
            }

            true
        } else {
            warn!("Attempted to reset non-existent budget: {}", key);
            false
        }
    }

    /// Get the number of tracked budgets
    pub fn budget_count(&self) -> usize {
        self.budgets.len()
    }

    /// Check if a budget exists for a scope
    pub fn has_budget(&self, scope: &BudgetScope) -> bool {
        self.budgets.contains_key(&scope.to_key())
    }

    /// Update a budget's configuration (max_budget, soft_limit, etc.)
    pub fn update_budget<F>(&self, scope: &BudgetScope, update_fn: F) -> bool
    where
        F: FnOnce(&mut Budget),
    {
        let key = scope.to_key();
        if let Some(mut budget) = self.budgets.get_mut(&key) {
            update_fn(&mut budget);
            budget.updated_at = chrono::Utc::now();
            true
        } else {
            false
        }
    }

    /// Get budgets that are over their soft limit
    pub fn get_warning_budgets(&self) -> Vec<Budget> {
        self.budgets
            .iter()
            .filter(|entry| entry.value().status() == BudgetStatus::Warning)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get budgets that are exceeded
    pub fn get_exceeded_budgets(&self) -> Vec<Budget> {
        self.budgets
            .iter()
            .filter(|entry| entry.value().status() == BudgetStatus::Exceeded)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get budgets by scope type
    pub fn get_budgets_by_type(&self, scope_type: &str) -> Vec<Budget> {
        self.budgets
            .iter()
            .filter(|entry| {
                let key = entry.key();
                key.starts_with(&format!("{}:", scope_type))
                    || (scope_type == "global" && key == "global")
            })
            .map(|entry| entry.value().clone())
            .collect()
    }

    // Internal helper methods for alert state management

    fn has_soft_limit_alert(&self, key: &str) -> bool {
        self.alert_states
            .get(key)
            .map(|state| state.soft_limit_alerted)
            .unwrap_or(false)
    }

    fn has_exceeded_alert(&self, key: &str) -> bool {
        self.alert_states
            .get(key)
            .map(|state| state.exceeded_alerted)
            .unwrap_or(false)
    }

    fn mark_soft_limit_alerted(&self, key: &str) {
        if let Some(mut state) = self.alert_states.get_mut(key) {
            state.soft_limit_alerted = true;
        }
    }

    fn mark_exceeded_alerted(&self, key: &str) {
        if let Some(mut state) = self.alert_states.get_mut(key) {
            state.exceeded_alerted = true;
        }
    }
}

/// Result of recording a spend operation
#[derive(Debug, Clone)]
pub struct SpendResult {
    /// Budget ID
    pub budget_id: String,
    /// Budget scope
    pub scope: BudgetScope,
    /// Status before the spend
    pub previous_status: BudgetStatus,
    /// Status after the spend
    pub new_status: BudgetStatus,
    /// Current total spend
    pub current_spend: f64,
    /// Maximum budget
    pub max_budget: f64,
    /// Remaining budget
    pub remaining: f64,
    /// Whether soft limit alert should be triggered
    pub should_alert_soft_limit: bool,
    /// Whether exceeded alert should be triggered
    pub should_alert_exceeded: bool,
}

impl SpendResult {
    /// Check if any alert should be triggered
    pub fn should_alert(&self) -> bool {
        self.should_alert_soft_limit || self.should_alert_exceeded
    }

    /// Check if the status changed
    pub fn status_changed(&self) -> bool {
        self.previous_status != self.new_status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::budget::types::ResetPeriod;

    fn create_test_budget(id: &str, scope: BudgetScope, max_budget: f64) -> Budget {
        Budget::new(id, format!("Test Budget {}", id), scope, max_budget)
    }

    #[test]
    fn test_tracker_creation() {
        let tracker = BudgetTracker::new();
        assert_eq!(tracker.budget_count(), 0);
    }

    #[test]
    fn test_tracker_with_capacity() {
        let tracker = BudgetTracker::with_capacity(100);
        assert_eq!(tracker.budget_count(), 0);
    }

    #[test]
    fn test_register_and_get_budget() {
        let tracker = BudgetTracker::new();
        let budget = create_test_budget("test-1", BudgetScope::Global, 100.0);

        tracker.register_budget(budget.clone());

        assert!(tracker.has_budget(&BudgetScope::Global));
        let retrieved = tracker.get_budget(&BudgetScope::Global).unwrap();
        assert_eq!(retrieved.id, "test-1");
        assert_eq!(retrieved.max_budget, 100.0);
    }

    #[test]
    fn test_unregister_budget() {
        let tracker = BudgetTracker::new();
        let budget = create_test_budget("test-1", BudgetScope::Global, 100.0);

        tracker.register_budget(budget);
        assert!(tracker.has_budget(&BudgetScope::Global));

        tracker.unregister_budget(&BudgetScope::Global);
        assert!(!tracker.has_budget(&BudgetScope::Global));
    }

    #[test]
    fn test_record_spend() {
        let tracker = BudgetTracker::new();
        let budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        tracker.register_budget(budget);

        let result = tracker.record_spend(&BudgetScope::Global, 25.0).unwrap();

        assert_eq!(result.current_spend, 25.0);
        assert_eq!(result.remaining, 75.0);
        assert_eq!(result.new_status, BudgetStatus::Ok);
    }

    #[test]
    fn test_record_spend_triggers_warning() {
        let tracker = BudgetTracker::new();
        let budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        tracker.register_budget(budget);

        // First spend to 79% - should still be OK
        let result1 = tracker.record_spend(&BudgetScope::Global, 79.0).unwrap();
        assert_eq!(result1.new_status, BudgetStatus::Ok);
        assert!(!result1.should_alert_soft_limit);

        // Second spend pushes to 80% - should trigger warning
        let result2 = tracker.record_spend(&BudgetScope::Global, 1.0).unwrap();
        assert_eq!(result2.new_status, BudgetStatus::Warning);
        assert!(result2.should_alert_soft_limit);
    }

    #[test]
    fn test_record_spend_triggers_exceeded() {
        let tracker = BudgetTracker::new();
        let budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        tracker.register_budget(budget);

        let result = tracker.record_spend(&BudgetScope::Global, 100.0).unwrap();

        assert_eq!(result.new_status, BudgetStatus::Exceeded);
        assert!(result.should_alert_exceeded);
    }

    #[test]
    fn test_record_spend_no_duplicate_alerts() {
        let tracker = BudgetTracker::new();
        let budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        tracker.register_budget(budget);

        // First spend triggers exceeded
        let result1 = tracker.record_spend(&BudgetScope::Global, 100.0).unwrap();
        assert!(result1.should_alert_exceeded);

        // Second spend should NOT trigger alert again
        let result2 = tracker.record_spend(&BudgetScope::Global, 10.0).unwrap();
        assert!(!result2.should_alert_exceeded);
    }

    #[test]
    fn test_check_budget() {
        let tracker = BudgetTracker::new();
        let budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        tracker.register_budget(budget);

        let result = tracker.check_budget(&BudgetScope::Global);

        assert!(result.allowed);
        assert_eq!(result.status, BudgetStatus::Ok);
        assert_eq!(result.max_budget, 100.0);
    }

    #[test]
    fn test_check_budget_no_budget() {
        let tracker = BudgetTracker::new();

        let result = tracker.check_budget(&BudgetScope::User("unknown".to_string()));

        assert!(result.allowed);
        assert!(result.max_budget.is_infinite());
    }

    #[test]
    fn test_check_spend() {
        let tracker = BudgetTracker::new();
        let mut budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        budget.current_spend = 90.0;
        tracker.register_budget(budget);

        let result_ok = tracker.check_spend(&BudgetScope::Global, 10.0);
        assert!(result_ok.allowed);

        let result_exceed = tracker.check_spend(&BudgetScope::Global, 11.0);
        assert!(!result_exceed.allowed);
    }

    #[test]
    fn test_get_remaining() {
        let tracker = BudgetTracker::new();
        let mut budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        budget.current_spend = 30.0;
        tracker.register_budget(budget);

        assert_eq!(tracker.get_remaining(&BudgetScope::Global), 70.0);
    }

    #[test]
    fn test_get_remaining_no_budget() {
        let tracker = BudgetTracker::new();

        assert!(tracker.get_remaining(&BudgetScope::Global).is_infinite());
    }

    #[test]
    fn test_get_current_spend() {
        let tracker = BudgetTracker::new();
        let mut budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        budget.current_spend = 45.0;
        tracker.register_budget(budget);

        assert_eq!(tracker.get_current_spend(&BudgetScope::Global), 45.0);
    }

    #[test]
    fn test_get_all_budgets() {
        let tracker = BudgetTracker::new();
        tracker.register_budget(create_test_budget("b1", BudgetScope::Global, 100.0));
        tracker.register_budget(create_test_budget(
            "b2",
            BudgetScope::User("user-1".to_string()),
            50.0,
        ));

        let budgets = tracker.get_all_budgets();
        assert_eq!(budgets.len(), 2);
    }

    #[test]
    fn test_reset_budget() {
        let tracker = BudgetTracker::new();
        let mut budget = create_test_budget("test-1", BudgetScope::Global, 100.0);
        budget.current_spend = 75.0;
        tracker.register_budget(budget);

        assert!(tracker.reset_budget(&BudgetScope::Global));
        assert_eq!(tracker.get_current_spend(&BudgetScope::Global), 0.0);
    }

    #[test]
    fn test_reset_budget_not_found() {
        let tracker = BudgetTracker::new();

        assert!(!tracker.reset_budget(&BudgetScope::Global));
    }

    #[test]
    fn test_update_budget() {
        let tracker = BudgetTracker::new();
        tracker.register_budget(create_test_budget("test-1", BudgetScope::Global, 100.0));

        let updated = tracker.update_budget(&BudgetScope::Global, |budget| {
            budget.max_budget = 200.0;
            budget.soft_limit = 160.0;
        });

        assert!(updated);
        let budget = tracker.get_budget(&BudgetScope::Global).unwrap();
        assert_eq!(budget.max_budget, 200.0);
        assert_eq!(budget.soft_limit, 160.0);
    }

    #[test]
    fn test_get_warning_budgets() {
        let tracker = BudgetTracker::new();

        let mut warning_budget = create_test_budget("warn", BudgetScope::Global, 100.0);
        warning_budget.current_spend = 85.0; // Over soft limit

        let ok_budget = create_test_budget("ok", BudgetScope::User("user-1".to_string()), 100.0);

        tracker.register_budget(warning_budget);
        tracker.register_budget(ok_budget);

        let warning_budgets = tracker.get_warning_budgets();
        assert_eq!(warning_budgets.len(), 1);
        assert_eq!(warning_budgets[0].id, "warn");
    }

    #[test]
    fn test_get_exceeded_budgets() {
        let tracker = BudgetTracker::new();

        let mut exceeded_budget = create_test_budget("exceeded", BudgetScope::Global, 100.0);
        exceeded_budget.current_spend = 150.0;

        let ok_budget = create_test_budget("ok", BudgetScope::User("user-1".to_string()), 100.0);

        tracker.register_budget(exceeded_budget);
        tracker.register_budget(ok_budget);

        let exceeded_budgets = tracker.get_exceeded_budgets();
        assert_eq!(exceeded_budgets.len(), 1);
        assert_eq!(exceeded_budgets[0].id, "exceeded");
    }

    #[test]
    fn test_get_budgets_by_type() {
        let tracker = BudgetTracker::new();

        tracker.register_budget(create_test_budget("global", BudgetScope::Global, 100.0));
        tracker.register_budget(create_test_budget(
            "user1",
            BudgetScope::User("user-1".to_string()),
            50.0,
        ));
        tracker.register_budget(create_test_budget(
            "user2",
            BudgetScope::User("user-2".to_string()),
            50.0,
        ));
        tracker.register_budget(create_test_budget(
            "team1",
            BudgetScope::Team("team-1".to_string()),
            75.0,
        ));

        let user_budgets = tracker.get_budgets_by_type("user");
        assert_eq!(user_budgets.len(), 2);

        let global_budgets = tracker.get_budgets_by_type("global");
        assert_eq!(global_budgets.len(), 1);

        let team_budgets = tracker.get_budgets_by_type("team");
        assert_eq!(team_budgets.len(), 1);
    }

    #[test]
    fn test_spend_result_helpers() {
        let result = SpendResult {
            budget_id: "test".to_string(),
            scope: BudgetScope::Global,
            previous_status: BudgetStatus::Ok,
            new_status: BudgetStatus::Warning,
            current_spend: 80.0,
            max_budget: 100.0,
            remaining: 20.0,
            should_alert_soft_limit: true,
            should_alert_exceeded: false,
        };

        assert!(result.should_alert());
        assert!(result.status_changed());
    }

    #[test]
    fn test_reset_budgets_by_period() {
        let tracker = BudgetTracker::new();

        // Create a budget that should reset (using Never period so it won't auto-reset)
        let mut budget = create_test_budget("test", BudgetScope::Global, 100.0);
        budget.reset_period = ResetPeriod::Never;
        budget.current_spend = 50.0;
        tracker.register_budget(budget);

        // Should not reset because period is Never
        let reset_ids = tracker.reset_budgets();
        assert!(reset_ids.is_empty());
        assert_eq!(tracker.get_current_spend(&BudgetScope::Global), 50.0);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(BudgetTracker::new());
        tracker.register_budget(create_test_budget("test", BudgetScope::Global, 1000.0));

        let mut handles = vec![];

        // Spawn multiple threads to record spend
        for _ in 0..10 {
            let tracker_clone = Arc::clone(&tracker);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    tracker_clone.record_spend(&BudgetScope::Global, 1.0);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Should have recorded 1000 spends of 1.0 each
        assert_eq!(tracker.get_current_spend(&BudgetScope::Global), 1000.0);
    }
}
