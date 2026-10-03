# A2A 1.0 HTTP gateway

Build with `gateway,a2a`. Configure existing AgentConfig fields under `gateway.a2a_agents`:

```yaml
a2a_agents:
  research:
    name: research
    url: https://agent.example/rpc
    provider: a2a
    api_key: YOUR_UPSTREAM_TOKEN
    timeout_ms: 60000
    capabilities:
      streaming: true
      input_types: [text/plain, application/pdf]
      output_types: [text/plain]
```

POST A2A 1.0 JSON-RPC to `/a2a/research` with `A2A-Version: 1.0` (header or query parameter), JSON content type and your configured gateway API-key header (`x-api-key` is also accepted) or Bearer JWT. Gateway credentials never replace the configured upstream credential. Credentials and static headers require an HTTPS upstream. Calls require `a2a.research` permission; key endpoint restrictions must allow the actual path. Anonymous calls are rejected even if model routes permit anonymous access. Existing HTTP/IP/rate middleware applies.

Supported methods: SendMessage, SendStreamingMessage, GetTask, CancelTask and SubscribeToTask. Message parts, artifacts, task statuses, RPC errors and SSE event bytes are preserved. The endpoint does not translate older protocol versions, aggregate agents, execute agents itself or acquire OAuth tokens. Task listing, push notifications and extended cards are unsupported and return the corresponding A2A unsupported-operation/push-notification error; unknown method names return JSON-RPC MethodNotFound. Unsupported configuration such as non-A2A adapters, per-agent billing/limits and push notifications is rejected.

GET `/a2a/research/.well-known/agent-card.json` with `A2A-Version: 1.0` and gateway authentication for a minimal gateway card generated from configuration. Input/output MIME types and the streaming flag must describe the upstream. CancelTask is a core operation; the older library task_cancellation setting does not gate this gateway. The card describes the gateway URL, contains no upstream credential, and publishes one generic gateway skill and does not claim upstream-specific skills or signatures. Empty mode lists default to `text/plain`; configure them explicitly for other media. Deploy behind HTTPS with the correct host/proxy configuration.

Task IDs, context IDs and referenced tasks are bound to the key (or JWT user) and configured upstream account. Query, cancel, subscribe and conversation continuation require a known owned ID. An account/URL change invalidates access; description, tags, timeout and capability edits preserve ownership. New messages reserve capacity before forwarding, and unused capacity is released on errors or disconnects. Ownership is process-local, bounded to 4096 identifiers and expires after an hour without an observed task update. Restart or expiry fails closed on old tasks; multi-replica deployments require affinity. This first HTTP implementation does not provide durable ownership recovery. Shared upstream credentials still mean a shared upstream account; the gateway cannot implement the remote agent's own resource authorization.

Header acquisition and finite response reads (including upstream errors) use timeout_ms and the configured body size limit. Streaming is bounded per event by server.max_body_size; stream interruption or malformed events fail instead of becoming successful results. Dropping the client body drops the upstream body. Model token budgets and LLM content guardrails do not inspect or meter opaque agent calls; agents may perform their own billable operations.

Validation uses local mock agents, not paid live providers. References: [A2A 1.0 specification](https://a2a-protocol.org/v1.0.0/specification/), [LiteLLM A2A gateway](https://docs.litellm.ai/docs/a2a).

Version query keys are case-insensitive. Configured discovery modes are parsed as MIME types; the public capability constructors use MIME values. Invalid upstream response envelopes return A2A InvalidAgentResponseError (-32006). Task streams must start with a Task and then contain status/artifact updates; a direct Message or terminal RPC error closes the stream immediately. Invalid envelopes cannot release capacity while leaving an untracked successful task.

With exactly one enabled agent, `/.well-known/agent-card.json` exposes the same authenticated card. With multiple agents, configure each named card URL directly (`/a2a/{agent_name}/.well-known/agent-card.json`); [A2A 1.0 section 8.2](https://a2a-protocol.org/v1.0.0/specification/#82-discovery-mechanisms) explicitly supports direct configuration. The root route does not choose an arbitrary agent.

Finite calls share one deadline for headers and body. Task streams close immediately after forwarding a terminal task/status event, even if the upstream leaves its connection open. Ownership expiry sweeps run at request reservation rather than on every artifact chunk.

Enabled agents require at least one gateway authentication method (API key or JWT). Stream chunks obey `server.stream_idle_timeout` in seconds; zero disables the idle bound. Finite successful responses must match the requested method: exactly one Task/Message for SendMessage, and a Task for GetTask/CancelTask.

Client messages require ROLE_USER. Upstream Task identity uses only id; Message/update identity uses taskId, and conflicting identity fields are rejected before ownership is recorded. Successful responses must contain the required status or message fields. Clean SSE EOF while a task is still active is an error; CancelTask is forwarded regardless of the older library cancellation flag and preserves TaskNotCancelableError from the upstream.

Agent Cards and RPC calls both negotiate `A2A-Version` through the header or case-insensitive query parameter. Unknown TaskState names are rejected before forwarding success or recording ownership. A task retains its observed context association; follow-up messages with mismatched owned task/context pairs fail before upstream dispatch. Named `a2a.<agent>` permissions can be provisioned through the public API-key creation interface. JSON-RPC error codes follow the A2A mapping; optional ErrorInfo details are not emitted by this initial binding (the JSON-RPC specification recommends them with SHOULD, unlike the separate HTTP binding).

Discovery is restricted to authenticated callers with the named agent permission. Operators provision gateway credentials out of band; anonymous discovery is outside this first scope. The [official discovery guidance](https://a2a-protocol.org/dev/topics/agent-discovery/#securing-agent-cards) permits endpoint access controls. The generated card version identifies the gateway release; protocolVersion separately identifies A2A 1.0.

Review boundaries: API keys need an explicit `a2a.<agent>` or administrator grant; empty permissions and `use:api` do not grant private-agent access. JWT user access follows existing user authorization. URL userinfo is rejected; query credentials require HTTPS. Agent Cards honor forwarded origin headers only from configured `server.trusted_proxies`; directly connected requests use their Host header and connection scheme. Non-immediate SendMessage rejects an upstream task still submitted/working. Retained context ownership renews together with its task.

Protocol details follow the [A2A 1.0 Message/Part definitions](https://a2a-protocol.org/v1.0.0/specification/#414-message):
Task contextId may be omitted; server Messages require a contextId. Part.data
accepts any JSON value, including null, while exactly one content variant is
required. SubscribeToTask rejects an initial terminal Task. SSE responses set
X-Accel-Buffering: no so a reverse proxy can forward events promptly.

Streams also close after INPUT_REQUIRED or AUTH_REQUIRED interruptions. A successful
CancelTask result must be CANCELED; other outcomes use the upstream error contract.
Task snapshots validate artifacts with the same rules as artifact updates and
honor requested historyLength (including zero). Invalid JSON-RPC IDs produce an
error with id null. SSE framing accepts independent CR, LF and CRLF line endings,
including mixed endings split across transport chunks.
