# Provider API Audit Summary (March 20, 2026)

## Overview

Comprehensive audit of 5 major AI provider implementations in litellm-rs against their real production APIs as of March 2026. Each provider was independently audited by a specialized agent that read all codebase files and verified against current API documentation via web search.

## Findings Summary

| Provider | CRITICAL | HIGH | MEDIUM | LOW | Total |
|----------|----------|------|--------|-----|-------|
| OpenAI | 5 | 9 | 12 | 8 | 34 |
| Anthropic | 6 | 5 | 4 | 3 | 18 |
| Gemini | 4 | 3 | 3 | 2 | 12 |
| OpenRouter | 3 | 5 | 5 | 4 | 17 |
| Mistral | 5 | 10 | 6 | 6 | 27 |
| **Total** | **23** | **32** | **30** | **23** | **108** |

## Cross-Provider Critical Themes

### 1. Model Catalogs 1-2 Years Behind (All 5 Providers)

- **OpenAI**: Missing GPT-5.3, GPT-5.4 (flagship March 2026), o3 base model
- **Anthropic**: Missing Claude Sonnet 4.6, Haiku 4.5; wrong model ID date stamps
- **Gemini**: Missing entire Gemini 3.1 series; deprecated Gemini 3 Pro still listed
- **Mistral**: Only 5 models from 2023 era; missing 36+ modern models
- **OpenRouter**: No auto/free router meta-models

### 2. Context Windows Wrong by 4-8x (4/5 Providers)

| Provider | Model | Codebase | Actual | Error |
|----------|-------|----------|--------|-------|
| OpenAI | GPT-4.1 | 128k | 1M | 7.8x |
| Anthropic | Opus 4.6 | 200k | 1M | 5x |
| Mistral | Large 3 | 32k | 262k | 8.2x |
| Mistral | Small 4 | 32k | 256k | 8x |

### 3. Responses API Missing (OpenAI + OpenRouter)

OpenAI's Responses API (`POST /v1/responses`) released March 2025 is the recommended API for all new development. Assistants API sunsets August 2026. Zero support in codebase.

### 4. Reasoning/Thinking Parameters Not Serialized (3/5 Providers)

- **OpenAI**: `reasoning_effort` parameter completely absent
- **Anthropic**: `thinking` field exists but never serialized to request
- **Gemini**: `thinkingConfig` not implemented

### 5. OpenAILike extra_params Silently Dropped

`transform_chat_request()` in `OpenAILikeProvider` manually constructs JSON from named fields. The `extra_params` HashMap is never forwarded. Affects all 44 Tier 1 catalog providers.

### 6. Vision/Multimodal Support Gaps

- **Mistral**: All models marked `supports_multimodal: false` but Large 3, Small 4, Pixtral all support vision
- **Gemini**: System instructions prepended to user message instead of using `systemInstruction` field

## Priority Fix Recommendations

### P0 — Immediate (Data Changes, High Impact, Low Risk)

1. Update all provider model catalogs (model IDs, context windows, output limits, pricing)
2. Fix `extra_params` forwarding in `OpenAILikeProvider.transform_chat_request()`
3. Add `Developer` to `MessageRole` enum
4. Add `reasoning_effort` parameter to request types

### P1 — Short Term (Feature Gaps)

5. Fix Gemini `systemInstruction` field handling
6. Enable Mistral vision support
7. Add OpenRouter HTTP headers (X-Title, HTTP-Referer)
8. Wire Anthropic extended thinking serialization

### P2 — Architecture (Design Required)

9. OpenAI Responses API support (`POST /v1/responses`)
10. Mistral Agents API support (`POST /v1/agents/completions`)
11. Mistral OCR/Audio endpoints
12. OpenRouter Responses API Beta

## Detailed Reports

- [01-openai-audit.md](01-openai-audit.md) — 34 findings
- [02-anthropic-audit.md](02-anthropic-audit.md) — 18 findings
- [03-gemini-audit.md](03-gemini-audit.md) — 12 findings
- [04-openrouter-audit.md](04-openrouter-audit.md) — 17 findings
- [05-mistral-audit.md](05-mistral-audit.md) — 27 findings
