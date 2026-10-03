# Realtime WebSocket gateway (first scope)

Build with `--features gateway,sqlite,websockets`. Connect to
`GET /v1/realtime?model=gpt-realtime-mini` with gateway HTTP authentication
(`x-api-key`, or a gateway JWT in `Authorization`). Provision keys with
`api.realtime`; model and endpoint restrictions remain enforced. Browser
credential subprotocols and OpenAI beta wire formats are not accepted.

The configured OpenAI deployment, router admission, model identity and pricing
snapshot determine the upstream. Credentials come from that deployment, never
from the caller. HTTPS, the existing endpoint DNS policy, and disabled redirects
apply to the upstream upgrade. Private endpoints still require the existing
explicit provider policy; production credentials require HTTPS. Content
security policies are not silently skipped: sessions are rejected when gateway
content guardrails are enabled (the default); operators must explicitly disable them with the existing `gateway.guardrails.enabled: false` setting for this first scope. Browser Origin must match an explicit CORS
origin. Model selection is fixed for the lifetime of the connection.

## Supported first increment

This is the OpenAI GA `/v1/realtime` **manual text/audio/function** interface.
It forwards native text, base64 audio, function-call events and upstream errors.
The gateway first disables automatic turn generation and input transcription
and requires an acknowledged session configuration before upgrading the caller.
Clients append/commit audio or create conversation items, then explicitly send
`response.create`. Only one response may be in flight. Each `response.create` consumes the existing
API-key RPM allowance (or enabled gateway default); the initial HTTP handshake
also counts as one request. Native model mappings are applied to the upstream
handshake. Client events supported
are `session.update`, `response.create`, `response.cancel`,
`conversation.item.create/delete/retrieve/truncate`, and
`input_audio_buffer.append/commit/clear`.

Automatic VAD, input transcription, image input, hosted MCP tools, session model
changes and unsupported client events return explicit errors before transport.
Only `function` tools are accepted; execution and any external charges are the
client application's responsibility. This increment does not support Azure,
Gemini, Bedrock, GPT-Live, WebRTC, SIP, browser ephemeral tokens or Responses
WebSocket mode. It does not implement the deprecated `core::realtime` client;
removal of those separate legacy types remains tracked in [#1402](https://github.com/majiayu000/litellm-rs/issues/1402).

Both peers' close codes/reasons and request-scoped native events are retained.
Frames use bounded buffers, with the existing server body-size setting as the
message limit; transport writes and idle connections use the provider timeout. Closing the downstream
socket drops the upstream connection. Router admission remains held for the
connection lifetime. No mid-session failover is attempted.

## Cost and budgets

Every `response.create` reserves the selected model's full input-context limit
plus the allowed maximum output, priced at the most expensive supported input
and output modality. The existing provider/model and API-key budget mechanisms
must both admit it **before** the event is forwarded. The output cap is the
smaller of the model limit and key limit; an explicit client cap must fit it.
This conservative approach intentionally requires more available budget than a
short actual response will cost. For the current embedded mini price row, the
full 32,000-input/4,096-output reservation is $0.40192. Lower budgets cannot
start that response even if the likely actual bill is much smaller.

A matching `response.done` settles text/audio and cached text/audio separately
using the pinned catalog prices. The [GPT-Realtime-2 model card](https://developers.openai.com/api/docs/models/gpt-realtime-2)
provides the cached-audio rate ($0.40 per million tokens), retained by the pricing synchronizer. Malformed/incomplete usage is never treated as
zero cost. Cancellation, interrupted connections, or upstream errors without
trusted usage settle the outstanding conservative reservation; unused reserves
are refunded only after valid usage. Errors before `response.created` are matched
to the originating `response.create` event ID. Completed tokens remain in router
TPM accounting if a later connection error occurs; error close frames count as
deployment failures. A failed key-usage database write is logged separately with
key ID, token count and settled cost, and does not suppress `response.done`;
automatic retries or durable reconciliation are not implemented. Abrupt task cancellation keeps the budget
reservation charged, but cannot asynchronously persist key usage, and process
crash reconciliation is not implemented. Thus this first increment must not
be represented as durable exactly-once accounting across process failures.

There is no separate price table, platform service, or session configuration
surface. Pricing/model metadata must already be reviewed in the gateway
catalog. Regions or custom endpoints with different billing rates require an
explicit, accurate existing pricing identity; account-specific discounts are
not inferred.

## Implementation decision and evidence

Required capability: a bounded Rust HTTP-to-WebSocket proxy using existing
provider routing, authentication and budgets, with local mock verification and
no paid calls. Decision: **adapt** the existing gateway transport and budget
primitives, **adopt** Actix WS/Tungstenite for WebSocket framing, and implement
only the Realtime route and modality settlement. No independent DNS/TLS stack,
generic realtime platform, or Python sidecar is introduced.

Verified protocol sources: [OpenAI WebSockets](https://developers.openai.com/api/docs/guides/voice-websockets)
and [Realtime billing](https://developers.openai.com/api/docs/guides/voice-latency-cost).
[LiteLLM's native handler](https://github.com/BerriAI/litellm/blob/main/litellm/llms/openai/realtime/handler.py)
provides comparable credential selection, forwarding and failure boundaries;
its Python transport cannot reuse this repository's restricted Rust client.
[Actix WS](https://docs.rs/actix-ws/latest/actix_ws/) and
[Tungstenite](https://docs.rs/tungstenite/latest/tungstenite/) own frame validation.
The principal tradeoff is conservative per-response budget admission instead
of speculative automatic-turn charging. Local tests cover bidirectional events,
authentication, budget/scope rejection, close propagation and modality/cache
costs. Real provider acceptance remains unverified without an account call.
