# GH838 subsystem migration and remaining removals

The 0.6 compatibility plan scheduled unused public APIs for removal in 0.7.
The published 0.7.0 still contains some of them. F16 removes verified unused
surfaces in unreleased source; the next release containing these removals must
use the breaking-release path in `.github/workflows/version-bump.yml`.

## Runtime and feature decisions

| Surface | Historical compatibility behavior | Current source / remaining work |
| --- | --- | --- |
| `guardrails`, `ip_access` | Wired into the request path | Keep wired |
| `core::integrations`, `core::observability::RuntimeObservability` | Configured callback backends receive real LLM lifecycle events; other `core::observability` exports are deprecated library-only compatibility types | Keep the canonical callback runtime; remove legacy exports |
| `core::audit` | `enterprise.audit_logging` explicitly enables request middleware and emits redacted JSON to stderr; default off | Keep wired |
| `core::mcp`, `core::a2a`, `core::webhooks` | Deprecated default-off library features (`mcp`, `a2a`, `webhooks`); no gateway routes | Remove unless a separately approved runtime design supersedes the decision |
| `core::realtime` | Deprecated default-off `websockets` library feature; no mounted route | Remove unless a separately approved runtime design supersedes the decision |
| `core::batch::BatchProcessor` | Deprecated; `/v1/batches` continues to use the provider proxy | Processor removed; provider proxy and async batch helpers retained |
| `core::semantic_cache` | Deprecated but retained with `storage` for 0.6 compatibility; config enablement is rejected | Remove module and rejected config fields |
| `core::analytics` | Deprecated and default-off behind `analytics` | Remove module and unwired config fields |
| `core::virtual_keys::VirtualKeyManager` | Deprecated duplicate; gateway runtime uses `core::keys::KeyManager` through `RuntimeVirtualKeyManager` | Manager removed; canonical KeyManager and storage records retained |
| `core::user_management::UserManager` | Deprecated and default-off behind `user-management`; compatibility record types remain because auth/storage use them | Manager and user-management feature removed; auth/storage records retained |

## Migration actions

- MCP/A2A/Webhook library users must enable the corresponding Cargo feature.
  These features never imply HTTP route registration.
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

Remaining F16 review: legacy analytics, semantic-cache and retry helpers; observability exports must be checked individually because config provider export still calls its redaction helpers. A2A/MCP/Realtime declarations follow their respective gateway implementations. Removal of the three unused managers does not complete the entire subsystem audit.
