## Tracing (OpenTelemetry)

### Tracing Setup

```rust
use opentelemetry::{global, sdk::trace as sdktrace, trace::TraceError};
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing(config: &TracingConfig) -> Result<(), TraceError> {
    // Create OTLP exporter
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&config.endpoint);

    // Create tracer provider
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(
            sdktrace::config()
                .with_sampler(sdktrace::Sampler::TraceIdRatioBased(config.sample_rate))
                .with_resource(opentelemetry::sdk::Resource::new(vec![
                    opentelemetry::KeyValue::new("service.name", "litellm-gateway"),
                    opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ])),
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    // Create OpenTelemetry layer
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Create fmt layer for console output
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .json();

    // Initialize subscriber
    tracing_subscriber::registry()
        .with(otel_layer)
        .with(fmt_layer)
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Ok(())
}
```

### Span Creation

```rust
use tracing::{info_span, instrument, Span};

/// Instrument a provider request with tracing
#[instrument(
    name = "provider_request",
    skip(request, context),
    fields(
        provider = %provider_name,
        model = %request.model,
        request_id = %context.request_id,
    )
)]
pub async fn traced_provider_request(
    provider_name: &str,
    request: ChatRequest,
    context: RequestContext,
) -> Result<ChatResponse, ProviderError> {
    let span = Span::current();

    // Add request attributes
    span.record("messages_count", request.messages.len());
    if let Some(max_tokens) = request.max_tokens {
        span.record("max_tokens", max_tokens);
    }

    let start = std::time::Instant::now();
    let result = provider.chat_completion(request, context).await;
    let duration = start.elapsed();

    // Record response attributes
    match &result {
        Ok(response) => {
            span.record("status", "success");
            if let Some(usage) = &response.usage {
                span.record("prompt_tokens", usage.prompt_tokens);
                span.record("completion_tokens", usage.completion_tokens);
            }
        }
        Err(e) => {
            span.record("status", "error");
            span.record("error", e.to_string());
        }
    }

    span.record("duration_ms", duration.as_millis() as i64);

    result
}
```

### Request Tracing Middleware

```rust
use actix_web::{dev::ServiceRequest, HttpMessage};
use tracing::{info_span, Instrument};

pub async fn tracing_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let request_id = req
        .headers()
        .get("X-Request-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let span = info_span!(
        "http_request",
        request_id = %request_id,
        method = %req.method(),
        path = %req.path(),
        user_agent = req.headers().get("User-Agent").and_then(|h| h.to_str().ok()).unwrap_or(""),
    );

    // Store request_id in extensions for later use
    req.extensions_mut().insert(RequestId(request_id.clone()));

    let response = next.call(req).instrument(span.clone()).await?;

    // Record response status
    span.record("status_code", response.status().as_u16());

    Ok(response)
}
```

---

## Structured Logging

### Log Configuration

```rust
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logging(config: &LoggingConfig) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(config.include_target)
        .with_file(config.include_file)
        .with_line_number(config.include_line);

    match config.format.as_str() {
        "json" => {
            subscriber.json().init();
        }
        "pretty" => {
            subscriber.pretty().init();
        }
        _ => {
            subscriber.init();
        }
    }
}
```

### Structured Log Events

```rust
use tracing::{info, warn, error, debug};

// Request logging
info!(
    request_id = %request_id,
    provider = %provider_name,
    model = %model,
    latency_ms = %latency.as_millis(),
    status = "success",
    "Chat completion request completed"
);

// Error logging
error!(
    request_id = %request_id,
    provider = %provider_name,
    error_type = "rate_limit",
    retry_after = ?retry_after,
    "Rate limit exceeded"
);

// Debug logging for detailed info
debug!(
    request_id = %request_id,
    tokens_prompt = %usage.prompt_tokens,
    tokens_completion = %usage.completion_tokens,
    cost_usd = %cost,
    "Token usage and cost"
);
```
