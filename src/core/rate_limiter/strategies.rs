//! Rate limiting strategy implementations

use super::limiter::RateLimiter;
use super::types::{RateLimitEntry, RateLimitResult};
use std::time::Instant;
use tracing::debug;

impl RateLimiter {
    /// Sliding window rate limiting implementation
    /// If `record` is true, atomically records the request if allowed
    pub(super) async fn check_sliding_window_impl(
        &self,
        key: &str,
        limit: u32,
        record: bool,
        now: Instant,
    ) -> RateLimitResult {
        let window_start = now - self.window;

        let mut entry = self.entries.entry(key.to_string()).or_default();

        // Lazily remove expired timestamps (per-key only, no global lock)
        entry.timestamps.retain(|&t| t > window_start);

        let current_count = entry.timestamps.len() as u32;
        let allowed = current_count < limit;
        let remaining = limit.saturating_sub(current_count);

        // Calculate reset time (time until oldest request expires)
        let reset_after_secs = if let Some(&oldest) = entry.timestamps.first() {
            let elapsed = now.duration_since(oldest);
            self.window.saturating_sub(elapsed).as_secs()
        } else {
            self.window.as_secs()
        };

        let retry_after_secs = if !allowed {
            Some(reset_after_secs.max(1))
        } else {
            // Atomically record if allowed and record flag is set
            if record {
                entry.timestamps.push(now);
            }
            None
        };

        if !allowed {
            debug!(
                "Rate limit exceeded for {}: {}/{} requests",
                key, current_count, limit
            );
        }

        RateLimitResult {
            allowed,
            current_count,
            limit,
            // Adjust remaining if we just recorded
            remaining: if record && allowed {
                remaining.saturating_sub(1)
            } else {
                remaining
            },
            reset_after_secs,
            retry_after_secs,
        }
    }

    /// Token bucket rate limiting implementation
    /// If `record` is true, atomically consumes a token if allowed
    pub(super) async fn check_token_bucket_impl(
        &self,
        key: &str,
        limit: u32,
        record: bool,
        now: Instant,
    ) -> RateLimitResult {
        let tokens_per_second = limit as f64 / 60.0;

        let mut entry = self
            .entries
            .entry(key.to_string())
            .or_insert_with(|| RateLimitEntry {
                tokens: limit as f64,
                last_refill: now,
                timestamps: Vec::new(),
            });

        let reservation_window = self.token_bucket_reservation_window();
        entry
            .timestamps
            .retain(|&ts| now.saturating_duration_since(ts) < reservation_window);

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(entry.last_refill);
        let new_tokens = elapsed.as_secs_f64() * tokens_per_second;
        entry.tokens = (entry.tokens + new_tokens).min(limit as f64);
        entry.last_refill = now;

        let allowed = entry.tokens >= 1.0;
        let current_count = (limit as f64 - entry.tokens) as u32;
        let remaining = entry.tokens as u32;

        // Calculate time until next token
        let reset_after_secs = if entry.tokens < 1.0 {
            ((1.0 - entry.tokens) / tokens_per_second).ceil() as u64
        } else {
            0
        };

        let retry_after_secs = if !allowed {
            Some(reset_after_secs.max(1))
        } else {
            // Atomically consume token if allowed and record flag is set
            if record {
                entry.tokens -= 1.0;
                entry.timestamps.push(now);
            }
            None
        };

        RateLimitResult {
            allowed,
            current_count,
            limit,
            // Adjust remaining if we just consumed a token
            remaining: if record && allowed {
                remaining.saturating_sub(1)
            } else {
                remaining
            },
            reset_after_secs,
            retry_after_secs,
        }
    }

    /// Fixed window rate limiting implementation
    /// If `record` is true, atomically records the request if allowed
    pub(super) async fn check_fixed_window_impl(
        &self,
        key: &str,
        limit: u32,
        record: bool,
        now: Instant,
    ) -> RateLimitResult {
        let mut entry = self.entries.entry(key.to_string()).or_default();

        if entry.timestamps.is_empty() {
            entry.last_refill = now;
        } else if now.duration_since(entry.last_refill) >= self.window {
            entry.timestamps.clear();
            entry.last_refill = now;
        }

        let current_count = entry.timestamps.len() as u32;
        let allowed = current_count < limit;
        let remaining = limit.saturating_sub(current_count);

        // Calculate reset time
        let elapsed = now.duration_since(entry.last_refill);
        let reset_after_secs = self.window.saturating_sub(elapsed).as_secs();

        let retry_after_secs = if !allowed {
            Some(reset_after_secs.max(1))
        } else {
            // Atomically record if allowed and record flag is set
            if record {
                entry.timestamps.push(now);
            }
            None
        };

        RateLimitResult {
            allowed,
            current_count,
            limit,
            // Adjust remaining if we just recorded
            remaining: if record && allowed {
                remaining.saturating_sub(1)
            } else {
                remaining
            },
            reset_after_secs,
            retry_after_secs,
        }
    }
}
