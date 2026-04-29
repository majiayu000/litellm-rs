# Google Gemini Provider Audit Report

**Date**: March 20, 2026
**Scope**: `src/core/providers/gemini/` vs Google Gemini API (ai.google.dev)

## CRITICAL (4)

### C-1: Missing Gemini 3.1 Series Entirely

Gemini 3.1 models completely absent from codebase:

- `gemini-3.1-pro-preview` — latest flagship
- `gemini-3.1-flash` — fast inference
- `gemini-3.1-flash-lite` — cheapest variant

### C-2: Deprecated Gemini 3 Pro Still Listed

Gemini 3 Pro was deprecated on March 9, 2026 but remains in the codebase model catalog without deprecation marking.

### C-3: System Instructions Handled Incorrectly

System instructions are prepended to the first user message instead of using the dedicated `systemInstruction` field in the Gemini API request body. This causes:

- Loss of system instruction semantics
- Incorrect behavior with multi-turn conversations
- Inconsistent handling across providers

**Fix**: Use the `systemInstruction` field directly:
```json
{
  "systemInstruction": {
    "parts": [{"text": "..."}]
  },
  "contents": [...]
}
```

### C-4: Tool/Function Calling Responses Silently Dropped

When Gemini returns function call responses, the response transformer silently drops them instead of converting to the unified tool call format. Users get empty or incomplete responses when using function calling.

## HIGH (3)

### H-1: thinkingConfig Not Implemented

Gemini 3 Pro/Flash support a `thinkingConfig` parameter with `thinkingBudget` for controlling reasoning token allocation. Not implemented in the request transformer.

### H-2: Gemini 2.0 Flash Deprecation Not Tracked

Gemini 2.0 Flash is deprecating June 1, 2026 but is still listed without any deprecation marking or migration guidance.

### H-3: Missing grounding/search_grounding Tool

Gemini supports a `google_search` grounding tool that provides real-time web search. Not represented in the tool types.

## MEDIUM (3)

| ID | Finding |
|----|---------|
| M-1 | Context window sizes not updated for 3.1 models |
| M-2 | Pricing data outdated |
| M-3 | Missing `cachedContent` support for context caching |

## LOW (2)

| ID | Finding |
|----|---------|
| L-1 | Safety settings format may be outdated |
| L-2 | Test fixtures reference old model names |

## Sources

- Google AI for Developers (ai.google.dev)
- Gemini API Reference
- Google Cloud Vertex AI Documentation
