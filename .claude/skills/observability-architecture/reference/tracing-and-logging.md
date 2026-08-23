## Contents

- Logging
- Request IDs
- Lifecycle Exporters (Callbacks)

## Logging

The log subscriber is initialized in the binary entry point, not from the `monitoring.logging`
config section (that section is parsed and validated but has no runtime consumer today).

```rust
// src/main.rs
fn init_logging(log_level: Option<&str>) {
    #[cfg(feature = "tracing")]
    {
        tracing_subscriber::fmt()
            .with_max_level(parse_log_level(log_level))
            .with_target(false)
            .with_thread_ids(false)
            .init();
    }
    #[cfg(not(feature = "tracing"))]
    let _ = log_level;
}
```

Facts a coding agent must know:

- The subscriber is plain `tracing_subscriber::fmt()` — human-readable text, **not** JSON.
  There is no JSON formatter wired anywhere in the runtime.
- Level parsing accepts `trace|debug|warn|warning|error`, defaulting to `info`.
- Actix's own `Logger::default()` access log is also installed (`src/server/http.rs`).
- The metrics middleware emits one structured-ish event per request:
  `info!("{} {} -> {} in {:?}", method, path, status_code, response_time)`.

Emit context-rich events with the `tracing` macros; fields over string interpolation:

```rust
use tracing::{info, error};

info!(
    request_id = %request_id,
    provider = %provider_name,
    model = %model,
    status = "success",
    "Chat completion request completed"
);

error!(
    operation = "rate_limit_check",
    mode = "redis_degraded",
    error = %err,
    "Redis distributed rate limiter degraded"
);
```

## Request IDs

`RequestIdMiddleware` (`src/server/middleware/request_id.rs`) runs outermost:

- Honors an incoming non-empty `x-request-id` header; otherwise generates a UUID v4 and
  inserts it into the request headers.
- Echoes `x-request-id` on every successful response and on gateway error responses, and
  threads it through error rendering via `with_error_response_request_id`
  (`src/utils/error/gateway_error.rs`), so client-reported IDs match server log lines.

```rust
// src/server/middleware/request_id.rs (abridged)
let request_id = /* incoming x-request-id or Uuid::new_v4().to_string() */;
with_error_response_request_id(request_id.clone(), async move {
    match fut.await {
        Ok(mut res) => {
            res.headers_mut().insert("x-request-id", header_value);
            Ok(res.map_into_boxed_body())
        }
        Err(err) => { /* attach request id to the error response body + headers */ }
    }
})
.await
```

## Lifecycle Exporters (Callbacks)

There are no `opentelemetry` / `opentelemetry-otlp` crates in `Cargo.toml`. Trace export is a
custom OTLP-over-HTTP implementation behind the callback system:

- `RuntimeObservability = crate::core::integrations::CallbackDispatcher` is stored in
  `AppState`; configured integrations receive LLM start/stream/end/error lifecycle events.
- Backends are configured under `monitoring.callbacks.backends` and built at startup by
  `build_callback_runtime` (`src/server/callbacks.rs`) with an `IntegrationManager`
  (parallel dispatch, `timeout_ms`, bounded queue `queue_capacity`).
- Backend types (`CallbackBackendConfig`): `opentelemetry`, `datadog`, `langfuse`.
  Duplicate backend kinds fail validation.

OpenTelemetry backend (`src/core/integrations/observability/opentelemetry/`):

```yaml
monitoring:
  callbacks:
    backends:
      - type: opentelemetry
        config:
          enabled: true                  # must be true if listed (validation)
          endpoint: http://localhost:4317   # spans POST to {endpoint}/v1/traces
          service_name: litellm-gateway
          service_version: null
          environment: null
          resource_attributes: {}
          export_traces: true
          export_metrics: true
          batch_interval_ms: 5000
          max_batch_size: 512
          timeout_ms: 10000
          sampling_ratio: 1.0
          headers: {}                    # e.g. auth headers; use env substitution
```

Spans are modeled locally (`span.rs`: `Span`, `SpanKind`, `SpanStatus`, `AttributeValue`)
and exported as OTLP JSON by `export_spans` / `build_otlp_payload` (`exporter.rs`). Export
failures are sampled and reported without panicking — telemetry problems never take down
request handling.

The legacy `PerformanceTracer`, `LogAggregator`, and `MetricsCollector` exports under
`src/core/observability/` are deprecated library-only surfaces scheduled for removal in 0.7;
do not wire new code to them.
