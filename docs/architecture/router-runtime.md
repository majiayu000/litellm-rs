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

## Distributed admission windows

Redis supplies the clock for shared RPM/TPM windows and lease expiry. RPM counts
arrivals in the current minute; outstanding TPM stays reserved across minute
boundaries until settlement, cancellation, or lease expiry. Settlement replaces
the reserved estimate with actual usage once.

All replicas sharing admission keys must use the same accounting implementation.
When upgrading from the admission script shipped in v0.7.0 or earlier, drain in-flight requests and
stop the old replicas before starting the updated replicas. Mixed-version writes
and repair of accounting left by the old script are not supported.

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

### SDK tool turns and streaming migration

Runtime-backed clients support complete tool turns with
`Message::tool_result(call_id, text)`. Preserve the assistant message containing
the tool calls, then append one tool-result message for each call being answered.
The SDK forwards `tool_call_id` to the canonical provider request.

Use `chat_stream_with_options(SdkChatRequest)` to send a model, tool definitions,
and tool choice with a streamed request. It enables streaming and requests usage
from providers that support usage reporting. This method requires
`LLMClient::from_runtime`; a legacy client returns `SDKError::NotSupported`.
The existing `chat_stream(messages)` entry point remains available for both
constructors.

This changes the Rust SDK's source-level DTO contract:

- Add `tool_call_id: None` to existing `Message` struct literals; use
  `Message::tool_result` for tool results.
- Add `usage: None` to existing `ChatChunk` literals. The optional field reuses
  canonical `responses::Usage` and preserves token details. A usage-only final
  chunk can have an empty `choices` list; consume the stream through its end.
- `MessageDelta.tool_calls` contains canonical `ToolCallDelta` values, re-exported
  from `sdk::types`. Accumulate argument fragments by `(choice.index, call.index)`.
  Each call's ID, type, function, and function name may be absent on later
  fragments; keep previously observed values. Complete non-streaming messages
  continue using `ToolCall`.

Existing text-message and text-stream JSON remains valid. Standard tool-call
responses need not include function parameter schemas. The runtime SDK still
returns `NotSupported` for thinking, audio, and legacy `function_call` deltas
that its facade cannot represent, instead of silently dropping them.

Physical removal of these compatibility surfaces is a 0.7 breaking change and
requires a published 0.6 migration window plus explicit release-policy
approval. It must not be hidden in a non-breaking version bump.
