//! Unified Cost Calculation Module
//!
//! This module provides a centralized cost calculation system for all providers,
//! eliminating code duplication and ensuring consistency across the codebase.
//!
//! ## Design Philosophy
//! Based on Python LiteLLM's successful pattern:
//! - Single source of truth for cost calculation logic
//! - Providers delegate to generic functions
//! - Centralized model pricing data
//! - Consistent cost structures across all providers

pub mod calculator;
pub mod types;
pub mod utils;

// Compatibility re-exports retained for the 0.6 migration window.
#[deprecated(
    note = "use core::cost::calculator::* directly or PricingService for runtime pricing; removal is no earlier than 0.7.0"
)]
pub use calculator::{
    CostCalculator, compare_model_costs, estimate_cost, generic_cost_per_token, get_model_pricing,
};
#[deprecated(
    note = "use core::pricing_service::CostResult; this legacy shape is retained only for compatibility and removal is no earlier than 0.7.0"
)]
pub use types::CostResult;
pub use types::{
    CostBreakdown, CostError, CostEstimate, CostSummary, CostTracker, ModelCostComparison,
    ModelPricing, ProviderPricing, UsageTokens,
};
pub use utils::{
    calculate_cost_component, format_cost, get_cost_per_unit, select_tiered_pricing, tokens_to_cost,
};

pub mod providers {
    //! Provider-specific cost calculation modules
    //! Each provider only needs to implement simple delegation functions

    pub mod anthropic;
    pub mod azure;
    pub mod generic;
    pub mod openai;
}
