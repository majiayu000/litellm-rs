## Health Tracking

### HealthTracker Implementation

```rust
pub struct HealthTracker {
    health_states: DashMap<&'static str, HealthState>,
    check_interval: Duration,
}

#[derive(Clone)]
struct HealthState {
    status: HealthStatus,
    last_check: Instant,
    consecutive_failures: u32,
    last_error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthTracker {
    pub fn new(check_interval: Duration) -> Self {
        Self {
            health_states: DashMap::new(),
            check_interval,
        }
    }

    pub fn is_healthy(&self, provider: &'static str) -> bool {
        self.health_states
            .get(provider)
            .map(|s| s.status != HealthStatus::Unhealthy)
            .unwrap_or(true) // Assume healthy if unknown
    }

    pub fn record_success(&self, provider: &'static str) {
        self.health_states
            .entry(provider)
            .and_modify(|s| {
                s.status = HealthStatus::Healthy;
                s.consecutive_failures = 0;
                s.last_check = Instant::now();
                s.last_error = None;
            })
            .or_insert(HealthState {
                status: HealthStatus::Healthy,
                last_check: Instant::now(),
                consecutive_failures: 0,
                last_error: None,
            });
    }

    pub fn record_failure(&self, provider: &'static str, error: &str) {
        self.health_states
            .entry(provider)
            .and_modify(|s| {
                s.consecutive_failures += 1;
                s.last_check = Instant::now();
                s.last_error = Some(error.to_string());

                // Update status based on failure count
                s.status = match s.consecutive_failures {
                    1..=2 => HealthStatus::Degraded,
                    _ => HealthStatus::Unhealthy,
                };
            })
            .or_insert(HealthState {
                status: HealthStatus::Degraded,
                last_check: Instant::now(),
                consecutive_failures: 1,
                last_error: Some(error.to_string()),
            });
    }

    pub async fn run_health_checks(&self, providers: &[Arc<dyn LLMProvider>]) {
        loop {
            for provider in providers {
                let status = provider.health_check().await;
                match status {
                    HealthStatus::Healthy => self.record_success(provider.name()),
                    _ => self.record_failure(provider.name(), "Health check failed"),
                }
            }
            tokio::time::sleep(self.check_interval).await;
        }
    }
}
```

---

## Fallback Chains

### FallbackChain Implementation

```rust
pub struct FallbackChain {
    primary: Arc<dyn LLMProvider>,
    fallbacks: Vec<Arc<dyn LLMProvider>>,
    health_tracker: Arc<HealthTracker>,
}

impl FallbackChain {
    pub async fn execute<F, T>(&self, operation: F) -> Result<T, ProviderError>
    where
        F: Fn(&dyn LLMProvider) -> Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send>>,
    {
        // Try primary first
        if self.health_tracker.is_healthy(self.primary.name()) {
            match operation(self.primary.as_ref()).await {
                Ok(result) => {
                    self.health_tracker.record_success(self.primary.name());
                    return Ok(result);
                }
                Err(e) if e.should_fallback() => {
                    self.health_tracker.record_failure(self.primary.name(), &e.to_string());
                }
                Err(e) => return Err(e),
            }
        }

        // Try fallbacks in order
        let mut last_error = None;
        for fallback in &self.fallbacks {
            if !self.health_tracker.is_healthy(fallback.name()) {
                continue;
            }

            match operation(fallback.as_ref()).await {
                Ok(result) => {
                    self.health_tracker.record_success(fallback.name());
                    return Ok(result);
                }
                Err(e) if e.should_fallback() => {
                    self.health_tracker.record_failure(fallback.name(), &e.to_string());
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ProviderError::routing_error(
                "gateway",
                vec![self.primary.name().to_string()],
                "All providers failed",
            )
        }))
    }
}
```

---

