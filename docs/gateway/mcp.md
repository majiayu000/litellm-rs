# MCP Streamable HTTP gateway

Build with `gateway,mcp` and configure `gateway.mcp_servers`. A server named `docs` is available at `/docs/mcp`; `/mcp` selects the only enabled server and otherwise requires a named path. Each request targets one upstream server. The gateway does not aggregate multiple servers into a synthetic tool list.

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

These fields are inside `gateway`, alongside `providers` and `auth`. Use your deployment's secret-management process for credentials. Upstream addresses must be public; the existing network policy restricts outbound DNS and disables redirects. Credentials, static upstream headers and URL queries require HTTPS. URL userinfo is rejected; configure credentials explicitly in auth or static_headers. Gateway credentials are consumed by gateway authentication and never forwarded upstream. The upstream credential above is shared by callers allowed to use this configured server, so authorization for upstream application resources remains the upstream's responsibility.

Clients send `Authorization: Bearer <gateway JWT>` or `x-api-key: <gateway API key>` on every request. A Bearer value is interpreted as a JWT. Enabled MCP servers require JWT or API-key authentication in configuration; anonymous MCP calls are always rejected. Existing key permissions can restrict a key to `mcp.docs`; `allowed_endpoints`, when configured, must permit the actual path, such as `/docs/mcp`. Existing HTTP/IP/rate-limit middleware also applies. Browser Origin values require an exact entry in `server.cors.allowed_origins`; a wildcard does not grant MCP access. Add any tool-specific `Mcp-Param-*` headers used by browser clients to the existing `server.cors.allowed_headers` list.

The gateway supports **MCP 2026-07-28** with stateless POST requests. There is no initialize handshake, transport session, standalone GET stream or DELETE operation. GET/DELETE return 405. Obsolete `Mcp-Session-Id` and `Last-Event-ID` headers are ignored and never forwarded or echoed. Requests can move between gateway instances without MCP transport affinity.

Every JSON-RPC request must carry matching HTTP/body metadata:

```http
POST /docs/mcp
Content-Type: application/json
Accept: application/json, text/event-stream
MCP-Protocol-Version: 2026-07-28
Mcp-Method: tools/call
Mcp-Name: get_weather
```

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_weather",
    "arguments": {"location": "Seattle"},
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {},
      "io.modelcontextprotocol/clientInfo": {"name": "example", "version": "1.0"}
    }
  }
}
```

`Mcp-Name` mirrors `params.name` for tools/call and prompts/get, or `params.uri` for resources/read. Non-ASCII, control characters, surrounding whitespace and literal Base64 sentinel strings use the specification's `=?base64?…?=` encoding. Missing/mismatched/duplicate standard headers return HTTP 400 with JSON-RPC -32020. Missing required request metadata returns -32602; unsupported versions return -32022 with the supported version list. Client info is optional. Tool-specific `Mcp-Param-*` headers pass unchanged to the upstream, which knows their schema and validates them. Configured gateway and upstream credentials cannot use these reserved header names or prefix.

Native discovery, tools, resources, templates and prompts retain their request/result fields, including pagination, `ttlMs`/`cacheScope`, and multi-round-trip `input_required` results and `inputResponses`. `subscriptions/listen` uses a POST response stream. JSON-RPC notifications from extensions are forwarded; the core specification defines no client notifications over this transport and does not impose request metadata headers on notification POSTs. The gateway preserves upstream HTTP statuses, JSON-RPC errors and Retry-After.

SSE bytes are passed through with `X-Accel-Buffering: no`. Request bodies are admitted before reading and bounded by `timeout_ms` and the configured body size limit. Upstream header acquisition is bounded by `timeout_ms`; finite response bodies have a further `timeout_ms` deadline. SSE remains open until either peer closes or `server.stream_idle_timeout` seconds pass without an upstream chunk (zero disables this idle bound). Dropping a client response closes its upstream body. Each instance permits at most 128 active MCP requests per authenticated key/user and 4,096 in total. These permits last until the response body finishes or is dropped, including streams that keep sending heartbeats; caller saturation returns 429 and total saturation returns 503. These are process-local admission counters, not protocol sessions. Responses are not stored in a shared HTTP cache. Upstream requests ask for identity encoding; unchanged bodies retain Content-Encoding metadata.

This route does not implement earlier MCP protocol versions, stdio or WebSocket transports, OAuth acquisition, arbitrary client-header forwarding, tools generated from OpenAPI, per-tool permissions, or billing for external tool charges. Unsupported configuration options are rejected. Model-call content guardrails and token budgets do not inspect or meter opaque MCP messages. The separate `core::mcp::McpGateway` library client still has its older JSON transport contract and is not used by this HTTP endpoint.

References: [MCP 2026-07-28 Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http), [per-request protocol metadata](https://modelcontextprotocol.io/specification/2026-07-28/basic/index), [LiteLLM endpoint and authentication reference](https://docs.litellm.ai/docs/mcp_config_reference).

API keys must explicitly grant `mcp.<server>` or administrator access. Empty permissions and legacy `use:api` do not grant access to configured private MCP servers. JWT access follows existing user authorization. Named routes are registered before gateway prefix scopes, so names such as `v1`, `v1beta`, `auth` and `health` remain usable. Exported JSON/YAML redact OAuth token URLs as well as explicit credential fields.
