# Mistral Provider Audit Report

**Date**: March 20, 2026
**Scope**: `src/core/providers/mistral/` + `src/core/providers/codestral/` vs Mistral AI API (docs.mistral.ai)

## CRITICAL (5)

### C-1: Missing ALL Modern Mistral Models (36+ Missing)

The codebase hardcodes exactly 5 models from the 2023 era:

| Codebase Model | Status |
|----------------|--------|
| `mistral-tiny` | DEPRECATED/RETIRED |
| `mistral-small` | Legacy alias (current: Small 4, 256k) |
| `mistral-medium` | Legacy alias (current: Medium 3.1, 131k) |
| `mistral-large` | Legacy alias (current: Large 3, 262k) |
| `mistral-embed` | Valid but only 8k context |

**Missing model families**:

| Family | Model IDs | Context | Input/Output $/1M |
|--------|-----------|---------|-------------------|
| **Mistral Large 3** | `mistral-large-2512` | 262k | $0.50/$1.50 |
| **Mistral Medium 3.1** | `mistral-medium-2508` | 131k | $0.40/$2.00 |
| **Mistral Small 4** | `mistral-small-4` | 256k | $0.15/$0.60 |
| **Mistral Small 3.2** | `mistral-small-2506` | 131k | $0.075/$0.20 |
| **Magistral Medium 1.2** | `magistral-medium-1-2` | 40k | $2.00/$5.00 |
| **Magistral Small 1.2** | `magistral-small-1-2` | 40k | est. |
| **Ministral 3 14B** | `ministral-14b-2512` | 262k | $0.20/$0.20 |
| **Ministral 3 8B** | `ministral-8b-2512` | 262k | $0.15/$0.15 |
| **Ministral 3 3B** | `ministral-3b-2512` | 131k | $0.10/$0.10 |
| **Pixtral Large** | `pixtral-large-2411` | 131k | $2.00/$6.00 |
| **Pixtral 12B** | `pixtral-12b-2409` | 128k | $0.15/$0.15 |
| **Devstral 2** | `devstral-2-2512` | 262k | $0.40/$0.90 |
| **Devstral Medium** | `devstral-medium-1-0` | 131k | $0.40/$2.00 |
| **Devstral Small** | `devstral-small-1-1` | 131k | $0.07/$0.28 |
| **Mistral Nemo 12B** | `mistral-nemo-12b` | 131k | $0.02/$0.04 |
| **Codestral Embed** | `codestral-embed-2505` | — | — |
| **Saba** | `saba` | 33k | $0.20/$0.60 |

### C-2: Context Windows Wrong by 4-8x

Every model hardcoded to `32000` tokens:

| Model | Codebase | Actual | Error |
|-------|----------|--------|-------|
| mistral-large (→ Large 3) | 32,000 | 262,000 | **8.2x** |
| mistral-small (→ Small 4) | 32,000 | 256,000 | **8x** |
| mistral-medium (→ Medium 3.1) | 32,000 | 131,000 | **4x** |

### C-3: Missing Vision/Multimodal Support

`supports_multimodal: false` for ALL models. `MISTRAL_CAPABILITIES` has no vision capability. `transform_messages` does not handle image content parts.

Models that actually support vision:
- Mistral Large 3, Medium 3.1, Small 4, Small 3.2
- Ministral 3 14B/8B/3B
- Pixtral Large, Pixtral 12B

Mistral expects: `{"type": "image_url", "image_url": "https://..."}` in content parts.

### C-4: Missing Agents API Endpoint

Mistral has full Agents API at `POST /v1/agents/completions`:

- `agent_id` (required) — references pre-configured agent
- Built-in tools: `web_search`, `code_interpreter`, `image_generation`, `document_library`
- Standard params: `messages`, `max_tokens`, `stream`, `tools`, etc.

Zero awareness in codebase.

### C-5: Codestral Provider Entirely Outdated

Only knows 4 models:

| Codebase | Context | Status |
|----------|---------|--------|
| `codestral-latest` | 32k | Outdated |
| `codestral-2405` | 32k | Outdated |
| `codestral-mamba-latest` | 256k | May be deprecated |
| `codestral-mamba-2407` | 256k | May be deprecated |

Missing:
- `codestral-2508` — current production, 256k context, $0.30/$0.90
- `codestral-embed-2505` — code embedding, 3072 dims
- `devstral-2-2512` — code agent, 262k context

Base URL `https://codestral.mistral.ai/v1` may be outdated; Mistral consolidating under `https://api.mistral.ai/v1`.

## HIGH (10)

### H-1 to H-2: Missing `frequency_penalty` and `presence_penalty`

Both supported by Mistral API (default: 0). Not in supported params list or mapping.

### H-3: Missing `n` Parameter

Multiple completions per request. Not supported.

### H-4: Missing `json_schema` Structured Output Mode

`response_format` passed through but `{"type": "json_schema", "json_schema": {...}}` not explicitly supported/validated.

### H-5: Missing `prediction` Parameter

Predicted outputs for improved response times. Not present.

### H-6: Missing `guardrails` Parameter

Replaced deprecated `safe_prompt`. Configurable safety backed by `mistral-moderation-2`. Not present.

### H-7: Missing `parallel_tool_calls` Parameter

Boolean (default: true). Not in supported parameters.

### H-8: Missing OCR API Endpoint

`POST /v1/ocr` with models `mistral-ocr-3` (25.12), `mistral-ocr-2` (25.05). Supports PDF/image processing with `table_format`, `extract_header`, `extract_footer`. ~1000 pages/dollar.

### H-9: Missing Audio/Transcription Models (Voxtral)

5 audio models:
- `voxtral-mini-transcribe-2` (26.02)
- `voxtral-mini-transcribe-realtime` (26.02)
- `voxtral-mini-transcribe` (25.07)
- `voxtral-mini` (25.07)
- `voxtral-small` (25.07)

### H-10: Missing Moderation API Endpoint

`POST /v1/moderations` with `mistral-moderation-2` (26.03, 128k context, jailbreak detection).

## MEDIUM (6)

| ID | Finding |
|----|---------|
| M-1 | All hardcoded pricing completely wrong (2023 prices) |
| M-2 | Zero Mistral entries in global pricing database fallback |
| M-3 | `safe_prompt` hardcoded to `true` (deprecated, overrides user intent) |
| M-4 | Embedding handler hardcodes model to `mistral-embed`, ignoring user request |
| M-5 | Temperature validation `[0.0, 1.0]` too restrictive |
| M-6 | `max_output_length: None` for all models (should have defined limits) |

## LOW (6)

| ID | Finding |
|----|---------|
| L-1 | Model names use legacy aliases (mistral-tiny/small/medium/large) |
| L-2 | No `-latest` alias models defined |
| L-3 | Missing `system_fingerprint` passthrough in response |
| L-4 | `FimRequest` lacks `min_tokens` parameter |
| L-5 | Unused `MistralChatHandler`/`MistralChatTransformation` (dead code) |
| L-6 | Codestral pricing conversion uses outdated per-million values |

## Assessment

The Mistral provider reflects the state of the API from early-to-mid 2024 — approximately **2 years behind**. A near-complete rewrite of the model catalog, parameter support, capability declarations, and endpoint coverage is needed.

## Sources

- [Mistral AI Models](https://docs.mistral.ai/getting-started/models)
- [Mistral Chat API](https://docs.mistral.ai/api/endpoint/chat)
- [Mistral Vision](https://docs.mistral.ai/capabilities/vision)
- [Mistral Structured Outputs](https://docs.mistral.ai/capabilities/structured_output)
- [Mistral OCR API](https://docs.mistral.ai/api/endpoint/ocr)
- [Mistral Guardrailing](https://docs.mistral.ai/capabilities/guardrailing)
- [Mistral Agents API](https://docs.mistral.ai/api/endpoint/agents)
- [Introducing Mistral 3](https://mistral.ai/news/mistral-3)
- [Mistral Medium 3](https://mistral.ai/news/mistral-medium-3)
- [Mistral Small 3.1](https://mistral.ai/news/mistral-small-3-1)
