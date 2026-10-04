# Final subsystem reconciliation — 2026-10-04

Issue #1402. This audit compares every current `CORE_SUBSYSTEMS` entry with
startup, mounted routes, feature gates and README declarations after the accepted
protocol implementation. It records source wiring, not supplier-account tests.
The final PR's CI and merge remain separate acceptance steps.

## Removal and retention evidence

Repository-wide Rust references to the old realtime, MCP and A2A clients were
confined to their implementations, exports, examples and client-only tests.
The four-file `core::realtime` library has been removed; its server WebSocket
route uses its own native GA events and budget implementation.

Removed MCP `McpGateway`, `McpServer`/registry/session implementation and permission
manager; removed A2A `A2AGateway`, provider adapters and `AgentRegistry`. Their
unused aggregate gateway configurations and deprecated top-level re-exports
were removed too. `GatewayConfig` still consumes `mcp::config::McpServerConfig`
and `a2a::config::AgentConfig`; canonical/gateway error conversions still consume
MCP/A2A error and response-error types. These references are retained. Shared
protocol, domain and MCP schema/validation modules remain library support and
are not advertised as another transport or runtime gateway.

The model audit disabled the obsolete GitHub, Meta Llama, Nova and v0 named
selectors. This batch removes their now-empty catalog hooks and the Meta/v0
parameter, header, health, model-price and configured-name route special cases.
The general OpenAI-compatible parameter contract and error mapping remain.
No replacement compatibility facade or new subsystem was introduced. The separately documented LLM sub-trait adapters target a future major release and remain library-only; they are not used to dispatch gateway calls.

## Complete current core matrix

Source links show the actual consumer or the intentional library/support module.
`FeatureGated` means the core declaration and mounted route share the named
opt-in feature; it is not a claim that every configured protocol is accepted.

| Module | Decision | Actual entry / scope | Source evidence |
| --- | --- | --- | --- |
| `a2a` | `FeatureGated` | Opt-in A2A 1.0 JSON-RPC HTTP gateway with caller-bound tasks and Streamable events; ownership is process-local. | [src/server/routes/a2a.rs](../../src/server/routes/a2a.rs) |
| `audio` | `Wired` | Audio request paths are mounted by server AI routes. | [src/server/routes/ai/audio/mod.rs](../../src/server/routes/ai/audio/mod.rs) |
| `audit` | `Wired` | enterprise.audit_logging explicitly enables request lifecycle audit events; the default remains disabled. | [src/server/http.rs](../../src/server/http.rs) |
| `batch` | `LibraryOnly` | The HTTP /v1/batches surface is a provider proxy; batch domain records and async batch execution remain library APIs. | [src/core/batch/mod.rs](../../src/core/batch/mod.rs) |
| `budget` | `Wired` | Budget state and reservation paths are constructed by the server. | [src/server/routes/ai/spend.rs](../../src/server/routes/ai/spend.rs) |
| `cache` | `Wired` | Deterministic response cache is constructed when cache.enabled=true. | [src/server/runtime.rs](../../src/server/runtime.rs) |
| `completion` | `LibraryOnly` | Library completion API is separate from gateway route handlers. | [src/core/completion/mod.rs](../../src/core/completion/mod.rs) |
| `cost` | `InternalDependency` | Cost helpers back providers and spend calculation rather than a standalone route. | [src/core/cost/mod.rs](../../src/core/cost/mod.rs) |
| `embedding` | `LibraryOnly` | Library embedding API is separate from gateway route handlers. | [src/core/embedding/mod.rs](../../src/core/embedding/mod.rs) |
| `fine_tuning` | `Wired` | Fine-tuning route handlers use the core provider adapters. | [src/server/routes/ai/fine_tuning.rs](../../src/server/routes/ai/fine_tuning.rs) |
| `function_calling` | `LibraryOnly` | Function-calling helpers are library/provider support code. | [src/core/function_calling/mod.rs](../../src/core/function_calling/mod.rs) |
| `guardrails` | `Wired` | Prompt-injection checks run before provider execution and on non-streaming output. | [src/server/guardrails.rs](../../src/server/guardrails.rs) |
| `health` | `LibraryOnly` | Server health routes are implemented under src/server/routes/health.rs. | [src/core/health/mod.rs](../../src/core/health/mod.rs) |
| `http` | `InternalDependency` | Shared outbound HTTP profile used by providers and integrations. | [src/core/http/mod.rs](../../src/core/http/mod.rs) |
| `integrations` | `Wired` | Configured Langfuse, OpenTelemetry, and Datadog callbacks are initialized and receive real request lifecycle events. | [src/server/callbacks.rs](../../src/server/callbacks.rs) |
| `ip_access` | `Wired` | Configured IP policies short-circuit before auth, handlers, or providers. | [src/server/http.rs](../../src/server/http.rs) |
| `keys` | `Wired` | Key management is constructed in AppState and exposed through server routes. | [src/server/state.rs](../../src/server/state.rs) |
| `mcp` | `FeatureGated` | Opt-in authenticated stateless Streamable HTTP proxy for configured servers; requests can move between instances. | [src/server/routes/mcp.rs](../../src/server/routes/mcp.rs) |
| `models` | `Wired` | OpenAI-compatible models are used by gateway routes and auth context. | [src/server/routes/admin.rs](../../src/server/routes/admin.rs) |
| `net` | `InternalDependency` | Network safety helpers are consumed by config/provider support code. | [src/core/net/mod.rs](../../src/core/net/mod.rs) |
| `observability` | `Wired` | RuntimeObservability is the canonical callback dispatcher; retained redaction helpers protect provider configuration output. | [src/server/state.rs](../../src/server/state.rs) |
| `pricing` | `InternalDependency` | Shared pricing helpers support runtime spend calculations. | [src/core/pricing.rs](../../src/core/pricing.rs) |
| `pricing_service` | `Wired` | PricingService is initialized at startup and shared with handlers. | [src/server/http_runtime.rs](../../src/server/http_runtime.rs) |
| `providers` | `Wired` | Gateway startup builds providers through the unified router. | [src/core/router/gateway_config.rs](../../src/core/router/gateway_config.rs) |
| `rate_limiter` | `Wired` | Global rate limiter is initialized by the server app factory. | [src/server/http.rs](../../src/server/http.rs) |
| `request_ledger` | `Wired` | storage.request_ledger persists one metadata-only terminal row per request when enabled; the default remains disabled. | [src/core/audit/middleware.rs](../../src/core/audit/middleware.rs) |
| `rerank` | `Wired` | Rerank requests are mounted in the AI route scope. | [src/server/routes/ai/mod.rs](../../src/server/routes/ai/mod.rs) |
| `router` | `Wired` | Gateway startup constructs UnifiedRouter from provider config. | [src/server/runtime.rs](../../src/server/runtime.rs) |
| `secret_managers` | `LibraryOnly` | Secret manager adapters are available as library support code. | [src/core/secret_managers/mod.rs](../../src/core/secret_managers/mod.rs) |
| `security` | `LibraryOnly` | Core security filters are library support; server security middleware is separate. | [src/core/security/mod.rs](../../src/core/security/mod.rs) |
| `streaming` | `Wired` | Streaming event types are used by mounted SSE handlers. | [src/server/routes/ai/responses.rs](../../src/server/routes/ai/responses.rs) |
| `subsystem_registry` | `InternalDependency` | This guardrail registry classifies the exported core modules. | [src/core/subsystem_registry.rs](../../src/core/subsystem_registry.rs) |
| `teams` | `Wired` | Team management is constructed in AppState and exposed through server routes. | [src/server/state.rs](../../src/server/state.rs) |
| `traits` | `InternalDependency` | Trait definitions support providers, integrations, and storage abstractions. | [src/core/traits/mod.rs](../../src/core/traits/mod.rs) |
| `types` | `Wired` | Gateway handlers use shared context, response, model, and media types. | [src/server/routes/ai/mod.rs](../../src/server/routes/ai/mod.rs) |
| `user_management` | `InternalDependency` | User/team domain records back current auth and storage paths; the unused legacy manager has been removed. | [src/core/user_management/mod.rs](../../src/core/user_management/mod.rs) |
| `virtual_keys` | `Wired` | The virtual-keys runtime facade resolves to the canonical KeyManager used by auth and /v1/keys; storage record types remain in use. | [src/server/state.rs](../../src/server/state.rs) |

## Protocol and startup boundaries

- MCP: `gateway,mcp` mounts authenticated stateless Streamable HTTP; requests can
  move between instances. Unsupported transport/authentication/config fields are
  rejected. The old process-local MCP client/session explanation has been removed.
- A2A: `gateway,a2a` mounts A2A 1.0 JSON-RPC and SSE; task/context ownership is
  still process-local. No native platform adapter is implied by `AgentProvider`.
- Realtime: `gateway,websockets` mounts `/v1/realtime` under server AI routes;
  there is no exported core realtime subsystem after removal. Its manual
  text/audio/function contract and exclusions remain documented in the gateway guide.
- Responses/Messages: native route selection, tool billing, budget reservations,
  callbacks, ledger and recovery use the existing server paths. Subsystem cleanup
  does not broaden the supported tool/protocol scope.
- Observability: `RuntimeObservability` is the canonical callback dispatcher;
  provider configuration redaction is retained. Startup builds configured
  callback integrations. Audit and request ledger remain explicit opt-ins.
- Current `GatewayConfig` enablement, middleware registration, canonical key
  runtime, storage domain records, provider batch proxy and budget-alert delivery
  remain consumers rather than removal candidates.

Verification: `cargo fmt --check`, `cargo check --locked`, default full tests and
all-target clippy, plus feature-enabled library tests and all-target clippy with
`gateway,sqlite,mcp-validation,a2a,websockets,providers-extra`. The existing MCP,
A2A and Realtime HTTP/WebSocket tests verify their retained paths. No production
credentials or paid upstream calls were used. The final local default suite and all-target clippy passed; the feature-enabled library suite passed 10,522 tests with one ignored. A wider feature integration run was stopped after a batch fixture did not terminate; the batch and moderation cases subsequently passed independently and in their parallel HTTP suites. This does not establish the intermittent hang root cause. CI/merge evidence is recorded in the PR.
