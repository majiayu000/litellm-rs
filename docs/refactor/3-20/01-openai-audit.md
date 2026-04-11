# OpenAI Provider Audit Report

**Date**: March 20, 2026
**Scope**: `src/core/providers/openai/` vs OpenAI API (developers.openai.com)

## CRITICAL (5)

### C-1: Missing GPT-5.4 Model Family

The codebase stops at GPT-5.2. GPT-5.4 (released March 5, 2026) and variants are completely absent:

- `gpt-5.4` — flagship model, 1.05M context, $2.50/$15.00 per 1M tokens
- `gpt-5.4-mini` — high-volume variant
- `gpt-5.4-nano` — cheapest variant, $0.20/$1.25 per 1M tokens
- `gpt-5.4-pro` — extended compute variant, $30/$180 per 1M tokens

No `OpenAIModelFamily::GPT54*` variants exist in `registry_types.rs`. No entries in `static_models.rs`.

**Files**: `static_models.rs`, `registry_types.rs`, `registry.rs`

### C-2: Missing GPT-5.3 Model Family

`gpt-5.3-codex` (Responses API-only coding model) and `gpt-5.3-instant` not present.

### C-3: No Responses API Support (`POST /v1/responses`)

The Responses API (released March 2025) is OpenAI's recommended API for all new development. Assistants API sunsets August 2026. Zero awareness in codebase:

- No `POST /v1/responses` endpoint routing
- No built-in tools: `web_search`, `file_search`, `code_interpreter`, `computer_use`, remote MCP servers
- No `previous_response_id` for conversation chaining
- No `input` parameter format (differs from `messages`)
- No response object structure handling

The `execute_chat_completion` in `client.rs` hardcodes `/chat/completions` path.

**Single largest architectural gap.**

### C-4: Missing `reasoning_effort` Parameter

The `reasoning` object with `effort` levels (`none`, `minimal`, `low`, `medium`, `high`, `xhigh`) is critical for o-series and GPT-5.x models. Zero results for `reasoning_effort` across entire OpenAI provider.

`ChatRequest` has `thinking: Option<ThinkingConfig>` but it's never serialized as `reasoning.effort`. `OpenAIChatRequest` in `api_types.rs` has no `reasoning` field.

**Files**: `chat.rs`, `api_types.rs`, `transformer/request.rs`, `client.rs`

### C-5: Missing `developer` Message Role

OpenAI introduced the `developer` role (replacing `system` for o-series+). `MessageRole` enum only has: `System`, `User`, `Assistant`, `Tool`, `Function`. Missing `Developer`.

**File**: `src/core/types/message.rs`

## HIGH (9)

### H-1: Missing Deep Research Models

`o3-deep-research` ($10/$40 per 1M) and `o4-mini-deep-research` ($2/$8 per 1M) are production models not listed.

### H-2: Missing GA Realtime Models

`get_supported_models()` in `realtime.rs` returns only `gpt-4o-realtime-preview` and `gpt-4o-realtime-preview-2024-10-01`. Missing:

- `gpt-realtime` — flagship GA realtime model
- `gpt-realtime-mini` — cost-efficient GA realtime model
- `gpt-4o-mini-realtime-preview`

### H-3: Built-in Tool Types Not Supported

`OpenAITool` struct only supports `type: "function"`. Missing:

- `type: "web_search"` with `web_search_options`
- `type: "file_search"` with `vector_store_ids`
- `type: "code_interpreter"` with `container` config
- `type: "computer_use"` with `display` config
- `type: "mcp"` with `server_url` and OAuth config

### H-4: Predicted Outputs Not Wired

`PredictionConfig` exists in `advanced_chat.rs` but not integrated into `ChatRequest` or `OpenAIChatRequest`.

### H-5: GPT-4.1 Context Window Incorrect

Static models list GPT-4.1/mini/nano all with `128000`. Real API: **1,000,000 tokens** (7.8x underestimate).

**File**: `static_models.rs` lines 52-76

### H-6: Structured Outputs Model List Outdated

`get_structured_output_models()` only lists 4 gpt-4o variants. Missing: GPT-4.1, GPT-5.x, o3, o4-mini.

### H-7: Reasoning Model List Outdated

`get_reasoning_models()` only lists `o1-preview` and `o1-mini` (both deprecated April 2025). Should include: `o1`, `o3`, `o3-mini`, `o3-pro`, `o4-mini`, GPT-5.x.

### H-8: O-Series Capabilities Wrong

`get_model_capabilities()` claims o-series doesn't support function calling, streaming, or temperature. Since o1 GA (December 2024), all are supported.

### H-9: O1 Family Params Too Restrictive

O1 param list: `["messages", "model", "max_completion_tokens", "stream", "user"]`. Missing: `tools`, `tool_choice`, `reasoning_effort`, `response_format`, `developer` role, `store`, `metadata`.

## MEDIUM (12)

| ID | Finding |
|----|---------|
| M-1 | Missing `o3` base model (only o3-mini and o3-pro listed) |
| M-2 | `o1-preview` and `o1-mini` deprecated April 2025 but still listed |
| M-3 | `codex-mini-latest` deprecated Nov 2025, removed Feb 2026, still listed |
| M-4 | Missing cached token pricing (50-90% discounts) |
| M-5 | Image generation utils only list DALL-E 2/3, not `gpt-image-1`/`gpt-image-1.5` |
| M-6 | Audio model list only has old previews, missing GA models |
| M-7 | GPT-5.2 context window may be wrong (400k vs possible 272k) |
| M-8 | Missing `store` parameter (boolean for model improvement) |
| M-9 | Missing `metadata` parameter in main request type |
| M-10 | Cost estimation only handles 6 old models |
| M-11 | Batch API support list too narrow |
| M-12 | Fine-tuning model list outdated (missing GPT-4o, GPT-4.1-mini) |

## LOW (8)

| ID | Finding |
|----|---------|
| L-1 | `babbage-002` and `davinci-002` deprecated, still listed |
| L-2 | `gpt-3.5-turbo-instruct` likely fully sunset |
| L-3 | Audio voice list may be outdated for Realtime |
| L-4 | Realtime max output tokens limit too low (4096) |
| L-5 | Missing `service_tier` parameter ("default"/"flex") |
| L-6 | Missing `web_search_options` parameter |
| L-7 | `gpt5_features` defaults to `false` (GPT-5 GA since mid-2025) |
| L-8 | Test model references stale (gpt-4-vision, o1-preview, o1-mini) |

## Sources

- [OpenAI Models](https://developers.openai.com/api/docs/models)
- [Introducing GPT-5.4](https://openai.com/index/introducing-gpt-5-4/)
- [GPT-5.4 mini and nano](https://openai.com/index/introducing-gpt-5-4-mini-and-nano/)
- [Migrate to Responses API](https://platform.openai.com/docs/guides/migrate-to-responses)
- [Reasoning Models Guide](https://developers.openai.com/api/docs/guides/reasoning)
- [OpenAI Pricing](https://developers.openai.com/api/docs/pricing)
- [Structured Outputs](https://developers.openai.com/api/docs/guides/structured-outputs)
- [MCP and Connectors](https://developers.openai.com/api/docs/guides/tools-connectors-mcp)
- [GPT-4.1 Introduction](https://openai.com/index/gpt-4-1/)
- [OpenAI Deprecations](https://developers.openai.com/api/docs/deprecations)
