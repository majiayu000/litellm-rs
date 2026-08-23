## Contents

- Metric Inventory
- How Metrics Are Recorded
- Rendering the /metrics Endpoint
- Unpriced-Model Metrics
- Deprecated Library Surfaces

## Metric Inventory

Every series exposed on `GET /metrics` in the current code. All names are rendered by hand —
there is no `prometheus` crate dependency.

| Series | Kind | Labels | Source |
|---|---|---|---|
| `gateway_uptime_seconds` | counter | — | `src/server/routes/health.rs` |
| `gateway_memory_usage_bytes` | gauge | — (0 without the `metrics` feature) | `src/server/routes/health.rs` |
| `gateway_cpu_usage_percent` | gauge | — (0.0 without the `metrics` feature) | `src/server/routes/health.rs` |
| `gateway_providers_total` | gauge | — (configured provider count) | `src/server/routes/health.rs` |
| `gateway_http_requests_total` | counter | — (`/metrics` scrapes excluded) | `src/server/middleware/metrics.rs` |
| `gateway_http_request_errors_total` | counter | — (status code >= 400) | `src/server/middleware/metrics.rs` |
| `gateway_http_responses_total` | counter | `class="1xx".."5xx"` | `src/server/middleware/metrics.rs` |
| `gateway_http_request_duration_ms_sum` | counter | — (sum of durations, milliseconds) | `src/server/middleware/metrics.rs` |
| `gateway_http_request_duration_ms_count` | counter | — | `src/server/middleware/metrics.rs` |
| `gateway_unpriced_events_total` | counter | `provider`, `model_bucket`, `policy`, `outcome` | `src/server/middleware/metrics.rs` |
| `gateway_unpriced_spend_total` | counter | same as events (USD) | `src/server/middleware/metrics.rs` |
| `rate_limiter_degraded_total` | counter | `operation`, `mode` | `src/core/rate_limiter/limiter.rs` |

Notes:

- Latency is exposed only as `_sum`/`_count` over millisecond values — there are **no**
  histogram buckets and therefore no `_bucket` series or `histogram_quantile()` support.
  Average latency is `gateway_http_request_duration_ms_sum / gateway_http_request_duration_ms_count`.
- The only series without the `gateway_` prefix is `rate_limiter_degraded_total`.
- There are no per-provider request/token/cost/health series on `/metrics`. Provider-level
  telemetry goes through the callback integrations instead.

## How Metrics Are Recorded

`MetricsMiddleware` keeps process-local atomics in a `static HTTP_METRICS: HttpMetricsRegistry`
(`AtomicU64` per value). Each request's recorder runs when the response body completes or drops
(`MetricsResponseBody` with `PinnedDrop`), so streaming responses are counted after the stream
finishes. Requests to `/metrics` are never recorded (`should_record_request_path`).

```rust
// src/server/middleware/metrics.rs (abridged)
static HTTP_METRICS: HttpMetricsRegistry = HttpMetricsRegistry {
    requests_total: AtomicU64::new(0),
    errors_total: AtomicU64::new(0),
    // status_1xx_total .. status_5xx_total, latency_micros_sum, latency_ms_count ...
};

fn record_http_metrics(status_code: u16, latency: Duration) {
    HTTP_METRICS.requests_total.fetch_add(1, Ordering::Relaxed);
    if status_code >= 400 {
        HTTP_METRICS.errors_total.fetch_add(1, Ordering::Relaxed);
    }
    // ... classify into status_1xx..5xx buckets
}
```

The middleware is attached conditionally on config:

```rust
// src/server/http.rs
let metrics_enabled = cfg.gateway.monitoring.metrics.enabled;
// ...
App::new()
    // ...
    .wrap(Condition::new(metrics_enabled, MetricsMiddleware))
```

## Rendering the /metrics Endpoint

`GET /metrics` is mounted by `routes::health::configure_routes` on the main HTTP server and
returns `text/plain; version=0.0.4; charset=utf-8`. The body concatenates three renderers:

```rust
// src/server/routes/health.rs
async fn metrics(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let metrics = render_prometheus_metrics(
        state.config.load().providers().len(),
        &MetricsMiddleware::render_prometheus(),
        &crate::core::rate_limiter::render_degraded_metrics(),
    );
    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(metrics))
}
```

`render_prometheus_metrics` emits uptime/memory/CPU/providers, then embeds the middleware block
and the rate-limiter block verbatim.

## Unpriced-Model Metrics

Unpriced-model policy events are recorded via `record_unpriced_event` /
`record_unpriced_spend` into a `BTreeMap<UnpricedMetricLabels, UnpricedMetricValue>`.
Cardinality is bounded by design: `model_bucket` comes from `unpriced_model_bucket`
(fixed set such as `embedding`, `image`, `audio`, `rerank`, `claude`, `gemini`, `llama`,
`mistral`, `openai_text`, `other`), while `policy` is limited to `reject` / `allow_unpriced` /
`unknown` and `outcome` to `reject_preflight` / `candidate_excluded` / `fallback_settled` /
`unknown`. Only the free-form `provider` label is escaped at render time.

## Deprecated Library Surfaces

`core::observability::MetricsCollector` (which renders legacy `litellm_requests_total`,
`litellm_errors_total`, `litellm_cache_hits_total`, `litellm_cache_misses_total`,
`litellm_provider_health`) is a deprecated library-only compatibility surface scheduled for
removal in 0.7 — it does not feed `/metrics`. The wired observability handle is
`RuntimeObservability = core::integrations::CallbackDispatcher`
(`src/core/subsystem_registry.rs`). Do not build new alerts on `litellm_*` series.
