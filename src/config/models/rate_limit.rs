//! Rate limit configuration types

use serde::{Deserialize, Serialize};

/// Default requests per minute
fn default_rpm() -> u32 {
    1000
}

/// Default tokens per minute
fn default_tpm() -> u32 {
    100_000
}

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    #[serde(default)]
    pub enabled: bool,
    /// Algorithm/strategy type
    #[serde(default, alias = "algorithm")]
    pub strategy: RateLimitStrategy,
    /// Default requests per minute (gateway-level)
    #[serde(default = "default_rpm")]
    pub default_rpm: u32,
    /// Default tokens per minute (gateway-level)
    #[serde(default = "default_tpm")]
    pub default_tpm: u32,
    /// Requests per second (per-provider/per-endpoint)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests_per_second: Option<u32>,
    /// Requests per minute (per-provider/per-endpoint)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests_per_minute: Option<u32>,
    /// Tokens per minute (per-provider/per-endpoint)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_minute: Option<u32>,
    /// Burst size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst_size: Option<u32>,
    /// Redis failure behavior for distributed rate limiting.
    #[serde(default)]
    pub redis_failure_mode: RedisFailureMode,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: RateLimitStrategy::default(),
            default_rpm: default_rpm(),
            default_tpm: default_tpm(),
            requests_per_second: None,
            requests_per_minute: None,
            tokens_per_minute: None,
            burst_size: None,
            redis_failure_mode: RedisFailureMode::default(),
        }
    }
}

impl RateLimitConfig {
    /// Effective gateway requests-per-minute limit.
    ///
    /// `requests_per_minute` is accepted for LiteLLM compatibility and takes
    /// precedence over the older `default_rpm` field when present.
    pub fn effective_rpm(&self) -> u32 {
        self.requests_per_minute.unwrap_or(self.default_rpm)
    }

    /// Fields parsed from config but not enforced by the current limiter.
    ///
    /// Keep this list shared with validation and runtime construction so a
    /// configured-but-ignored limit cannot pass validation silently.
    pub fn unimplemented_runtime_field_names(&self) -> Vec<&'static str> {
        let defaults = RateLimitConfig::default();
        let mut fields = Vec::new();
        if self.default_tpm != defaults.default_tpm {
            fields.push("default_tpm");
        }
        if self.requests_per_second.is_some() {
            fields.push("requests_per_second");
        }
        if self.tokens_per_minute.is_some() {
            fields.push("tokens_per_minute");
        }
        if self.burst_size.is_some() {
            fields.push("burst_size");
        }
        fields
    }

    /// Merge rate limit configurations
    pub fn merge(mut self, other: Self) -> Self {
        if other.enabled {
            self.enabled = other.enabled;
        }
        if other.default_rpm != default_rpm() {
            self.default_rpm = other.default_rpm;
            if other.requests_per_minute.is_none() {
                self.requests_per_minute = None;
            }
        }
        if other.default_tpm != default_tpm() {
            self.default_tpm = other.default_tpm;
        }
        self.strategy = other.strategy;
        if other.requests_per_second.is_some() {
            self.requests_per_second = other.requests_per_second;
        }
        if other.requests_per_minute.is_some() {
            self.requests_per_minute = other.requests_per_minute;
        }
        if other.tokens_per_minute.is_some() {
            self.tokens_per_minute = other.tokens_per_minute;
        }
        if other.burst_size.is_some() {
            self.burst_size = other.burst_size;
        }
        if other.redis_failure_mode != RedisFailureMode::default() {
            self.redis_failure_mode = other.redis_failure_mode;
        }
        self
    }
}

/// Redis outage policy for distributed rate limiting.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RedisFailureMode {
    /// Reject rate-limited requests when Redis enforcement is unavailable.
    #[default]
    FailClosed,
    /// Explicitly fall back to process-local limits when Redis fails.
    FailOpenLocal,
}

impl RedisFailureMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FailClosed => "fail_closed",
            Self::FailOpenLocal => "fail_open_local",
        }
    }
}

/// Rate limit strategy/algorithm
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitStrategy {
    /// Token bucket algorithm
    #[default]
    TokenBucket,
    /// Fixed window
    FixedWindow,
    /// Sliding window
    SlidingWindow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_strategy_default() {
        let strategy = RateLimitStrategy::default();
        assert_eq!(strategy, RateLimitStrategy::TokenBucket);
    }

    #[test]
    fn test_rate_limit_strategy_serialization() {
        assert_eq!(
            serde_json::to_string(&RateLimitStrategy::TokenBucket).unwrap(),
            "\"token_bucket\""
        );
        assert_eq!(
            serde_json::to_string(&RateLimitStrategy::SlidingWindow).unwrap(),
            "\"sliding_window\""
        );
        assert_eq!(
            serde_json::to_string(&RateLimitStrategy::FixedWindow).unwrap(),
            "\"fixed_window\""
        );
    }

    #[test]
    fn test_rate_limit_strategy_deserialization() {
        let tb: RateLimitStrategy = serde_json::from_str("\"token_bucket\"").unwrap();
        assert_eq!(tb, RateLimitStrategy::TokenBucket);
        let sw: RateLimitStrategy = serde_json::from_str("\"sliding_window\"").unwrap();
        assert_eq!(sw, RateLimitStrategy::SlidingWindow);
        let fw: RateLimitStrategy = serde_json::from_str("\"fixed_window\"").unwrap();
        assert_eq!(fw, RateLimitStrategy::FixedWindow);
    }

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.default_rpm, 1000);
        assert_eq!(config.default_tpm, 100_000);
        assert_eq!(config.strategy, RateLimitStrategy::TokenBucket);
        assert!(config.requests_per_second.is_none());
        assert!(config.burst_size.is_none());
        assert_eq!(config.redis_failure_mode, RedisFailureMode::FailClosed);
    }

    #[test]
    fn test_rate_limit_config_full() {
        let config = RateLimitConfig {
            enabled: true,
            strategy: RateLimitStrategy::SlidingWindow,
            default_rpm: 500,
            default_tpm: 50_000,
            requests_per_second: Some(10),
            requests_per_minute: None,
            tokens_per_minute: Some(100_000),
            burst_size: Some(20),
            redis_failure_mode: RedisFailureMode::FailOpenLocal,
        };
        assert!(config.enabled);
        assert_eq!(config.requests_per_second, Some(10));
        assert_eq!(config.burst_size, Some(20));
        assert_eq!(config.redis_failure_mode, RedisFailureMode::FailOpenLocal);
    }

    #[test]
    fn test_rate_limit_config_serialization() {
        let config = RateLimitConfig {
            enabled: true,
            strategy: RateLimitStrategy::FixedWindow,
            default_rpm: 600,
            default_tpm: 60_000,
            requests_per_second: Some(50),
            requests_per_minute: None,
            tokens_per_minute: None,
            burst_size: None,
            redis_failure_mode: RedisFailureMode::FailClosed,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["strategy"], "fixed_window");
        assert_eq!(json["default_rpm"], 600);
        assert_eq!(json["requests_per_second"], 50);
        assert_eq!(json["redis_failure_mode"], "fail_closed");
    }

    #[test]
    fn test_rate_limit_config_deserialization_defaults() {
        let json = r#"{}"#;
        let config: RateLimitConfig = serde_json::from_str(json).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.default_rpm, 1000);
        assert_eq!(config.default_tpm, 100_000);
        assert_eq!(config.redis_failure_mode, RedisFailureMode::FailClosed);
    }

    #[test]
    fn test_rate_limit_config_deserialization_redis_failure_mode() {
        let json = r#"{"redis_failure_mode": "fail_open_local"}"#;
        let config: RateLimitConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.redis_failure_mode, RedisFailureMode::FailOpenLocal);
    }

    #[test]
    fn test_rate_limit_config_deserialization_algorithm_alias() {
        let json = r#"{"algorithm": "sliding_window"}"#;
        let config: RateLimitConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.strategy, RateLimitStrategy::SlidingWindow);
    }

    #[test]
    fn test_rate_limit_config_merge() {
        let base = RateLimitConfig::default();
        let other = RateLimitConfig {
            enabled: true,
            strategy: RateLimitStrategy::SlidingWindow,
            default_rpm: 500,
            default_tpm: 100_000,
            requests_per_second: Some(10),
            requests_per_minute: None,
            tokens_per_minute: None,
            burst_size: Some(20),
            redis_failure_mode: RedisFailureMode::FailOpenLocal,
        };
        let merged = base.merge(other);
        assert!(merged.enabled);
        assert_eq!(merged.default_rpm, 500);
        assert_eq!(merged.strategy, RateLimitStrategy::SlidingWindow);
        assert_eq!(merged.requests_per_second, Some(10));
        assert_eq!(merged.burst_size, Some(20));
        assert_eq!(merged.redis_failure_mode, RedisFailureMode::FailOpenLocal);
    }

    #[test]
    fn test_rate_limit_config_merge_no_change() {
        let base = RateLimitConfig::default();
        let other = RateLimitConfig::default();
        let merged = base.merge(other);
        assert!(!merged.enabled);
        assert_eq!(merged.default_rpm, 1000);
        assert_eq!(merged.default_tpm, 100_000);
    }

    #[test]
    fn test_rate_limit_config_merge_default_rpm_clears_alias() {
        let base = RateLimitConfig {
            requests_per_minute: Some(25),
            ..RateLimitConfig::default()
        };
        let other = RateLimitConfig {
            default_rpm: 500,
            ..RateLimitConfig::default()
        };

        let merged = base.merge(other);

        assert_eq!(merged.requests_per_minute, None);
        assert_eq!(merged.effective_rpm(), 500);
    }

    #[test]
    fn test_rate_limit_effective_rpm_uses_requests_per_minute_alias() {
        let config = RateLimitConfig {
            default_rpm: 1000,
            requests_per_minute: Some(25),
            ..RateLimitConfig::default()
        };

        assert_eq!(config.effective_rpm(), 25);
    }

    #[test]
    fn test_rate_limit_effective_rpm_falls_back_to_default_rpm() {
        let config = RateLimitConfig {
            default_rpm: 500,
            requests_per_minute: None,
            ..RateLimitConfig::default()
        };

        assert_eq!(config.effective_rpm(), 500);
    }

    #[test]
    fn test_rate_limit_config_skip_none_serialization() {
        let config = RateLimitConfig {
            enabled: false,
            strategy: RateLimitStrategy::TokenBucket,
            default_rpm: 1000,
            default_tpm: 100_000,
            requests_per_second: None,
            requests_per_minute: Some(1000),
            tokens_per_minute: None,
            burst_size: None,
            redis_failure_mode: RedisFailureMode::FailClosed,
        };
        let json = serde_json::to_value(&config).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("requests_per_second"));
        assert!(!obj.contains_key("tokens_per_minute"));
        assert!(!obj.contains_key("burst_size"));
        assert!(obj.contains_key("requests_per_minute"));
    }

    #[test]
    fn test_unimplemented_runtime_field_names_default_empty() {
        let config = RateLimitConfig::default();
        assert!(config.unimplemented_runtime_field_names().is_empty());
    }

    #[test]
    fn test_unimplemented_runtime_field_names_lists_set_fields() {
        let config = RateLimitConfig {
            default_tpm: 50_000,
            requests_per_second: Some(10),
            tokens_per_minute: Some(60_000),
            burst_size: Some(20),
            ..RateLimitConfig::default()
        };

        assert_eq!(
            config.unimplemented_runtime_field_names(),
            vec![
                "default_tpm",
                "requests_per_second",
                "tokens_per_minute",
                "burst_size"
            ]
        );
    }
}
