//! Single-key Lua budget lease operations (cluster-safe: `KEYS[1]` only).

use super::pool::{RedisLiveConnection, RedisPool};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use std::sync::OnceLock;

const BUDGET_LEASE_SCRIPT: &str = r#"
local op = ARGV[1]
local now = tonumber(ARGV[2]) or 0
local period_epoch = tonumber(ARGV[3]) or -1

local function delete_leases()
  local fields = redis.call('HGETALL', KEYS[1])
  for i = 1, #fields, 2 do
    if string.sub(fields[i], 1, 2) == 'l:' then
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
  if epoch ~= period_epoch then
    delete_leases()
    write_state(0, 0, period_epoch)
  end
end

local function reclaim()
  local committed, outstanding, epoch = read_state()
  local fields = redis.call('HGETALL', KEYS[1])
  for i = 1, #fields, 2 do
    local field = fields[i]
    if string.sub(field, 1, 2) == 'l:' then
      local amount, expiry, lease_epoch = string.match(fields[i + 1], '^(%d+):(%d+):(%-?%d+)$')
      amount = tonumber(amount)
      expiry = tonumber(expiry)
      lease_epoch = tonumber(lease_epoch)
      if amount ~= nil and expiry ~= nil and expiry <= now then
        if lease_epoch == epoch then
          outstanding = outstanding - amount
          if outstanding < 0 then outstanding = 0 end
        end
        redis.call('HDEL', KEYS[1], field)
      elseif amount == nil or expiry == nil or lease_epoch == nil then
        redis.call('HDEL', KEYS[1], field)
      end
    end
  end
  redis.call('HSET', KEYS[1], 'o', outstanding)
  return committed, outstanding, epoch
end

seed(tonumber(ARGV[6]) or 0)
maybe_period_reset()
local committed, outstanding, epoch = reclaim()

if op == 'reset' then
  local force = tonumber(ARGV[5]) or 0
  if force == 1 then
    delete_leases()
    local new_epoch = epoch
    if period_epoch >= 0 then new_epoch = period_epoch end
    write_state(0, 0, new_epoch)
    return {1, 0, 0}
  end
  return {1, committed, outstanding}
end

if op == 'reserve' then
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

if op == 'settle' then
  local reserved = tonumber(ARGV[4]) or 0
  local actual = tonumber(ARGV[5]) or 0
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

fn budget_runtime_connections() -> &'static tokio::sync::Mutex<HashMap<String, RedisLiveConnection>>
{
    static CACHE: OnceLock<tokio::sync::Mutex<HashMap<String, RedisLiveConnection>>> =
        OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

async fn connection_on_current_runtime(pool: &RedisPool) -> Result<RedisLiveConnection> {
    let cache_key = format!("{}|{}", pool.config.url, pool.config.cluster);
    {
        let cache = budget_runtime_connections().lock().await;
        if let Some(conn) = cache.get(&cache_key) {
            return Ok(conn.clone());
        }
    }
    let conn = pool.open_live_connection().await?;
    let mut cache = budget_runtime_connections().lock().await;
    Ok(cache.entry(cache_key).or_insert(conn).clone())
}

fn parse_budget_lease_state(values: Vec<i64>) -> Result<BudgetLeaseState> {
    if values.len() != 3 {
        return Err(GatewayError::Storage(format!(
            "Unexpected Redis budget-lease result length: {}",
            values.len()
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

        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| GatewayError::Internal("Redis semaphore closed".to_string()))?;
        let mut conn = connection_on_current_runtime(self).await?;

        let values: Vec<i64> = redis::Script::new(BUDGET_LEASE_SCRIPT)
            .key(key)
            .arg(args.op)
            .arg(args.now_ms)
            .arg(args.period_epoch)
            .arg(args.amount)
            .arg(args.max_or_actual_or_force)
            .arg(args.seed_committed)
            .arg(args.lease_id)
            .arg(args.ttl_ms)
            .invoke_async(&mut conn)
            .await
            .map_err(GatewayError::from)?;
        parse_budget_lease_state(values)
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
        assert!(parse_budget_lease_state(vec![-1, 0, 0]).is_err());
        assert!(parse_budget_lease_state(vec![1, 0]).is_err());
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
