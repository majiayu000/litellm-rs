# Issue Tracker — Codebase + Provider API Audit (March 2026)

All issues created on GitHub: https://github.com/majiayu000/litellm-rs/issues

## Critical Issues (4)

| Issue | Title | Audit Ref |
|-------|-------|-----------|
| [#256](https://github.com/majiayu000/litellm-rs/issues/256) | fix(core): enable user_management module | C-01 |
| [#257](https://github.com/majiayu000/litellm-rs/issues/257) | fix(core): enable virtual_keys module | C-02 |
| [#258](https://github.com/majiayu000/litellm-rs/issues/258) | fix(config): YAML env var substitution not implemented | D-01 |
| [#259](https://github.com/majiayu000/litellm-rs/issues/259) | fix(providers): forward extra_params in OpenAILikeProvider | OpenRouter C-1 |

## High Severity Issues (17)

| Issue | Title | Audit Ref |
|-------|-------|-----------|
| [#260](https://github.com/majiayu000/litellm-rs/issues/260) | refactor(providers): split factory.rs (1227 lines) | A-01 |
| [#261](https://github.com/majiayu000/litellm-rs/issues/261) | fix(errors): replace unsafe .unwrap() in production hot paths | B-01 |
| [#262](https://github.com/majiayu000/litellm-rs/issues/262) | refactor(errors): split oversized error files | B-02 |
| [#263](https://github.com/majiayu000/litellm-rs/issues/263) | test(errors): GatewayError variant mapping coverage | B-03 |
| [#264](https://github.com/majiayu000/litellm-rs/issues/264) | ci: compile-check disabled modules to prevent silent rot | C-03 |
| [#265](https://github.com/majiayu000/litellm-rs/issues/265) | fix(config): wire hot reload or remove dead code | D-02 |
| [#266](https://github.com/majiayu000/litellm-rs/issues/266) | fix(config): hardcoded string comparison in StorageConfig merge | D-04 |
| [#267](https://github.com/majiayu000/litellm-rs/issues/267) | feat(openai): add GPT-5.4 family, fix GPT-4.1 context window | OpenAI C-1 |
| [#268](https://github.com/majiayu000/litellm-rs/issues/268) | feat(openai): add reasoning_effort + developer message role | OpenAI C-4/C-5 |
| [#269](https://github.com/majiayu000/litellm-rs/issues/269) | feat(anthropic): add Sonnet 4.6/Haiku 4.5, fix context windows | Anthropic C-1~C-6 |
| [#270](https://github.com/majiayu000/litellm-rs/issues/270) | feat(gemini): add Gemini 3.1, fix systemInstruction, tool calls | Gemini C-1~C-4 |
| [#271](https://github.com/majiayu000/litellm-rs/issues/271) | feat(mistral): overhaul model catalog, 36+ models, vision | Mistral C-01~C-05 |
| [#272](https://github.com/majiayu000/litellm-rs/issues/272) | fix(openrouter): HTTP-Referer/X-Title headers + reasoning | OpenRouter C-3 |
| [#273](https://github.com/majiayu000/litellm-rs/issues/273) | feat(mistral): missing params, safe_prompt fix, Agents/OCR | Mistral H-01~H-08 |
| [#274](https://github.com/majiayu000/litellm-rs/issues/274) | fix(openai): update hardcoded capability lists | OpenAI H-6~H-9 |
| [#280](https://github.com/majiayu000/litellm-rs/issues/280) | feat(openai): Responses API (POST /v1/responses) | OpenAI C-3 |
| [#281](https://github.com/majiayu000/litellm-rs/issues/281) | fix(core): resolve critical TODOs — TeamManager, Redis, monitoring | C-03 TODOs |
| [#284](https://github.com/majiayu000/litellm-rs/issues/284) | feat(anthropic): beta headers, structured outputs, server-side tools | Anthropic H-1~H-4 |

## Medium Severity Issues (8)

| Issue | Title | Audit Ref |
|-------|-------|-----------|
| [#275](https://github.com/majiayu000/litellm-rs/issues/275) | fix(providers): ProviderType validation + trait object usability | A-02/A-03 |
| [#276](https://github.com/majiayu000/litellm-rs/issues/276) | refactor(core): reduce pub visibility, replace wildcard re-exports | C-04/C-05 |
| [#277](https://github.com/majiayu000/litellm-rs/issues/277) | refactor(config): split gateway.rs, fix pricing file path | D-03/D-05 |
| [#278](https://github.com/majiayu000/litellm-rs/issues/278) | chore(cleanup): resolve 58 dead_code suppressions | C-06 |
| [#279](https://github.com/majiayu000/litellm-rs/issues/279) | fix(errors): preserve provider context, split error utils | B-04/B-05 |
| [#282](https://github.com/majiayu000/litellm-rs/issues/282) | chore(providers): resolve DU provider files, document Tier boundary | A-04 |
| [#283](https://github.com/majiayu000/litellm-rs/issues/283) | fix(openai): store/metadata params, image models, deprecated models | OpenAI M-1~M-5 |

## Recommended Fix Order

### Wave 1 — P0 Data/Config Fixes (Low Risk, High Impact)
1. #258 YAML env var substitution
2. #259 extra_params forwarding (fixes all 44 Tier 1 providers)
3. #267 OpenAI model catalog + GPT-5.4
4. #268 reasoning_effort + developer role
5. #269 Anthropic models + context windows
6. #270 Gemini 3.1 + systemInstruction fix
7. #271 Mistral model catalog overhaul

### Wave 2 — P0 Core Features (Required for completeness)
8. #256 Enable user_management
9. #257 Enable virtual_keys
10. #264 CI compile-check for disabled modules
11. #281 Critical TODOs

### Wave 3 — P1 Quality
12. #261 .unwrap() audit
13. #263 Error mapping tests
14. #265 Hot reload dead code
15. #266 StorageConfig merge
16. #272 OpenRouter headers
17. #273 Mistral parameters
18. #274 OpenAI capability lists

### Wave 4 — P2 Architecture
19. #260 factory.rs split
20. #262 Error file splits
21. #276 Visibility reduction
22. #280 Responses API
23. #284 Anthropic features
