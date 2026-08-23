---
name: observability-architecture
description: LiteLLM-RS Observability Architecture. Covers the gateway_* Prometheus-format metrics rendered by the metrics middleware and /metrics endpoint, health/readiness endpoints, request logging, and the OpenTelemetry/Datadog/Langfuse callback exporters. Use when adding or changing metrics or log instrumentation, implementing or debugging health checks, wiring Prometheus alert rules, or configuring the monitoring stack.
---

# Observability Architecture Guide

## Overview

Observability in LiteLLM-RS is built from three runtime pieces:

1. **HTTP metrics** — `MetricsMiddleware` (`src/server/middleware/metrics.rs`) counts requests with process-local atomics and renders Prometheus text format on demand. No `prometheus` crate is used; every series is hand-rendered with the `gateway_` prefix.
2. **Health/status routes** — `src/server/routes/health.rs` mounts `/health`, `/health/ready`, `/health/detailed`, `/status`, `/version`, and `/metrics` on the main HTTP server.
3. **Callback exporters** — configured under `monitoring.callbacks`, the `OpenTelemetryIntegration` (OTLP/HTTP JSON), `DataDogIntegration`, and `LangfuseIntegration` receive real LLM lifecycle events through the `CallbackDispatcher` stored in `AppState` (exposed as `RuntimeObservability`).

```
┌─────────────────────────────────────────────────────────────────┐
│                    LiteLLM Gateway                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐       │
│  │  Metrics      │  │ Health/status │  │  Callback     │       │
│  │  middleware   │  │ routes        │  │  dispatcher   │       │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘       │
└──────────┼──────────────────┼──────────────────┼───────────────┘
           ▼                  ▼                  ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ Prometheus       │ │ LB / K8s probes  │ │ OTLP / Datadog / │
│ scrapes /metrics │ │ + JSON status    │ │ Langfuse backends│
└──────────────────┘ └──────────────────┘ └──────────────────┘
```

---

## Configuration

The section is `monitoring:` at the top level of `config/gateway.yaml` (deserialized as
`GatewayConfig.monitoring`, `src/config/models/gateway.rs`). Every struct uses
`#[serde(deny_unknown_fields)]` — unknown keys fail config parsing.

```yaml
monitoring:
  metrics:
    enabled: true          # gates MetricsMiddleware (src/server/http.rs)
    port: 9090             # default 9090; validated > 0 when enabled
    path: "/metrics"       # validated non-empty, starts with '/'
    interval_seconds: 15
  tracing:
    enabled: false
    endpoint: null         # REQUIRED when enabled: true (config validation)
    service_name: "litellm-rs"
    sampling_rate: 0.1
    jaeger: null           # or {agent_endpoint, service_name}
  health:
    path: "/health"
    detailed: true
  logging: null            # or {level, format: text|json|structured, outputs}
  callbacks:
    queue_capacity: 1024
    timeout_ms: 5000
    backends: []           # {type: opentelemetry|datadog|langfuse, config: {...}}
```

Wiring notes (verified against current code):

- `metrics.enabled` is the only metrics key with runtime effect: it wraps the app in
  `Condition::new(metrics_enabled, MetricsMiddleware)` (`src/server/http.rs`). The
  `/metrics` route itself is hardcoded in `routes::health::configure_routes`; `port`,
  `path`, and `interval_seconds` are parsed and validated but not consumed by runtime
  wiring today.
- `tracing.enabled` only appears in the startup summary log (`src/lib.rs`); OTLP trace
  export is configured through `callbacks.backends`, not the `tracing:` section.
- `logging` is parsed/validated but the log subscriber is initialized in `src/main.rs`
  `init_logging` from the CLI/env level, not from this section.

---

## References

- [reference/metrics.md](reference/metrics.md) — the full `gateway_*` metric inventory, how the middleware records, and the /metrics renderer.
- [reference/tracing-and-logging.md](reference/tracing-and-logging.md) — log subscriber init, request IDs, access-log events, and the OTLP/Datadog/Langfuse callback exporters.
- [reference/health-checks.md](reference/health-checks.md) — health/readiness/detailed endpoints, response models, and the aggregate readiness rule.
- [reference/alerting.md](reference/alerting.md) — Prometheus alert rules built on real `gateway_*` series.
- [reference/best-practices.md](reference/best-practices.md) — metric type selection, log context, label cardinality, and graceful telemetry degradation.
