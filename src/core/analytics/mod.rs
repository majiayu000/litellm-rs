//! Advanced analytics and reporting system
//!
//! This module provides comprehensive analytics, cost optimization suggestions,
//! and detailed reporting capabilities.

mod collector;
mod engine;
mod optimizer;
mod reports;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
#[deprecated(
    since = "0.6.0",
    note = "core::analytics is scheduled for removal in 0.7.0; use wired request metrics and callback integrations"
)]
pub use collector::MetricsCollector;
#[deprecated(
    since = "0.6.0",
    note = "core::analytics is scheduled for removal in 0.7.0; use wired request metrics and callback integrations"
)]
pub use engine::AnalyticsEngine;
#[deprecated(
    since = "0.6.0",
    note = "core::analytics is scheduled for removal in 0.7.0; use wired request metrics and callback integrations"
)]
pub use optimizer::{
    CostOptimizer, OptimizationDifficulty, OptimizationRule, OptimizationSuggestion,
    OptimizationType,
};
#[deprecated(
    since = "0.6.0",
    note = "core::analytics is scheduled for removal in 0.7.0; use wired request metrics and callback integrations"
)]
pub use reports::{
    ChartData, DataPoint, GeneratedReport, ReportFormat, ReportGenerator, ReportSection,
    ReportSectionData, ReportSectionType, ReportSummary, ReportTemplate,
};
#[deprecated(
    since = "0.6.0",
    note = "core::analytics is scheduled for removal in 0.7.0; use wired request metrics and callback integrations"
)]
pub use types::{
    AnalyticsRequestMetrics, BudgetUtilization, CostBreakdown, CostMetrics, CostTrend, DailyCost,
    ModelUsage, ProviderMetrics, RequestSizeDistribution, SeasonalTrend, TokenUsage, UsagePatterns,
    UserMetrics,
};
