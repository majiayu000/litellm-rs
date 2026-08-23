## Performance Characteristics

All strategies run over an immutable `Vec<RoutingContext>` (Copy structs, built in one
pass per selection — `strategy_impl.rs:16-27`), so there is no shared mutable state to
lock during selection. Verified complexity from `src/core/router/strategy_impl.rs`:

| Strategy | Function | Selection Complexity | Notes |
|----------|----------|----------------------|-------|
| SimpleShuffle | `weighted_random_from_context` | O(n) | One RNG draw + cumulative weight walk |
| RoundRobin | `round_robin_from_context` | O(1) after counter | Per-model `DashMap<String, AtomicUsize>` |
| LeastBusy | `least_busy_from_context` | O(n) | Random tie-break via reservoir sampling |
| LatencyBased | `lowest_latency_from_context` | O(n) | Two passes: pool average then minimum |
| PriorityBased | `lowest_priority_from_context` | O(n) | Simple minimum scan |
| UsageBased | `lowest_usage_from_context` | O(n) | TPM percentage comparison |
| RateLimitAware | `rate_limit_aware_from_context` | O(n) | min(TPM distance, RPM distance) |

Candidate filtering (health, cooldown, RPM/TPM/parallel limits) is a separate O(n) pass
in `selection.rs:226-373` that all strategies share. Reservation is one atomic increment;
the RAII release on `DeploymentLease` drop is one atomic decrement. Router-wide counters
(`provider_selected_count`, `strategy_used_count`, `fallback_triggered_count`) are
`AtomicU64`s exposed via `Router::routing_metrics()` (`unified.rs:38-45,369-375`).

---

## Best Practices

### 1. Use the Lease API

```rust
// Good - lease releases active_requests on drop, even on early return or cancel
let lease = router.select_deployment_lease(&model)?;
let deployment = lease.clone_deployment();
// ... execute ...

// Avoid - deprecated ID-returning selectors skip RAII release
let id = router.select_deployment(&model)?; // #[deprecated(since = "0.5.0")]
```

### 2. Feed Outcomes Back to the Router

Selection quality depends on recorded state (`record_success` /
`record_failure_with_reason` update latency averages, failure counts, and the cooldown
breaker). The built-in execution methods (`execute_with_selected_deployment`,
`execute_with_selected_deployment_retry`) do this automatically — prefer them over manual
select + call.

### 3. Give Data-Driven Strategies Data

UsageBased and RateLimitAware treat unlimited deployments as 0% used / max distance, so
they ignore deployments without limits. Set `tpm` / `rpm` / `max_concurrent_requests` on
providers (YAML) for these strategies to differentiate; set `weight` for SimpleShuffle
and `priority` for PriorityBased tiering.

### 4. Register Fallbacks Explicitly and Validate

Fallbacks only trigger if configured on `FallbackConfig`; build with
`add_general` / `add_context_window` / `add_content_policy` / `add_rate_limit` and call
`validate()` to catch cycles before startup. Tune chain depth with `max_fallbacks`
(default 5) and retry behavior with `num_retries`.

### 5. Keep Cooldown Tuning Consistent With Retries

Retries use `CooldownReason::ConsecutiveFailures` so a deployment is not cooled down
mid-retry unless it exceeds `allowed_fails` within a minute
(`execute_impl.rs:144-157`). If you raise `num_retries`, check that
`circuit_breaker.failure_threshold` and `min_requests` still make sense, otherwise every
retry storm trips the breaker.
