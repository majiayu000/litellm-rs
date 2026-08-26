# Best Practices

### 1. Use Appropriate Metric Types

The runtime has no `prometheus` crate — HTTP metrics are plain atomics rendered as text. Pick
the shape by semantics, not by available types:

```rust
// Good - monotonically increasing totals are counters
static HTTP_METRICS: HttpMetricsRegistry = /* AtomicU64 per total */;
gateway_http_requests_total / gateway_http_request_errors_total

// Good - values that can go down are gauges (sysinfo-backed)
gateway_memory_usage_bytes / gateway_cpu_usage_percent

// Bad - a counter for memory usage (it decreases)
```

Latency is recorded as `_sum` + `_count` pairs (`gateway_http_request_duration_ms_sum` /
`_count`). If you add a distribution-valued metric, add matching `_bucket` series too —
without them Prometheus cannot compute quantiles.

### 2. Include Request Context in Logs

The request id comes from `RequestIdMiddleware`; read it from the `x-request-id`
response header contract rather than inventing a new field.

```rust
// Good - includes request context
info!(
    request_id = %request_id,
    provider = %provider_name,
    model = %model,
    "Request completed"
);

// Bad - missing context
info!("Request completed");
```

### 3. Use Bounded Label Cardinality

Follow the pattern already enforced in `src/server/middleware/metrics.rs`:
`unpriced_model_bucket` maps free-form model names onto a fixed bucket set, and
policy/outcome labels are whitelisted to known values with an `unknown` fallback.

```rust
// Good - bounded buckets over free-form input
let model_bucket = unpriced_model_bucket(model); // embedding|image|audio|rerank|...|other

// Bad - raw high-cardinality label values
labels = ["request_id", "user_id", "model"]  // unbounded series explosion
```

Only the free-form `provider` label is user-controlled today; it is escaped at render time
(`escape_prometheus_label`). Keep any new free-form label behind the same bounding treatment.

### 4. Graceful Degradation

Telemetry failures must never break request handling — this is how the current code behaves:

```rust
// Good - metrics middleware never panics: atomics + Relaxed ordering,
// and rendering locks a Mutex only to read a snapshot.
// src/server/middleware/metrics.rs
static HTTP_METRICS: HttpMetricsRegistry = /* atomic counters */;

// Good - exporter failures are sampled and logged, requests still succeed.
// src/core/integrations/observability/opentelemetry/integration_impl.rs
warn!(backend = backend.kind(), "Callback backend initialization failed; continuing without it");

// Bad - unwrap/panic inside the request path on telemetry errors
REGISTRY.register(metric.clone()).unwrap();
```

Also gate optional instrumentation on config so deployments can disable it cleanly:
`Condition::new(metrics_enabled, MetricsMiddleware)` in `src/server/http.rs`, and
`monitoring.callbacks.backends` entries that fail init log a warning and are skipped.
