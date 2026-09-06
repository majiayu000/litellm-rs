//! Budget tracking implementation
//!
//! Provides lock-free concurrent budget tracking using DashMap for high-performance
//! budget monitoring in multi-threaded environments.

use super::types::{Budget, BudgetCheckResult, BudgetScope, BudgetStatus};
use super::{
    BudgetAmount, BudgetAmountError, add_budget_spend, budget_can_spend, release_budget_spend,
    settle_budget_spend,
};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
#[derive(Clone)]
pub struct BudgetTracker {
    budgets: Arc<DashMap<String, Budget>>,
    alert_states: Arc<DashMap<String, AlertState>>,
}
#[derive(Debug, Clone, Default)]
struct AlertState {
    soft_limit_alerted: bool,
    exceeded_alerted: bool,
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetTracker {
    pub fn new() -> Self {
        Self {
            budgets: Arc::new(DashMap::new()),
            alert_states: Arc::new(DashMap::new()),
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            budgets: Arc::new(DashMap::with_capacity(capacity)),
            alert_states: Arc::new(DashMap::with_capacity(capacity)),
        }
    }
    pub fn register_budget(&self, budget: Budget) {
        let key = budget.scope.to_key();
        debug!("Registering budget: {} ({})", budget.name, key);
        self.budgets.insert(key.clone(), budget);
        self.alert_states.insert(key, AlertState::default());
    }
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
    pub fn unregister_budget(&self, scope: &BudgetScope) {
        let key = scope.to_key();
        debug!("Unregistering budget: {}", key);
        self.budgets.remove(&key);
        self.alert_states.remove(&key);
    }
    pub fn record_spend(&self, scope: &BudgetScope, amount: f64) -> Option<SpendResult> {
        if BudgetAmount::from_f64(amount).is_err() {
            return None;
        }
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
    pub fn reserve_spend(
        &self,
        scope: &BudgetScope,
        max_amount: f64,
    ) -> Result<BudgetReservation, BudgetReservationError> {
        let reserved = BudgetAmount::from_f64(max_amount)?;
        let key = scope.to_key();

        let Some(mut budget) = self.budgets.get_mut(&key) else {
            return Ok(BudgetReservation::untracked(
                self.clone(),
                scope.clone(),
                reserved,
            ));
        };
        if !budget.enabled {
            return Ok(BudgetReservation {
                tracker: self.clone(),
                scope: scope.clone(),
                key: Some(key),
                reserved: BudgetAmount::zero(),
                reservation_reset_at: budget.last_reset_at,
                previous_status: budget.status(),
                settled: false,
            });
        }
        if !budget_can_spend(
            budget.current_spend,
            budget.max_budget,
            budget.enabled,
            max_amount,
        )? {
            return Err(BudgetReservationError::BudgetExceeded);
        }

        let previous_status = budget.status();
        let reservation_reset_at = budget.last_reset_at;
        budget.current_spend = add_budget_spend(budget.current_spend, max_amount)?.as_f64();
        budget.updated_at = chrono::Utc::now();
        Ok(BudgetReservation {
            tracker: self.clone(),
            scope: scope.clone(),
            key: Some(key),
            reserved,
            reservation_reset_at,
            previous_status,
            settled: false,
        })
    }
    pub fn check_budget(&self, scope: &BudgetScope) -> BudgetCheckResult {
        let key = scope.to_key();

        match self.budgets.get(&key) {
            Some(budget) => BudgetCheckResult::from_budget(&budget, 0.0),
            None => BudgetCheckResult::no_budget(),
        }
    }
    pub fn check_spend(&self, scope: &BudgetScope, amount: f64) -> BudgetCheckResult {
        let key = scope.to_key();

        match self.budgets.get(&key) {
            Some(budget) => BudgetCheckResult::from_budget(&budget, amount),
            None => BudgetCheckResult::no_budget(),
        }
    }
    pub fn get_remaining(&self, scope: &BudgetScope) -> f64 {
        let key = scope.to_key();

        match self.budgets.get(&key) {
            Some(budget) => budget.remaining(),
            None => f64::INFINITY,
        }
    }
    pub fn get_current_spend(&self, scope: &BudgetScope) -> f64 {
        let key = scope.to_key();

        match self.budgets.get(&key) {
            Some(budget) => budget.current_spend,
            None => 0.0,
        }
    }
    pub fn get_budget(&self, scope: &BudgetScope) -> Option<Budget> {
        let key = scope.to_key();
        self.budgets.get(&key).map(|b| b.clone())
    }
    pub fn get_all_budgets(&self) -> Vec<Budget> {
        self.budgets
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
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
    pub fn budget_count(&self) -> usize {
        self.budgets.len()
    }
    pub fn has_budget(&self, scope: &BudgetScope) -> bool {
        self.budgets.contains_key(&scope.to_key())
    }
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
    pub fn get_warning_budgets(&self) -> Vec<Budget> {
        self.budgets
            .iter()
            .filter(|entry| entry.value().status() == BudgetStatus::Warning)
            .map(|entry| entry.value().clone())
            .collect()
    }
    pub fn get_exceeded_budgets(&self) -> Vec<Budget> {
        self.budgets
            .iter()
            .filter(|entry| entry.value().status() == BudgetStatus::Exceeded)
            .map(|entry| entry.value().clone())
            .collect()
    }
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

    fn release_reserved(
        &self,
        key: &str,
        reserved: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) {
        if let Some(mut budget) = self.budgets.get_mut(key)
            && let Ok(current_spend) = release_budget_spend(
                budget.current_spend,
                reserved,
                budget.last_reset_at == reservation_reset_at,
            )
        {
            budget.current_spend = current_spend.as_f64();
            budget.updated_at = chrono::Utc::now();
        }
    }

    fn settle_reserved(
        &self,
        key: &str,
        reserved: BudgetAmount,
        actual: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
        reservation_previous_status: BudgetStatus,
    ) -> Result<Option<SpendResult>, BudgetReservationError> {
        let Some(mut budget) = self.budgets.get_mut(key) else {
            return Ok(None);
        };
        let same_reset_epoch = budget.last_reset_at == reservation_reset_at;
        let previous_status = if same_reset_epoch {
            reservation_previous_status
        } else {
            budget.status()
        };
        let current_spend =
            settle_budget_spend(budget.current_spend, reserved, actual, same_reset_epoch)?;
        budget.current_spend = current_spend.as_f64();
        budget.updated_at = chrono::Utc::now();
        let new_status = budget.status();

        let should_alert_soft_limit = new_status == BudgetStatus::Warning
            && previous_status == BudgetStatus::Ok
            && !self.has_soft_limit_alert(key);
        let should_alert_exceeded = new_status == BudgetStatus::Exceeded
            && previous_status != BudgetStatus::Exceeded
            && !self.has_exceeded_alert(key);

        if should_alert_soft_limit {
            self.mark_soft_limit_alerted(key);
        }
        if should_alert_exceeded {
            self.mark_exceeded_alerted(key);
        }

        Ok(Some(SpendResult {
            budget_id: budget.id.clone(),
            scope: budget.scope.clone(),
            previous_status,
            new_status,
            current_spend: budget.current_spend,
            max_budget: budget.max_budget,
            remaining: budget.remaining(),
            should_alert_soft_limit,
            should_alert_exceeded,
        }))
    }
}

pub struct BudgetReservation {
    tracker: BudgetTracker,
    scope: BudgetScope,
    key: Option<String>,
    reserved: BudgetAmount,
    reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    previous_status: BudgetStatus,
    settled: bool,
}

impl BudgetReservation {
    fn untracked(tracker: BudgetTracker, scope: BudgetScope, reserved: BudgetAmount) -> Self {
        Self {
            tracker,
            scope,
            key: None,
            reserved,
            reservation_reset_at: None,
            previous_status: BudgetStatus::Ok,
            settled: false,
        }
    }

    pub fn scope(&self) -> &BudgetScope {
        &self.scope
    }

    pub fn reserved_amount(&self) -> f64 {
        self.reserved.as_f64()
    }

    pub fn settle(
        mut self,
        actual_amount: f64,
    ) -> Result<Option<SpendResult>, BudgetReservationError> {
        let actual = BudgetAmount::from_f64(actual_amount)?;
        let result = self
            .key
            .as_deref()
            .map(|key| {
                self.tracker.settle_reserved(
                    key,
                    self.reserved,
                    actual,
                    self.reservation_reset_at,
                    self.previous_status,
                )
            })
            .transpose()?
            .flatten();
        self.settled = true;
        Ok(result)
    }

    pub fn cancel(mut self) {
        if let Some(key) = self.key.as_deref() {
            self.tracker
                .release_reserved(key, self.reserved, self.reservation_reset_at);
        }
        self.settled = true;
    }
}

impl Drop for BudgetReservation {
    fn drop(&mut self) {
        if !self.settled
            && let Some(key) = self.key.as_deref()
        {
            self.tracker
                .release_reserved(key, self.reserved, self.reservation_reset_at);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetReservationError {
    InvalidAmount(BudgetAmountError),
    BudgetExceeded,
    ProviderBudgetExceeded,
    ModelBudgetExceeded,
    ActualExceedsReservation,
    /// Shared Redis/backend was unreachable. Fail closed rather than overspend.
    BackendUnavailable,
}

impl From<BudgetAmountError> for BudgetReservationError {
    fn from(error: BudgetAmountError) -> Self {
        Self::InvalidAmount(error)
    }
}
#[derive(Debug, Clone)]
pub struct SpendResult {
    pub budget_id: String,
    pub scope: BudgetScope,
    pub previous_status: BudgetStatus,
    pub new_status: BudgetStatus,
    pub current_spend: f64,
    pub max_budget: f64,
    pub remaining: f64,
    pub should_alert_soft_limit: bool,
    pub should_alert_exceeded: bool,
}

impl SpendResult {
    pub fn should_alert(&self) -> bool {
        self.should_alert_soft_limit || self.should_alert_exceeded
    }
    pub fn status_changed(&self) -> bool {
        self.previous_status != self.new_status
    }
}
