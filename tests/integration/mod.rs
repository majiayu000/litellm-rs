//! Integration tests for litellm-rs
//!
//! These tests verify the interaction between multiple components
//! and test real system behavior without mocking.

pub mod auth_middleware_tests;
pub mod completions_route_tests;
pub mod config_validation_tests;
pub mod database_tests;
pub mod error_handling_tests;
pub mod provider_factory_tests;
pub mod provider_tests;
pub mod router_tests;
pub mod types_tests;
