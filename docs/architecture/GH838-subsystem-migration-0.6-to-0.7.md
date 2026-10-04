# GH838 subsystem migration and remaining removals

The 0.6 compatibility plan scheduled unused public APIs for removal in 0.7.
The published 0.7.0 still contains some of them. F16 removes verified unused
surfaces in unreleased source; the next release containing these removals must
use the breaking-release path in `.github/workflows/version-bump.yml`.

## Runtime and feature decisions

| Surface | Historical compatibility behavior | Current source / remaining work |
| --- | --- | --- |
| `guardrails`, `ip_access` | Wired into the request path | Keep wired |
| `core::integrations`, `core::observability::RuntimeObservability` | Configured callback backends receive real LLM lifecycle events | Keep the callback runtime and configuration redaction; unused legacy observability APIs removed |
| `core::audit` | `enterprise.audit_logging` explicitly enables request middleware and emits redacted JSON to stderr; default off | Keep wired |
| `core::mcp` | Originally deprecated as a library-only surface | Retained: `gateway,mcp` mounts authenticated Streamable HTTP; see [MCP gateway](../gateway/mcp.md) |
| `core::a2a` | Deprecated default-off library feature; no gateway routes in this source revision | Pending the dedicated gateway implementation |
| `core::webhooks` | Deprecated default-off library feature | Removed with the `webhooks` feature; independent budget-alert delivery remains |
| Former `core::realtime` | Removed unused legacy client/types | Use the separate `gateway,websockets` `/v1/realtime` HTTP gateway |
| `core::batch::BatchProcessor` | Deprecated; `/v1/batches` continues to use the provider proxy | Processor removed; provider proxy and async batch helpers retained |
| `core::semantic_cache` | Removed from unreleased source | Use deterministic `core::cache`; remove semantic cache fields |
| `core::analytics` | Removed from unreleased source, including the `analytics` Cargo feature | Use runtime request metrics and callback integrations |
| `core::virtual_keys::VirtualKeyManager` | Deprecated duplicate; gateway runtime uses `core::keys::KeyManager` through `RuntimeVirtualKeyManager` | Manager removed; canonical KeyManager and storage records retained |
| `core::user_management::UserManager` | Deprecated and default-off behind `user-management`; compatibility record types remain because auth/storage use them | Manager and user-management feature removed; auth/storage records retained |

## Migration actions

- MCP gateway users enable `gateway,mcp` and configure upstream servers.
  The A2A library feature alone does not imply HTTP route registration.
  Remove the deleted `webhooks` feature from explicit feature selections.
- Batch users should call the OpenAI-compatible `/v1/batches` proxy instead of
  constructing `BatchProcessor`.
- Virtual-key users should migrate to `core::keys::KeyManager`; the gateway has
  one key runtime and does not construct the legacy manager.
- Do not enable `cache.semantic_cache` or `enterprise.advanced_analytics`; both
  remain rejected because no request lifecycle consumes them.

The removal-bearing release needs validation of the retained public APIs and
the existing version workflow's `confirm_breaking_changes=true` selection.
These removals must not be described as part of the already published 0.7.0 or
shipped as a patch release.

## F16 removal verification (2026-10-03)

Issue #1402 removes the unreachable BatchProcessor, duplicate VirtualKeyManager, unused UserManager and user-management Cargo feature, and resolved GH838 temporary-exemption constants/variant. Repository references were confined to the implementations, exports and compatibility-only tests. Batch async helpers, RuntimeVirtualKeyManager (canonical KeyManager), and user/team/virtual-key records remain because current runtime/storage paths consume them. This is a source-breaking removal, without replacement shims.

The final F16 batch removes the legacy realtime library and unused MCP/A2A clients and re-exports. MCP/A2A configuration, domain/error and schema support remains; callers use the retained qualified module paths. The manual `/v1/realtime` route, MCP stateless proxy, A2A process-local ownership, callbacks and observability redaction remain wired. See [the complete subsystem reconciliation](../audit/subsystem-reconciliation-2026-10-04.md).

## Legacy observability and webhook removal

The unreleased source removes the unused `core::webhooks` module and `webhooks`
Cargo feature, and the legacy observability destinations, histogram, logging,
metrics, tracing and record APIs. Remove explicit `webhooks` feature selections.
Use configured callback integrations through `RuntimeObservability`; provider
configuration still uses the retained redaction helpers. Existing budget-alert
webhook delivery and provider-native webhook request fields are separate and
remain supported. No replacement compatibility facade is introduced.

The next unreleased source also removes the unused semantic knobs from
`LLMCacheConfig` and `/admin/cache` output. Drop `analytics` from explicit Cargo
feature lists; `enterprise` now selects only vector search. Build workflows,
Docker defaults and configuration examples have been updated together. This is
an intentional source/configuration break, not a change to already published
v0.7.0 artifacts. Ordinary cache TTL/size, audit logging, metrics, user usage
records and callback integrations remain wired and supported.

## Expired duplicate provider modules

F16 removes the unused native modules `amazon_nova`, `github`, `meta_llama`,
`v0`, and `custom_api` from unreleased source. F10 also disables the Nova, Meta
Llama and v0 named selectors because their previously declared endpoint/model
contracts cannot be verified. Move Nova configurations to the retained native
Bedrock provider, or use an explicitly configured `openai_compatible` deployment
with a verified endpoint and model contract. Meta Llama and v0 custom endpoints
likewise require explicit `openai_compatible` configuration. F10 removes
the GitHub Models selector because the service retired on 2026-07-30; see the
[retirement audit](../audit/github-models-retirement-2026-10-03.md). Custom servers use
`openai_compatible` when compatible, or the public `ExternalProvider` registration
API for custom protocols. GitHub Copilot is a separate retained native provider.
Current routing, capability, model, health and error tests cover the supported
entries. See the [remaining catalog audit](../audit/remaining-static-model-catalog-2026-10-04.md)
for the provider-specific evidence and limitations.
