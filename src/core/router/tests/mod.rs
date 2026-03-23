//! Router tests module
//!
//! Contains comprehensive tests for the unified router system.

// Unified router tests
mod cooldown_tests;
mod execution_tests;
mod fallback_tests;
mod router_tests;
mod strategy_tests;

// Concurrency and edge case tests (issue #216)
mod concurrency_edge_case_tests;

// Selection logic tests (check_parallel_limit, check_rate_limit, mixed states)
mod selection_tests;

// Legacy module tests (moved from embedded tests)
mod deployment_tests;
mod strategy_impl_tests;
