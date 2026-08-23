## Contents

- Health Tracking
- Fallback Chains

## Health Tracking

There is no `HealthTracker` struct. Health lives on each deployment as an atomic enum
value plus cooldown bookkeeping, driven by two mechanisms: a passive circuit breaker fed
by request outcomes, and optional active probes.

### Per-Deployment Health State

`HealthStatus` (`src/core/router/deployment.rs:109-117`) is stored as an `AtomicU8` in
`DeploymentState.health`: `Unknown=0, Healthy=1, Degraded=2, Unhealthy=3, Cooldown=4`.
Related fields: `cooldown_until` (unix secs), `consecutive_successes`,
`fails_this_minute`, `rpm_current`/`tpm_current`, `probe_unhealthy`
(`deployment.rs:210-252`). Selection skips deployments where `is_in_cooldown()` or
`!is_healthy()`.

### Circuit Breaker (passive)

`Router::record_failure` (`src/core/router/unified.rs:617-643`) trips cooldown only when
BOTH thresholds hold:

```rust
if fails >= self.config.allowed_fails            // default 3 (YAML: circuit_breaker.failure_threshold)
    && total_this_minute >= self.config.min_requests  // default 10
{
    deployment.enter_cooldown(self.config.cooldown_time_secs);  // YAML: recovery_timeout
}
```

`record_failure_with_reason` (`unified.rs:646-692`) refines this by `CooldownReason`:
`RateLimit | AuthError | NotFound | Timeout | Manual` cool down immediately;
`ConsecutiveFailures` uses the threshold check above; `HighFailureRate` needs
`min_requests` total and a >50% failure rate. The reason is inferred from the
`ProviderError` by `infer_cooldown_reason` (`execution.rs:124-159`), e.g. HTTP 429 ->
`RateLimit`, 401 -> `AuthError`.

Recovery is half-open: expired cooldown demotes to `Degraded`; `record_success` counts
`consecutive_successes` and promotes Degraded -> Healthy once it reaches
`success_threshold` (default 3) (`unified.rs:594-610`). A background task
(`start_minute_reset_task`, `unified.rs:764-772`) resets per-minute counters every 60s so
failure counts cannot accumulate across minutes.

### Active Probes

`Router::start_configured_health_checks` (`src/core/router/health_probe.rs:34`) is gated
on `enable_pre_call_checks` (YAML `load_balancer.health_check_enabled`). Deployments with
a `HealthCheckPolicy` are grouped per provider into a `ProbeGroup`; one tokio task per
provider loops forever (`run_probe_loop`):

- Probe = GET of the configured custom `endpoint` (accepting any of `expected_codes`) or,
  without an endpoint, the native `provider.health_check()`; bounded by the deployment timeout.
- Success -> all deployments in the group become Healthy, next delay `interval_secs`.
- Failure -> `Degraded` below `failure_threshold`, `Unhealthy` at it; once unhealthy the
  next delay is `recovery_timeout_secs`.
- Writes never override a request-driven `Cooldown`; the observed-unhealthy flag is kept
  separately in `probe_unhealthy` and re-applied after cooldown expires
  (`update_probe_health`, `health_probe.rs:220-254`).

---

## Fallback Chains

There is no `FallbackChain` struct. Fallbacks are **model-keyed lists** in
`FallbackConfig` (`src/core/router/fallback.rs:66-79`): four
`RwLock<HashMap<String, Vec<String>>>` maps — `general`, `context_window`,
`content_policy`, `rate_limit` — each mapping a model name to ordered fallback model
names.

```rust
let fallback_config = FallbackConfig::new()
    .add_general("gpt-4o", vec!["gpt-4o-mini".into(), "claude-3-5-sonnet-latest".into()])
    .add_context_window("gpt-4o", vec!["gpt-4o".into()])   // larger-context variant
    .add_rate_limit("gpt-4o", vec!["gpt-4o-mini".into()]);
fallback_config.validate()?; // DFS over all four maps, Err(Vec<String>) lists cycles
```

### Type Selection and Resolution

- `FallbackType` (`fallback.rs:22-31`): `General`, `ContextWindow`, `ContentPolicy`,
  `RateLimit`.
- `infer_fallback_type` (`execution.rs:105-119`): `ContextLengthExceeded` ->
  ContextWindow, `ContentFiltered` -> ContentPolicy, `RateLimit` -> RateLimit, everything
  else -> General.
- `Router::get_fallbacks` (`unified.rs:702-716`) resolves aliases first, reads the
  type-specific list, and falls back to the `General` list when that is empty.

### Execution Flow

`Router::execute_with_selected_deployment` (`src/core/router/execute_impl.rs:339-428`)
implements the full chain:

1. Build `models_to_try` = resolved model + its fallbacks
   (`get_models_with_fallbacks_for_snapshot`), deduplicated to prevent cycles, capped at
   `1 + max_fallbacks` entries — with the default `max_fallbacks = 5`, the original model
   plus up to 5 fallback models.
2. For each model, run the retry loop: up to `num_retries + 1` attempts
   (`num_retries` default 3), with `RetryPolicy.decide` choosing whether/when to retry
   (streaming stage, idempotency, deadline aware — `retry_policy.rs`). Failures feed the
   cooldown circuit breaker above; budget-scope errors exclude just that deployment and
   reselect.
3. If all retries fail, advance to the next fallback model (incrementing
   `fallback_triggered_count`), until a model succeeds or the list is exhausted.
4. Success returns `ExecutionResult<T>` (`fallback.rs:43-56`) with `result`,
   `deployment_id`, `attempts` (total across retries+fallbacks), `model_used`,
   `used_fallback`, and `latency_us`.
