# Product Spec

## Linked Issue

GH-1066 / #1066

## User Problem

The gateway exposes Prometheus metrics, but its existing OpenTelemetry,
Datadog, and Langfuse integrations are not connected to startup or to real LLM
request execution. Operators can configure and import these components without
receiving request lifecycle data, while integration failures have no explicit
runtime isolation contract.

## Goals

- Let operators enable OpenTelemetry, Datadog, and Langfuse callback backends
  through gateway configuration.
- Emit start and exactly one terminal success/error event for each provider-backed
  chat, completion, response, and embedding request, including streaming requests.
- Export request ID, selected provider/model, latency, token usage, cost when
  pricing is available, and errors without delaying or failing the client request.
- Provide one reusable callback plugin boundary for built-in and custom backends.

## Non-Goals

- Capturing prompt or generated content; lifecycle events are metadata-only by
  default.
- Replacing the existing Prometheus HTTP metrics middleware.
- Adding new external observability SDK dependencies or changing provider
  routing, retry, cache, budget, or pricing policy.
- Instrumenting non-LLM file, batch-management, fine-tuning, image, audio,
  moderation, or rerank operations in this issue.

## Behavior Invariants

1. `monitoring.callbacks.backends` is empty by default; default gateway behavior
   and external network activity remain unchanged.
2. Each configured backend is initialized once at gateway startup. A backend
   initialization failure is logged with the backend name and does not prevent
   other backends or the HTTP server from starting.
3. Built-in and custom callbacks use the same lifecycle contract: one start
   event followed by exactly one success or error event for each observed
   provider-backed request.
4. Callback delivery is ordered per gateway dispatcher and never waits in the
   client request path. Queue closure or capacity exhaustion is reported as an
   error log rather than silently dropping data or failing the request.
5. Success events include the request ID, selected provider/model, elapsed
   latency, input/output tokens when supplied by the provider, and calculated
   cost when the existing pricing authority can calculate it. Unavailable data
   remains absent.
6. Error events include the request ID, selected provider/model when known,
   error text, and elapsed latency metadata. Callback failures never replace or
   alter the original gateway response.
7. Streaming requests emit one terminal event on normal completion, upstream
   error, idle timeout, conversion/serialization failure, or client disconnect.
   They do not emit an early success merely because response headers were sent.
8. Callback metadata must not include API keys, authorization headers, prompt
   content, generated content, or configured backend secrets.
9. Configuration rejects duplicate backend kinds and invalid queue/timeout
   values before server startup.

## Acceptance Criteria

- [ ] Gateway configuration can enable each of `opentelemetry`, `datadog`, and
      `langfuse`, while an omitted callback section remains disabled.
- [ ] Startup tests prove configured backends are registered and one failed
      backend does not block healthy backends or server construction.
- [ ] A test integration attached to a real provider-backed request observes
      ordered start plus success/error events with request ID, provider/model,
      latency, tokens, and cost when available.
- [ ] Streaming tests prove normal completion and each terminal failure path
      emit exactly one terminal event.
- [ ] Callback backpressure and backend errors are observable but do not change
      client response status or body.
- [ ] Prompt/output payloads and secrets are absent from lifecycle fixtures.

## Edge Cases

- A request rejected before provider execution does not emit an LLM start event.
- A cache hit does not claim a provider execution or fabricate token/cost data.
- Provider retries still describe one logical request; the terminal event names
  the final selected target when known.
- Missing provider usage or pricing produces absent token/cost fields rather
  than zero or guessed values.
- Shutdown drains queued callback events and flushes registered backends after
  the HTTP server stops accepting requests.

## Rollout Notes

The callback section is opt-in. Operators should enable one backend at a time,
confirm exporter connectivity in logs, and size the queue for peak request
volume. Reverting the implementation or removing all configured backends
restores the previous no-export behavior.
