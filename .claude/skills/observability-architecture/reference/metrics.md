## Contents

- Metrics (Prometheus)

## Metrics (Prometheus)

### Metric Types

```rust
use prometheus::{Counter, Gauge, Histogram, IntCounter, IntGauge, Registry};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Request counters
    pub static ref HTTP_REQUESTS_TOTAL: IntCounter = IntCounter::new(
        "litellm_http_requests_total",
        "Total number of HTTP requests"
    ).unwrap();

    pub static ref PROVIDER_REQUESTS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("litellm_provider_requests_total", "Total provider requests"),
        &["provider", "model", "status"]
    ).unwrap();

    // Latency histograms
    pub static ref REQUEST_LATENCY_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "litellm_request_latency_seconds",
            "Request latency in seconds"
        ).buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["provider", "model", "endpoint"]
    ).unwrap();

    pub static ref PROVIDER_LATENCY_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "litellm_provider_latency_seconds",
            "Provider API latency in seconds"
        ).buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
        &["provider", "model"]
    ).unwrap();

    // Token counters
    pub static ref TOKENS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("litellm_tokens_total", "Total tokens processed"),
        &["provider", "model", "type"]  // type: prompt, completion
    ).unwrap();

    // Cost tracking
    pub static ref COST_TOTAL: CounterVec = CounterVec::new(
        Opts::new("litellm_cost_usd_total", "Total cost in USD"),
        &["provider", "model"]
    ).unwrap();

    // Active connections
    pub static ref ACTIVE_CONNECTIONS: IntGauge = IntGauge::new(
        "litellm_active_connections",
        "Number of active connections"
    ).unwrap();

    pub static ref ACTIVE_STREAMS: IntGaugeVec = IntGaugeVec::new(
        Opts::new("litellm_active_streams", "Number of active streaming connections"),
        &["provider"]
    ).unwrap();

    // Cache metrics
    pub static ref CACHE_HITS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("litellm_cache_hits_total", "Total cache hits"),
        &["cache_tier"]  // l1, l2, l3
    ).unwrap();

    pub static ref CACHE_MISSES_TOTAL: IntCounter = IntCounter::new(
        "litellm_cache_misses_total",
        "Total cache misses"
    ).unwrap();

    // Health metrics
    pub static ref PROVIDER_HEALTH: IntGaugeVec = IntGaugeVec::new(
        Opts::new("litellm_provider_health", "Provider health status (1=healthy, 0=unhealthy)"),
        &["provider"]
    ).unwrap();

    // Error metrics
    pub static ref ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("litellm_errors_total", "Total errors"),
        &["provider", "error_type"]
    ).unwrap();

    // Rate limiting
    pub static ref RATE_LIMIT_HITS: IntCounterVec = IntCounterVec::new(
        Opts::new("litellm_rate_limit_hits_total", "Rate limit hits"),
        &["provider", "limit_type"]  // rpm, tpm
    ).unwrap();
}
```

### Metrics Registration

```rust
pub fn register_metrics() {
    REGISTRY.register(Box::new(HTTP_REQUESTS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(PROVIDER_REQUESTS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(REQUEST_LATENCY_SECONDS.clone())).unwrap();
    REGISTRY.register(Box::new(PROVIDER_LATENCY_SECONDS.clone())).unwrap();
    REGISTRY.register(Box::new(TOKENS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(COST_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(ACTIVE_CONNECTIONS.clone())).unwrap();
    REGISTRY.register(Box::new(ACTIVE_STREAMS.clone())).unwrap();
    REGISTRY.register(Box::new(CACHE_HITS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(CACHE_MISSES_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(PROVIDER_HEALTH.clone())).unwrap();
    REGISTRY.register(Box::new(ERRORS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(RATE_LIMIT_HITS.clone())).unwrap();
}
```

### Metrics Endpoint

```rust
use actix_web::{HttpResponse, web};
use prometheus::Encoder;

pub async fn metrics_handler() -> HttpResponse {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = REGISTRY.gather();

    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(buffer)
}

// Register in routes
pub fn configure_metrics(cfg: &mut web::ServiceConfig) {
    cfg.route("/metrics", web::get().to(metrics_handler));
}
```
