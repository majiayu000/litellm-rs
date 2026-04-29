# OpenRouter Provider Audit Report

**Date**: March 20, 2026
**Scope**: `src/core/providers/registry/catalog.rs` + `src/core/providers/openai_like/` vs OpenRouter API (openrouter.ai/docs)

## Architecture Note

OpenRouter is implemented as a **Tier 1 catalog provider** — a single data entry in `catalog.rs` instantiated as a generic `OpenAILikeProvider`. There is no dedicated OpenRouter module. It shares the identical code path as all other OpenAI-compatible providers (Groq, Together, DeepInfra, etc.). This is the root cause of nearly every gap below.

## CRITICAL (3)

### C-1: `extra_params` Silently Dropped

**File**: `openai_like/provider.rs`, lines 198-263

`transform_chat_request` manually constructs outgoing JSON from named fields only. The `ChatRequest.extra_params` HashMap (populated via `#[serde(flatten)]`) is **never forwarded**. All OpenRouter-specific parameters are silently discarded:

- `provider` object (order, sort, quantizations, max_price, allow_fallbacks, data_collection, require_parameters, ignore)
- `models` array (model fallback chain)
- `plugins` array (web search, file-parser, response-healing, context-compression)
- `reasoning` object (effort levels for thinking models)
- `route` parameter

**Impact**: Users cannot control provider routing, enable model fallbacks, use plugins, or control reasoning effort. These are OpenRouter's core differentiating features.

### C-2: Standard OpenAI Parameters Also Dropped

Same file, same method. Fields that exist on `ChatRequest` and are in `get_supported_openai_params` but **not forwarded**:

- `frequency_penalty`
- `presence_penalty`
- `logprobs` / `top_logprobs`
- `logit_bias`
- `parallel_tool_calls`

**Impact**: Affects all 44 Tier 1 catalog providers, not just OpenRouter.

### C-3: No OpenRouter-Specific HTTP Headers

**File**: `openai_like/provider.rs`, lines 97-121

`get_request_headers()` only sends `Authorization: Bearer` and optional `OpenAI-Organization`. Missing:

- `HTTP-Referer` — app URL for rankings
- `X-Title` / `X-OpenRouter-Title` — app display name
- `X-OpenRouter-Categories` — marketplace categories

`OR_SITE_URL` and `OR_APP_NAME` env vars are read in `auto_config.rs` but **never converted to HTTP headers**.

## HIGH (5)

### H-1: No Model Variant Suffixes Support

OpenRouter supports suffixes that modify routing:

- `:nitro` — prioritize throughput
- `:floor` — lowest price provider
- `:extended` — extended context windows
- `:online` — enable web search
- `:exacto` — quality-first tool-use routing

No validation, documentation, or handling in codebase.

### H-2: No Responses API Support

OpenRouter offers Responses API Beta (`POST /api/v1/responses`). Zero awareness in codebase.

### H-3: No Response Metadata Extraction

OpenRouter responses include extra metadata (cost, native_finish_reason, provider_responses). `transform_chat_response` deserializes directly to `ChatResponse`, losing all OpenRouter-specific fields.

### H-4: `reasoning` Parameter Not Forwarded

OpenRouter has `reasoning` request parameter with effort levels. The `ThinkingConfig` system in `thinking/providers.rs` has an `openrouter_thinking` module but it's **never wired into the actual request transform**.

### H-5: Structured Outputs Require Special Header

OpenRouter requires `x-openrouter-beta: structured-outputs-2025-11-13` for strict tool use. No mechanism to detect strict tools and add this header.

## MEDIUM (5)

| ID | Finding |
|----|---------|
| M-1 | Thinking detection uses substring matching, misses GPT-5.x, Grok, GLM 4.7 |
| M-2 | No pricing data for any OpenRouter model (cost calc returns 0.0) |
| M-3 | `openrouter/auto` and `openrouter/free` meta-model IDs not recognized |
| M-4 | Sends deprecated `max_tokens` instead of `max_completion_tokens` |
| M-5 | No BYOK (Bring Your Own Key) support via `provider.byok` |

## LOW (4)

| ID | Finding |
|----|---------|
| L-1 | `ProviderType::OpenRouter` enum variant exists but unused at runtime |
| L-2 | SDK maps "openrouter" to `ProviderType::OpenAI`, conflicting with provider_type.rs |
| L-3 | Tests reference deprecated model names (o1-preview, claude-3-sonnet) |
| L-4 | Base URL `https://openrouter.ai/api/v1` duplicated in 3 locations |

## Recommended Priority Fix

**Single highest-impact fix**: Forward `extra_params` in `OpenAILikeProvider.transform_chat_request()`. Add a loop over `request.extra_params` to inject them into outgoing JSON. This immediately unblocks all OpenRouter-specific parameters AND fixes C-2 for all 44 Tier 1 providers.

## Sources

- [OpenRouter API Reference](https://openrouter.ai/docs/api/reference/parameters)
- [OpenRouter Provider Routing](https://openrouter.ai/docs/guides/routing/provider-selection)
- [OpenRouter Model Fallbacks](https://openrouter.ai/docs/guides/routing/model-fallbacks)
- [OpenRouter Structured Outputs](https://openrouter.ai/docs/guides/features/structured-outputs)
- [OpenRouter Reasoning Tokens](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)
- [OpenRouter Responses API Beta](https://openrouter.ai/docs/api/reference/responses/overview)
- [OpenRouter App Attribution](https://openrouter.ai/docs/app-attribution)
