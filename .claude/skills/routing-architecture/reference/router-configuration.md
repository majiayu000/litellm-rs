## Router Configuration

### YAML Configuration

```yaml
routing:
  strategy: "latency_based"  # Options: simple_shuffle, round_robin, least_busy, latency_based, priority_based, usage_based, rate_limit_aware

  health_check:
    enabled: true
    interval_seconds: 30
    consecutive_failures_threshold: 3

  fallback:
    enabled: true
    max_retries: 3
    retry_delay_ms: 1000

  load_balancing:
    enabled: true
    weights:
      openai: 0.5
      anthropic: 0.3
      azure: 0.2

  rate_limit:
    track_per_provider: true
    default_retry_after_seconds: 60
```

### Router Factory

```rust
pub fn create_router(config: &RoutingConfig, providers: Vec<Arc<dyn LLMProvider>>) -> Box<dyn Router> {
    let health_tracker = Arc::new(HealthTracker::new(
        Duration::from_secs(config.health_check.interval_seconds),
    ));

    match config.strategy.as_str() {
        "simple_shuffle" => Box::new(SimpleShuffleRouter::new(providers, health_tracker)),
        "round_robin" => Box::new(RoundRobinRouter::new(providers, health_tracker)),
        "least_busy" => Box::new(LeastBusyRouter::new(providers, health_tracker)),
        "latency_based" => Box::new(LatencyBasedRouter::new(providers, health_tracker)),
        "priority_based" => Box::new(PriorityBasedRouter::new(providers, health_tracker)),
        "usage_based" => Box::new(UsageBasedRouter::new(providers, health_tracker)),
        "rate_limit_aware" => Box::new(RateLimitAwareRouter::new(providers, health_tracker)),
        _ => Box::new(SimpleShuffleRouter::new(providers, health_tracker)),
    }
}
```

---
