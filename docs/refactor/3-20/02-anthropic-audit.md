# Anthropic Provider Audit Report

**Date**: March 20, 2026
**Scope**: `src/core/providers/anthropic/` vs Anthropic API (docs.anthropic.com)

## CRITICAL (6)

### C-1: Missing Claude Sonnet 4.6 and Haiku 4.5

Two current-generation models completely absent:

- `claude-sonnet-4-6` — latest Sonnet, released early 2026
- `claude-haiku-4-5-20251001` — latest Haiku, released October 2025

### C-2: Wrong Model IDs with Fabricated Date Stamps

Codebase uses incorrect date suffixes:

| Codebase ID | Correct ID |
|-------------|------------|
| `claude-sonnet-4-5-20251101` | `claude-sonnet-4-5-20250929` |
| Other fabricated dates | Need verification against API |

### C-3: Context Window Wrong for Opus 4.6

Codebase: 200,000 tokens. Actual: **1,000,000 tokens** (5x underestimate).

### C-4: Max Output Tokens Wrong for Opus 4.6

Codebase: 32,768 tokens. Actual: **128,000 tokens** (4x underestimate).

### C-5: Extended Thinking Parameter Never Serialized

The `thinking` field exists on `ChatRequest` but is **never serialized** into the outgoing Anthropic API request. The thinking configuration is defined but dead code.

### C-6: MAX_OUTPUT_TOKENS Constant Outdated

Set to 8,192 — severely outdated. Current Anthropic models support 8k-128k depending on model.

## HIGH (5)

### H-1: No Beta Headers Support

Anthropic uses `anthropic-beta` header for feature gating (e.g., extended thinking, computer use). No mechanism to send these headers.

### H-2: No Structured Outputs Support

Anthropic now supports JSON schema-based structured outputs. Not implemented.

### H-3: No Citations Support

Anthropic's citation feature for grounded responses is not supported.

### H-4: No Server-Side Tools

Anthropic's built-in tools (web search, computer use) are not represented in the tool types.

### H-5: Missing Model Capabilities Metadata

Token limits, supported features, and pricing for Claude 4.x generation are not accurately represented.

## MEDIUM (4)

| ID | Finding |
|----|---------|
| M-1 | Pricing data outdated for all Claude models |
| M-2 | No cached token pricing support |
| M-3 | Missing `top_k` parameter in request |
| M-4 | Tool use format may be outdated |

## LOW (3)

| ID | Finding |
|----|---------|
| L-1 | Test fixtures reference deprecated model IDs |
| L-2 | Error messages reference old model names |
| L-3 | No support for Anthropic's prompt caching headers |

## Sources

- Anthropic API Documentation (docs.anthropic.com)
- Claude Model Cards
- Anthropic Pricing Page
