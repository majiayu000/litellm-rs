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
also counts as one request when that limiter is enabled. Every response rechecks the current key status, expiry, owner, permissions, model/output restrictions, budget and RPM policy. JWT sessions also reauthenticate the token and reload the active user and current team membership before each response. Native model mappings are applied to the upstream
handshake. Client events supported
are `session.update`, `response.create`, `response.cancel`,
`conversation.item.create/delete/retrieve/truncate`, and
`input_audio_buffer.append/commit/clear`.

Automatic VAD, input transcription, image input, hosted MCP tools, session model
changes and unsupported client events return explicit errors before transport.
Only `function` tools are accepted; execution and any external charges are the
client application's responsibility. This increment does not support Azure,
Gemini, Bedrock, GPT-Live, WebRTC, SIP, browser ephemeral tokens or Responses
WebSocket mode. The unused `core::realtime` client and its separate legacy types
have been removed; the executable entry is `server::routes::ai::realtime`.
See the [subsystem reconciliation](../audit/subsystem-reconciliation-2026-10-04.md).

Both peers' close codes/reasons and request-scoped native events are retained.
Frames use bounded buffers, with the existing server body-size setting as the
message limit; transport writes and idle connections use the provider timeout. Closing the downstream
socket drops the upstream connection. Initialization failures can retry another eligible deployment before the client upgrade. Every generation reacquires admission for the pinned deployment, including RPM/TPM and parallel-request limits; completed usage is settled immediately, and idle sockets release generation slots. TPM follows the existing streaming admission behavior (a minimal initial token estimate, settled to actual usage), rather than a predictive full-context token reservation. No mid-session failover is attempted.

## Cost and budgets

Every `response.create` reserves the selected model's full input-context limit
plus the allowed maximum output, priced at the most expensive supported input
and output modality. The existing provider/model and API-key budget mechanisms
must both admit it **before** the event is forwarded. The output cap is the
smaller of the model limit and key limit; an explicit client cap must fit it. Omitted caps preserve the last explicit session cap and are additionally bounded by the current key policy. The reservation uses this effective response cap, including later reductions in the key limit.
Session caps change only after an upstream `session.updated` acknowledgment;
a rejected update does not change the inherited response cap.
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
deployment failures, including normal close codes during an unfinished response. Failed response outcomes and provider errors before `response.created` affect provider health; rate-limit and authentication errors use the existing cooldown policy. Invalid client requests and cancelled/incomplete responses do not penalize provider health. Client protocol errors and disconnects do not penalize provider health. A failed key-usage database write is logged separately with
key ID, token count and settled cost, and does not suppress `response.done`;
automatic retries or durable reconciliation are not implemented. Abrupt task cancellation keeps the budget
reservation charged, but cannot asynchronously persist key usage, and process
crash reconciliation is not implemented. Thus this first increment must not
be represented as durable exactly-once accounting across process failures.

Budget settlement failures are logged separately with provider/model, key ID,
tokens and cost. Settlement of the other budget ledger and the key-usage write
are still attempted, and a trusted completed `response.done` is forwarded.
The settlement helper still returns an error; a failed budget backend can leave
accounting unresolved. No automatic retries or durable reconciliation are added.

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

Interrupted responses without trusted terminal usage retain one deployment RPM
and the reserved input/output token upper bound in TPM, without penalizing a
provider for a client disconnect. These are conservative admission counters,
not measured token usage. Budget rejection before forwarding cancels deployment
admission. Zero-output key policies are rejected before deployment selection.
The protocol's `max_output_tokens: "inf"` maps to the pinned model/current key
maximum; an unchanged public or wire model in `session.update` is accepted and
stripped, while model changes remain rejected. See the official
[session lifecycle](https://developers.openai.com/api/docs/guides/realtime-conversations#session-lifecycle-events)
and [output limit](https://developers.openai.com/api/reference/cli/resources/realtime/subresources/calls/methods/accept).

Provider budget reservations and settlement use the configured deployment provider
name; canonical OpenAI identity remains the source for pricing. For example, an
OpenAI deployment named `prod-openai` charges its `prod-openai` budget.

Session updates are serialized: clients must wait for session.updated (or the correlated rejection) before another update or response.create. Early creates receive a local invalid_request_error and are not forwarded. Cancelled/incomplete terminal responses lacking trustworthy modality usage remain visible; conservative reservation/admission is retained without marking a healthy provider failed. Provider-rejected creates still count toward deployment RPM.


2026-10-04 review corrections: native completed/failed/cancelled/incomplete terminal events remain forwarded when optional usage or modality details are absent. Exact valid usage settles actual cost; otherwise the existing conservative reservation is charged and admission retains estimated tokens. Known output/input cap violations and inconsistent present counters remain errors. Cancelled/incomplete and invalid-request outcomes stay neutral even with valid usage; provider failures still affect health and all forwarded creates count toward deployment RPM. Budget settlement failures remain logged while the native provider error is delivered. Each create reloads the live gateway default RPM unless an explicit current key RPM applies. Initialization rejects a reported session model that differs from the configured wire model before upgrading the client.

The optional usage contract is verified against the [official Realtime SDK types](https://github.com/openai/openai-node/blob/master/src/resources/realtime/realtime.ts); missing optional data is not fabricated as metered usage. Model identity validation does not establish vendor account/model availability. No new configuration or compatibility alias table is added.

Runtime policy updates also apply to existing sockets: enabling content guardrails blocks new responses. Correlated provider-side session update errors penalize deployment health; invalid client updates remain neutral. The official numeric output limit is 1–4096; model-maximum output above this range uses `inf` on the wire while reserving and enforcing the resolved model maximum internally. A key cap above 4096 but below that maximum cannot be represented by the upstream contract and returns HTTP 400 before opening an upstream connection. This caller policy error remains neutral for deployment health.

2026-10-04 follow-up acceptance: each generation rebinds the selected deployment
ID to the current runtime router. Removed/disabled deployments or changed OpenAI
transport/account/model configuration close the socket before generation; unchanged
transport uses the new router's current health and RPM/TPM/parallel admission.
Current key output caps are applied to the catalog model maximum, so explicit
larger responses can use raised or removed key caps without reconnecting. Omitted
response caps still inherit the last acknowledged session setting. Both initial
and later complete session acknowledgments must retain disabled input VAD and
transcription. Image checks inspect protocol input content only, not arbitrary
metadata or function schemas. Initialization errors retain upstream status and
cooldown classification; admission backend/health outages report server errors,
while actual RPM/TPM/parallel denials report rate limits.

Later session acknowledgments must also match the normalized output cap requested
by that update, or retain the last agreed cap when no change was requested.
Ignored, malformed or unexpectedly raised caps close the socket before generation.
Failed terminal responses retain authentication, permission and rate-limit error
classification for immediate deployment cooldown; invalid client requests remain neutral.

Live zero-output key policy is an authentication failure. Missing or unknown
terminal statuses are upstream protocol errors, never successful generations.
Initial acknowledgments must retain the requested empty tool list; later sessions
accept client function tools only and reject changed reported models. Provider
errors from forwarded conversation/audio operations affect health once, while
invalid client requests remain neutral.
