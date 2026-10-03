# DeepSeek, xAI and Cohere review — 2026-10-03

Baseline: `fc6a437c3ee23de7cded5be25fbd3c9ab4799785`, issue #1373.
This is a bounded review of the existing native DeepSeek price identities, all 73 xAI static IDs/aliases, and all 29 pre-change Cohere static entries. It does not finish F10 or certify account-level availability.

Confirmed changes:

- DeepSeek native `deepseek-chat` / `deepseek-reasoner` retired 2026-07-24; retain four historical rows as pricing_only. Current Flash IDs still resolve to V4.1 Flash; do not remove accepted redirects. Qualified Pro row had 8192 output while the official contract and bare row say 384K; make limits consistent. Current first-party price card matches current catalog rates.
- Cohere deprecated Command models remain available to existing customers; no shutdown date is stated. Keep them. Model-level capabilities must match chat/embedding/rerank; no Cohere audio transcription implementation exists in the provider, so transcribe must not be offered as a callable chat model. Rerank search-unit prices must not be listed as token rates.
- xAI multi-agent is Responses-only and has no client function tools, despite the model overview card saying function calling. The dedicated multi-agent protocol guide is explicit; do not route it to Chat Completions. Other old xAI slugs are redirected, not uniformly uncallable. Output limits independent of context are not published by the reviewed model cards; the static 4096 default is unsupported.

Unresolved:

- xAI API `/v1/language-models` returned 401 without credentials. Several legacy static aliases are absent from the current individual model cards; absence alone is not grounds to delete them.
- Grok 4.5 card lists xhigh while the reasoning guide says xhigh maps to high. Preserve current high mapping and document conflict.
- DeepSeek official price windows exclude Chinese public holidays. Current weekly schedule has no holiday exception; this batch does not introduce a calendar service.
- Cohere API availability is account-dependent; vision/reasoning protocol support and pricing are not inferred from a live model name. Supplier model features do not establish complete gateway protocol coverage.
- No paid provider calls were run.

Sources retrieved 2026-10-03 (raw local snapshots `/tmp/litellm-dxc-sources/`; full copied pages are not committed):

- [deepseek-start](https://api-docs.deepseek.com/), SHA-256 `7ce9db1b1cc7e2efafe7cbfd57b9d46d240c20399f7bd87672c7e3a5250ccdd0`.
- [deepseek-pricing](https://api-docs.deepseek.com/quick_start/pricing/), SHA-256 `210f102275ccf1a6542f08a3bc9e4b4c7c83278cb74b35217bffa112df6363b2`.
- [deepseek-updates](https://api-docs.deepseek.com/updates/), SHA-256 `2922113bc4e0e970fa6d3c4065c3cff3ec661a4db03d671cc2310545c5b5bb71`.
- [deepseek-thinking](https://api-docs.deepseek.com/guides/thinking_mode/), SHA-256 `35350389e59872d3733f7b7f99f5485c6cbc95f1bc2a71a47ba3ecf30bd7a060`.
- [cohere-models](https://docs.cohere.com/v2/docs/models.md), SHA-256 `ada3af943113fdaaaada15b61d7b537f9ec2e340ca51adba6bfab852933dd8f1`.
- [cohere-deprecations](https://docs.cohere.com/v2/docs/deprecations.md), SHA-256 `63d958e7f397d1a312c8ba2e1ecf89d16be646f65f9fb54e4042b923cd0c6923`.
- [cohere-pricing](https://cohere.com/pricing), SHA-256 `44bb9b5cd91df04417fe84e24441d3acdf9a2b094d1e86870a49b7544293a626`.
- [xai-retirements](https://docs.x.ai/developers/migration/may-15-retirement.md), SHA-256 `d228ec29ccd39efefa16af6347b02069fb7ef1fc29085bded9004c71c3437b6c`.
- [xai-reasoning](https://docs.x.ai/developers/model-capabilities/text/reasoning.md), SHA-256 `9de01f2390b83f76617a645c1245fbb0119ebdbf94bfbe8ed5431111e7ec4ec1`.
- [xai-multi-guide](https://docs.x.ai/developers/model-capabilities/text/multi-agent.md), SHA-256 `87f9e1523552545908be1f515767756fa0c97af6aee3da7defe27d32d87a264d`.
- [xai-43](https://docs.x.ai/developers/models/grok-4.3.md), SHA-256 `9bcd456b37a6fa5bb1b1fed6acd53a2d8128e8f1b60d3eea76f44e3badc4ebc2`.
- [xai-45](https://docs.x.ai/developers/models/grok-4.5.md), SHA-256 `81ddddf9109893f5a84c92cf524984afe446009043dada7aeb4b85cbbb781b2b`.
- [xai-46](https://docs.x.ai/developers/models/grok-4.6.md), SHA-256 `0c227f45a9c4c7c6c86e671fbeaf2284138e0fbbc36d57918400e25e5801b679`.
- [xai-47](https://docs.x.ai/developers/models/grok-4.7.md), SHA-256 `3ed635f3374714d627a45b4632753004c43b09418529273f8c75075a5ae6c3e4`.
- [xai-build](https://docs.x.ai/developers/models/grok-build-0.1.md), SHA-256 `e8c0cbcb733852896245ef05be4475e4bf08e54d741b47a5d031fd7d213baef0`.
- [xai-multi](https://docs.x.ai/developers/models/grok-4.20-multi-agent-0309.md), SHA-256 `6de12a68529773f220cb4bde47f13c452fa6df66fa5171e834b5a0ff52a87ee2`.
- [xai-420r](https://docs.x.ai/developers/models/grok-4.20-reasoning.md), SHA-256 `99057608aeb1a4e6ace5d87bf6c4b27b3bff4af5248babad475500da31fdc2e3`.
- [xai-420n](https://docs.x.ai/developers/models/grok-4.20-non-reasoning.md), SHA-256 `4077bae7964e9b5942763b4d95958234879f82720a3eb5ed37b83347ee1024a8`.

## Interpretation and runtime scope

All evidence below was retrieved on 2026-10-03. “Confirmed” means a first-party model or lifecycle page explicitly names the ID, not that a paid request succeeded. “Unconfirmed” means the reviewed current official sources do not establish the inherited claim; the row is retained, without inventing a shutdown date. Prices are USD per million tokens (input/output) unless marked otherwise. Historical pricing is preserved independently of routing.

DeepSeek uses the generic OpenAI-compatible provider and accepts configured upstream IDs; the authority ledger currently enforces retirement for OpenAI/Azure, not DeepSeek. Marking retired DeepSeek rows `pricing_only` removes their audited callable claim but is not a new runtime denylist. The thinking helper also handles third-party/self-hosted DeepSeek formats; historical R1 recognition is not a native API availability claim. The DeepSeek provider guide and implementation example now use current Flash/Pro names.

Cohere uses its native provider. Per-model capabilities now participate in actual Router selection: embedding/rerank cannot become Chat/ToolCalling candidates. Existing chat, embedding and rerank HTTP dispatch remains implemented. Transcribe has an official endpoint but no native `CohereProvider` audio-transcription implementation, so its price entry stays while its static callable row is removed. Model image support does not imply that the generic text-only embedding request can submit image embeddings. Reasoning response/thinking protocol completeness remains outside this catalog correction.

xAI uses the OpenAI-like provider. All seven 4.20 multi-agent aliases are explicitly Responses-only in the dedicated protocol guide. They now advertise no implemented gateway capabilities and cannot be chosen for Chat; adding native xAI Responses belongs to F06. The model overview's generic function-calling claim conflicts with the dedicated guide's exclusion of client tools; this batch follows the endpoint-specific guide. Current non-multi-agent models keep existing routing. Unknown configured IDs remain pass-through by design, without a new alias validator.

## DeepSeek: every native catalog identity

Sources: [API names and aliases](https://api-docs.deepseek.com/), [limits/prices](https://api-docs.deepseek.com/quick_start/pricing/), [retirement and changes](https://api-docs.deepseek.com/updates/), [thinking/tools](https://api-docs.deepseek.com/guides/thinking_mode/).

| Existing ID | Official callable/lifecycle evidence | Context/output | Tools/vision/reasoning | Price evidence and action |
| --- | --- | --- | --- | --- |
| `deepseek-chat` | Discontinued 2026-07-24 | Historical metadata retained; not current limits | Historical metadata retained | Historical prices retained; `pricing_only` |
| `deepseek-reasoner` | Discontinued 2026-07-24 | Historical metadata retained; not current limits | Historical metadata retained | Historical prices retained; `pricing_only` |
| `deepseek/deepseek-chat` | Discontinued 2026-07-24 | Historical metadata retained; not current limits | Historical metadata retained | Historical prices retained; `pricing_only` |
| `deepseek/deepseek-coder` | Current native API ID unconfirmed; historical name retained | 128000 / 4096 (inherited, unconfirmed) | Inherited, not current supplier certification | Historical upstream price metadata only; no new callable claim |
| `deepseek/deepseek-r1` | Current native API ID unconfirmed; historical name retained | 65536 / 8192 (inherited, unconfirmed) | Inherited, not current supplier certification | Historical upstream price metadata only; no new callable claim |
| `deepseek/deepseek-reasoner` | Discontinued 2026-07-24 | Historical metadata retained; not current limits | Historical metadata retained | Historical prices retained; `pricing_only` |
| `deepseek/deepseek-v3` | Current native API ID unconfirmed; historical name retained | 65536 / 8192 (inherited, unconfirmed) | Inherited, not current supplier certification | Historical upstream price metadata only; no new callable claim |
| `deepseek/deepseek-v3.2` | Current native API ID unconfirmed; historical name retained | 163840 / 163840 (inherited, unconfirmed) | Inherited, not current supplier certification | Historical upstream price metadata only; no new callable claim |
| `deepseek-flash` | Confirmed Flash; old V4/vision-exp aliases redirect to V4.1 Flash | 1,048,576 / 393,216 | yes / yes / optional | 0.15 / 0.60, cached 0.003; matches official card |
| `deepseek-v4-flash` | Confirmed Flash; old V4/vision-exp aliases redirect to V4.1 Flash | 1,048,576 / 393,216 | yes / yes / optional | 0.15 / 0.60, cached 0.003; matches official card |
| `deepseek-v4-flash-vision-exp` | Confirmed Flash; old V4/vision-exp aliases redirect to V4.1 Flash | 1,048,576 / 393,216 | yes / yes / optional | 0.15 / 0.60, cached 0.003; matches official card |
| `deepseek-v4-pro` | Confirmed Pro; no announced shutdown | 1,048,576 / 393,216 | yes / no / optional | 0.66 / 1.98, cached 0.022; peak double; corrected qualified Pro limits |
| `deepseek/deepseek-flash` | Confirmed Flash; old V4/vision-exp aliases redirect to V4.1 Flash | 1,048,576 / 393,216 | yes / yes / optional | 0.15 / 0.60, cached 0.003; matches official card |
| `deepseek/deepseek-v4-flash` | Confirmed Flash; old V4/vision-exp aliases redirect to V4.1 Flash | 1,048,576 / 393,216 | yes / yes / optional | 0.15 / 0.60, cached 0.003; matches official card |
| `deepseek/deepseek-v4-flash-vision-exp` | Confirmed Flash; old V4/vision-exp aliases redirect to V4.1 Flash | 1,048,576 / 393,216 | yes / yes / optional | 0.15 / 0.60, cached 0.003; matches official card |
| `deepseek/deepseek-v4-pro` | Confirmed Pro; no announced shutdown | 1,048,576 / 393,216 | yes / no / optional | 0.66 / 1.98, cached 0.022; peak double; corrected qualified Pro limits |
| `deepseek-coder` | Current native API ID unconfirmed; historical name retained | 32768 / 4096 (inherited, unconfirmed) | Inherited, not current supplier certification | Historical upstream price metadata only; no new callable claim |

Peak Pro windows are weekday 01:00–04:00 and 06:00–10:00 UTC **except Chinese public holidays**. Existing weekly runtime prices cannot express that holiday exception and may overcharge on those dates; this remains an explicit pricing limitation. Flash/Pro native supplier Responses/Anthropic endpoints do not imply this gateway implements them.

## Cohere: every baseline static entry

The [official model table](https://docs.cohere.com/v2/docs/models) provides endpoint, context/output and modality. [Deprecations](https://docs.cohere.com/v2/docs/deprecations) distinguishes deprecated access for existing customers from an actual shutdown. [Pricing](https://cohere.com/pricing) supplies the confirmed classic-model rates below. A+ and Reasoning are explicit reasoning models; generic descriptions of complex reasoning do not certify a thinking protocol.

Tools evidence: [Command A](https://docs.cohere.com/v2/docs/command-a), [A+](https://docs.cohere.com/v2/docs/command-a-plus), [A Reasoning](https://docs.cohere.com/v2/docs/command-a-reasoning), [A Vision](https://docs.cohere.com/v2/docs/command-a-vision), [R7B](https://docs.cohere.com/v2/docs/command-r7b), [R](https://docs.cohere.com/v2/docs/command-r), [R+](https://docs.cohere.com/v2/docs/command-r-plus). Rows marked unconfirmed retain the previous flag rather than inventing support or a removal.

| Existing ID | Official status / endpoint | Context/output | Tools / images / explicit reasoning | Price verification / action |
| --- | --- | --- | --- | --- |
| `command-a-plus-05-2026` | Live; Chat | 128000 / 64000 | yes / yes / yes | Unknown; retain unspecified rates |
| `command-a-03-2025` | Live; Chat | 256000 / 8000 | yes / no / not declared | 2.5 / 10; official pricing |
| `command-a-reasoning-08-2025` | Live; Chat | 256000 / 32000 | yes / no / yes | Unknown; retain unspecified rates |
| `command-a-vision-07-2025` | Live; Chat | 128000 / 8000 | no declared tool support / yes / not declared | Unknown; retain unspecified rates |
| `command-a-translate-08-2025` | Live; Chat | 8000 / 8000 | no declared tool support / no / not declared | Unknown; retain unspecified rates |
| `command-r7b-12-2024` | Live; Chat | 128000 / 4096 | yes / no / not declared | Unknown; retain unspecified rates |
| `command-r-plus-08-2024` | Live; Chat | 128000 / 4096 | yes / no / not declared | 2.5 / 10; official pricing |
| `command-r-08-2024` | Live; Chat | 128000 / 4096 | yes / no / not declared | 0.15 / 0.6; official pricing |
| `command-r-plus` | Deprecated 2025-09-15; existing customers only; no announced shutdown; Chat | 128000 / 4096 | yes / no / not declared | 3 / 15; official pricing |
| `command-r` | Deprecated 2025-09-15; existing customers only; no announced shutdown; Chat | 128000 / 4096 | yes / no / not declared | 0.5 / 1.5; official pricing |
| `command` | Deprecated 2025-09-15; existing customers only; no announced shutdown; Chat | 4096 / 4096 | inherited yes, current tool contract unconfirmed / no / not declared | 1 / 2; official pricing |
| `command-light` | Deprecated 2025-09-15; existing customers only; no announced shutdown; Chat | 4096 / 4096 | no declared tool support / no / not declared | 0.3 / 0.6; official pricing |
| `embed-v4.0` | Live; Embed | 128000 / n/a | no declared tool support / yes / not declared | Unknown; retain unspecified rates |
| `embed-english-v3.0` | Live; Embed | 512 / n/a | no declared tool support / yes / not declared | Inherited 0.10 / 0; current rate not independently confirmed |
| `embed-multilingual-v3.0` | Live; Embed | 512 / n/a | no declared tool support / yes / not declared | Inherited 0.10 / 0; current rate not independently confirmed |
| `embed-english-light-v3.0` | Live; Embed | 512 / n/a | no declared tool support / yes / not declared | Inherited 0.10 / 0; current rate not independently confirmed |
| `embed-multilingual-light-v3.0` | Live; Embed | 512 / n/a | no declared tool support / yes / not declared | Inherited 0.10 / 0; current rate not independently confirmed |
| `rerank-v4.0-pro` | Live; Rerank | 32000 / n/a | no declared tool support / no / not declared | Search-unit billing; no token rate asserted |
| `rerank-v4.0-fast` | Live; Rerank | 32000 / n/a | no declared tool support / no / not declared | Search-unit billing; no token rate asserted |
| `rerank-v3.5` | Live; Rerank | 4096 / n/a | no declared tool support / no / not declared | Search-unit billing; no token rate asserted |
| `rerank-english-v3.0` | Live; Rerank | 4096 / n/a | no declared tool support / no / not declared | Search-unit billing; no token rate asserted |
| `rerank-multilingual-v3.0` | Live; Rerank | 4096 / n/a | no declared tool support / no / not declared | Search-unit billing; no token rate asserted |
| `cohere-transcribe-03-2026` | Live; Audio Transcriptions; no runtime implementation | Audio duration, not a token context | no declared tool support / no / not declared | Price metadata retained; remove unsupported callable static row |
| `tiny-aya-global` | Live; Chat | 8000 / 8000 | no declared tool support / no / not declared | Unknown; retain unspecified rates |
| `tiny-aya-earth` | Live; Chat | 8000 / 8000 | no declared tool support / no / not declared | Unknown; retain unspecified rates |
| `tiny-aya-fire` | Live; Chat | 8000 / 8000 | no declared tool support / no / not declared | Unknown; retain unspecified rates |
| `tiny-aya-water` | Live; Chat | 8000 / 8000 | no declared tool support / no / not declared | Unknown; retain unspecified rates |
| `c4ai-aya-expanse-32b` | Live; Chat | 128000 / 4096 | no declared tool support / no / not declared | Unknown; retain unspecified rates |
| `c4ai-aya-vision-32b` | Live; Chat | 16000 / 4096 | no declared tool support / yes / not declared | Unknown; retain unspecified rates |

The reviewed Embed v2 and Aya Expanse 8B shutdowns (2026-04-04) do not occur in this baseline's static registry. North/Embed v5 and other new supplier offerings are not automatically added by this bounded existing-entry review. Command A+ catalog zero-price metadata is not sufficient to prove unrestricted free production usage; the native static provider leaves its price unspecified.

## xAI: every baseline static ID/alias

The model cards provide context, text/image inputs, function calling, reasoning and rates. They do not specify an independent output limit, so the unsupported inherited 4096 output default is now `None`. Sources: [4.3](https://docs.x.ai/developers/models/grok-4.3), [4.20 reasoning](https://docs.x.ai/developers/models/grok-4.20-reasoning), [4.20 non-reasoning](https://docs.x.ai/developers/models/grok-4.20-non-reasoning), [multi-agent](https://docs.x.ai/developers/model-capabilities/text/multi-agent), [build](https://docs.x.ai/developers/models/grok-build-0.1), [4.5](https://docs.x.ai/developers/models/grok-4.5), [4.6](https://docs.x.ai/developers/models/grok-4.6), [4.7](https://docs.x.ai/developers/models/grok-4.7), [May 15 redirects](https://docs.x.ai/developers/migration/may-15-retirement).

| Existing ID | Explicit official ID/alias/lifecycle evidence | Context / independent output | Tools / images / reasoning | Price card (input / output; cache) |
| --- | --- | --- | --- | --- |
| `grok-4.3` | Confirmed 43 card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.3-latest` | Confirmed 43 card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3` | Explicit 2026-05-15 redirect to 4.3; old slug still accepted | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-3-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-beta` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-fast` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-fast-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-fast-beta` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-beta` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-fast` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-fast-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-fast-beta` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-high` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-high-beta` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-fast-high` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-3-mini-fast-high-beta` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4-0709` | Explicit 2026-05-15 redirect to 4.3; old slug still accepted | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4-fast-reasoning` | Explicit 2026-05-15 redirect to 4.3; old slug still accepted | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4-fast` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4-fast-reasoning-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4-fast-non-reasoning` | Explicit 2026-05-15 redirect to 4.3; old slug still accepted | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4-fast-non-reasoning-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4-1-fast-reasoning` | Explicit 2026-05-15 redirect to 4.3; old slug still accepted | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4-1-fast` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4-1-fast-reasoning-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4-1-fast-non-reasoning` | Explicit 2026-05-15 redirect to 4.3; old slug still accepted | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4-1-fast-non-reasoning-latest` | Unconfirmed legacy alias; retained, not certified | 1,000,000 inherited / unpublished | inherited flags unconfirmed | Inherited 4.3 rates; alias target unconfirmed |
| `grok-4.20-multi-agent-0309` | Confirmed multi card ID/alias; no announced shutdown | 1,000,000 / unpublished | no client tools / yes / yes; Responses-only, not currently routed | 1.25 / 2.50; 0.20 |
| `grok-4.20-multi-agent` | Confirmed multi card ID/alias; no announced shutdown | 1,000,000 / unpublished | no client tools / yes / yes; Responses-only, not currently routed | 1.25 / 2.50; 0.20 |
| `grok-4.20-multi-agent-latest` | Confirmed multi card ID/alias; no announced shutdown | 1,000,000 / unpublished | no client tools / yes / yes; Responses-only, not currently routed | 1.25 / 2.50; 0.20 |
| `grok-4.20-multi-agent-beta-latest` | Confirmed multi card ID/alias; no announced shutdown | 1,000,000 / unpublished | no client tools / yes / yes; Responses-only, not currently routed | 1.25 / 2.50; 0.20 |
| `grok-4.20-multi-agent-experimental-beta-0304` | Confirmed multi card ID/alias; no announced shutdown | 1,000,000 / unpublished | no client tools / yes / yes; Responses-only, not currently routed | 1.25 / 2.50; 0.20 |
| `grok-4.20-multi-agent-experimental-beta-latest` | Confirmed multi card ID/alias; no announced shutdown | 1,000,000 / unpublished | no client tools / yes / yes; Responses-only, not currently routed | 1.25 / 2.50; 0.20 |
| `grok-4.20-multi-agent-beta-0309` | Confirmed multi card ID/alias; no announced shutdown | 1,000,000 / unpublished | no client tools / yes / yes; Responses-only, not currently routed | 1.25 / 2.50; 0.20 |
| `grok-4.20-0309-reasoning` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-reasoning-latest` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-reasoning` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-0309` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta-0309-reasoning` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta-0309` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta-latest` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta-latest-reasoning` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta-reasoning` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-experimental-beta-0304-reasoning` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-experimental-beta-0304` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-experimental-beta-reasoning-latest` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-experimental-beta-latest` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-reasoning-gv2` | Confirmed 420r card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / yes | 1.25 / 2.50; 0.20 |
| `grok-4.20-0309-non-reasoning` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-4.20-non-reasoning` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-4.20-non-reasoning-latest` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta-non-reasoning` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta-latest-non-reasoning` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-4.20-experimental-beta-0304-non-reasoning` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-4.20-experimental-beta-non-reasoning-latest` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-4.20-beta-0309-non-reasoning` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-4.20-non-reasoning-gv2` | Confirmed 420n card ID/alias; no announced shutdown | 1,000,000 / unpublished | yes / yes / no | 1.25 / 2.50; 0.20 |
| `grok-build-0.1` | Confirmed build card ID/alias; no announced shutdown | 256,000 / unpublished | yes / yes / yes | 1 / 2; 0.20 |
| `grok-code-fast-1` | Confirmed build card ID/alias; no announced shutdown | 256,000 / unpublished | yes / yes / yes | 1 / 2; 0.20 |
| `grok-code-fast` | Confirmed build card ID/alias; no announced shutdown | 256,000 / unpublished | yes / yes / yes | 1 / 2; 0.20 |
| `grok-code-fast-1-0825` | Confirmed build card ID/alias; no announced shutdown | 256,000 / unpublished | yes / yes / yes | 1 / 2; 0.20 |
| `grok-4.5` | Confirmed 45 card ID/alias; no announced shutdown | 500,000 / unpublished | yes / yes / yes | 2 / 6; 0.30 |
| `grok-4.5-latest` | Confirmed 45 card ID/alias; no announced shutdown | 500,000 / unpublished | yes / yes / yes | 2 / 6; 0.30 |
| `grok-build-latest` | Confirmed 45 card ID/alias; no announced shutdown | 500,000 / unpublished | yes / yes / yes | 2 / 6; 0.30 |
| `grok-4.6` | Confirmed 46 card ID/alias; no announced shutdown | 500,000 / unpublished | yes / yes / yes | 2 / 6; 0.50 |
| `grok-4.7` | Confirmed 47 card ID/alias; no announced shutdown | 500,000 / unpublished | yes / yes / yes | 2 / 6; 0.50 |

All confirmed card prices double at prompts of at least 200K tokens (for all tokens in the request). The static simple per-1K fields are only the short-context base; runtime catalog time/threshold pricing remains the billing source. 4.5's overview includes `xhigh`, while the [reasoning guide](https://docs.x.ai/developers/model-capabilities/text/reasoning) says it maps to `high`; preserve the existing mapping pending clarification. Unconfirmed legacy aliases' inherited 4.3 identity, prices and features remain an unresolved catalog risk, not validated facts. The authenticated model-list API returned 401 without credentials, so this review cannot resolve account-level alias availability.

## Verification and remaining F10 scope

The focused regressions exercise real configured Router/Provider selection for Cohere and xAI, plus local HTTP chat/embed/rerank dispatch. Python sync tests cover the persistent Pro limit overlay and catalog reproducibility. Validation passed: `cargo fmt --check`, `cargo check`, default `cargo test -- --test-threads=2` (7,267 library tests plus integration/doc suites), default all-target clippy, and complete `providers-extended,gateway,sqlite` test/clippy (10,170 library tests plus integration/doc suites). Both library runs ignored one existing test. All 52 Python sync tests and a repeated source-pinned sync check passed. No paid or authenticated supplier model requests were run.

This batch leaves all other native static providers outside its scope, along with the explicitly unconfirmed old xAI aliases, historical native DeepSeek IDs, Cohere account access/unspecified prices, and DeepSeek holiday pricing noted above. This audit is evidence for a bounded F10 increment, not completion of F10.
