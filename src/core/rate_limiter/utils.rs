//! Utility functions for rate limiter

use super::limiter::RateLimiter;
use super::types::RateLimitResult;
use crate::config::models::rate_limit::RateLimitStrategy;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl RateLimiter {
    /// Cleanup expired entries (background, no global lock — DashMap per-shard locks)
    pub async fn cleanup(&self) {
        let now = Instant::now();
        let window_start = now - self.window;

        let limit = self.config.effective_rpm() as f64;
        self.entries.retain(|_, entry| {
            match self.config.strategy {
                RateLimitStrategy::SlidingWindow => {
                    entry.timestamps.retain(|&t| t > window_start);
                    !entry.timestamps.is_empty()
                }
                RateLimitStrategy::FixedWindow => {
                    if now.duration_since(entry.last_refill) >= self.window {
                        entry.timestamps.clear();
                    }
                    !entry.timestamps.is_empty()
                }
                RateLimitStrategy::TokenBucket => {
                    let reservation_window = self.token_bucket_reservation_window();
                    entry
                        .timestamps
                        .retain(|&t| now.saturating_duration_since(t) < reservation_window);
                    // Keep entry if it has recent timestamps OR has consumed tokens (not full bucket).
                    // A full bucket (tokens == limit) with no timestamps means the key is idle.
                    !entry.timestamps.is_empty() || entry.tokens < limit
                }
            }
        });
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>) {
        let limiter = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                limiter.cleanup().await;
            }
        });
    }

    /// Get current status for a key
    pub async fn status(&self, key: &str) -> Option<RateLimitResult> {
        if !self.config.enabled {
            return None;
        }

        Some(self.check(key).await)
    }

    /// Check if rate limiting is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the configured limit
    pub fn limit(&self) -> u32 {
        self.config.effective_rpm()
    }
}
