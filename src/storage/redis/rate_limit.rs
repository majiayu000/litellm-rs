//! Redis-backed rate limit operations.

use super::pool::RedisPool;
use crate::core::rate_limiter::RateLimitResult;
use crate::utils::error::gateway_error::{GatewayError, Result};

const CHECK_AND_RECORD_SCRIPT: &str = r#"
local limit = tonumber(ARGV[1])
local window = tonumber(ARGV[2])
local current = tonumber(redis.call("GET", KEYS[1]) or "0")
local ttl = redis.call("TTL", KEYS[1])

if limit <= 0 then
  if ttl < 0 then ttl = window end
  return {0, current, limit, 0, ttl}
end

if current >= limit then
  if ttl < 0 then
    redis.call("EXPIRE", KEYS[1], window)
    ttl = window
  end
  return {0, current, limit, 0, ttl}
end

current = redis.call("INCR", KEYS[1])
if current == 1 or ttl < 0 then
  redis.call("EXPIRE", KEYS[1], window)
  ttl = window
else
  ttl = redis.call("TTL", KEYS[1])
end

local remaining = limit - current
if remaining < 0 then remaining = 0 end
return {1, current, limit, remaining, ttl}
"#;

const STATUS_SCRIPT: &str = r#"
local limit = tonumber(ARGV[1])
local window = tonumber(ARGV[2])
local current = tonumber(redis.call("GET", KEYS[1]) or "0")
local ttl = redis.call("TTL", KEYS[1])
if ttl < 0 then ttl = window end
local allowed = 0
if current < limit then allowed = 1 end
local remaining = limit - current
if remaining < 0 then remaining = 0 end
return {allowed, current, limit, remaining, ttl}
"#;

const RELEASE_SCRIPT: &str = r#"
local reservation_ttl = tonumber(ARGV[1])
if reservation_ttl == nil or reservation_ttl <= 0 then
  return 0
end

local current = tonumber(redis.call("GET", KEYS[1]) or "0")
if current <= 0 then
  return 0
end

local ttl = redis.call("TTL", KEYS[1])
if ttl < 0 then
  return current
end

if ttl > (reservation_ttl + 1) then
  return current
end

current = redis.call("DECR", KEYS[1])
if current <= 0 then
  redis.call("DEL", KEYS[1])
  return 0
end

return current
"#;

fn redis_rate_limit_key(key: &str) -> String {
    format!("litellm-rs:rate_limit:v1:{}", key)
}

fn parse_rate_limit_result(values: Vec<i64>) -> Result<RateLimitResult> {
    if values.len() != 5 {
        return Err(GatewayError::Storage(format!(
            "Unexpected Redis rate-limit result length: {}",
            values.len()
        )));
    }

    let allowed = values[0] == 1;
    let current_count = values[1].max(0) as u32;
    let limit = values[2].max(0) as u32;
    let remaining = values[3].max(0) as u32;
    let reset_after_secs = values[4].max(0) as u64;

    Ok(RateLimitResult {
        allowed,
        current_count,
        limit,
        remaining,
        reset_after_secs,
        retry_after_secs: if allowed {
            None
        } else {
            Some(reset_after_secs.max(1))
        },
    })
}

impl RedisPool {
    /// Atomically check and record one request against a distributed fixed window.
    pub async fn rate_limit_check_and_record(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> Result<RateLimitResult> {
        if self.noop_mode {
            return Ok(RateLimitResult {
                allowed: true,
                current_count: 0,
                limit,
                remaining: limit,
                reset_after_secs: 0,
                retry_after_secs: None,
            });
        }

        let redis_key = redis_rate_limit_key(key);
        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let values: Vec<i64> = redis::Script::new(CHECK_AND_RECORD_SCRIPT)
                .key(redis_key)
                .arg(limit)
                .arg(window_secs.max(1))
                .invoke_async(c)
                .await
                .map_err(GatewayError::from)?;
            parse_rate_limit_result(values)
        } else {
            Ok(RateLimitResult {
                allowed: true,
                current_count: 0,
                limit,
                remaining: limit,
                reset_after_secs: 0,
                retry_after_secs: None,
            })
        }
    }

    /// Read current distributed fixed-window status without recording a request.
    pub async fn rate_limit_status(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> Result<RateLimitResult> {
        if self.noop_mode {
            return Ok(RateLimitResult {
                allowed: true,
                current_count: 0,
                limit,
                remaining: limit,
                reset_after_secs: 0,
                retry_after_secs: None,
            });
        }

        let redis_key = redis_rate_limit_key(key);
        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let values: Vec<i64> = redis::Script::new(STATUS_SCRIPT)
                .key(redis_key)
                .arg(limit)
                .arg(window_secs.max(1))
                .invoke_async(c)
                .await
                .map_err(GatewayError::from)?;
            parse_rate_limit_result(values)
        } else {
            Ok(RateLimitResult {
                allowed: true,
                current_count: 0,
                limit,
                remaining: limit,
                reset_after_secs: 0,
                retry_after_secs: None,
            })
        }
    }

    /// Release one previously-recorded request from a distributed fixed window.
    pub async fn rate_limit_release(&self, key: &str, reservation_ttl_secs: u64) -> Result<()> {
        if self.noop_mode || reservation_ttl_secs == 0 {
            return Ok(());
        }

        let redis_key = redis_rate_limit_key(key);
        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let _: i64 = redis::Script::new(RELEASE_SCRIPT)
                .key(redis_key)
                .arg(reservation_ttl_secs)
                .invoke_async(c)
                .await
                .map_err(GatewayError::from)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::storage::RedisConfig;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn must<T>(result: Result<T>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(err) => panic!("{context}: {err}"),
        }
    }

    #[test]
    fn parses_redis_rate_limit_result() {
        let result = parse_rate_limit_result(vec![0, 5, 5, 0, 42]).unwrap();
        assert!(!result.allowed);
        assert_eq!(result.current_count, 5);
        assert_eq!(result.limit, 5);
        assert_eq!(result.retry_after_secs, Some(42));
    }

    #[tokio::test]
    async fn disabled_redis_pool_allows_rate_limit_checks() {
        let pool = RedisPool::new(&RedisConfig {
            enabled: false,
            ..RedisConfig::default()
        })
        .await
        .expect("disabled Redis should create a no-op pool");

        let result = pool
            .rate_limit_check_and_record("client", 1, 60)
            .await
            .unwrap();
        assert!(result.allowed);
        assert_eq!(result.remaining, 1);
    }

    #[tokio::test]
    async fn live_redis_rate_limit_state_is_shared() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };

        let key = unique_test_key("rate-limit");
        let first = pool
            .rate_limit_check_and_record(&key, 1, 30)
            .await
            .expect("first check should succeed");
        let second = pool
            .rate_limit_check_and_record(&key, 1, 30)
            .await
            .expect("second check should succeed");

        assert!(first.allowed);
        assert!(!second.allowed);
        assert_eq!(second.remaining, 0);

        let _ = pool.delete(&redis_rate_limit_key(&key)).await;
    }

    #[tokio::test]
    async fn live_redis_release_skips_newer_window_after_original_expiry() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };

        let key = unique_test_key("rate-limit-release-expired");
        let first = must(
            pool.rate_limit_check_and_record(&key, 1, 1).await,
            "first check should succeed",
        );
        assert!(first.allowed);

        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

        let second = must(
            pool.rate_limit_check_and_record(&key, 1, 30).await,
            "second check should succeed in a new window",
        );
        assert!(second.allowed);

        must(
            pool.rate_limit_release(&key, 1).await,
            "stale release should be handled",
        );

        let status = must(
            pool.rate_limit_status(&key, 1, 30).await,
            "status should succeed",
        );
        assert_eq!(status.current_count, 1);
        assert_eq!(status.remaining, 0);

        if let Err(err) = pool.delete(&redis_rate_limit_key(&key)).await {
            eprintln!("failed to clean up Redis rate-limit test key {key}: {err}");
        }
    }

    async fn live_redis_pool() -> Option<RedisPool> {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
        let config = RedisConfig {
            url: redis_url.clone(),
            enabled: true,
            max_connections: 10,
            connection_timeout: 1,
            cluster: false,
            cluster_configured: false,
            allow_degraded: false,
        };

        match RedisPool::new(&config).await {
            Ok(pool) => match pool.health_check().await {
                Ok(()) => Some(pool),
                Err(err) => {
                    if std::env::var("CI").is_ok() {
                        panic!("Redis should pass health check in CI at {redis_url}: {err}");
                    }

                    eprintln!("Skipping live Redis rate-limit integration test: {err}");
                    None
                }
            },
            Err(err) => {
                if std::env::var("CI").is_ok() {
                    panic!("Redis should be reachable in CI at {redis_url}: {err}");
                }

                eprintln!("Skipping live Redis rate-limit integration test: {err}");
                None
            }
        }
    }

    fn unique_test_key(suffix: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        format!("litellm-rs:test:{suffix}:{nanos}")
    }
}
