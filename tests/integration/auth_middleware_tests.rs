//! Authentication middleware integration tests
//!
//! Covers pass/fail paths and request context propagation.

#[cfg(all(test, feature = "storage"))]
#[path = "auth_middleware_tests_parts/mod.rs"]
mod tests;
