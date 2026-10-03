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
      task_cancellation: true
      input_types: [text/plain, application/pdf]
      output_types: [text/plain]
```

POST A2A 1.0 JSON-RPC to `/a2a/research` with `A2A-Version: 1.0` (header or query parameter), JSON content type and your configured gateway API-key header (`x-api-key` is also accepted) or Bearer JWT. Gateway credentials never replace the configured upstream credential. Calls require `a2a.research` permission; key endpoint restrictions must allow the actual path. Anonymous calls are rejected even if model routes permit anonymous access. Existing HTTP/IP/rate middleware applies.

Supported methods: SendMessage, SendStreamingMessage, GetTask, CancelTask and SubscribeToTask. Message parts, artifacts, task statuses, RPC errors and SSE event bytes are preserved. The endpoint does not translate older protocol versions, aggregate agents, execute agents itself or acquire OAuth tokens. Task listing, push notifications and extended cards are unsupported and return a method error. Unsupported configuration such as non-A2A adapters, per-agent billing/limits and push notifications is rejected.

GET `/a2a/research/.well-known/agent-card.json` with gateway authentication for a minimal gateway card generated from configuration. Input/output MIME types and streaming/cancellation flags must describe the upstream. The card describes the gateway URL, contains no upstream credential, and publishes one generic gateway skill and does not claim upstream-specific skills or signatures. Empty mode lists default to `text/plain`; configure them explicitly for other media. Deploy behind HTTPS with the correct host/proxy configuration.

Task IDs, context IDs and referenced tasks are bound to the key (or JWT user) and configured upstream account. Query, cancel, subscribe and conversation continuation require a known owned ID. An account/URL change invalidates access; description, tags, timeout and capability edits preserve ownership. New messages reserve capacity before forwarding, and unused capacity is released on errors or disconnects. Ownership is process-local, bounded to 4096 identifiers and expires after an hour without an observed task update. Restart or expiry fails closed on old tasks; multi-replica deployments require affinity. This first HTTP implementation does not provide durable ownership recovery. Shared upstream credentials still mean a shared upstream account; the gateway cannot implement the remote agent's own resource authorization.

Header acquisition and finite response reads (including upstream errors) use timeout_ms and the configured body size limit. Streaming is bounded per event by server.max_body_size; stream interruption or malformed events fail instead of becoming successful results. Dropping the client body drops the upstream body. Model token budgets and LLM content guardrails do not inspect or meter opaque agent calls; agents may perform their own billable operations.

Validation uses local mock agents, not paid live providers. References: [A2A 1.0 specification](https://a2a-protocol.org/latest/specification/), [LiteLLM A2A gateway](https://docs.litellm.ai/docs/a2a).
