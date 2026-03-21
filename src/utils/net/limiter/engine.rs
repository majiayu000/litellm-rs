//! Core RateLimiter implementation
//!
//! This module contains the main RateLimiter struct and its core methods.

use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::types::{LimiterConfig, RateLimitKey, RateLimitResult, SlidingWindow, TokenBucket};

/// Rate limiter implementation
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Rate limit configurations
    pub(super) configs: Arc<RwLock<HashMap<String, LimiterConfig>>>,
    /// Token buckets for rate limiting
    pub(super) buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    /// Sliding windows for request counting
    pub(super) windows: Arc<RwLock<HashMap<String, SlidingWindow>>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            buckets: Arc::new(RwLock::new(HashMap::new())),
            windows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add rate limit configuration
    pub async fn add_config(&self, key: String, config: LimiterConfig) {
        let mut configs = self.configs.write().await;
        configs.insert(key, config);
    }

    /// Check rate limit for a request
    pub async fn check_rate_limit(
        &self,
        key: &RateLimitKey,
        tokens: u32,
    ) -> Result<RateLimitResult> {
        let key_str = self.build_key_string(key);

        // Get configuration
        let configs = self.configs.read().await;
        let config = configs
            .get(&key_str)
            .or_else(|| configs.get("default"))
            .ok_or_else(|| GatewayError::Config("No rate limit config found".to_string()))?;

        // Check different rate limits
        let mut result = RateLimitResult {
            allowed: true,
            remaining_requests: None,
            remaining_tokens: None,
            reset_time: None,
            retry_after: None,
            limit_type: None,
        };

        // Check RPM (requests per minute)
        if let Some(rpm) = config.rpm {
            let rpm_result = self
                .check_sliding_window_requests(
                    &format!("{}_rpm", key_str),
                    Duration::from_secs(60),
                    rpm,
                )
                .await?;

            if !rpm_result.allowed {
                result.allowed = false;
                result.limit_type = Some("rpm".to_string());
                result.retry_after = rpm_result.retry_after;
                return Ok(result);
            }
            result.remaining_requests = rpm_result.remaining_requests;
        }

        // Check TPM (tokens per minute)
        if let Some(tpm) = config.tpm {
            let tpm_result = self
                .check_sliding_window_tokens(
                    &format!("{}_tpm", key_str),
                    Duration::from_secs(60),
                    tpm,
                    tokens,
                )
                .await?;

            if !tpm_result.allowed {
                result.allowed = false;
                result.limit_type = Some("tpm".to_string());
                result.retry_after = tpm_result.retry_after;
                return Ok(result);
            }
            result.remaining_tokens = tpm_result.remaining_tokens;
        }

        // Check RPD (requests per day)
        if let Some(rpd) = config.rpd {
            let rpd_result = self
                .check_sliding_window_requests(
                    &format!("{}_rpd", key_str),
                    Duration::from_secs(86400), // 24 hours
                    rpd,
                )
                .await?;

            if !rpd_result.allowed {
                result.allowed = false;
                result.limit_type = Some("rpd".to_string());
                result.retry_after = rpd_result.retry_after;
                return Ok(result);
            }
        }

        // Check TPD (tokens per day)
        if let Some(tpd) = config.tpd {
            let tpd_result = self
                .check_sliding_window_tokens(
                    &format!("{}_tpd", key_str),
                    Duration::from_secs(86400), // 24 hours
                    tpd,
                    tokens,
                )
                .await?;

            if !tpd_result.allowed {
                result.allowed = false;
                result.limit_type = Some("tpd".to_string());
                result.retry_after = tpd_result.retry_after;
                return Ok(result);
            }
        }

        // If allowed, record the request
        if result.allowed {
            self.record_request(&key_str, tokens).await?;
        }

        Ok(result)
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}
