//! Single-key Lua deployment circuit state (cluster-safe: `KEYS[1]` only).

use super::pool::{RedisLiveConnection, RedisPool};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use std::sync::OnceLock;

const CIRCUIT_SCRIPT: &str = r#"
local op = ARGV[1]
local now = tonumber(ARGV[2]) or 0
local epoch = tonumber(ARGV[3]) or 0
local token = ARGV[4]
local allowed = tonumber(ARGV[5]) or 3
local min_req = tonumber(ARGV[6]) or 10
local cooldown = tonumber(ARGV[7]) or 5
local success_th = tonumber(ARGV[8]) or 3
local reason = tonumber(ARGV[9]) or 0

local function load_state()
  local f = tonumber(redis.call('HGET', KEYS[1], 'f') or '0') or 0
  local r = tonumber(redis.call('HGET', KEYS[1], 'r') or '0') or 0
  local tot = tonumber(redis.call('HGET', KEYS[1], 'tot') or '0') or 0
  local fail = tonumber(redis.call('HGET', KEYS[1], 'fail') or '0') or 0
  local opened = tonumber(redis.call('HGET', KEYS[1], 'opened') or '0') or 0
  local consec = tonumber(redis.call('HGET', KEYS[1], 'consec') or '0') or 0
  local e = tonumber(redis.call('HGET', KEYS[1], 'e') or '0') or 0
  local h = tonumber(redis.call('HGET', KEYS[1], 'h') or '1') or 1
  local owner = redis.call('HGET', KEYS[1], 'owner')
  if not owner then owner = '' end
  local own_until = tonumber(redis.call('HGET', KEYS[1], 'own_until') or '0') or 0
  return f, r, tot, fail, opened, consec, e, h, owner, own_until
end

local function save(f, r, tot, fail, opened, consec, e, h, owner, own_until)
  redis.call('HSET', KEYS[1], 'f', f, 'r', r, 'tot', tot, 'fail', fail,
    'opened', opened, 'consec', consec, 'e', e, 'h', h, 'owner', owner,
    'own_until', own_until)
end

local f, r, tot, fail, opened, consec, e, h, owner, own_until = load_state()
local function claim_owner()
  if owner == '' or own_until <= now then
    owner = token
    own_until = now + cooldown
    if own_until < now + 1 then own_until = now + 1 end
    h = 2
  end
end
local function clear_owner()
  owner = ''
  own_until = 0
end
if e ~= epoch then
  f = 0
  r = 0
  e = epoch
end

if op == 'fail' then
  tot = tot + 1
  fail = fail + 1
  f = f + 1
  consec = 0
  if opened > now then
    h = 4
  elseif opened > 0 then
    opened = now + cooldown
    clear_owner()
    h = 4
  else
    local trip = 0
    if reason == 1 then
      trip = 1
    elseif reason == 2 then
      if tot >= min_req and (fail * 100 / tot) > 50 then trip = 1 end
    elseif f >= allowed and (r + f) >= min_req then
      trip = 1
    end
    if trip == 1 then
      opened = now + cooldown
      clear_owner()
      h = 4
    elseif h == 1 or h == 0 then
      h = 2
    end
  end
elseif op == 'ok' then
  tot = tot + 1
  r = r + 1
  consec = consec + 1
  if opened > now then
    h = 4
  elseif opened > 0 then
    if owner == token or owner == '' or own_until <= now then
      if consec >= success_th then
        opened = 0
        clear_owner()
        h = 1
      else
        claim_owner()
      end
    end
  elseif h == 2 and consec >= success_th then
    h = 1
  end
end

local status = 0
local owned = 1
if opened > now then
  status = 1
  owned = 0
elseif opened > 0 then
  status = 2
  claim_owner()
  if owner == token then owned = 1 else owned = 0 end
end

if op ~= 'observe' or opened > 0 then
  save(f, r, tot, fail, opened, consec, e, h, owner, own_until)
end
return {status, opened, f, consec, h, owned}
"#;

const STATUS_OPEN: i64 = 1;
const STATUS_HALF: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CircuitState {
    pub status: i64,
    pub opened_until: i64,
    pub fails: i64,
    pub consecutive_successes: i64,
    pub health: i64,
    pub owned: bool,
}

impl CircuitState {
    pub(crate) fn blocks_selection(self) -> bool {
        self.status == STATUS_OPEN || (self.status == STATUS_HALF && !self.owned)
    }
}

pub(crate) struct CircuitArgs<'a> {
    pub op: &'a str,
    pub now_secs: i64,
    pub window_epoch: i64,
    pub token: &'a str,
    pub allowed_fails: i64,
    pub min_requests: i64,
    pub cooldown_secs: i64,
    pub success_threshold: i64,
    pub reason: i64,
}

fn circuit_script() -> &'static redis::Script {
    static SCRIPT: OnceLock<redis::Script> = OnceLock::new();
    SCRIPT.get_or_init(|| redis::Script::new(CIRCUIT_SCRIPT))
}

fn circuit_runtime_connections() -> &'static tokio::sync::Mutex<HashMap<String, RedisLiveConnection>>
{
    static CACHE: OnceLock<tokio::sync::Mutex<HashMap<String, RedisLiveConnection>>> =
        OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

async fn connection_on_current_runtime(pool: &RedisPool) -> Result<RedisLiveConnection> {
    let cache_key = format!("{}|{}", pool.config.url, pool.config.cluster);
    {
        let cache = circuit_runtime_connections().lock().await;
        if let Some(conn) = cache.get(&cache_key) {
            return Ok(conn.clone());
        }
    }
    let conn = pool.open_live_connection().await?;
    let mut cache = circuit_runtime_connections().lock().await;
    Ok(cache.entry(cache_key).or_insert(conn).clone())
}

fn parse_circuit_state(values: Vec<i64>) -> Result<CircuitState> {
    if values.len() != 6 {
        return Err(GatewayError::Storage(format!(
            "Unexpected Redis circuit result length: {}",
            values.len()
        )));
    }
    Ok(CircuitState {
        status: values[0],
        opened_until: values[1].max(0),
        fails: values[2].max(0),
        consecutive_successes: values[3].max(0),
        health: values[4],
        owned: values[5] == 1,
    })
}

impl RedisPool {
    pub(crate) fn circuit_key(deployment_id: &str) -> String {
        format!("litellm-rs:circuit:v1:{deployment_id}")
    }

    pub(crate) async fn circuit_invoke(
        &self,
        key: &str,
        args: CircuitArgs<'_>,
    ) -> Result<CircuitState> {
        if self.noop_mode {
            return Err(GatewayError::Storage(
                "circuit redis backend is unavailable".to_string(),
            ));
        }

        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| GatewayError::Internal("Redis semaphore closed".to_string()))?;
        let mut conn = connection_on_current_runtime(self).await?;

        let values: Vec<i64> = circuit_script()
            .key(key)
            .arg(args.op)
            .arg(args.now_secs)
            .arg(args.window_epoch)
            .arg(args.token)
            .arg(args.allowed_fails)
            .arg(args.min_requests)
            .arg(args.cooldown_secs)
            .arg(args.success_threshold)
            .arg(args.reason)
            .invoke_async(&mut conn)
            .await
            .map_err(GatewayError::from)?;
        parse_circuit_state(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::storage::RedisConfig;

    #[test]
    fn parses_circuit_state() {
        let open = parse_circuit_state(vec![1, 10, 3, 0, 4, 0]).unwrap();
        assert!(open.blocks_selection());
        assert_eq!(open.opened_until, 10);

        let half_owned = parse_circuit_state(vec![2, 5, 3, 1, 2, 1]).unwrap();
        assert!(!half_owned.blocks_selection());
        assert!(half_owned.owned);

        let half_foreign = parse_circuit_state(vec![2, 5, 3, 0, 2, 0]).unwrap();
        assert!(half_foreign.blocks_selection());
        assert!(parse_circuit_state(vec![1, 0]).is_err());
    }

    #[tokio::test]
    async fn noop_redis_pool_fails_closed_on_circuit_observe() {
        let pool = RedisPool::new(&RedisConfig {
            enabled: false,
            ..RedisConfig::default()
        })
        .await
        .expect("disabled Redis should create a no-op pool");

        let err = pool
            .circuit_invoke(
                "litellm-rs:circuit:v1:noop-test",
                CircuitArgs {
                    op: "observe",
                    now_secs: 1,
                    window_epoch: 0,
                    token: "tok",
                    allowed_fails: 3,
                    min_requests: 1,
                    cooldown_secs: 5,
                    success_threshold: 1,
                    reason: 0,
                },
            )
            .await
            .expect_err("no-op Redis must not fail-open circuit observe");
        assert!(err.to_string().contains("unavailable"));
    }
}
