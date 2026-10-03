# MCP Streamable HTTP gateway

Build with `gateway,mcp` and configure `gateway.mcp_servers`. A server named `docs` is available at `/docs/mcp`; `/mcp` selects the only enabled server and otherwise requires a named path. The gateway forwards one upstream server per connection; it does not aggregate multiple servers into a synthetic tool list.

```yaml
mcp_servers:
  docs:
    name: docs
    url: https://your-mcp-server.example/mcp
    transport: http
    timeout_ms: 30000
    auth:
      type: bearer_token
      value: YOUR_UPSTREAM_TOKEN
```

These fields are inside `gateway`, alongside `providers` and `auth`. Use your deployment's secret-management process for credentials. The upstream URL must resolve to public addresses; outbound DNS and redirects are restricted by the existing network policy. Gateway credentials are consumed by gateway authentication and never forwarded upstream. The upstream credential above is shared by callers allowed to use this configured server.

Clients send `Authorization: Bearer <gateway JWT>` or `x-api-key: <gateway API key>` on every request. A Bearer value is interpreted as a JWT, not as an API key. Anonymous MCP calls are rejected even when anonymous model calls are enabled. Existing key permissions can restrict a key to `mcp.docs`; `allowed_endpoints`, when configured, must permit the actual path, such as `/docs/mcp`. Existing HTTP/IP/rate-limit middleware also applies. Browser Origin values require an exact entry in `server.cors.allowed_origins`; a wildcard does not grant MCP access.

Enabled MCP servers require JWT or API-key authentication in gateway configuration. The endpoint proxies POST, GET and DELETE; initialization, notifications, tools, resources, templates, prompts, JSON-RPC errors and SSE bytes are preserved. POST requires JSON and Accept containing both `application/json` and `text/event-stream`; GET requires `text/event-stream`. Send the returned `Mcp-Session-Id` and negotiated `MCP-Protocol-Version` on subsequent requests. Resume GET requests pass `Last-Event-ID` only for sessionful upstream servers. Upstream HTTP status and Retry-After are preserved. Header acquisition is bounded by `timeout_ms`; SSE remains open until either peer closes or `server.stream_idle_timeout` seconds pass without an upstream chunk (zero disables this idle bound). Dropping a client body closes the corresponding upstream body.

Sessions are bound to the authenticated key (or user for JWT), server and configured upstream account. The gateway replaces upstream session IDs with random gateway IDs. Sessions expire after one hour; successful DELETE or upstream 404 removes them. There are at most 4,096 process-local sessions. Restarting requires reinitialization. Multiple gateway replicas require session affinity; cross-replica session persistence is not implemented. Shared upstream credentials still imply a shared upstream account: gateway session isolation does not implement the upstream application's resource authorization.

This route supports Streamable HTTP, not legacy HTTP+SSE, stdio or WebSocket MCP transports. It does not acquire OAuth credentials, forward arbitrary client headers, generate tools from OpenAPI, apply per-tool permissions, or meter external tool charges. Unsupported configuration fields are rejected. Model-call content guardrails and token budgets do not inspect or meter opaque MCP messages. The separate `core::mcp::McpGateway` library client still has its older JSON transport contract.

References: [MCP Streamable HTTP specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [LiteLLM endpoint and authentication reference](https://docs.litellm.ai/docs/mcp_config_reference).

Expired sessions and sessions invalidated by URL/credential/header changes are reaped every five seconds using their original upstream credentials. The gateway attempts DELETE with at most 32 concurrent cleanup calls and a five-second maximum per attempt, then releases local capacity. Upstreams that reject DELETE or cannot be reached must provide their own expiration; cleanup failure is logged without credentials. Pending initializations keep their reserved slots. If headers allocate an upstream session but initialization fails or is cancelled, the same reaper attempts cleanup before releasing capacity. Before negotiation completes, that best-effort DELETE uses the requested protocol version (or the missing-version transport default); the upstream may still require its own expiration.

The 4096-session global bound also limits each authenticated owner to 128 sessions (429 on that owner limit). Only a successful JSON/SSE InitializeResult commits a session; application-level initialization errors preserve the upstream error; allocated upstream sessions are queued for cleanup. Initialization is buffered within the configured body/time bounds until the matching InitializeResult arrives. For SSE, complete events are inspected incrementally; the result and remaining stream are forwarded immediately without waiting for upstream EOF. Negotiated version strings are limited to 64 bytes. Transport forwarding excludes the configured gateway credential header. Upstream requests ask for identity encoding; unchanged response bodies retain Content-Encoding metadata.
