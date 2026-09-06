//! Single-key Lua deployment admission (cluster-safe: `KEYS[1]` only).

use super::pool::{RedisLiveConnection, RedisPool};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use std::sync::OnceLock;

const ADMISSION_SCRIPT: &str = r#"
local op = ARGV[1]
local now = tonumber(ARGV[2]) or 0
local epoch = tonumber(ARGV[3]) or 0
local max_p = tonumber(ARGV[4]) or -1
local max_r = tonumber(ARGV[5]) or -1
local max_t = tonumber(ARGV[6]) or -1
local rpm_inc = tonumber(ARGV[7]) or 0
local tpm_inc = tonumber(ARGV[8]) or 0
local lease_id = ARGV[9]
local ttl = tonumber(ARGV[10]) or 1
local actual_tpm = tonumber(ARGV[11]) or 0

local function nums()
  local p = tonumber(redis.call('HGET', KEYS[1], 'p') or '0') or 0
  local r = tonumber(redis.call('HGET', KEYS[1], 'r') or '0') or 0
  local t = tonumber(redis.call('HGET', KEYS[1], 't') or '0') or 0
  local e = tonumber(redis.call('HGET', KEYS[1], 'e') or '0') or 0
  return p, r, t, e
end

local function save(p, r, t, e)
  if p < 0 then p = 0 end
  if r < 0 then r = 0 end
  if t < 0 then t = 0 end
  redis.call('HSET', KEYS[1], 'p', p, 'r', r, 't', t, 'e', e)
end

local function reclaim()
  local p, r, t, e = nums()
  if e ~= epoch then
    r = 0
    t = 0
    e = epoch
  end
  local fields = redis.call('HGETALL', KEYS[1])
  for i = 1, #fields, 2 do
    if string.sub(fields[i], 1, 2) == 'l:' then
      local pInc, rpmInc, tpmInc, lease_epoch, expiry = string.match(
        fields[i + 1], '^(%d+):(%d+):(%d+):(%-?%d+):(%d+)$'
      )
      pInc = tonumber(pInc)
      rpmInc = tonumber(rpmInc)
      tpmInc = tonumber(tpmInc)
      lease_epoch = tonumber(lease_epoch)
      expiry = tonumber(expiry)
      if pInc == nil or expiry == nil or expiry <= now then
        if pInc ~= nil and expiry ~= nil and expiry <= now then
          p = p - pInc
          if lease_epoch == e then
            r = r - rpmInc
            t = t - tpmInc
          end
        end
        redis.call('HDEL', KEYS[1], fields[i])
      end
    end
  end
  save(p, r, t, e)
  return p, r, t, e
end

local p, r, t, e = reclaim()

if op == 'reserve' then
  if (max_p >= 0 and p + 1 > max_p)
      or (max_r >= 0 and r + rpm_inc > max_r)
      or (max_t >= 0 and t + tpm_inc > max_t) then
    return {0, p, r, t}
  end
  p = p + 1
  r = r + rpm_inc
  t = t + tpm_inc
  if ttl < 1 then ttl = 1 end
  save(p, r, t, e)
  redis.call(
    'HSET',
    KEYS[1],
    'l:' .. lease_id,
    '1:' .. tostring(rpm_inc) .. ':' .. tostring(tpm_inc) .. ':' .. tostring(e) .. ':' .. tostring(now + ttl)
  )
  return {1, p, r, t}
end

if op == 'settle' or op == 'cancel' then
  local field = 'l:' .. lease_id
  local lease = redis.call('HGET', KEYS[1], field)
  if lease then
    local pInc, rpmInc, tpmInc, lease_epoch = string.match(lease, '^(%d+):(%d+):(%d+):(%-?%d+):')
    pInc = tonumber(pInc) or 1
    rpmInc = tonumber(rpmInc) or 0
    tpmInc = tonumber(tpmInc) or 0
    lease_epoch = tonumber(lease_epoch)
    p = p - pInc
    if lease_epoch == e then
      if op == 'cancel' then
        r = r - rpmInc
        t = t - tpmInc
      else
        t = t - tpmInc + actual_tpm
      end
    elseif op == 'settle' then
      t = t + actual_tpm
    end
    redis.call('HDEL', KEYS[1], field)
    save(p, r, t, e)
  end
  return {1, p, r, t}
end

return {-1, p, r, t}
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdmissionState {
    pub allowed: bool,
    pub parallel: i64,
    pub rpm: i64,
    pub tpm: i64,
}

pub(crate) struct AdmissionReserveArgs<'a> {
    pub key: &'a str,
    pub max_parallel: i64,
    pub max_rpm: i64,
    pub max_tpm: i64,
    pub rpm_inc: i64,
    pub tpm_inc: i64,
    pub lease_id: &'a str,
    pub now_ms: i64,
    pub window_epoch: i64,
    pub ttl_ms: i64,
}

fn admission_script() -> &'static redis::Script {
    static SCRIPT: OnceLock<redis::Script> = OnceLock::new();
    SCRIPT.get_or_init(|| redis::Script::new(ADMISSION_SCRIPT))
}

fn admission_runtime_connections()
-> &'static tokio::sync::Mutex<HashMap<String, RedisLiveConnection>> {
    static CACHE: OnceLock<tokio::sync::Mutex<HashMap<String, RedisLiveConnection>>> =
        OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

async fn connection_on_current_runtime(pool: &RedisPool) -> Result<RedisLiveConnection> {
    let cache_key = format!("{}|{}", pool.config.url, pool.config.cluster);
    {
        let cache = admission_runtime_connections().lock().await;
        if let Some(conn) = cache.get(&cache_key) {
            return Ok(conn.clone());
        }
    }
    let conn = pool.open_live_connection().await?;
    let mut cache = admission_runtime_connections().lock().await;
    Ok(cache.entry(cache_key).or_insert(conn).clone())
}

fn parse_admission_state(values: Vec<i64>) -> Result<AdmissionState> {
    if values.len() != 4 {
        return Err(GatewayError::Storage(format!(
            "Unexpected Redis admission result length: {}",
            values.len()
        )));
    }
    if values[0] < 0 {
        return Err(GatewayError::Storage(
            "Redis admission script returned an error status".to_string(),
        ));
    }
    Ok(AdmissionState {
        allowed: values[0] == 1,
        parallel: values[1].max(0),
        rpm: values[2].max(0),
        tpm: values[3].max(0),
    })
}

struct AdmissionArgs<'a> {
    op: &'a str,
    now_ms: i64,
    window_epoch: i64,
    max_parallel: i64,
    max_rpm: i64,
    max_tpm: i64,
    rpm_inc: i64,
    tpm_inc: i64,
    lease_id: &'a str,
    ttl_ms: i64,
    actual_tpm: i64,
}

impl RedisPool {
    pub(crate) fn admission_key(deployment_id: &str) -> String {
        format!("litellm-rs:admission:v1:{deployment_id}")
    }

    async fn invoke_admission(&self, key: &str, args: AdmissionArgs<'_>) -> Result<AdmissionState> {
        if self.noop_mode {
            return Err(GatewayError::Storage(
                "admission redis backend is unavailable".to_string(),
            ));
        }

        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| GatewayError::Internal("Redis semaphore closed".to_string()))?;
        let mut conn = connection_on_current_runtime(self).await?;

        let values: Vec<i64> = admission_script()
            .key(key)
            .arg(args.op)
            .arg(args.now_ms)
            .arg(args.window_epoch)
            .arg(args.max_parallel)
            .arg(args.max_rpm)
            .arg(args.max_tpm)
            .arg(args.rpm_inc)
            .arg(args.tpm_inc)
            .arg(args.lease_id)
            .arg(args.ttl_ms)
            .arg(args.actual_tpm)
            .invoke_async(&mut conn)
            .await
            .map_err(GatewayError::from)?;
        parse_admission_state(values)
    }

    pub(crate) async fn admission_reserve(
        &self,
        args: AdmissionReserveArgs<'_>,
    ) -> Result<AdmissionState> {
        self.invoke_admission(
            args.key,
            AdmissionArgs {
                op: "reserve",
                now_ms: args.now_ms,
                window_epoch: args.window_epoch,
                max_parallel: args.max_parallel,
                max_rpm: args.max_rpm,
                max_tpm: args.max_tpm,
                rpm_inc: args.rpm_inc,
                tpm_inc: args.tpm_inc,
                lease_id: args.lease_id,
                ttl_ms: args.ttl_ms,
                actual_tpm: 0,
            },
        )
        .await
    }

    pub(crate) async fn admission_settle(
        &self,
        key: &str,
        lease_id: &str,
        actual_tpm: i64,
        window_epoch: i64,
        now_ms: i64,
    ) -> Result<AdmissionState> {
        self.admission_finish("settle", key, lease_id, actual_tpm, window_epoch, now_ms)
            .await
    }

    pub(crate) async fn admission_cancel(
        &self,
        key: &str,
        lease_id: &str,
        window_epoch: i64,
        now_ms: i64,
    ) -> Result<AdmissionState> {
        self.admission_finish("cancel", key, lease_id, 0, window_epoch, now_ms)
            .await
    }

    async fn admission_finish(
        &self,
        op: &str,
        key: &str,
        lease_id: &str,
        actual_tpm: i64,
        window_epoch: i64,
        now_ms: i64,
    ) -> Result<AdmissionState> {
        self.invoke_admission(
            key,
            AdmissionArgs {
                op,
                now_ms,
                window_epoch,
                max_parallel: -1,
                max_rpm: -1,
                max_tpm: -1,
                rpm_inc: 0,
                tpm_inc: 0,
                lease_id,
                ttl_ms: 0,
                actual_tpm,
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::storage::RedisConfig;

    #[test]
    fn parses_admission_state() {
        let allowed = parse_admission_state(vec![1, 2, 3, 4]).unwrap();
        assert!(allowed.allowed);
        assert_eq!(allowed.parallel, 2);
        assert_eq!(allowed.rpm, 3);
        assert_eq!(allowed.tpm, 4);

        let denied = parse_admission_state(vec![0, 1, 0, 0]).unwrap();
        assert!(!denied.allowed);
        assert!(parse_admission_state(vec![-1, 0, 0, 0]).is_err());
        assert!(parse_admission_state(vec![1, 0]).is_err());
    }

    #[tokio::test]
    async fn noop_redis_pool_fails_closed_on_admission_reserve() {
        let pool = RedisPool::new(&RedisConfig {
            enabled: false,
            ..RedisConfig::default()
        })
        .await
        .expect("disabled Redis should create a no-op pool");

        let err = pool
            .admission_reserve(AdmissionReserveArgs {
                key: "litellm-rs:admission:v1:noop-test",
                max_parallel: 1,
                max_rpm: -1,
                max_tpm: -1,
                rpm_inc: 0,
                tpm_inc: 0,
                lease_id: "lease",
                now_ms: 1,
                window_epoch: 0,
                ttl_ms: 1_000,
            })
            .await
            .expect_err("no-op Redis must not fail-open admission");
        assert!(err.to_string().contains("unavailable"));
    }
}
