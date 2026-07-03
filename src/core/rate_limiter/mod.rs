//! Rate Limiting Implementation
//!
//! Provides sliding window rate limiting with support for multiple strategies

mod limiter;
mod strategies;
mod types;
mod utils;

#[cfg(test)]
mod tests;

// Re-export public types
#[cfg(test)]
pub(crate) use limiter::RateLimitRecordSource;
pub(crate) use limiter::RateLimitReservation;
pub use limiter::{RateLimiter, render_rate_limiter_prometheus};
pub use types::RateLimitResult;

use crate::config::models::rate_limit::RateLimitConfig;
use std::sync::Arc;

/// Global rate limiter singleton
static GLOBAL_RATE_LIMITER: std::sync::OnceLock<Arc<RateLimiter>> = std::sync::OnceLock::new();

/// Initialize the global rate limiter
pub fn init_global_rate_limiter(config: RateLimitConfig) {
    let limiter = Arc::new(RateLimiter::new(config));
    let _ = GLOBAL_RATE_LIMITER.set(limiter.clone());

    // Start cleanup task
    limiter.start_cleanup_task();
}

/// Initialize the global rate limiter with an optional Redis-backed distributed store.
#[cfg(feature = "gateway")]
pub fn init_global_rate_limiter_with_redis(
    config: RateLimitConfig,
    redis: Arc<crate::storage::redis::RedisPool>,
) {
    let limiter = Arc::new(RateLimiter::with_redis(config, redis));
    let _ = GLOBAL_RATE_LIMITER.set(limiter.clone());

    // Local cleanup remains useful for fallback entries when Redis is unavailable.
    limiter.start_cleanup_task();
}

/// Get the global rate limiter
pub fn get_global_rate_limiter() -> Option<Arc<RateLimiter>> {
    GLOBAL_RATE_LIMITER.get().cloned()
}
