//! Single-key Lua budget lease operations (cluster-safe: `KEYS[1]` only).
//!
//! Expiry and period reset release outstanding capacity, but retain a pending
//! marker until the reservation is settled or explicitly cancelled. Consuming
//! either a live lease or a pending marker makes ordinary completion idempotent
//! without storing a receipt for every successful request. Durable Responses
//! additionally retain a receipt for replay after SQL recovery.

use super::pool::{RedisLiveConnection, RedisPool};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Bound live plus pending reservation identities for each budget. Durable
/// Responses receipts have a separate replay contract and do not use this cap.
pub(crate) const MAX_UNSETTLED_BUDGET_LEASES: usize = 65_536;
const BUDGET_PHASE_TIMEOUT: Duration = Duration::from_secs(5);

fn deadline_error(phase: &str) -> GatewayError {
    GatewayError::Unavailable(format!("Redis budget {phase} deadline exceeded"))
}

const BUDGET_LEASE_SCRIPT: &str = r#"
local op = ARGV[1]
local now = tonumber(ARGV[2]) or 0
local period_epoch = tonumber(ARGV[3]) or -1
local max_unsettled = tonumber(ARGV[9]) or 0

local function release_leases()
  local fields = redis.call('HGETALL', KEYS[1])
  for i = 1, #fields, 2 do
    if string.sub(fields[i], 1, 2) == 'l:' then
      redis.call('HSET', KEYS[1], 'p:' .. string.sub(fields[i], 3), fields[i + 1])
      redis.call('HDEL', KEYS[1], fields[i])
    end
  end
end

local function seed(seed_committed)
  local epoch = period_epoch
  if epoch < 0 then epoch = 0 end
  redis.call('HSETNX', KEYS[1], 'c', seed_committed)
  redis.call('HSETNX', KEYS[1], 'o', 0)
  redis.call('HSETNX', KEYS[1], 'e', epoch)
end

local function read_state()
  local committed = tonumber(redis.call('HGET', KEYS[1], 'c') or '0') or 0
  local outstanding = tonumber(redis.call('HGET', KEYS[1], 'o') or '0') or 0
  local epoch = tonumber(redis.call('HGET', KEYS[1], 'e') or '0') or 0
  return committed, outstanding, epoch
end

local function write_state(committed, outstanding, epoch)
  redis.call('HSET', KEYS[1], 'c', committed, 'o', outstanding, 'e', epoch)
end

local function maybe_period_reset()
  if period_epoch < 0 then
    return
  end
  local _, _, epoch = read_state()
  -- A late finish carries its reservation's epoch and must not rewind the period.
  if period_epoch > epoch then
    release_leases()
    write_state(0, 0, period_epoch)
  end
end

local function reclaim()
  local committed, outstanding, epoch = read_state()
  local fields = redis.call('HGETALL', KEYS[1])
  local unsettled = 0
  for i = 1, #fields, 2 do
    local field = fields[i]
    local kind = string.sub(field, 1, 2)
    if kind == 'l:' then
      local amount, expiry, lease_epoch = string.match(fields[i + 1], '^(%d+):(%d+):(%-?%d+)$')
      amount = tonumber(amount)
      expiry = tonumber(expiry)
      lease_epoch = tonumber(lease_epoch)
      if amount ~= nil and expiry ~= nil and expiry <= now then
        if lease_epoch == epoch then
          outstanding = outstanding - amount
          if outstanding < 0 then outstanding = 0 end
        end
        -- Reclaim only capacity. A late actual charge must still be accepted.
        -- If a pending field already exists, its scan entry counts it once.
        unsettled = unsettled + redis.call(
          'HSET', KEYS[1], 'p:' .. string.sub(field, 3), fields[i + 1]
        )
        redis.call('HDEL', KEYS[1], field)
      elseif amount == nil or expiry == nil or lease_epoch == nil then
        redis.call('HDEL', KEYS[1], field)
      else
        unsettled = unsettled + 1
      end
    elseif kind == 'p:' then
      unsettled = unsettled + 1
    end
  end
  redis.call('HSET', KEYS[1], 'o', outstanding)
  return committed, outstanding, epoch, unsettled
end

seed(tonumber(ARGV[6]) or 0)
maybe_period_reset()
local committed, outstanding, epoch, unsettled = reclaim()

if op == 'reset' then
  local force = tonumber(ARGV[5]) or 0
  if force == 1 then
    release_leases()
    local new_epoch = epoch
    if period_epoch >= 0 then new_epoch = period_epoch end
    write_state(0, 0, new_epoch)
    return {1, 0, 0}
  end
  return {1, committed, outstanding}
end

if op == 'reserve' then
  -- Unfinished identities cannot be aged out without losing late actual cost.
  -- Stop accepting new work at capacity; terminal operations remain available.
  if unsettled >= max_unsettled then
    return {-2, committed, outstanding}
  end
  local amount = tonumber(ARGV[4]) or 0
  local max = tonumber(ARGV[5]) or 0
  if committed + outstanding + amount > max then
    return {0, committed, outstanding}
  end
  outstanding = outstanding + amount
  local lease_id = ARGV[7]
  local ttl = tonumber(ARGV[8]) or 0
  if ttl < 1 then ttl = 1 end
  write_state(committed, outstanding, epoch)
  redis.call(
    'HSET',
    KEYS[1],
    'l:' .. lease_id,
    tostring(amount) .. ':' .. tostring(now + ttl) .. ':' .. tostring(epoch)
  )
  return {1, committed, outstanding}
end

-- Durable Responses retries may arrive after the reservation expires. Keep the
-- receipt with the budget counter; SQL acknowledges only after this atomic step.
if op == 'settle_response' then
  local receipt = 'r:' .. ARGV[7]
  if redis.call('HEXISTS', KEYS[1], receipt) == 1 then
    return {0, committed, outstanding}
  end
  local actual = tonumber(ARGV[5]) or 0
  local field = 'l:' .. ARGV[7]
  local lease = redis.call('HGET', KEYS[1], field)
  if lease then
    local amount, _, lease_epoch = string.match(lease, '^(%d+):(%d+):(%-?%d+)$')
    if tonumber(lease_epoch) == epoch then
      outstanding = math.max(0, outstanding - (tonumber(amount) or 0))
    end
    redis.call('HDEL', KEYS[1], field)
  end
  redis.call('HDEL', KEYS[1], 'p:' .. ARGV[7])
  committed = committed + actual
  write_state(committed, outstanding, epoch)
  redis.call('HSET', KEYS[1], receipt, actual)
  return {1, committed, outstanding}
end

if op == 'settle' then
  local reserved = tonumber(ARGV[4]) or 0
  local actual = tonumber(ARGV[5]) or 0
  local field = 'l:' .. ARGV[7]
  local pending = 'p:' .. ARGV[7]
  local lease = redis.call('HGET', KEYS[1], field)
  if not lease and redis.call('HEXISTS', KEYS[1], pending) == 0 then
    -- Already settled/cancelled, or not a reservation created by this backend.
    return {1, committed, outstanding}
  end
  if lease then
    local amount, _, lease_epoch = string.match(lease, '^(%d+):(%d+):(%-?%d+)$')
    amount = tonumber(amount) or reserved
    lease_epoch = tonumber(lease_epoch)
    if lease_epoch == epoch then
      outstanding = outstanding - amount
      if outstanding < 0 then outstanding = 0 end
    end
    redis.call('HDEL', KEYS[1], field)
  end
  redis.call('HDEL', KEYS[1], pending)
  -- The outstanding hold belongs to its reservation epoch. Actual spend is
  -- charged to the current counter epoch, matching the in-process backend.
  committed = committed + actual
  write_state(committed, outstanding, epoch)
  return {1, committed, outstanding}
end

if op == 'cancel' then
  local reserved = tonumber(ARGV[4]) or 0
  local field = 'l:' .. ARGV[7]
  local lease = redis.call('HGET', KEYS[1], field)
  if lease then
    local amount, _, lease_epoch = string.match(lease, '^(%d+):(%d+):(%-?%d+)$')
    amount = tonumber(amount) or reserved
    lease_epoch = tonumber(lease_epoch)
    if lease_epoch == epoch then
      outstanding = outstanding - amount
      if outstanding < 0 then outstanding = 0 end
    end
    redis.call('HDEL', KEYS[1], field)
  end
  -- Explicit cancellation is terminal even after capacity has expired. A
  -- racing settle/cancel is resolved atomically by whichever executes first.
  redis.call('HDEL', KEYS[1], 'p:' .. ARGV[7])
  write_state(committed, outstanding, epoch)
  return {1, committed, outstanding}
end

return {-1, committed, outstanding}
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BudgetLeaseState {
    pub allowed: bool,
    pub committed: i64,
    pub outstanding: i64,
}

struct BudgetLeaseArgs<'a> {
    op: &'a str,
    now_ms: i64,
    period_epoch: i64,
    amount: i64,
    max_or_actual_or_force: i64,
    seed_committed: i64,
    lease_id: &'a str,
    ttl_ms: i64,
}

pub(crate) struct BudgetReserveArgs<'a> {
    pub key: &'a str,
    pub amount: i64,
    pub max: i64,
    pub seed_committed: i64,
    pub period_epoch: i64,
    pub lease_id: &'a str,
    pub now_ms: i64,
    pub ttl_ms: i64,
}

#[derive(Default)]
struct BudgetRuntimeConnection {
    live: tokio::sync::Mutex<Option<Arc<RedisLiveConnection>>>,
}

fn budget_runtime_connections()
-> &'static tokio::sync::Mutex<HashMap<String, Arc<BudgetRuntimeConnection>>> {
    static CACHE: OnceLock<tokio::sync::Mutex<HashMap<String, Arc<BudgetRuntimeConnection>>>> =
        OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

async fn connection_on_current_runtime(pool: &RedisPool) -> Arc<BudgetRuntimeConnection> {
    let cache_key = format!("{}|{}", pool.config.url, pool.config.cluster);
    let mut cache = budget_runtime_connections().lock().await;
    Arc::clone(cache.entry(cache_key).or_default())
}

impl BudgetRuntimeConnection {
    async fn connect(&self, pool: &RedisPool) -> Result<Arc<RedisLiveConnection>> {
        // Serialize connection creation only for this endpoint. Healthy script
        // invocations clone the connection and release the lock before I/O.
        let mut live = tokio::time::timeout(BUDGET_PHASE_TIMEOUT, self.live.lock())
            .await
            .map_err(|_| deadline_error("connection lock"))?;
        if let Some(connection) = live.as_ref() {
            return Ok(Arc::clone(connection));
        }
        // The Cluster client may perform multiple discovery/retry steps. Bound
        // the whole setup, in addition to its per-connection transport timeout.
        let connection = Arc::new(
            tokio::time::timeout(
                Duration::from_secs(pool.config.connection_timeout).min(BUDGET_PHASE_TIMEOUT),
                pool.open_budget_connection(BUDGET_PHASE_TIMEOUT),
            )
            .await
            .map_err(|_| deadline_error("connection setup"))??,
        );
        *live = Some(Arc::clone(&connection));
        Ok(connection)
    }

    async fn invalidate(&self, failed: &Arc<RedisLiveConnection>) {
        let Ok(mut live) = tokio::time::timeout(BUDGET_PHASE_TIMEOUT, self.live.lock()).await
        else {
            tracing::warn!("Redis budget connection invalidation deadline exceeded");
            return;
        };
        // A late error from an old generation must not discard a replacement
        // that another request has already opened.
        if live
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, failed))
        {
            *live = None;
        }
    }

    async fn invoke(
        &self,
        pool: &RedisPool,
        key: &str,
        args: BudgetLeaseArgs<'_>,
    ) -> Result<BudgetLeaseState> {
        let live = self.connect(pool).await?;
        let mut conn = live.as_ref().clone();
        let script = redis::Script::new(BUDGET_LEASE_SCRIPT);
        let mut invocation = script.prepare_invoke();
        invocation
            .key(key)
            .arg(args.op)
            .arg(args.now_ms)
            .arg(args.period_epoch)
            .arg(args.amount)
            .arg(args.max_or_actual_or_force)
            .arg(args.seed_committed)
            .arg(args.lease_id)
            .arg(args.ttl_ms)
            .arg(MAX_UNSETTLED_BUDGET_LEASES);
        let values: redis::RedisResult<Vec<i64>> =
            match tokio::time::timeout(BUDGET_PHASE_TIMEOUT, invocation.invoke_async(&mut conn))
                .await
            {
                Ok(values) => values,
                Err(_) => {
                    self.invalidate(&live).await;
                    // Timeout does not prove the write was unapplied. Keep the
                    // original identity visible for reconciliation; never replay.
                    tracing::warn!(
                        operation = args.op,
                        lease_id = args.lease_id,
                        "Redis budget command timed out; write outcome is uncertain"
                    );
                    return Err(deadline_error("command"));
                }
            };
        match values {
            Ok(values) => parse_budget_lease_state(values),
            Err(error) => {
                if error.is_unrecoverable_error()
                    || error.is_timeout()
                    || error.redirect_node().is_some()
                {
                    self.invalidate(&live).await;
                }
                tracing::warn!(
                    operation = args.op,
                    lease_id = args.lease_id,
                    error = %error,
                    "Redis budget command failed; write outcome may be uncertain"
                );
                // The server may have applied a write before its reply was
                // lost. Fail this operation closed; reconnect on the next one
                // without automatically replaying an ambiguous reservation.
                Err(GatewayError::from(error))
            }
        }
    }
}

fn parse_budget_lease_state(values: Vec<i64>) -> Result<BudgetLeaseState> {
    if values.len() != 3 {
        return Err(GatewayError::Storage(format!(
            "Unexpected Redis budget-lease result length: {}",
            values.len()
        )));
    }
    if values[0] == -2 {
        return Err(GatewayError::Unavailable(format!(
            "Redis budget reservation capacity reached ({MAX_UNSETTLED_BUDGET_LEASES} unfinished leases)"
        )));
    }
    if values[0] < 0 {
        return Err(GatewayError::Storage(
            "Redis budget-lease script returned an error status".to_string(),
        ));
    }
    Ok(BudgetLeaseState {
        allowed: values[0] == 1,
        committed: values[1].max(0),
        outstanding: values[2].max(0),
    })
}

impl RedisPool {
    pub(crate) fn budget_lease_key(scope: &str, name: &str) -> String {
        format!("litellm-rs:budget:v1:{scope}:{name}")
    }

    async fn invoke_budget_lease(
        &self,
        key: &str,
        args: BudgetLeaseArgs<'_>,
    ) -> Result<BudgetLeaseState> {
        if self.noop_mode {
            return Err(GatewayError::Storage(
                "budget redis backend is unavailable".to_string(),
            ));
        }

        let _permit =
            tokio::time::timeout(BUDGET_PHASE_TIMEOUT, self.semaphore.clone().acquire_owned())
                .await
                .map_err(|_| deadline_error("connection permit"))?
                .map_err(|_| GatewayError::Internal("Redis semaphore closed".to_string()))?;
        connection_on_current_runtime(self)
            .await
            .invoke(self, key, args)
            .await
    }

    pub(crate) async fn budget_reserve(
        &self,
        args: BudgetReserveArgs<'_>,
    ) -> Result<BudgetLeaseState> {
        self.invoke_budget_lease(
            args.key,
            BudgetLeaseArgs {
                op: "reserve",
                now_ms: args.now_ms,
                period_epoch: args.period_epoch,
                amount: args.amount,
                max_or_actual_or_force: args.max,
                seed_committed: args.seed_committed,
                lease_id: args.lease_id,
                ttl_ms: args.ttl_ms,
            },
        )
        .await
    }

    pub(crate) async fn budget_settle(
        &self,
        key: &str,
        reserved: i64,
        actual: i64,
        period_epoch: i64,
        lease_id: &str,
        now_ms: i64,
    ) -> Result<BudgetLeaseState> {
        self.invoke_budget_lease(
            key,
            BudgetLeaseArgs {
                op: "settle",
                now_ms,
                period_epoch,
                amount: reserved,
                max_or_actual_or_force: actual,
                seed_committed: 0,
                lease_id,
                ttl_ms: 0,
            },
        )
        .await
    }

    pub(crate) async fn budget_settle_response(
        &self,
        key: &str,
        reserved: i64,
        actual: i64,
        period_epoch: i64,
        lease_id: &str,
        now_ms: i64,
    ) -> Result<BudgetLeaseState> {
        self.invoke_budget_lease(
            key,
            BudgetLeaseArgs {
                op: "settle_response",
                now_ms,
                period_epoch,
                amount: reserved,
                max_or_actual_or_force: actual,
                seed_committed: 0,
                lease_id,
                ttl_ms: 0,
            },
        )
        .await
    }

    pub(crate) async fn budget_cancel(
        &self,
        key: &str,
        reserved: i64,
        period_epoch: i64,
        lease_id: &str,
        now_ms: i64,
    ) -> Result<BudgetLeaseState> {
        self.invoke_budget_lease(
            key,
            BudgetLeaseArgs {
                op: "cancel",
                now_ms,
                period_epoch,
                amount: reserved,
                max_or_actual_or_force: 0,
                seed_committed: 0,
                lease_id,
                ttl_ms: 0,
            },
        )
        .await
    }

    pub(crate) async fn budget_reset(
        &self,
        key: &str,
        period_epoch: i64,
        now_ms: i64,
        force: bool,
    ) -> Result<BudgetLeaseState> {
        self.invoke_budget_lease(
            key,
            BudgetLeaseArgs {
                op: "reset",
                now_ms,
                period_epoch,
                amount: 0,
                max_or_actual_or_force: if force { 1 } else { 0 },
                seed_committed: 0,
                lease_id: "",
                ttl_ms: 0,
            },
        )
        .await
    }
}

#[cfg(test)]
#[path = "budget_connection_tests.rs"]
mod connection_tests;

#[cfg(test)]
#[path = "budget_deadline_tests.rs"]
mod deadline_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::storage::RedisConfig;

    #[test]
    fn parses_budget_lease_state() {
        let allowed = parse_budget_lease_state(vec![1, 3, 4]).unwrap();
        assert!(allowed.allowed);
        assert_eq!(allowed.committed, 3);
        assert_eq!(allowed.outstanding, 4);

        let denied = parse_budget_lease_state(vec![0, 10, 0]).unwrap();
        assert!(!denied.allowed);
        assert!(matches!(
            parse_budget_lease_state(vec![-2, 0, 0]),
            Err(GatewayError::Unavailable(_))
        ));
        assert!(parse_budget_lease_state(vec![-1, 0, 0]).is_err());
        assert!(parse_budget_lease_state(vec![1, 0]).is_err());
    }

    async fn capacity_script_step(
        conn: &mut RedisLiveConnection,
        key: &str,
        (op, now_ms, period_epoch, amount, max_or_actual_or_force, lease_id): (
            &str,
            i64,
            i64,
            i64,
            i64,
            &str,
        ),
    ) -> Result<BudgetLeaseState> {
        // Keep this test's driver on its own connection. The production budget
        // connection cache belongs to the long-lived budget I/O runtime.
        let values = redis::Script::new(BUDGET_LEASE_SCRIPT)
            .key(key)
            .arg(op)
            .arg(now_ms)
            .arg(period_epoch)
            .arg(amount)
            .arg(max_or_actual_or_force)
            .arg(0)
            .arg(lease_id)
            .arg(1_000)
            .arg(MAX_UNSETTLED_BUDGET_LEASES)
            .invoke_async(conn)
            .await
            .map_err(GatewayError::from)?;
        parse_budget_lease_state(values)
    }

    async fn expect_reservation_capacity_error(
        conn: &mut RedisLiveConnection,
        key: &str,
        now_ms: i64,
        period_epoch: i64,
    ) {
        let error = capacity_script_step(
            conn,
            key,
            ("reserve", now_ms, period_epoch, 1, 100, "denied"),
        )
        .await
        .expect_err("unfinished reservation metadata must have a hard capacity");
        assert!(matches!(error, GatewayError::Unavailable(_)));
    }

    #[tokio::test]
    async fn reservation_capacity_preserves_late_settlement_and_releases_terminal_slots() {
        let Ok(url) = std::env::var("REDIS_URL") else {
            assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
            return;
        };
        let pool = RedisPool::new(&RedisConfig {
            url,
            enabled: true,
            allow_degraded: false,
            ..RedisConfig::default()
        })
        .await
        .unwrap();
        let key = RedisPool::budget_lease_key("provider", &uuid::Uuid::new_v4().to_string());
        let mut conn = pool.open_live_connection().await.unwrap();

        // Seed a pre-upgrade hash one identity over capacity in one command,
        // avoiding quadratic fixture setup through thousands of full scans.
        let _: i64 = redis::Script::new(
            r#"
            redis.call('HSET', KEYS[1], 'c', 0, 'o', 2, 'e', 10,
              'l:live', '1:999999999:10', 'l:expires', '1:1100:10',
              'p:late', '10:100:10', 'p:excess', '1:100:10', 'r:durable', 7)
            for i = 1, tonumber(ARGV[1]) - 3 do
              redis.call('HSET', KEYS[1], 'p:orphan-' .. i, '1:100:10')
            end
            return 1
            "#,
        )
        .key(&key)
        .arg(MAX_UNSETTLED_BUDGET_LEASES)
        .invoke_async(&mut conn)
        .await
        .unwrap();

        expect_reservation_capacity_error(&mut conn, &key, 1_000, 10).await;
        let state = capacity_script_step(&mut conn, &key, ("cancel", 1_001, 10, 1, 0, "excess"))
            .await
            .unwrap();
        assert_eq!((state.committed, state.outstanding), (0, 2));
        expect_reservation_capacity_error(&mut conn, &key, 1_002, 10).await;
        // Expiry releases monetary capacity, but must still count its pending identity.
        expect_reservation_capacity_error(&mut conn, &key, 1_100, 10).await;

        let late_now = 2 * 24 * 60 * 60 * 1_000;
        for force in [false, true] {
            let state = capacity_script_step(
                &mut conn,
                &key,
                ("reset", late_now, 11, 0, i64::from(force), ""),
            )
            .await
            .unwrap();
            assert_eq!((state.committed, state.outstanding), (0, 0));
            expect_reservation_capacity_error(&mut conn, &key, late_now, 11).await;
        }
        let fields: usize = redis::cmd("HLEN")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .unwrap();
        assert_eq!(fields, MAX_UNSETTLED_BUDGET_LEASES + 4);

        // A reservation older than 24 hours remains settleable. A repeated
        // finish neither charges twice nor releases an additional identity slot.
        for _ in 0..2 {
            let state =
                capacity_script_step(&mut conn, &key, ("settle", late_now + 1, 10, 10, 4, "late"))
                    .await
                    .unwrap();
            assert_eq!((state.committed, state.outstanding), (4, 0));
        }
        for lease_id in ["new-cancelled", "new-live"] {
            let state = capacity_script_step(
                &mut conn,
                &key,
                ("reserve", late_now + 2, 11, 3, 100, lease_id),
            )
            .await
            .expect("a terminal operation must free one reservation slot");
            assert_eq!((state.committed, state.outstanding), (4, 3));
            expect_reservation_capacity_error(&mut conn, &key, late_now + 2, 11).await;
            if lease_id == "new-cancelled" {
                capacity_script_step(
                    &mut conn,
                    &key,
                    ("cancel", late_now + 2, 11, 3, 0, lease_id),
                )
                .await
                .unwrap();
            }
        }

        for expected_applied in [true, false] {
            let state = capacity_script_step(
                &mut conn,
                &key,
                ("settle_response", late_now + 3, 10, 1, 2, "orphan-1"),
            )
            .await
            .unwrap();
            assert_eq!(state.allowed, expected_applied);
            assert_eq!((state.committed, state.outstanding), (6, 3));
        }
        let state = capacity_script_step(
            &mut conn,
            &key,
            ("reserve", late_now + 4, 11, 2, 100, "after-response"),
        )
        .await
        .expect("durable receipts do not consume unfinished reservation capacity");
        assert_eq!((state.committed, state.outstanding), (6, 5));
        expect_reservation_capacity_error(&mut conn, &key, late_now + 4, 11).await;
        let duplicate =
            capacity_script_step(&mut conn, &key, ("settle", late_now + 5, 10, 10, 4, "late"))
                .await
                .unwrap();
        assert_eq!((duplicate.committed, duplicate.outstanding), (6, 5));
        let receipts: (i64, i64, Option<i64>) = redis::cmd("HMGET")
            .arg(&key)
            .arg(&["r:durable", "r:orphan-1", "r:late"])
            .query_async(&mut conn)
            .await
            .unwrap();
        assert_eq!(receipts, (7, 2, None));
        redis::cmd("DEL")
            .arg(&key)
            .query_async::<i64>(&mut conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn lease_finish_is_idempotent_after_settlement_expiry_and_rollover() {
        let Ok(url) = std::env::var("REDIS_URL") else {
            assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
            return;
        };
        let pool = RedisPool::new(&RedisConfig {
            url,
            enabled: true,
            allow_degraded: false,
            ..RedisConfig::default()
        })
        .await
        .unwrap();
        let mut conn = pool.open_live_connection().await.unwrap();

        for scenario in [
            "settled",
            "cancelled",
            "expired",
            "expiry_at_settlement",
            "cancel_after_expiry",
            "rollover",
            "manual_reset",
            "unknown",
        ] {
            let key = RedisPool::budget_lease_key("provider", &uuid::Uuid::new_v4().to_string());
            let steps = match scenario {
                "settled" => vec![
                    ("reserve", 1_000, 10, 10, 10, "old", 0, 10),
                    ("settle", 1_001, 10, 10, 4, "old", 4, 0),
                    ("settle", 1_002, 10, 10, 4, "old", 4, 0),
                    ("cancel", 1_003, 10, 10, 0, "old", 4, 0),
                ],
                "cancelled" => vec![
                    ("reserve", 1_000, 10, 10, 10, "old", 0, 10),
                    ("cancel", 1_001, 10, 10, 0, "old", 0, 0),
                    ("reserve", 1_002, 10, 10, 10, "new", 0, 10),
                    ("settle", 1_003, 10, 10, 4, "old", 0, 10),
                ],
                "expired" => vec![
                    ("reserve", 1_000, 10, 10, 10, "old", 0, 10),
                    ("reserve", 1_101, 10, 10, 10, "new", 0, 10),
                    ("settle", 1_102, 10, 10, 4, "old", 4, 10),
                    ("settle", 1_103, 10, 10, 4, "old", 4, 10),
                    ("settle", 1_104, 10, 10, 3, "new", 7, 0),
                ],
                "expiry_at_settlement" => vec![
                    ("reserve", 1_000, -1, 10, 10, "old", 0, 10),
                    ("settle", 1_100, -1, 10, 12, "old", 12, 0),
                    ("settle", 1_101, -1, 10, 12, "old", 12, 0),
                ],
                "cancel_after_expiry" => vec![
                    ("reserve", 1_000, 10, 10, 10, "old", 0, 10),
                    ("reserve", 1_100, 10, 10, 10, "new", 0, 10),
                    ("cancel", 1_101, 10, 10, 0, "old", 0, 10),
                    ("cancel", 1_102, 10, 10, 0, "old", 0, 10),
                    ("settle", 1_103, 10, 10, 4, "old", 0, 10),
                ],
                "rollover" => vec![
                    ("reserve", 1_000, 10, 10, 10, "old", 0, 10),
                    ("reserve", 1_001, 11, 10, 10, "new", 0, 10),
                    ("settle", 1_002, 10, 10, 4, "old", 4, 10),
                    ("cancel", 1_003, 10, 10, 0, "old", 4, 10),
                    ("settle", 1_004, 11, 10, 3, "new", 7, 0),
                    ("reset", 1_005, 12, 0, 0, "", 0, 0),
                    ("settle", 1_006, 10, 10, 4, "old", 0, 0),
                ],
                "manual_reset" => vec![
                    ("reserve", 1_000, 10, 10, 10, "old", 0, 10),
                    ("reset", 1_001, 10, 0, 1, "", 0, 0),
                    ("reserve", 1_002, 10, 10, 10, "new", 0, 10),
                    ("settle", 1_003, 10, 10, 4, "old", 4, 10),
                    ("settle", 1_004, 10, 10, 3, "new", 7, 0),
                ],
                _ => vec![
                    ("reserve", 1_000, 10, 10, 10, "new", 0, 10),
                    ("settle", 1_001, 10, 10, 4, "old", 0, 10),
                ],
            };
            for (op, now, epoch, amount, actual_or_max, lease, committed, outstanding) in steps {
                let values = redis::Script::new(BUDGET_LEASE_SCRIPT)
                    .key(&key)
                    .arg(op)
                    .arg(now)
                    .arg(epoch)
                    .arg(amount)
                    .arg(actual_or_max)
                    .arg(0)
                    .arg(lease)
                    .arg(100)
                    .arg(MAX_UNSETTLED_BUDGET_LEASES)
                    .invoke_async(&mut conn)
                    .await
                    .unwrap();
                let state = parse_budget_lease_state(values).unwrap();
                assert!(state.allowed, "{scenario}: {op}");
                assert_eq!(
                    (state.committed, state.outstanding),
                    (committed, outstanding),
                    "{scenario}: {op} {lease}"
                );
            }
            let pending: bool = redis::cmd("HEXISTS")
                .arg(&key)
                .arg("p:old")
                .query_async(&mut conn)
                .await
                .unwrap();
            assert!(
                !pending,
                "{scenario}: terminal operation must consume pending marker"
            );
            let ttl: i64 = redis::cmd("PTTL")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .unwrap();
            assert_eq!(
                ttl, -1,
                "budget state must not expire with its capacity lease"
            );
            redis::cmd("DEL")
                .arg(&key)
                .query_async::<i64>(&mut conn)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn noop_redis_pool_fails_closed_on_budget_reserve() {
        let pool = RedisPool::new(&RedisConfig {
            enabled: false,
            ..RedisConfig::default()
        })
        .await
        .expect("disabled Redis should create a no-op pool");

        let err = pool
            .budget_reserve(BudgetReserveArgs {
                key: "litellm-rs:budget:v1:provider:noop-test",
                amount: 1,
                max: 10,
                seed_committed: 0,
                period_epoch: 0,
                lease_id: "lease",
                now_ms: 1,
                ttl_ms: 1_000,
            })
            .await
            .expect_err("no-op Redis must not fail-open budget reservations");
        assert!(err.to_string().contains("unavailable"));
    }
}
