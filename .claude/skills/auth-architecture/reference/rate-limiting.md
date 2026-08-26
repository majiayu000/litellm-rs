## Rate Limiting

### Core limiter

`RateLimiter` (`src/core/rate_limiter/limiter.rs`) keeps per-key state in a
concurrent map — per-key locking, no global mutex:

```rust
pub struct RateLimiter {
    config: RateLimitConfig,
    entries: Arc<DashMap<String, RateLimitEntry>>,  // key -> timestamps (+ tokens)
    window: Duration,                               // default 60 s
    redis: Option<Arc<dyn RedisRateLimitBackend>>,  // optional distributed backend
}
```

Three strategies (`RateLimitStrategy`, `src/config/models/rate_limit.rs`),
selected by `rate_limit.strategy`: `token_bucket` (default), `fixed_window`,
`sliding_window`. This setting selects the process-local algorithm, including a
`fail_open_local` fallback. With a live Redis backend, `check()` and
`check_and_record()` use Redis's distributed fixed-window operations regardless of this
setting. The atomic `check_and_record(key)` is the preferred entry
point; separate `check()` + `record()` has a documented TOCTOU race and
`record()` is deprecated. Results are `RateLimitResult { allowed,
current_count, limit, remaining, reset_after_secs, retry_after_secs }`.

### Distributed backend

`RateLimiter::with_redis(config, redis_pool)` adds Redis-backed enforcement for
multi-instance deployments. The live backend calls `RedisPool::rate_limit_status` /
`rate_limit_check_and_record`, both distributed fixed-window operations. On Redis
failure the limiter records a degraded-
operation metric and applies `redis_failure_mode`:

- `fail_closed` (default) — reject while Redis is unavailable.
- `fail_open_local` — fall back to process-local limits.

`RateLimitReservation` records which backend consumed the pre-auth slot. A
successful authentication releases that network-key reservation before the
inner, identity-keyed limiter runs; an authentication infrastructure `Err`
also releases it. A semantic authentication failure (`Ok` with
`success == false`) deliberately retains the reservation so rejected
credentials consume gateway rate-limit capacity.

### Configuration surface

Top-level `rate_limit:` section (not under `auth:`):

```yaml
rate_limit:
  enabled: true
  strategy: token_bucket        # alias: algorithm; local/fail_open_local path only
  default_rpm: 1000             # gateway-level RPM
  requests_per_minute: null     # LiteLLM alias; overrides default_rpm when set
  default_tpm: 100000           # parsed but NOT enforced yet
  requests_per_second: null     # parsed but NOT enforced yet
  tokens_per_minute: null       # parsed but NOT enforced yet
  burst_size: null              # parsed but NOT enforced yet
  redis_failure_mode: fail_closed
```

`effective_rpm()` = `requests_per_minute.unwrap_or(default_rpm)`. When rate
limiting is enabled, setting any unsupported nondefault field makes validation
return an error listing it; `validate-config` and normal startup therefore
fail rather than merely logging and continuing.

### Key policy and middleware

`RateLimitMiddleware` (`src/server/middleware/rate_limit.rs`) runs after
authentication. Key selection:

1. `api_key:{id}` when an API key is authenticated.
2. `user:{id}` when only a user is authenticated.
3. Network key from client IP honoring trusted proxies otherwise.

Per-request limit: `api_key.rate_limits.rpm` if present, else the configured
default (`effective_requests_per_minute`). If the global limiter is not
initialized, a capped in-process fallback store (10,000 trackers) enforces a
60 s sliding window instead.

Rejections return 429 with `Retry-After` and `X-RateLimit-Limit` headers and a
`GatewayError::RateLimit` body carrying `retry_after` / `rpm_limit`.

### Auth brute-force limiter

Distinct from request rate limiting, `AuthRateLimiter`
(`src/server/middleware/auth_rate_limiter.rs`) is a DashMap-based lockout
tracker for authentication failures: 5 failures / 300 s window → 60 s lockout
doubling per repeat (exponential backoff). See middleware-pipeline.md.
