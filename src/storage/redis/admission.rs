//! Single-key Lua deployment admission (cluster-safe: `KEYS[1]` only).

use super::pool::{RedisLiveConnection, RedisPool};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use std::sync::OnceLock;

const ADMISSION_SCRIPT: &str = r#"
local op = ARGV[1]
-- All replicas use the clock at this atomic Redis boundary.
local clock = redis.call('TIME')
local now = tonumber(clock[1]) * 1000 + math.floor(tonumber(clock[2]) / 1000)
local epoch = math.floor(tonumber(clock[1]) / 60)
local max_p = tonumber(ARGV[2]) or -1
local max_r = tonumber(ARGV[3]) or -1
local max_t = tonumber(ARGV[4]) or -1
local rpm_inc = tonumber(ARGV[5]) or 0
local tpm_inc = tonumber(ARGV[6]) or 0
local lease_id = ARGV[7]
local ttl = tonumber(ARGV[8]) or 1
local actual_tpm = tonumber(ARGV[9]) or 0

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
  local window_changed = epoch > e
  if window_changed then
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
      elseif window_changed then
        -- Carry outstanding TPM, but charge RPM only in the arrival window.
        rpmInc = 0
        t = t + tpmInc
        redis.call(
          'HSET', KEYS[1], fields[i],
          tostring(pInc) .. ':' .. tostring(rpmInc) .. ':' .. tostring(tpmInc) .. ':' .. tostring(e) .. ':' .. tostring(expiry)
        )
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

if op == 'settle' or op == 'cancel' or op == 'retain' then
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
      elseif op == 'settle' then
        t = t - tpmInc + actual_tpm
      end
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
    ) -> Result<AdmissionState> {
        self.admission_finish("settle", key, lease_id, actual_tpm)
            .await
    }

    pub(crate) async fn admission_cancel(
        &self,
        key: &str,
        lease_id: &str,
    ) -> Result<AdmissionState> {
        self.admission_finish("cancel", key, lease_id, 0).await
    }

    pub(crate) async fn admission_retain(
        &self,
        key: &str,
        lease_id: &str,
    ) -> Result<AdmissionState> {
        self.admission_finish("retain", key, lease_id, 0).await
    }

    async fn admission_finish(
        &self,
        op: &str,
        key: &str,
        lease_id: &str,
        actual_tpm: i64,
    ) -> Result<AdmissionState> {
        self.invoke_admission(
            key,
            AdmissionArgs {
                op,
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

    async fn live_connection() -> Option<RedisLiveConnection> {
        let Ok(url) = std::env::var("REDIS_URL") else {
            assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
            return None;
        };
        let pool = RedisPool::new(&RedisConfig {
            url,
            enabled: true,
            allow_degraded: false,
            ..RedisConfig::default()
        })
        .await
        .unwrap();
        // Keep these short-lived test runtimes out of the bridge's connection cache.
        Some(pool.open_live_connection().await.unwrap())
    }

    async fn invoke_script(
        conn: &mut RedisLiveConnection,
        key: &str,
        op: &str,
        lease_id: &str,
        tokens: i64,
        max_rpm: i64,
    ) -> AdmissionState {
        let values = admission_script()
            .key(key)
            .arg(op)
            .arg(-1)
            .arg(max_rpm)
            .arg(10)
            .arg(1)
            .arg(tokens)
            .arg(lease_id)
            .arg(600_000)
            .arg(tokens)
            .invoke_async(conn)
            .await
            .unwrap();
        parse_admission_state(values).unwrap()
    }

    async fn redis_time(conn: &mut RedisLiveConnection) -> (i64, i64) {
        let (seconds, micros): (i64, i64) = redis::cmd("TIME").query_async(conn).await.unwrap();
        (seconds * 1_000 + micros / 1_000, seconds / 60)
    }

    // Age stored accounting instead of waiting for a real minute boundary.
    async fn age_window(conn: &mut RedisLiveConnection, key: &str, lease_id: &str) {
        let (_, epoch) = redis_time(conn).await;
        let field = format!("l:{lease_id}");
        let lease: String = redis::cmd("HGET")
            .arg(key)
            .arg(&field)
            .query_async(conn)
            .await
            .unwrap();
        let mut parts: Vec<String> = lease.split(':').map(str::to_owned).collect();
        parts[3] = (epoch - 1).to_string();
        redis::cmd("HSET")
            .arg(key)
            .arg("e")
            .arg(epoch - 1)
            .arg(field)
            .arg(parts.join(":"))
            .query_async::<i64>(conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn window_rollover_carries_live_reservations_until_finish() {
        let Some(mut conn) = live_connection().await else {
            return;
        };
        for op in ["settle", "cancel", "expire"] {
            let key = RedisPool::admission_key(&uuid::Uuid::new_v4().to_string());
            assert!(
                invoke_script(&mut conn, &key, "reserve", "old", 10, 2)
                    .await
                    .allowed
            );
            for _ in 0..2 {
                age_window(&mut conn, &key, "old").await;
                for _ in 0..2 {
                    let denied = invoke_script(&mut conn, &key, "reserve", "new", 1, 2).await;
                    assert!(!denied.allowed, "live TPM must survive rollover ({op})");
                    assert_eq!((denied.parallel, denied.rpm, denied.tpm), (1, 0, 10));
                }
            }
            if op == "expire" {
                let (_, epoch) = redis_time(&mut conn).await;
                redis::cmd("HSET")
                    .arg(&key)
                    .arg("l:old")
                    .arg(format!("1:0:10:{epoch}:0"))
                    .query_async::<i64>(&mut conn)
                    .await
                    .unwrap();
            }
            let finish_op = if op == "expire" { "cancel" } else { op };
            let finished = invoke_script(&mut conn, &key, finish_op, "old", 4, 2).await;
            let tpm = if op == "settle" { 4 } else { 0 };
            assert_eq!((finished.parallel, finished.rpm, finished.tpm), (0, 0, tpm));
            let admitted = invoke_script(&mut conn, &key, "reserve", "new", 10 - tpm, 2).await;
            assert!(admitted.allowed, "finish must release the carried estimate");
            assert_eq!((admitted.parallel, admitted.rpm, admitted.tpm), (1, 1, 10));
            let repeated = invoke_script(&mut conn, &key, finish_op, "old", 4, 2).await;
            assert_eq!(
                repeated, admitted,
                "duplicate finish must leave other leases intact"
            );
            age_window(&mut conn, &key, "new").await;
            let carried = invoke_script(&mut conn, &key, "reserve", "third", 5, 2).await;
            assert!(!carried.allowed);
            assert_eq!(
                (carried.parallel, carried.rpm, carried.tpm),
                (1, 0, 10 - tpm),
                "only outstanding TPM should carry into the next window"
            );
            redis::cmd("DEL")
                .arg(&key)
                .query_async::<i64>(&mut conn)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn same_window_commands_preserve_settled_usage() {
        let Some(mut conn) = live_connection().await else {
            return;
        };
        let key = RedisPool::admission_key(&uuid::Uuid::new_v4().to_string());
        assert!(
            invoke_script(&mut conn, &key, "reserve", "old", 10, 2)
                .await
                .allowed
        );
        age_window(&mut conn, &key, "old").await;
        assert!(
            !invoke_script(&mut conn, &key, "reserve", "new", 1, 2)
                .await
                .allowed
        );
        let settled = invoke_script(&mut conn, &key, "settle", "old", 4, 2).await;
        assert_eq!((settled.parallel, settled.rpm, settled.tpm), (0, 0, 4));
        let admitted = invoke_script(&mut conn, &key, "reserve", "new", 6, 2).await;
        assert!(admitted.allowed);
        assert_eq!((admitted.parallel, admitted.rpm, admitted.tpm), (1, 1, 10));
        let denied = invoke_script(&mut conn, &key, "reserve", "third", 1, 2).await;
        assert!(
            !denied.allowed,
            "settled usage must survive another command"
        );
        assert_eq!((denied.parallel, denied.rpm, denied.tpm), (1, 1, 10));
        let cancelled = invoke_script(&mut conn, &key, "cancel", "new", 0, 2).await;
        assert_eq!(
            (cancelled.parallel, cancelled.rpm, cancelled.tpm),
            (0, 0, 4)
        );
        redis::cmd("DEL")
            .arg(&key)
            .query_async::<i64>(&mut conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn admission_uses_redis_time_for_windows_and_lease_expiry() {
        let Some(mut conn) = live_connection().await else {
            return;
        };
        let key = RedisPool::admission_key(&uuid::Uuid::new_v4().to_string());
        let (before, first_epoch) = redis_time(&mut conn).await;
        assert!(
            invoke_script(&mut conn, &key, "reserve", "lease", 0, 1)
                .await
                .allowed
        );
        let (epoch, lease): (i64, String) = redis::cmd("HMGET")
            .arg(&key)
            .arg(&["e", "l:lease"])
            .query_async(&mut conn)
            .await
            .unwrap();
        let (after, last_epoch) = redis_time(&mut conn).await;
        assert!(
            (first_epoch..=last_epoch).contains(&epoch),
            "admission must use the Redis window: {epoch}"
        );
        let expiry: i64 = lease.rsplit(':').next().unwrap().parse().unwrap();
        assert!((before + 600_000..=after + 600_000).contains(&expiry));
        redis::cmd("DEL")
            .arg(&key)
            .query_async::<i64>(&mut conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn window_rollover_does_not_charge_old_arrivals_again() {
        let Some(mut conn) = live_connection().await else {
            return;
        };
        for op in ["settle", "cancel", "expire"] {
            let key = RedisPool::admission_key(&uuid::Uuid::new_v4().to_string());
            assert!(
                invoke_script(&mut conn, &key, "reserve", "old", 0, 1)
                    .await
                    .allowed
            );
            age_window(&mut conn, &key, "old").await;
            let admitted = invoke_script(&mut conn, &key, "reserve", "new", 0, 1).await;
            assert!(
                admitted.allowed,
                "old arrivals must not consume new RPM ({op})"
            );
            assert_eq!((admitted.parallel, admitted.rpm, admitted.tpm), (2, 1, 0));
            if op == "expire" {
                let (_, epoch) = redis_time(&mut conn).await;
                redis::cmd("HSET")
                    .arg(&key)
                    .arg("l:old")
                    .arg(format!("1:0:0:{epoch}:0"))
                    .query_async::<i64>(&mut conn)
                    .await
                    .unwrap();
            }
            let finish_op = if op == "expire" { "cancel" } else { op };
            let finished = invoke_script(&mut conn, &key, finish_op, "old", 0, 1).await;
            assert_eq!((finished.parallel, finished.rpm, finished.tpm), (1, 1, 0));
            assert!(
                !invoke_script(&mut conn, &key, "reserve", "third", 0, 1)
                    .await
                    .allowed
            );
            redis::cmd("DEL")
                .arg(&key)
                .query_async::<i64>(&mut conn)
                .await
                .unwrap();
        }
    }

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
                ttl_ms: 1_000,
            })
            .await
            .expect_err("no-op Redis must not fail-open admission");
        assert!(err.to_string().contains("unavailable"));
    }
}
