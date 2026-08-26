## Alerting Integration

### Alert Rules (Prometheus)

```yaml
# prometheus/alerts.yml
groups:
  - name: litellm_alerts
    rules:
      # High error rate
      - alert: HighErrorRate
        expr: |
          sum(rate(litellm_errors_total[5m])) /
          sum(rate(litellm_http_requests_total[5m])) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value | printf \"%.2f\" }}%"

      # Provider unhealthy
      - alert: ProviderUnhealthy
        expr: litellm_provider_health == 0
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Provider {{ $labels.provider }} is unhealthy"

      # High latency
      - alert: HighLatency
        expr: |
          histogram_quantile(0.95, rate(litellm_request_latency_seconds_bucket[5m])) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "P95 latency is above 10 seconds"
          description: "P95 latency: {{ $value | printf \"%.2f\" }}s"

      # Rate limit approaching
      - alert: RateLimitApproaching
        expr: |
          rate(litellm_rate_limit_hits_total[5m]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Frequent rate limit hits for {{ $labels.provider }}"

      # No requests
      - alert: NoRequests
        expr: |
          sum(rate(litellm_http_requests_total[10m])) == 0
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "No requests received in 10 minutes"
```
