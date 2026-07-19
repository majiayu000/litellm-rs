# Tech Spec

## Linked Issue

GH-1066 / #1066

## Product Spec

See `specs/GH1066/product.md` (invariants P1-P9).

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Callback contract | `src/core/traits/integration.rs` | `Integration` already defines LLM start/end/error, stream, embedding, flush, and shutdown hooks | Reuse as the single plugin boundary |
| Dispatcher | `src/core/integrations/manager.rs` | Registers enabled integrations and isolates hook failures with timeout/fail-fast policy, but callers must await it | Needs a non-blocking ordered runtime queue |
| Built-in exporters | `src/core/integrations/observability/{datadog.rs,opentelemetry/*}` | Both implement `Integration`; neither is constructed by gateway startup | Register from config |
| Langfuse | `src/core/integrations/langfuse/{logger.rs,config.rs}` | Implements a separate synchronous `LlmCallback`, not `Integration` | Add a translation adapter, not another runtime contract |
| Configuration | `src/config/models/monitoring.rs`, `src/config/validation/monitoring_validators.rs`, `config/gateway.yaml.example` | Metrics/tracing config exists; no callback backend list or dispatcher settings | Add opt-in typed callback config |
| Startup/shutdown | `src/server/http.rs`, `src/server/state.rs` | Builds AppState and drains storage/budget workers; no integration runtime | Construct once, inject dispatcher, drain/flush on shutdown |
| Request lifecycle | `src/server/routes/ai/{chat.rs,chat_streaming.rs,completions*.rs,embeddings.rs,responses*.rs}` | Provider execution, settlement, streaming terminal paths, and request context exist; no external lifecycle events | Emit metadata from the real execution boundaries |

## Proposed Design

### Canonical callback boundary

Keep `crate::core::traits::integration::Integration` as the only runtime plugin
trait. `IntegrationManager` remains responsible for backend registration,
per-hook timeout, parallel/sequential backend dispatch, and fail-open error
isolation. Add `LangfuseIntegration`, which translates canonical lifecycle
events to the existing `LangfuseLogger` request/response/error DTOs.

### Non-blocking runtime

Add a callback runtime beside `IntegrationManager`:

- `CallbackDispatcher` is cloneable and exposes synchronous `try_send` methods
  for canonical events.
- A single bounded Tokio channel preserves enqueue order and prevents request
  handlers from awaiting exporter I/O.
- Full/closed queue errors are returned to the caller and logged at the request
  integration point; they never change the gateway response.
- `CallbackRuntime` owns the worker, drains queued events on shutdown, then
  calls manager `flush` and `shutdown`.
- The runtime accepts an existing `Arc<IntegrationManager>` so embedders and
  tests can register custom implementations of the public trait.

`IntegrationManagerConfig` is fixed to `fail_fast=false`; configured timeout is
applied per backend hook. Backend failures are logged by the manager and the
worker continues.

### Configuration and startup

Extend `MonitoringConfig` with a default-empty `callbacks` object:

```yaml
monitoring:
  callbacks:
    queue_capacity: 1024
    timeout_ms: 5000
    backends:
      - type: opentelemetry
        config: { ...existing OpenTelemetryConfig fields... }
      - type: datadog
        config: { ...existing DataDogConfig fields... }
      - type: langfuse
        config: { ...existing LangfuseConfig fields... }
```

The tagged backend enum embeds the existing backend config types; it does not
duplicate their fields. Validation enforces positive capacity/timeout, unique
backend kinds, required credentials/endpoints, valid sampling values, and
positive batch settings. Secrets continue to use the existing `${ENV_VAR}`
configuration substitution.

`HttpServer::new` constructs the manager, attempts each backend independently,
logs initialization failures, starts the runtime if at least one backend was
registered, and injects the dispatcher into `AppState`. `HttpServer::start`
shuts the runtime down after the Actix server has stopped and before storage is
closed.

### Request lifecycle

Add an AI-route lifecycle helper that owns request ID, requested model, start
instant, dispatcher, and a synchronized final selected target. It emits
metadata-only canonical events and centralizes terminal-once behavior.

- Unary chat/completion/response and embeddings: enqueue start immediately
  before provider selection; record the selected provider/pricing identity
  inside the retryable operation; enqueue success after settlement and error
  on the returned gateway error.
- Streaming chat/completion/response: enqueue start before stream creation;
  move the lifecycle into the existing stream worker; enqueue one success after
  final usage settlement, or one error on upstream error, idle timeout,
  conversion/serialization failure, or disconnect.
- Cache hits return before lifecycle creation and do not fabricate a provider
  callback.

Token usage comes only from provider usage structures. Cost uses
`PricingService::calculate_loaded_settlement_cost_for_provider` with the same
selected pricing identity and usage used by spend settlement; failure to
calculate is logged and leaves `cost_usd` absent. Event input/output values
remain null and headers are never copied.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1, P9 | monitoring config + validators | deserialize/default/duplicate/invalid-value unit tests |
| P2 | server backend builder | registration and partial-initialization-failure tests |
| P3, P4 | callback runtime + manager | ordered start/terminal, queue-full, closed-queue, failing-backend tests |
| P5, P6 | AI lifecycle helper + unary routes | real mock-provider success/error tests with metadata assertions |
| P7 | existing streaming workers | normal/error/timeout/disconnect terminal-once tests |
| P8 | canonical event fixtures + Langfuse adapter | negative assertions for prompt/output/header/secret values |
| Shutdown | `HttpServer::start` + callback runtime | deterministic drain/flush/shutdown unit test |

## Data Flow

`GatewayConfig` → validate callback config → construct enabled backends →
register in `IntegrationManager` → start `CallbackRuntime` →
inject `CallbackDispatcher` into `AppState` → real provider execution enqueues
start/terminal metadata → worker invokes integrations → exporter batches send
externally → graceful shutdown drains, flushes, and stops.

No callback event mutates routing, provider responses, budgets, cache entries,
or persistent application state.

## Alternatives Considered

- Keep both `Integration` and Langfuse `LlmCallback` as gateway contracts:
  rejected because it creates two lifecycle sources and inconsistent failure
  behavior.
- Await `IntegrationManager` directly in handlers: rejected because exporter
  timeouts would add latency to the request path.
- Spawn one task per hook: rejected because start and terminal events can race
  and task growth is unbounded.
- Add vendor SDKs: rejected because the repository already contains HTTP
  exporters and the issue does not require dependency expansion.

## Risks

- Security: lifecycle metadata can still identify users; emit only existing
  request/user IDs and never headers, API keys, prompt, or output content.
- Compatibility: the config schema only gains a defaulted optional field;
  existing configs keep their current behavior.
- Performance: `try_send` adds a bounded allocation/enqueue on enabled paths;
  disabled paths are a cheap no-op.
- Maintenance: route terminal paths can drift; terminal-once helper and source
  tests keep streaming exits explicit.

## Test Plan

- [ ] Unit tests: callback config, Langfuse adapter mapping, runtime ordering,
      queue errors, backend error isolation, shutdown drain.
- [ ] Integration tests: configured startup registration and real mock-provider
      unary/streaming success/error lifecycle events.
- [ ] Deterministic verification: focused callback/AI tests, `cargo fmt --check`,
      `cargo check`, strict clippy, full `cargo test`, SpecRail checks, scope and
      overlap guards.

## Rollback Plan

Remove all configured callback backends for an operational rollback without
changing request behavior. A code rollback removes the defaulted config field,
runtime injection, lifecycle calls, and Langfuse adapter together; provider,
budget, cache, and pricing behavior remains unchanged.
