# Native Responses billing boundaries

Unreleased work for issue #1372 / PR #1374, verified 2026-10-03. This is a bounded first implementation, not complete hosted-tool billing. The native wire path and the billing support boundary are separate.

## Supported request scope

- Bounded native OpenAI `file_search` is supported for JSON/SSE requests with an explicit `max_tool_calls`; see the follow-up scope below.

- Text, instructions, reasoning and client-executed `function` / `custom` tools retain native JSON and SSE fields. Standard OpenAI service tier is sent explicitly so a project priority-tier default cannot silently change rates. Requests selecting another tier are rejected.
- Inline image data, uploaded image IDs, inline PDFs and uploaded PDF IDs use `POST /responses/input_tokens` on the selected OpenAI deployment before generation. Its strict integer count enters the existing prompt-plus-output budget reservation, replacing local multimodal estimates. The count includes the complete input, instructions, tool definitions, reasoning/text settings and any supplied predecessor. The inference request is not replaced by the count projection.
- Opaque inline reasoning/compaction data also requires counting. Server-side item references are rejected because this path has no owner binding for them. Remote image/file URLs are rejected: their bytes can change between counting and generation. Uploaded file contents or inline bytes must remain available to the provider; upstream rejection is preserved.
- Counting uses the same configured endpoint, credentials, model mapping and restricted connection pool as generation. Failed/unsupported counting, malformed counts and insufficient budget prevent the generation POST. No local-estimate fallback or Chat Completions retry is used to bypass counting.
- Copilot/Bedrock input-count support is not established by OpenAI’s documentation; these providers reject inputs requiring this count until their separate endpoint contract is implemented. Model endpoint eligibility belongs to F06, lifecycle/owner binding and predecessor storage to F07.

## Explicitly unsupported charges

Hosted web search, code interpreter/containers, image generation, remote MCP, hosted shell/computer tools and unknown tool types are rejected even if a caller hopes they will not be selected. Prompt templates, conversation handles, explicit containers and context-management settings are rejected because they can introduce context or charges outside this reservation. Audio/video input pricing is not implemented here.

OpenAI bills search calls and retrieved content, file-search calls/storage, container sessions and generated images separately from ordinary response token totals. This gateway does not infer those charges from tool transport, a price-table row, or an output token total. No new hosted-tool tariff configuration or generic billing engine was added.

Completed, incomplete and failed terminal responses use validated input/output/cached/reasoning usage; reasoning is already included in output tokens and is not billed twice. Missing or malformed usage, interrupted SSE and native error events retain the conservative reservation in provider/model and key budget accounting. The request ledger retains unknown cost; valid token usage remains available to the ledger, key statistics and callbacks even when tool-call charges are unknown. Key usage records an unpriced request without inventing actual cost or missing token counts. That reserved upper bound is budget protection, not a supplier invoice. Stateless unknown outcomes have no upstream recovery job; F07 separately persists background recovery obligations.

Compaction selects OpenAI deployments only, even when a model group contains another Responses-capable provider. Because compact has no output-limit parameter, its reservation uses the selected pricing snapshot's verified model maximum output tokens. Missing bounds are rejected before generation. Only the default service tier is supported.

Responses usage preserves `input_tokens_details.cache_write_tokens` through the existing cache-creation accounting field. Cached reads and cache writes must together fit within input tokens; invalid usage remains unknown. Admission covers the larger ordinary-input or cache-write cost, and terminal settlement uses the observed split. This follows [OpenAI prompt-caching billing](https://developers.openai.com/api/docs/guides/prompt-caching) and the [compact usage contract](https://developers.openai.com/api/reference/java/resources/responses/methods/compact).

## Evidence and implementation choice

Verified primary sources:

- [OpenAI token counting](https://developers.openai.com/api/docs/guides/token-counting): processed input counts cover image inputs and currently PDF file inputs, including file IDs and inline data.
- [OpenAI generated count parameters](https://github.com/openai/openai-python/blob/main/src/openai/types/responses/input_token_count_params.py): count request fields differ from generation-only stream/output/storage controls.
- [OpenAI pricing](https://developers.openai.com/api/docs/pricing): hosted tools have additional billing units; ordinary token settlement alone is insufficient.
- [LiteLLM Responses normalization](https://github.com/BerriAI/litellm/blob/main/litellm/responses/utils.py), `_transform_response_api_usage_to_chat_usage`: preserves cached/reasoning and provider-specific tool usage metadata rather than treating transport as billing evidence.
- [LiteLLM OpenAI cost calculation](https://github.com/BerriAI/litellm/blob/main/litellm/llms/openai/cost_calculation.py): delegates token pricing with service-tier/data-residency context.

Decision: adapt the official count endpoint and reuse existing catalog pricing, budget reservations and strict terminal usage normalization. Reject hosted-tool scope until both request bounds and terminal billing units are verified. Rejected alternatives are guessing PDF/image tokens from JSON size, treating unknown tools as free, or adding a new tariff platform. Model callability is not inferred from these price sources. Deployment-level regional pricing differences remain a separate pricing-audit limitation.

Validation uses local HTTP servers with no paid provider calls: image/PDF/file-ID JSON and SSE preservation; count-body projection; count errors/malformed counts; budget refusal before generation; hosted-tool/remote-input rejection; completed/incomplete/failed/unknown/error settlement; unknown actual key usage. Real supplier-account calls were not run.

Completed local checks for this batch: `cargo fmt --check`, `cargo check`, `cargo test` (7,266 library tests passed, 1 ignored, plus integration/doctests), `cargo clippy --all-targets -- -D warnings`, `cargo clippy --features gateway,sqlite --all-targets -- -D warnings`, `cargo test --features gateway,sqlite --test native_responses_routes` (16 passed), and `cargo test --features gateway,sqlite --lib responses_native` (5 passed). CI, merge and release are separate outcomes.


## Bounded OpenAI file search (#1372 follow-up, 2026-10-04)

`file_search` is supported on the native OpenAI path for synchronous JSON and
SSE requests with an explicit positive `max_tool_calls`. The original vector
store IDs, filters, result limit, tool choice, tool fields and native events are
forwarded unchanged. Other compatible providers, compaction and background
file-search recovery remain unsupported; other hosted tools are still rejected.

Admission counts the original processed input on the selected deployment and
reserves that input plus one verified model context window per possible tool
call, the bounded output, and $0.0025 per possible file-search call. The context
allowance conservatively covers additional model turns and retrieved content;
it does not assume the default chunk size or infer a token bound from
`max_num_results`. Missing model pricing/context, count failures, overflow or
insufficient provider/model/key budget prevent generation. This reuses the
existing request pricing snapshot and a single provider/key reservation.

Settlement combines validated native token/cache usage with distinct completed
`file_search_call` output items. SSE progress events and the request maximum are
not billed as executed calls. Missing/ambiguous call data, unsuccessful tool
statuses, duplicate IDs, calls beyond the admitted limit, missing token usage
and interruptions retain the reservation and record unknown actual cost. This
preserves the original response/error semantics; a reserved budget is never
presented as an invoice. Existing vector-store storage charges are account-level
charges and are outside this gateway request, which does not create/upload stores.

Verified: [OpenAI pricing](https://developers.openai.com/api/docs/pricing) lists
$2.50 per 1,000 Responses file-search calls and separate storage charges;
[Responses parameters](https://platform.openai.com/docs/api-reference/responses/create)
define `max_tool_calls` across built-in tools;
[file search](https://developers.openai.com/api/docs/guides/tools-file-search)
shows native `file_search_call` output items and retrieval options.

Decision: adapt these wire units into existing `PricingUsage` and request budget
settlement. No tariff configuration, SDK, generic tool billing layer or new
recovery schema was added. Web search was not selected because its content-token
billing needs a separate verified normalization contract. Local mock HTTP tests
exercise the gateway; no paid supplier-account call has been run.

Fresh follow-up verification: `cargo fmt --check`, `cargo check --locked`,
`cargo test --locked`, default all-target clippy, `gateway,sqlite,providers-extended` all-target
clippy, all 42 native Responses HTTP tests and all seven callback tests passed. CI, merge, supplier-account
calls and release acceptance are separate from these local results.

When `max_output_tokens` is omitted, file-search admission uses the verified model maximum output tokens. Missing model output bounds are rejected before generation; it never uses the generic 100-token estimate for this hosted-tool path.

## Ambiguous dispatches

Native Responses creation and compaction are non-idempotent even before response
headers arrive. The gateway does not retry or fail over their provider-operation
errors; ordinary chat retry behavior and safe pre-call budget/unpriced fallback
are unchanged. There is no assumed upstream idempotency-key contract.

A foreground network failure or response-header timeout retains its provider,
model and API-key budget upper bounds through unknown-usage settlement. API-key
actual cost/tokens remain unknown rather than treating the reservation as a bill.
Known foreground rejection, such as HTTP 401, releases its reservation. Background
creation retains its one durable dispatch obligation for existing reconciliation;
missing response headers do not cause another POST or another obligation.

Mock HTTP regression tests receive the complete POST before closing the connection
or withholding headers. They cover unary/streaming creation, compaction, known
rejection, and background reconciliation without paid provider calls.
