# Canonical router runtime

`UnifiedRouter` is the only provider construction, deployment selection, retry,
fallback, and runtime-state authority. A request pins one `RuntimeHandle` from a
`RuntimeBinding`; replacing the process default publishes a new immutable
generation without mutating in-flight requests.

## Public entry points

| Entry point | Runtime ownership | Adapter responsibility |
| --- | --- | --- |
| HTTP gateway | `AppState` owns an `Arc<UnifiedRouter>` built by the canonical provider factory | Validate HTTP input and map the typed result |
| `completion()` / `completion_stream()` | Bind the process-default runtime once per operation | Convert compatibility request and response types |
| `DefaultRouter::from_runtime` | Uses the supplied `RuntimeBinding` | Preserve the legacy trait-shaped API without registry or provider construction |
| `LLMClient::from_runtime` | Uses the supplied `RuntimeBinding` | Convert SDK request, response, stream, and typed error shapes |

The completion facade no longer reads provider environment variables, scans a
`ProviderRegistry`, or constructs request-scoped providers. Unary and streaming
calls select and execute the deployment recorded in the pinned runtime snapshot.
Request-level credentials and endpoints fail closed on this path.

## Compatibility-only surfaces

`DefaultRouter`, the completion `Router` trait, and `ProviderRegistry` remain
source-compatible during the 0.6 window. `DefaultRouter` and `Router` are now
thin adapters; `ProviderRegistry` is hidden from generated documentation and is
not a routing authority. The only remaining in-tree owner is the legacy
high-level embedding compatibility path.

`LLMClient::new(ClientConfig)` also remains as the 0.6 SDK compatibility
transport. New callers should use `LLMClient::from_runtime`, which shares the
same router generation, provider instance, selection state, and typed error
mapping as the gateway and completion facade. The compatibility constructor is
not used as a fallback by the runtime-backed client.

Physical removal of these compatibility surfaces is a 0.7 breaking change and
requires a published 0.6 migration window plus explicit release-policy
approval. It must not be hidden in a non-breaking version bump.
