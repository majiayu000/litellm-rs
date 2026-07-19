//! Budget Management System
//!
//! This module provides comprehensive budget management for the LiteLLM-RS gateway,
//! including budget tracking, alerting, and middleware for request interception.
//!
//! ## Features
//!
//! - **Budget Types**: Flexible budget scopes (user, team, API key, provider, model, global)
//! - **Tracking**: Lock-free concurrent budget tracking using DashMap
//! - **Management**: CRUD operations for budget configuration
//! - **Middleware**: Actix-web middleware for request budget checking
//! - **Alerts**: Webhook-based alerting for budget thresholds
//!
//! ## Usage
//!
//! ```rust,ignore
//! use litellm_rs::core::budget::{BudgetManager, BudgetScope, BudgetConfig};
//!
//! // Create a budget manager
//! let manager = BudgetManager::new();
//!
//! // Create a budget for a user
//! let config = BudgetConfig::new("User Budget", 100.0);
//! let budget = manager.create_budget(
//!     BudgetScope::User("user-123".to_string()),
//!     config
//! ).await?;
//!
//! // Record spend
//! manager.record_spend(&BudgetScope::User("user-123".to_string()), 5.50).await;
//!
//! // Check remaining budget
//! let remaining = manager.get_remaining(&BudgetScope::User("user-123".to_string()));
//! ```
//!
//! ## Middleware Integration
//!
//! ```rust,ignore
//! use litellm_rs::core::budget::{BudgetCheckMiddleware, BudgetManager};
//! use actix_web::{App, web};
//! use std::sync::Arc;
//!
//! let manager = Arc::new(BudgetManager::new());
//! let middleware = BudgetCheckMiddleware::new(Arc::clone(&manager));
//!
//! App::new()
//!     .wrap(middleware)
//!     .app_data(web::Data::new(manager))
//! ```
//!
//! ## Alert Configuration
//!
//! ```rust,ignore
//! use litellm_rs::core::budget::{BudgetAlertManager, WebhookConfig, AlertSeverity};
//!
//! let alert_manager = BudgetAlertManager::new();
//!
//! // Add a webhook for critical alerts
//! alert_manager.add_webhook(WebhookConfig {
//!     url: "https://example.com/webhook".to_string(),
//!     severities: vec![AlertSeverity::Critical],
//!     ..Default::default()
//! }).await?;
//! ```

mod alerts;
mod config;
mod manager;
#[cfg(feature = "gateway")]
mod middleware;
mod provider_limits;
mod provider_reservations;
mod tracker;
mod types;

#[cfg(test)]
mod manager_tests;
#[cfg(test)]
mod provider_reservation_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tracker_reservation_tests;
#[cfg(test)]
mod tracker_tests;
#[cfg(test)]
mod types_tests;

// Re-export public types
pub use alerts::{AlertConfig, AlertStats, BudgetAlertManager, BudgetWebhookError, WebhookConfig};
pub use config::{
    BudgetConfig, BudgetLimitKind, BudgetLimitSnapshot, BudgetPersistenceEvent,
    BudgetPersistenceSender, ModelLimitConfig, ProviderLimitConfig,
};
pub use manager::{BudgetManager, BudgetManagerConfig, BudgetSummary};
#[cfg(feature = "gateway")]
pub use middleware::{
    BudgetCheckMiddleware, BudgetCheckMiddlewareService, BudgetMiddleware, BudgetMiddlewareService,
    BudgetRecorder, BudgetRecorderExt,
};
pub use provider_limits::{ModelBudgetManager, ProviderBudgetManager, UnifiedBudgetLimits};
pub use provider_reservations::{
    ModelBudgetReservation, ProviderBudgetReservation, UnifiedBudgetReservation,
};
pub use tracker::{BudgetReservation, BudgetReservationError, BudgetTracker, SpendResult};
pub use types::{
    AlertSeverity, Budget, BudgetAlert, BudgetAlertType, BudgetCheckResult, BudgetScope,
    BudgetStatus, Currency, ModelBudget, ModelUsageStats, ProviderBudget, ProviderUsageStats,
    ResetPeriod,
};

const BUDGET_AMOUNT_SCALE: f64 = 1_000_000_000.0;

/// Fixed-point amount used for budget authorization math.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BudgetAmount(i128);

impl BudgetAmount {
    pub fn zero() -> Self {
        Self(0)
    }

    pub fn from_f64(amount: f64) -> Result<Self, BudgetAmountError> {
        if !amount.is_finite() {
            return Err(BudgetAmountError::NonFinite);
        }
        if amount < 0.0 {
            return Err(BudgetAmountError::Negative);
        }

        let scaled = (amount * BUDGET_AMOUNT_SCALE).round();
        if !scaled.is_finite() || scaled > i128::MAX as f64 {
            return Err(BudgetAmountError::Overflow);
        }

        Ok(Self(scaled as i128))
    }

    pub fn as_f64(self) -> f64 {
        self.0 as f64 / BUDGET_AMOUNT_SCALE
    }

    pub fn checked_add(self, other: Self) -> Result<Self, BudgetAmountError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(BudgetAmountError::Overflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, BudgetAmountError> {
        if self < other {
            return Err(BudgetAmountError::Negative);
        }
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(BudgetAmountError::Negative)
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0).max(0))
    }
}

/// Invalid money value at a budget boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetAmountError {
    NonFinite,
    Negative,
    Overflow,
}

impl std::fmt::Display for BudgetAmountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite => write!(f, "budget amount must be finite"),
            Self::Negative => write!(f, "budget amount must not be negative"),
            Self::Overflow => write!(f, "budget amount is too large"),
        }
    }
}

impl std::error::Error for BudgetAmountError {}

pub(crate) fn budget_can_spend(
    current_spend: f64,
    max_budget: f64,
    enabled: bool,
    amount: f64,
) -> Result<bool, BudgetAmountError> {
    let amount = BudgetAmount::from_f64(amount)?;
    if !enabled {
        return Ok(true);
    }

    let current = BudgetAmount::from_f64(current_spend)?;
    let max = BudgetAmount::from_f64(max_budget)?;
    Ok(current.checked_add(amount)? <= max)
}

pub(crate) fn add_budget_spend(
    current_spend: f64,
    amount: f64,
) -> Result<BudgetAmount, BudgetAmountError> {
    let current = BudgetAmount::from_f64(current_spend)?;
    current.checked_add(BudgetAmount::from_f64(amount)?)
}

pub(crate) fn settle_budget_spend(
    current_spend: f64,
    reserved: BudgetAmount,
    actual: BudgetAmount,
    same_reset_epoch: bool,
) -> Result<BudgetAmount, BudgetAmountError> {
    let current_spend = BudgetAmount::from_f64(current_spend)?;
    let settled_base = if same_reset_epoch {
        current_spend.saturating_sub(reserved)
    } else {
        current_spend
    };
    settled_base.checked_add(actual)
}

pub(crate) fn release_budget_spend(
    current_spend: f64,
    reserved: BudgetAmount,
    same_reset_epoch: bool,
) -> Result<BudgetAmount, BudgetAmountError> {
    let current_spend = BudgetAmount::from_f64(current_spend)?;
    if same_reset_epoch {
        Ok(current_spend.saturating_sub(reserved))
    } else {
        Ok(current_spend)
    }
}

use std::sync::Arc;

/// Initialize a complete budget system with default configuration
///
/// Returns a tuple of (BudgetManager, BudgetAlertManager) that can be used
/// together for complete budget management with alerts.
pub fn init_budget_system() -> (Arc<BudgetManager>, Arc<BudgetAlertManager>) {
    let manager = Arc::new(BudgetManager::new());
    let alert_manager = Arc::new(BudgetAlertManager::new());

    (manager, alert_manager)
}

/// Initialize a budget system with custom configuration
pub fn init_budget_system_with_config(
    manager_config: BudgetManagerConfig,
    alert_config: AlertConfig,
) -> (Arc<BudgetManager>, Arc<BudgetAlertManager>) {
    let manager = Arc::new(BudgetManager::with_config(manager_config));
    let alert_manager = Arc::new(BudgetAlertManager::with_config(alert_config));

    (manager, alert_manager)
}

/// Global budget manager singleton (optional usage pattern)
static GLOBAL_BUDGET_MANAGER: std::sync::OnceLock<Arc<BudgetManager>> = std::sync::OnceLock::new();

/// Initialize the global budget manager
pub fn init_global_budget_manager(config: BudgetManagerConfig) {
    let manager = Arc::new(BudgetManager::with_config(config));
    let _ = GLOBAL_BUDGET_MANAGER.set(manager);
}

/// Get the global budget manager
pub fn get_global_budget_manager() -> Option<Arc<BudgetManager>> {
    GLOBAL_BUDGET_MANAGER.get().cloned()
}
