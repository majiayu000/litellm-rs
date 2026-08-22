## Performance Characteristics

| Strategy | Selection Time | Memory Overhead | Best For |
|----------|---------------|-----------------|----------|
| SimpleShuffle | O(n) | Low | General use |
| RoundRobin | O(n) | Low | Even distribution |
| LeastBusy | O(n) | Medium | High concurrency |
| LatencyBased | O(n) | Medium | Latency-sensitive |
| PriorityBased | O(n) | Low | Priority-based routing |
| UsageBased | O(n) | High | Quota management |
| RateLimitAware | O(n) | Medium | High volume |

---

## Best Practices

### 1. Always Enable Health Tracking

```rust
// Good - health-aware routing
let healthy_providers: Vec<_> = providers
    .iter()
    .filter(|p| health_tracker.is_healthy(p.name()))
    .collect();

// Bad - ignores health status
let provider = providers.first().unwrap();
```

### 2. Implement Graceful Degradation

```rust
// Good - fallback to any available provider
if healthy.is_empty() {
    // Return degraded provider instead of failing
    return providers.first().cloned();
}

// Bad - fails immediately
if healthy.is_empty() {
    return None;
}
```

### 3. Record Metrics for All Operations

```rust
// Good - tracks all outcomes
match result {
    Ok(_) => {
        health_tracker.record_success(provider.name());
        latency_tracker.record(provider.name(), elapsed);
    }
    Err(e) => {
        health_tracker.record_failure(provider.name(), &e.to_string());
        if let ProviderError::RateLimit { retry_after, .. } = e {
            rate_limit_tracker.mark_limited(provider.name(), retry_after.unwrap_or(60));
        }
    }
}
```

### 4. Use Atomic Operations

```rust
// Good - lock-free counter
self.current_index.fetch_add(1, Ordering::SeqCst)

// Bad - requires mutex
let mut guard = self.current_index.lock().unwrap();
*guard += 1;
```
