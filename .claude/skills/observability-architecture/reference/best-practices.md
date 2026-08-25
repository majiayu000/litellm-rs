## Best Practices

### 1. Use Appropriate Metric Types

```rust
// Good - Counter for monotonically increasing values
pub static ref REQUESTS_TOTAL: IntCounter = IntCounter::new(...).unwrap();

// Good - Gauge for values that can go up and down
pub static ref ACTIVE_CONNECTIONS: IntGauge = IntGauge::new(...).unwrap();

// Good - Histogram for latency/size distributions
pub static ref REQUEST_LATENCY: Histogram = Histogram::new(...).unwrap();

// Bad - Counter for active connections (can decrease)
pub static ref ACTIVE_CONNECTIONS: IntCounter = IntCounter::new(...).unwrap();
```

### 2. Include Request Context in Logs

```rust
// Good - includes request context
info!(
    request_id = %request_id,
    user_id = %user_id,
    provider = %provider_name,
    "Request completed"
);

// Bad - missing context
info!("Request completed");
```

### 3. Use Proper Cardinality

```rust
// Good - limited label cardinality
pub static ref ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
    Opts::new("errors_total", "Total errors"),
    &["provider", "error_type"]  // Low cardinality
).unwrap();

// Bad - high cardinality labels
pub static ref ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
    Opts::new("errors_total", "Total errors"),
    &["request_id", "user_id"]  // Infinite cardinality!
).unwrap();
```

### 4. Graceful Degradation

```rust
// Good - handle missing telemetry gracefully
if let Err(e) = METRICS_REGISTRY.register(metric.clone()) {
    tracing::warn!("Failed to register metric: {}", e);
    // Continue without metrics
}

// Bad - panic on telemetry errors
METRICS_REGISTRY.register(metric.clone()).unwrap();
```
