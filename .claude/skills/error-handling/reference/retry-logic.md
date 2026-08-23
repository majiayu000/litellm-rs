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

```rust
pub async fn execute_with_retry<F, T, E>(
    operation: F,
    max_retries: u32,
    base_delay: Duration,
) -> Result<T, E>
where
    F: Fn() -> Pin<Box<dyn Future<Output = Result<T, E>> + Send>>,
    E: std::fmt::Debug,
{
    let mut attempts = 0;
    let mut last_error;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                last_error = e;

                if attempts >= max_retries {
                    break;
                }

                // Exponential backoff
                let delay = base_delay * 2u32.pow(attempts - 1);
                tokio::time::sleep(delay).await;
            }
        }
    }

    Err(last_error)
}
```
