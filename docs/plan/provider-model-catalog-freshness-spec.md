# Provider Model Catalog Freshness Spec

## 0. Metadata

- Task: provider model catalog freshness audit and follow-up issues
- Repository: litellm-rs
- Date checked: 2026-05-24 Asia/Shanghai
- Compatibility strategy: required
- Submission strategy: per provider milestone

## 1. Goal

Refresh the large-provider model catalogs so user-visible model metadata,
pricing fallbacks, aliases, and docs do not describe stale "latest" models.

This spec covers static model registries, pricing fallbacks, provider docs, and
examples. It does not require exhaustive static model lists for pass-through
OpenAI-compatible providers unless this repository already exposes a static
registry for that provider.

## 2. Source Policy

Model data must come from official provider documentation or official launch
posts. Do not infer model IDs, context windows, prices, deprecation dates, or
alias behavior from third-party lists.

Official sources used for this audit:

- OpenAI models docs: https://platform.openai.com/docs/models
- OpenAI GPT-5.5 announcement: https://openai.com/index/introducing-gpt-5-5/
- OpenAI GPT-5.5 Instant announcement: https://openai.com/index/gpt-5-5-instant/
- Anthropic Claude models docs: https://docs.anthropic.com/en/docs/about-claude/models
- Google Gemini API models docs: https://ai.google.dev/gemini-api/docs/models
- Mistral models docs: https://docs.mistral.ai/getting-started/models/
- Cohere models docs: https://docs.cohere.com/docs/models
- Cohere Embed v4 docs: https://docs.cohere.com/docs/embed-v4
- Cohere rerank docs: https://docs.cohere.com/docs/rerank-overview
- DeepSeek API docs: https://api-docs.deepseek.com/
- Amazon Bedrock model cards: https://docs.aws.amazon.com/bedrock/latest/userguide/model-cards.html
- xAI models docs: https://docs.x.ai/docs/models
- Meta Llama model docs: https://www.llama.com/docs/model-cards-and-prompt-formats/

Each implementation issue must re-check the matching source before editing
because provider model catalogs change frequently.

## 3. Current Findings

| Provider | Repository evidence | Current-source delta | Priority |
| --- | --- | --- | --- |
| OpenAI | `src/core/providers/openai/models/static_models.rs` includes GPT-5.4 family, GPT image/audio/realtime 1.5, and many dated GPT-5 variants. | Official OpenAI sources now list GPT-5.5 as current flagship and include API model IDs such as `gpt-5.5` and `gpt-5.5-pro`; the repo has no GPT-5.5 entries. Treat the separate Instant API alias as `chat-latest`, not as a GPT-5.5-prefixed model ID. | P1 |
| Anthropic | `src/core/providers/anthropic/models.rs` and `src/core/providers/anthropic/models/catalog.rs` include Claude Opus 4.7, its latest alias, pricing, and focused tests. | Official Anthropic docs now recommend Claude Opus 4.8 (`claude-opus-4-8`) as the highest-capability model; the repo has no Opus 4.8 catalog/docs coverage yet. | P1 |
| Gemini | `src/core/providers/gemini/models.rs` includes Gemini 3.1 and older Gemini 3.0 / 2.5 entries; no `gemini-3.5-flash` was found. | Official Gemini API docs expose a current Gemini 3.5 family path, including Gemini 3.5 Flash. | P1 |
| Cohere | `src/core/providers/cohere/provider.rs` lists Command R/R+, legacy `command`, Embed v3, and Rerank v3. | Official Cohere docs list Command A generation, Embed v4, and Rerank v4 families; old `command` models are deprecated in Cohere docs. | P1 |
| DeepSeek | `docs/providers/deepseek.md` and pricing fallbacks describe DeepSeek V3.1 aliases, `deepseek-chat`, and `deepseek-reasoner`. | Official DeepSeek docs list `deepseek-v4-flash` and `deepseek-v4-pro`, with legacy `deepseek-chat` / `deepseek-reasoner` scheduled for deprecation on 2026-07-24. | P1 |
| Mistral | `src/core/providers/mistral/mod.rs` includes Large 2512, Small 4, Small 2506, Medium 2508, and Devstral 2 2512. | Official Mistral docs include current Mistral 3 / Medium 3.5 era model families; aliases and dated IDs need a focused refresh. | P2 |
| Amazon Nova / Bedrock | `src/core/providers/amazon_nova/models.rs` includes first-generation Nova text models and Nova Premier. | AWS has moved model ID data to Bedrock model cards; the Nova registry should be checked against current text, image, video, audio, and embedding support before adding or removing entries. | P2 |
| xAI | `src/core/providers/registry/catalog.rs` has an OpenAI-compatible xAI provider entry but no static xAI model registry. | Official xAI docs now document newer Grok aliases and model families. Because xAI is pass-through here, the task is docs/config smoke coverage, not necessarily a full static registry. | P2 |
| Meta Llama | `src/core/providers/meta_llama/mod.rs` includes Llama 4 Scout and Maverick plus Llama 3.x entries. | No urgent stale flagship gap was confirmed in this audit; keep as low-priority validation for pricing, docs, and aliases. | P3 |

## 4. Implementation Rules

- Preserve backwards-compatible aliases unless the upstream source confirms
  removal or deprecation semantics.
- Prefer deprecation metadata or docs warnings over deleting still-routable
  model IDs.
- Do not mark a model "latest" unless the official source currently says so.
- Update pricing fallbacks only when the official source provides pricing.
- If pricing is not official or not available, leave pricing blank rather than
  inventing values.
- For OpenAI-compatible pass-through providers, document pass-through behavior
  and add representative smoke coverage instead of pretending to own exhaustive
  static catalogs.
- For provider models whose runtime availability can vary by account, region, or
  cloud marketplace entitlement, verify availability before enabling live
  runtime support or documenting live smoke-test readiness.
- Every provider refresh issue must include focused tests for lookup,
  capabilities, aliases, pricing, and docs examples touched by that issue.

## 5. Provider Work Items

### Step A1 - OpenAI and Anthropic flagship refresh

- Status: pending
- Target files:
  - `src/core/providers/openai/models/static_models.rs`
  - `src/core/providers/openai/models/registry.rs`
  - `src/core/providers/openai/models/registry_types.rs`
  - `src/core/cost/calculator/pricing.rs`
  - `src/core/cost/utils.rs`
  - `src/config/builder/presets.rs`
  - `src/sdk/config.rs`
  - `src/core/providers/anthropic/models.rs`
  - `src/core/providers/anthropic/models/catalog.rs`
  - related provider tests and docs
- Changes:
  - Add GPT-5.5 canonical and alias entries from official OpenAI sources, without inventing undocumented aliases such as `gpt-5.5-chat-latest`.
  - Update OpenAI registry family detection, capability matching, and defaults so helper behavior matches the new static catalog entries.
  - Update OpenAI pricing fallbacks, model-category helpers, config presets, and SDK defaults so runtime cost estimation and generated configs do not keep stale GPT-5.4-only defaults.
  - Add Claude Opus 4.8 catalog/docs coverage from official Anthropic docs, then verify account and region availability before enabling live runtime support claims.
  - Update Anthropic SDK defaults and the `models/catalog.rs` registrations so convenience configs and provider listings do not keep stale Claude Opus 4.7-only coverage.
  - Keep GPT-5.4 and Claude 4.6 entries routable unless upstream deprecates them.
- Tests:
  - `cargo check`
  - provider-specific registry tests

### Step A2 - Gemini and Cohere refresh

- Status: pending
- Target files:
  - `src/core/providers/gemini/models.rs`
  - `src/core/cost/calculator/pricing.rs`
  - `src/core/cost/utils.rs`
  - `src/core/providers/cohere/provider.rs`
  - `src/core/providers/cohere/config.rs`
  - related provider tests and docs
- Changes:
  - Add Gemini 3.5 Flash/current Gemini 3.5 entries confirmed by Google docs.
  - Update shared Vertex/Gemini cost fallbacks so runtime cost does not keep stale generic `gemini-flash` pricing after the registry refresh.
  - Add Cohere Command A, Command A+ (`command-a-plus-05-2026`), Embed v4, and Rerank v4 entries.
  - Route Cohere Rerank v4 requests through the documented v2 rerank endpoint instead of only updating the static model list.
  - Mark or document deprecated Cohere legacy `command` models.
- Tests:
  - `cargo check`
  - provider-specific registry tests

### Step A3 - DeepSeek V4 refresh

- Status: pending
- Target files:
  - `docs/providers/deepseek.md`
  - `examples/deepseek_completion.rs`
  - `examples/README.md`
  - `examples/providers/README.md`
  - `src/core/cost/calculator/pricing.rs`
  - `src/core/providers/openai_like/models.rs`
  - DeepSeek provider tests or registry tests
- Changes:
  - Add `deepseek-v4-flash` and `deepseek-v4-pro` docs/pricing metadata where official pricing is available.
  - Add DeepSeek V4 runtime model info for the OpenAI-like provider path so provider-level `calculate_cost` does not return zero for known DeepSeek V4 IDs.
  - Document `deepseek-chat` and `deepseek-reasoner` as legacy aliases with the official 2026-07-24 deprecation date.
  - Update shipped DeepSeek examples so they either use the current V4 IDs or explicitly label legacy aliases with the deprecation date.
- Tests:
  - `cargo check`
  - pricing or registry tests covering both new and legacy IDs

### Step A4 - Mistral and Amazon Nova / Bedrock refresh

- Status: pending
- Target files:
  - `src/core/providers/mistral/mod.rs`
  - `config/model_prices_extended.json`
  - `src/core/providers/amazon_nova/models.rs`
  - `src/core/providers/bedrock/model_config.rs`
  - `src/core/providers/bedrock/utils/cost.rs`
  - related provider tests and docs
- Changes:
  - Reconcile Mistral aliases and dated IDs with the current official Mistral model docs.
  - Reconcile both the standalone Amazon Nova registry and native Bedrock Nova catalog/cost tables with current Bedrock model cards, adding only model families supported by this repository's provider surface.
- Tests:
  - `cargo check`
  - provider-specific registry tests

### Step A5 - Pass-through provider docs and smoke coverage

- Status: pending
- Target files:
  - `src/core/providers/registry/catalog.rs`
  - docs/examples/config files for xAI/OpenAI-compatible providers
  - provider routing tests
- Changes:
  - Clarify which providers intentionally do not maintain exhaustive static model registries.
  - Add representative smoke coverage for current xAI/Grok aliases if a test harness exists.
  - Validate Meta Llama docs/pricing aliases without making speculative changes.
- Tests:
  - `cargo check`
  - provider routing/catalog tests

## 6. Completion Criteria

- All P1 issues have official-source links in the issue body or implementation PR.
- Static registries no longer label stale models as "latest".
- Public docs do not present deprecated aliases as the newest model line.
- New or changed model IDs are covered by focused tests.
- Fresh verification output is produced before claiming completion:
  - `cargo check`
  - relevant provider-specific tests
  - `cargo test` before final submission or merge

## 7. Issue Map

- P1 OpenAI: add GPT-5.5 and update media/realtime aliases.
- P1 Anthropic: add Claude Opus 4.8 catalog/docs coverage, then verify
  account/region availability before making live support claims.
- P1 Gemini + Cohere: update Gemini 3.5 and Cohere Command A / Embed v4 / Rerank v4.
- P1 DeepSeek: add V4 models and document legacy alias deprecation.
- P2 Mistral + Amazon Nova + pass-through providers: reconcile remaining large-provider gaps and docs.
