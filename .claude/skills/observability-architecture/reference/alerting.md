# Alerting Integration

Rules below are keyed on the series the gateway actually exposes on `/metrics` (see
[metrics.md](metrics.md)). Every expression uses a real series name from
`src/server/middleware/metrics.rs`, `src/server/routes/health.rs`, or
`src/core/rate_limiter/limiter.rs`.

```yaml
# prometheus/alerts.yml
groups:
  - name: gateway_alerts
    rules:
      # High error rate (any response with status >= 400)
      - alert: HighErrorRate
        expr: |
          sum(rate(gateway_http_request_errors_total[5m])) /
          sum(rate(gateway_http_requests_total[5m])) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value | printf \"%.2f\" }}"

      # Server-side errors spiking
      - alert: ServerErrorsSpike
        expr: |
          sum(rate(gateway_http_responses_total{class="5xx"}[5m])) > 0.5
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "5xx responses exceed 0.5/s for 5 minutes"

      # Average latency (only _sum/_count exist; no histogram_quantile possible)
      - alert: HighAverageLatency
        expr: |
          (
            sum(rate(gateway_http_request_duration_ms_sum[5m])) /
            sum(rate(gateway_http_request_duration_ms_count[5m]))
          ) > 5000
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Mean request latency above 5 seconds"

      # Redis distributed rate limiter degraded operations
      - alert: RateLimiterDegraded
        expr: |
          sum by (operation, mode) (rate(rate_limiter_degraded_total[5m])) > 0
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Redis rate limiter degraded ({{ $labels.operation }}/{{ $labels.mode }})"

      # Unpriced-model fallback spend accumulating in USD
      - alert: UnpricedSpendGrowing
        expr: |
          sum(increase(gateway_unpriced_spend_total[1h])) > 10
        for: 0m
        labels:
          severity: warning
        annotations:
          summary: "More than $10/h of unpriced fallback spend"

      # Traffic stall (metrics scrapes themselves are excluded from counting)
      - alert: NoRequests
        expr: |
          sum(rate(gateway_http_requests_total[10m])) == 0
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "No requests received in 10 minutes"
```

What cannot be expressed today — do not write rules against these:

- **P95/P99 latency**: there are no `_bucket` series (latency is exposed only as
  `gateway_http_request_duration_ms_sum`/`_count`), so `histogram_quantile()` has no input.
- **Provider health gauges**: no per-provider metric exists. Probe readiness out-of-band by
  alerting on `GET /health/ready` returning 503 (blackbox exporter), which fails when storage
  is down, providers are unhealthy/unknown, or audit logging is unavailable. When auth is
  enabled, give the probe valid credentials; otherwise it receives 401 before readiness is
  evaluated.
- **Token/cost/provider-request counters**: none are exported by `/metrics`; provider
  request lifecycle telemetry flows through callback integrations instead.
- **Cache counters**: callbacks receive no cache-hit/miss lifecycle events or cache fields.
  Use `LLMCache::combined_stats()` programmatically or the `GET /admin/cache` status
  surface for chat and embedding cache statistics.
- Any `litellm_*` series: those names appear only in deprecated library-only code paths and
  are never served by `/metrics`.
