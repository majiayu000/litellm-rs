//! Core router for AI provider management and request routing
//!
//! This module provides intelligent routing, load balancing, and failover
//! across multiple AI providers.
//!
//! ## Module Structure
//!
//! The router is organized into modular components following the single-responsibility principle:
//!
//! - `config` - Router configuration and routing strategy definitions
//! - `error` - Error types and cooldown reasons
//! - `fallback` - Fallback configuration and execution results
//! - `deployment` - Deployment management and health tracking
//! - `unified` - Core Router struct and deployment management
//! - `selection` - Deployment selection logic
//! - `strategy_impl` - Routing strategy implementations
//! - `execution` - Execution helpers and error conversion
//! - `execute_impl` - Execute methods with retry and fallback support
//! - `gateway_config` - Gateway configuration integration

// New modular router components
pub mod budget_routing;
pub mod config;
pub mod deployment;
pub mod error;
pub mod execute_impl;
pub mod execution;
pub mod fallback;
pub mod gateway_config;
pub mod selection;
pub mod strategy_impl;
pub mod unified;

#[cfg(test)]
mod tests;

// Re-exports from deployment module
pub use deployment::{Deployment, DeploymentConfig, DeploymentId, DeploymentState, HealthStatus};

// Re-exports from new modular router (UnifiedRouter)
pub use budget_routing::{BudgetAwareRouter, BudgetAwareRouting, RequestBudgetCheck};
pub use config::{RouterConfig, RoutingStrategy as UnifiedRoutingStrategy};
pub use error::{CooldownReason, RouterError};
pub use fallback::{ExecutionResult, FallbackConfig, FallbackType};
pub use unified::{Router as UnifiedRouter, RoutingMetrics};
