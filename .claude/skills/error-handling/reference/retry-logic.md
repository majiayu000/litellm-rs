## Retry Logic

### Retryable Error Detection

```rust
impl ProviderError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimit { .. }
                | Self::Timeout { .. }
                | Self::Network { .. }
                | Self::ProviderUnavailable { .. }
        )
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimit { retry_after, .. } => {
                retry_after.map(Duration::from_secs)
            }
            Self::Timeout { .. } => Some(Duration::from_secs(1)),
            Self::Network { .. } => Some(Duration::from_millis(500)),
            Self::ProviderUnavailable { .. } => Some(Duration::from_secs(5)),
            _ => None,
        }
    }

    pub fn should_fallback(&self) -> bool {
        matches!(
            self,
            Self::ProviderUnavailable { .. }
                | Self::RateLimit { .. }
                | Self::QuotaExceeded { .. }
                | Self::ModelNotFound { .. }
        )
    }
}
```

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
