# Native Responses billing boundaries

Unreleased work for issue #1372 / PR #1374, verified 2026-10-03. This is a bounded first implementation, not complete hosted-tool billing. The native wire path and the billing support boundary are separate.

## Supported request scope

- Text, instructions, reasoning and client-executed `function` / `custom` tools retain native JSON and SSE fields. Standard OpenAI service tier is sent explicitly so a project priority-tier default cannot silently change rates. Requests selecting another tier are rejected.
- Inline image data, uploaded image IDs, inline PDFs and uploaded PDF IDs use `POST /responses/input_tokens` on the selected OpenAI deployment before generation. Its strict integer count enters the existing prompt-plus-output budget reservation, replacing local multimodal estimates. The count includes the complete input, instructions, tool definitions, reasoning/text settings and any supplied predecessor. The inference request is not replaced by the count projection.
- Opaque inline reasoning/compaction data also requires counting. Server-side item references are rejected because this path has no owner binding for them. Remote image/file URLs are rejected: their bytes can change between counting and generation. Uploaded file contents or inline bytes must remain available to the provider; upstream rejection is preserved.
- Counting uses the same configured endpoint, credentials, model mapping and restricted connection pool as generation. Failed/unsupported counting, malformed counts and insufficient budget prevent the generation POST. No local-estimate fallback or Chat Completions retry is used to bypass counting.
- Copilot/Bedrock input-count support is not established by OpenAI’s documentation; these providers reject inputs requiring this count until their separate endpoint contract is implemented. Model endpoint eligibility belongs to F06, lifecycle/owner binding and predecessor storage to F07.

## Explicitly unsupported charges

Hosted web/file search, code interpreter/containers, image generation, remote MCP, hosted shell/computer tools and unknown tool types are rejected even if a caller hopes they will not be selected. Prompt templates, conversation handles, explicit containers and context-management settings are rejected because they can introduce context or charges outside this reservation. Audio/video input pricing is not implemented here.

OpenAI bills search calls and retrieved content, file-search calls/storage, container sessions and generated images separately from ordinary response token totals. This gateway does not infer those charges from tool transport, a price-table row, or an output token total. No new hosted-tool tariff configuration or generic billing engine was added.

Completed, incomplete and failed terminal responses use validated input/output/cached/reasoning usage; reasoning is already included in output tokens and is not billed twice. Missing or malformed usage, interrupted SSE and native error events retain the conservative reservation in provider/model and key budget accounting. The request ledger retains unknown usage/cost, and key usage records an unpriced request with zero inferred actual tokens/cost. That reserved upper bound is budget protection, not a supplier invoice. Stateless unknown outcomes have no upstream recovery job; F07 separately persists background recovery obligations.

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
