---
name: observability-architecture
description: LiteLLM-RS Observability Architecture. Covers Prometheus metrics, OpenTelemetry tracing, structured logging, health checks, and alerting integration. Use when adding or changing metrics, tracing, or logging instrumentation, implementing or debugging health checks, wiring Prometheus alert rules, or configuring the observability stack.
---

# Observability Architecture Guide

## Overview

LiteLLM-RS implements comprehensive observability through three pillars: metrics (Prometheus), tracing (OpenTelemetry), and logging (structured JSON). This enables complete visibility into gateway operations.

### Observability Stack

```
┌─────────────────────────────────────────────────────────────────┐
│                    LiteLLM Gateway                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐      │
│  │    Metrics    │  │    Tracing    │  │    Logging    │      │
│  │  (Prometheus) │  │ (OpenTelemetry)│  │   (tracing)   │      │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘      │
│          │                  │                  │                │
└──────────┼──────────────────┼──────────────────┼────────────────┘
           │                  │                  │
           ▼                  ▼                  ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│    Prometheus    │ │      Jaeger/     │ │    ELK Stack/    │
│    + Grafana     │ │      Tempo       │ │      Loki        │
└──────────────────┘ └──────────────────┘ └──────────────────┘
```

---

## Configuration

```yaml
observability:
  metrics:
    enabled: true
    endpoint: "/metrics"
    include_labels:
      - provider
      - model
      - status
      - error_type
    histogram_buckets:
      latency: [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
      tokens: [10, 50, 100, 500, 1000, 5000, 10000]

  tracing:
    enabled: true
    exporter: "otlp"
    endpoint: ${OTEL_EXPORTER_OTLP_ENDPOINT:-http://localhost:4317}
    sample_rate: 0.1
    propagation: "tracecontext,baggage"

  logging:
    level: "info"
    format: "json"
    include_timestamp: true
    include_target: true
    include_file: false
    include_line: false

  health:
    enabled: true
    endpoints:
      health: "/health"
      live: "/health/live"
      ready: "/health/ready"
    check_interval_seconds: 30
```

---

## References

- [reference/metrics.md](reference/metrics.md) — Prometheus metric definitions, registration, and the /metrics endpoint handler.
- [reference/tracing-and-logging.md](reference/tracing-and-logging.md) — OpenTelemetry tracer init, span creation, request tracing middleware, and structured log events.
- [reference/health-checks.md](reference/health-checks.md) — Health response model, component checks, and liveness/readiness/detailed endpoints.
- [reference/alerting.md](reference/alerting.md) — Prometheus alert rules for error rate, provider health, latency, rate limits, and traffic stalls.
- [reference/best-practices.md](reference/best-practices.md) — Metric type selection, log context, label cardinality, and graceful telemetry degradation.
