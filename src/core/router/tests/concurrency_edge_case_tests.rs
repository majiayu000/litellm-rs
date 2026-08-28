//! Concurrency and edge case tests for the routing module
//!
//! Covers gaps identified in issue #216:
//! 1. Concurrent `select_deployment` under DashMap contention
//! 2. `set_model_list` atomicity with concurrent readers
//! 3. Weighted random statistical distribution verification
//! 4. EMA latency calculation edge cases (overflow, boundary values)
//! 5. Cooldown expiry race conditions

#![allow(deprecated)]

use super::router_tests::create_test_deployment;
use crate::core::router::config::{RouterConfig, RoutingStrategy};
use crate::core::router::deployment::{Deployment, DeploymentConfig, HealthStatus};
use crate::core::router::strategy_impl::{
    RoutingContext, build_routing_contexts, weighted_random_from_context,
};
use crate::core::router::unified::Router;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

mod additional_edge_case_tests;
mod concurrent_selection_tests;
mod cooldown_expiry_tests;
mod ema_latency_tests;
mod model_list_swap_tests;
mod weighted_random_tests;
