//! HTTP middleware implementations
//!
//! This module provides various middleware for request processing:
//! - Authentication and authorization
//! - Rate limiting (auth-specific and general)
//! - Request ID tracking
//! - Metrics collection
//! - Security headers

#![allow(dead_code)]

mod auth;
mod auth_rate_limiter;
mod helpers;
mod metrics;
mod rate_limit;
mod request_id;
mod security;

#[cfg(test)]
mod tests;

// Re-export all middleware
pub use auth::{AuthMiddleware, AuthMiddlewareService, get_request_context};
pub use auth_rate_limiter::{AuthRateLimiter, get_auth_rate_limiter};
pub use helpers::{extract_auth_method, is_admin_route, is_api_route, is_public_route};
pub use metrics::{MetricsMiddleware, MetricsMiddlewareService, MiddlewareRequestMetrics};
pub use rate_limit::{RateLimitMiddleware, RateLimitMiddlewareService};
pub use request_id::{RequestIdMiddleware, RequestIdMiddlewareService};
pub use security::{SecurityHeadersMiddleware, SecurityHeadersMiddlewareService};
