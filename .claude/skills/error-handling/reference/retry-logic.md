## Retry Logic

### Legacy Helpers on ProviderError

```rust
// src/core/providers/unified_provider_methods.rs

/// Check if this error is retryable.
/// DEPRECATED since 0.6.0: use RetryPolicy::decide with ProviderFailureFacts.
pub fn is_retryable(&self) -> bool;

/// Get retry delay in seconds (legacy hint).
pub fn retry_delay(&self) -> Option<u64>;
```

There are no `retry_after()` or `should_fallback()` methods on
`ProviderError`. Structured retry data lives in the `RateLimit` variant
fields (`retry_after`, `rpm_limit`, `tpm_limit`) and in
`ProviderFailureFacts` (`src/core/providers/failure.rs`), which both helpers
delegate to.

### What the Legacy Helpers Report

Per `ProviderFailureFacts::from_error` in `src/core/providers/failure.rs`:

| Variant | Retryable | Delay |
|---------|-----------|-------|
| `RateLimit` | Yes | `retry_after` field |
| `Network`, `Timeout` | Yes | 1s |
| `ProviderUnavailable` | Yes | 5s |
| `ContentFiltered` | Only if `potentially_retryable == Some(true)` | 10s |
| `ApiError` status 429 | Yes | 60s |
| `ApiError` status 500-599 | Yes | 3s |
| `ApiError` Bedrock-modeled 424 | Yes | 3s |
| `DeploymentError` | Yes | 5s |
| `Streaming` | Yes | 2s |
| All other variants | No | - |

### Canonical Decision Path: RetryPolicy

```rust
// src/core/router/retry_policy.rs
let decision = RetryPolicy.decide(&router_config, &error, retry_context);
if decision.should_retry {
    if let Some(delay) = decision.delay {
        tokio::time::sleep(delay).await;
    }
}
```

`RetryDecision` carries `should_retry: bool`, `delay: Option<Duration>`, and
a `RetryDecisionReason`. The policy also enforces attempt limits and a retry
budget via `RetryContext` (`attempt`, `max_attempts`,
`retry_budget_remaining`). For per-deployment schedules use
`decide_for_deployment`.

### Retry Implementation

Two similarly named APIs serve different layers:

```rust
// src/utils/net/client/utils.rs: a low-level helper for synchronous closures.
let result = ClientUtils::execute_with_retry(operation, &retry_config).await?;
```

`ClientUtils` runs the closure over `0..=config.max_retries` (one initial
attempt plus the configured retries) and sleeps between failures using the
`RetryConfig` backoff calculation. Its closure is `Fn() -> Result<T, E>` where
`E: Into<ProviderError> + Clone`; it is not an async-operation executor.

Router traffic instead uses
`Router::execute_with_selected_deployment_retry(model_name, callback)`. Its
callback receives the selected `Arc<Deployment>`, and the router applies
`RetryPolicy`, retry budgets, snapshot-safe reselection and cooldown behavior.
The older `Router::execute_with_retry` callback receives only a
`DeploymentId` and is deprecated; do not confuse either Router method with
the `ClientUtils` helper.
