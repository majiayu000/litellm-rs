//! Budget-aware routing
//!
//! This module provides budget-aware routing capabilities that filter out
//! providers and models that have exceeded their budget limits.

use crate::core::budget::{BudgetStatus, UnifiedBudgetLimits};
use std::sync::Arc;
use tracing::{debug, warn};

/// Budget-aware router wrapper
///
/// Wraps routing decisions with budget checking to skip providers
/// that have exceeded their budget limits.
#[derive(Clone)]
pub struct BudgetAwareRouter {
    /// Budget limits manager
    budget_limits: Arc<UnifiedBudgetLimits>,
    /// Whether to log warnings when approaching limits
    log_warnings: bool,
    /// Warning threshold percentage (0.0 to 1.0)
    warning_threshold: f64,
}

impl BudgetAwareRouter {
    /// Create a new budget-aware router
    pub fn new(budget_limits: Arc<UnifiedBudgetLimits>) -> Self {
        Self {
            budget_limits,
            log_warnings: true,
            warning_threshold: 0.8,
        }
    }

    /// Set whether to log warnings
    pub fn with_warnings(mut self, log_warnings: bool) -> Self {
        self.log_warnings = log_warnings;
        self
    }

    /// Set warning threshold
    pub fn with_warning_threshold(mut self, threshold: f64) -> Self {
        self.warning_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Get the budget limits manager
    pub fn budget_limits(&self) -> &UnifiedBudgetLimits {
        &self.budget_limits
    }

    /// Filter providers based on budget availability
    ///
    /// Returns providers that have not exceeded their budget limits.
    /// Providers without configured budgets are always included.
    pub fn filter_available_providers(&self, providers: Vec<String>) -> Vec<String> {
        let available: Vec<String> = providers
            .into_iter()
            .filter(|provider| {
                let status = self.budget_limits.providers.check_provider_budget(provider);
                let is_available = status != BudgetStatus::Exceeded;

                if !is_available {
                    debug!("Provider '{}' filtered out: budget exceeded", provider);
                } else if self.log_warnings
                    && status == BudgetStatus::Warning
                    && let Some(usage) = self.budget_limits.providers.get_provider_usage(provider)
                {
                    warn!(
                        "Provider '{}' approaching budget limit: ${:.2} / ${:.2} ({:.1}%)",
                        provider, usage.current_spend, usage.max_budget, usage.usage_percentage
                    );
                }

                is_available
            })
            .collect();

        if available.is_empty() {
            debug!("All providers have exceeded budget limits");
        }

        available
    }

    /// Filter models based on budget availability
    pub fn filter_available_models(&self, models: Vec<String>) -> Vec<String> {
        models
            .into_iter()
            .filter(|model| {
                let status = self.budget_limits.models.check_model_budget(model);
                let is_available = status != BudgetStatus::Exceeded;

                if !is_available {
                    debug!("Model '{}' filtered out: budget exceeded", model);
                }

                is_available
            })
            .collect()
    }

    /// Check if a specific provider is available
    pub fn is_provider_available(&self, provider: &str) -> bool {
        self.budget_limits.is_provider_available(provider)
    }

    /// Check if a specific model is available
    pub fn is_model_available(&self, model: &str) -> bool {
        self.budget_limits.is_model_available(model)
    }

    /// Check if a request can be made with the given provider and model
    pub fn can_make_request(
        &self,
        provider: &str,
        model: &str,
        estimated_cost: f64,
    ) -> RequestBudgetCheck {
        let provider_check = self.check_provider(provider, estimated_cost);
        let model_check = self.check_model(model, estimated_cost);

        RequestBudgetCheck {
            allowed: provider_check.allowed && model_check.allowed,
            provider_status: provider_check.status,
            model_status: model_check.status,
            provider_remaining: provider_check.remaining,
            model_remaining: model_check.remaining,
            reason: if !provider_check.allowed {
                Some(format!("Provider '{}' has exceeded budget", provider))
            } else if !model_check.allowed {
                Some(format!("Model '{}' has exceeded budget", model))
            } else {
                None
            },
        }
    }

    /// Check provider budget
    fn check_provider(&self, provider: &str, estimated_cost: f64) -> BudgetCheckResult {
        let can_spend = self
            .budget_limits
            .providers
            .can_provider_spend(provider, estimated_cost);
        let status = self.budget_limits.providers.check_provider_budget(provider);
        let remaining = self
            .budget_limits
            .providers
            .get_provider_usage(provider)
            .map(|u| u.remaining)
            .unwrap_or(f64::INFINITY);

        BudgetCheckResult {
            allowed: can_spend,
            status,
            remaining,
        }
    }

    /// Check model budget
    fn check_model(&self, model: &str, estimated_cost: f64) -> BudgetCheckResult {
        let can_spend = self
            .budget_limits
            .models
            .can_model_spend(model, estimated_cost);
        let status = self.budget_limits.models.check_model_budget(model);
        let remaining = self
            .budget_limits
            .models
            .get_model_usage(model)
            .map(|u| u.remaining)
            .unwrap_or(f64::INFINITY);

        BudgetCheckResult {
            allowed: can_spend,
            status,
            remaining,
        }
    }

    /// Record spend after a request completes
    pub fn record_spend(&self, provider: &str, model: &str, cost: f64) {
        self.budget_limits.record_spend(provider, model, cost);
    }

    /// Get a fallback provider when the primary is over budget
    ///
    /// Returns the first available provider from the fallback list,
    /// or None if all providers have exceeded their budgets.
    pub fn get_fallback_provider(&self, fallbacks: &[String]) -> Option<String> {
        for provider in fallbacks {
            if self.is_provider_available(provider) {
                debug!("Using fallback provider: {}", provider);
                return Some(provider.clone());
            }
        }
        None
    }

    /// Get providers sorted by remaining budget (highest first)
    pub fn get_providers_by_remaining_budget(&self, providers: Vec<String>) -> Vec<String> {
        let mut provider_budgets: Vec<(String, f64)> = providers
            .into_iter()
            .filter_map(|p| {
                let remaining = self
                    .budget_limits
                    .providers
                    .get_provider_usage(&p)
                    .map(|u| u.remaining)
                    .unwrap_or(f64::INFINITY);

                // Only include providers that aren't exceeded
                if self.is_provider_available(&p) {
                    Some((p, remaining))
                } else {
                    None
                }
            })
            .collect();

        // Sort by remaining budget (highest first)
        provider_budgets.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        provider_budgets.into_iter().map(|(p, _)| p).collect()
    }
}

/// Internal budget check result
struct BudgetCheckResult {
    allowed: bool,
    status: BudgetStatus,
    remaining: f64,
}

/// Result of checking if a request is within budget
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequestBudgetCheck {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Provider budget status
    pub provider_status: BudgetStatus,
    /// Model budget status
    pub model_status: BudgetStatus,
    /// Remaining provider budget
    pub provider_remaining: f64,
    /// Remaining model budget
    pub model_remaining: f64,
    /// Reason if not allowed
    pub reason: Option<String>,
}

/// Extension trait for adding budget awareness to routers
pub trait BudgetAwareRouting {
    /// Filter providers based on budget availability
    fn filter_by_budget(
        &self,
        providers: Vec<String>,
        budget_router: &BudgetAwareRouter,
    ) -> Vec<String>;
}

impl<T> BudgetAwareRouting for T {
    fn filter_by_budget(
        &self,
        providers: Vec<String>,
        budget_router: &BudgetAwareRouter,
    ) -> Vec<String> {
        budget_router.filter_available_providers(providers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};

    fn create_test_router() -> BudgetAwareRouter {
        let limits = Arc::new(UnifiedBudgetLimits::new());
        BudgetAwareRouter::new(limits)
    }

    #[test]
    fn test_budget_aware_router_creation() {
        let router = create_test_router();
        assert!(router.log_warnings);
        assert!((router.warning_threshold - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_filter_available_providers_no_limits() {
        let router = create_test_router();
        let providers = vec!["openai".to_string(), "anthropic".to_string()];

        let available = router.filter_available_providers(providers.clone());
        assert_eq!(available, providers);
    }

    #[test]
    fn test_filter_available_providers_with_exceeded() {
        let limits = Arc::new(UnifiedBudgetLimits::new());
        limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        limits.providers.set_provider_limit(
            "anthropic",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );

        // Exceed openai budget
        limits.providers.record_provider_spend("openai", 150.0);

        let router = BudgetAwareRouter::new(limits);
        let providers = vec!["openai".to_string(), "anthropic".to_string()];

        let available = router.filter_available_providers(providers);
        assert_eq!(available.len(), 1);
        assert_eq!(available[0], "anthropic");
    }

    #[test]
    fn test_is_provider_available() {
        let limits = Arc::new(UnifiedBudgetLimits::new());
        limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );

        let router = BudgetAwareRouter::new(limits.clone());

        assert!(router.is_provider_available("openai"));
        assert!(router.is_provider_available("unknown")); // No limit = available

        limits.providers.record_provider_spend("openai", 150.0);
        assert!(!router.is_provider_available("openai"));
    }

    #[test]
    fn test_can_make_request() {
        let limits = Arc::new(UnifiedBudgetLimits::new());
        limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        limits
            .models
            .set_model_limit("gpt-4", ModelLimitConfig::new(50.0, ResetPeriod::Monthly));

        let router = BudgetAwareRouter::new(limits.clone());

        // Should allow
        let check = router.can_make_request("openai", "gpt-4", 10.0);
        assert!(check.allowed);
        assert!(check.reason.is_none());

        // Exceed model budget
        limits.models.record_model_spend("gpt-4", 60.0);

        let check = router.can_make_request("openai", "gpt-4", 10.0);
        assert!(!check.allowed);
        assert!(check.reason.is_some());
        assert!(check.reason.unwrap().contains("gpt-4"));
    }

    #[test]
    fn test_record_spend() {
        let limits = Arc::new(UnifiedBudgetLimits::new());
        limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        limits
            .models
            .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));

        let router = BudgetAwareRouter::new(limits.clone());
        router.record_spend("openai", "gpt-4", 25.0);

        let provider_usage = limits.providers.get_provider_usage("openai").unwrap();
        let model_usage = limits.models.get_model_usage("gpt-4").unwrap();

        assert_eq!(provider_usage.current_spend, 25.0);
        assert_eq!(model_usage.current_spend, 25.0);
    }

    #[test]
    fn test_get_fallback_provider() {
        let limits = Arc::new(UnifiedBudgetLimits::new());
        limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        limits.providers.set_provider_limit(
            "anthropic",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        limits.providers.set_provider_limit(
            "google",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );

        // Exceed openai and anthropic
        limits.providers.record_provider_spend("openai", 150.0);
        limits.providers.record_provider_spend("anthropic", 150.0);

        let router = BudgetAwareRouter::new(limits);

        let fallbacks = vec![
            "openai".to_string(),
            "anthropic".to_string(),
            "google".to_string(),
        ];

        let fallback = router.get_fallback_provider(&fallbacks);
        assert_eq!(fallback, Some("google".to_string()));
    }

    #[test]
    fn test_get_providers_by_remaining_budget() {
        let limits = Arc::new(UnifiedBudgetLimits::new());
        limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        limits.providers.set_provider_limit(
            "anthropic",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        limits.providers.set_provider_limit(
            "google",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );

        // Different spend amounts
        limits.providers.record_provider_spend("openai", 80.0); // 20 remaining
        limits.providers.record_provider_spend("anthropic", 30.0); // 70 remaining
        limits.providers.record_provider_spend("google", 50.0); // 50 remaining

        let router = BudgetAwareRouter::new(limits);

        let providers = vec![
            "openai".to_string(),
            "anthropic".to_string(),
            "google".to_string(),
        ];

        let sorted = router.get_providers_by_remaining_budget(providers);

        // Should be sorted by remaining budget (highest first)
        assert_eq!(sorted[0], "anthropic"); // 70 remaining
        assert_eq!(sorted[1], "google"); // 50 remaining
        assert_eq!(sorted[2], "openai"); // 20 remaining
    }

    #[test]
    fn test_filter_available_models() {
        let limits = Arc::new(UnifiedBudgetLimits::new());
        limits
            .models
            .set_model_limit("gpt-4", ModelLimitConfig::new(100.0, ResetPeriod::Monthly));
        limits.models.set_model_limit(
            "gpt-3.5-turbo",
            ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
        );

        // Exceed gpt-4 budget
        limits.models.record_model_spend("gpt-4", 150.0);

        let router = BudgetAwareRouter::new(limits);

        let models = vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()];
        let available = router.filter_available_models(models);

        assert_eq!(available.len(), 1);
        assert_eq!(available[0], "gpt-3.5-turbo");
    }
}
