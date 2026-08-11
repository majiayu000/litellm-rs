# GH838 subsystem migration: 0.6 to 0.7

GH838 closes the declaration/execution gap without presenting module-only code
as a gateway capability. The 0.6 line is the compatibility window; removal of
public Rust symbols is a breaking 0.7 change and requires the explicit breaking
release confirmation in `.github/workflows/version-bump.yml`.

## Runtime and feature decisions

| Surface | 0.6 behavior | 0.7 direction |
| --- | --- | --- |
| `guardrails`, `ip_access` | Wired into the request path | Keep wired |
| `core::integrations`, `core::observability` | Configured callback backends receive real LLM lifecycle events | Keep the canonical callback runtime |
| `core::audit` | `enterprise.audit_logging` explicitly enables request middleware and emits redacted JSON to stderr; default off | Keep wired |
| `core::mcp`, `core::a2a`, `core::webhooks` | Deprecated default-off library features (`mcp`, `a2a`, `webhooks`); no gateway routes | Remove unless a separately approved runtime design supersedes the decision |
| `core::realtime` | Deprecated default-off `websockets` library feature; no mounted route | Remove unless a separately approved runtime design supersedes the decision |
| `core::batch::BatchProcessor` | Deprecated; `/v1/batches` continues to use the provider proxy | Remove the unreachable processor, retain the proxy |
| `core::semantic_cache` | Deprecated but retained with `storage` for 0.6 compatibility; config enablement is rejected | Remove module and rejected config fields |
| `core::analytics` | Deprecated and default-off behind `analytics` | Remove module and unwired config fields |
| `core::virtual_keys::VirtualKeyManager` | Deprecated duplicate; gateway runtime uses `core::keys::KeyManager` through `RuntimeVirtualKeyManager` | Remove the duplicate manager and compatibility records after migration checks |
| `core::user_management::UserManager` | Deprecated and default-off behind `user-management`; compatibility record types remain because auth/storage use them | Remove the manager; migrate records only after storage paths no longer consume them |

## Migration actions

- MCP/A2A/Webhook library users must enable the corresponding Cargo feature.
  These features never imply HTTP route registration.
- Batch users should call the OpenAI-compatible `/v1/batches` proxy instead of
  constructing `BatchProcessor`.
- Virtual-key users should migrate to `core::keys::KeyManager`; the gateway has
  one key runtime and does not construct the legacy manager.
- Do not enable `cache.semantic_cache` or `enterprise.advanced_analytics`; both
  remain rejected because no request lifecycle consumes them.

Before a 0.7 removal, publish a 0.6 release containing these deprecations, run
the public compatibility checks, update this migration note with final symbol
replacements, and dispatch the version workflow with
`confirm_breaking_changes=true`. A patch release cannot carry these removals.
