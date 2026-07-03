# Tech Spec

## Linked Issue

GH-837 / #837

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Provider enum | `src/core/providers/mod.rs:406-430` | 14 个可构造变体（3 个 `providers-extra`、6 个 `providers-extended` gate） | 可达性的第一入口 |
| Factory | `src/core/providers/factory/registry.rs:52-258` | `from_config_async` 分支即全部 Tier-2 构造点 | 可达性判定依据 |
| Tier-1 catalog | `src/core/providers/registry/catalog.rs` | 数据驱动 OpenAI 兼容 provider → `Provider::OpenAILike`；catalog path 不等于同名 native module 可达 | demote 目标位置，但不能掩盖重复 native 实现 |
| Public exports | `src/core/providers/mod.rs` | 多个孤儿目录仍以 `pub mod` 导出，feature gate 后下游 crate 可能直接 import | 删除前必须评估 public API / semver |
| Macro providers | `src/core/providers/*/provider.rs` + `src/core/providers/macros/` | `custom_api`、`deepl` 等通过 `define_http_provider_with_hooks!` 生成 `LLMProvider` impl | 守护测试不能只搜 literal `impl LLMProvider` |
| Non-LLM surfaces | `runwayml`、`recraft`、`stability`、audio/vector/embedding-only 目录 | 可能暴露 image/video/audio/vector/embedding capability，而非 chat LLM；声明 `ProviderCapability::ChatCompletion` 的 translation/search adapter 必须回到 LLM lane | 进入 non-llm-lane 前必须用 capability 证据排除 chat surface |
| Orphan dirs | `src/core/providers/{custom_api,deepgram,ollama,elevenlabs,huggingface,sagemaker,watsonx,voyage,databricks,triton,jina,...}` | `pub mod` 声明 + 完整实现，但无 native factory/dispatch 构造点 | 处置对象（~41 个） |
| Registry types | `src/core/providers/registry/{types.rs,lifecycle.rs,support_matrix.rs}` | `PROVIDER_TYPE_REGISTRY` 等元数据 | 守护测试挂载点 |
| Prior art | #137 / #140 / #714（均 CLOSED） | 清理过一轮后回归 | 说明需要守护测试 |

## 设计方案

**Phase 1 — 处置矩阵（本 spec 附录，人工批复）**

对 66 个目录逐一标注六类 lane：

- `keep-infra`：base、factory、registry、macros、thinking、openai_like；仅限 shared infra。
- `wired-native`：openai、anthropic、bedrock、mistral、cloudflare、azure、azure_ai、vertex_ai、gemini、
  github_copilot、fal_ai、cohere、replicate 等已有 native enum/factory/dispatch 构造点者。
- `catalog-only-with-native-duplicate`：catalog 已支持但同名 native 目录仍存在者（当前至少 v0 / meta_llama）；
  catalog 条目不能算 native module 可达，必须转为 `demote-to-catalog` 删除 native 目录，或进入显式豁免。
- `demote-to-catalog`：OpenAI 兼容且 catalog runtime 能等价表达的 provider。每个候选必须先证明
  static base URL 足够、无 per-model/dynamic endpoint 构造、无 native-only 非 chat endpoint（如 FIM）、
  auth env fallback 可由 catalog `ProviderDefinition` 表达、`ProviderCapability` / model metadata 与
  native 行为等价；否则进入 wire/delete/exempt 或要求先扩展 catalog 能力。Snowflake、Baseten
  dynamic deployment URL、Codestral FIM、需要 alternate auth env vars 的 Vercel/Codestral 等不能作为
  plain `def()` demote 候选。demote 完成条件必须包含 native 目录删除或显式豁免。
- `delete-native`：chat/LLM native module 非 OpenAI 兼容、无用户需求证据、无构造点，且 public API 影响已记录者
  （候选需从矩阵证据得出；不得把 image/video/translation/search/vector/embedding-only provider 混入）。
- `non-llm-lane`：只能由 declared capability / route behavior 推导，不能按名称 seed。若 provider
  声明 `ProviderCapability::ChatCompletion`（例如某些 search/translation adapters），必须回到
  LLM wire/delete/demote/exempt 矩阵；只有纯 search/vector/audio/image/video/embedding-only 等
  非 chat 能力才先决定产品上是否保留，再决定 wire/delete。
- `exempt`：如 `custom_api` 这类不是 shared infra、但需要产品/架构单独决策的 provider；必须记录 issue、
  owner、期限和后续 lane，不能永久静默豁免。

判定脚本（附录附命令）：不得使用裸 `rg "<TypeName>" src` 作为可达性证据。每目录至少记录：

- native construction/dispatch evidence：`Provider` enum variant、`ProviderType` match arm、factory `Box::new(...)` /
  `Arc::new(...)`、route selector 中的 typed dispatch，或等价 Rust symbol；
- catalog evidence：`registry/catalog.rs` 的 `def()` 只能证明 `Provider::OpenAILike` 路径存在；
  当同名 native 目录仍存在时，不从 native orphan set 中扣除；
- endpoint/auth/capability equivalence evidence：demote 候选必须记录 base_url 是否静态、是否有
  dynamic endpoint 或 provider-specific 非 chat endpoint、primary/alternate auth env vars、native
  capability set 与 catalog `OpenAILikeProvider` capability set 是否等价；
- public export evidence：`src/core/providers/mod.rs` 中 `pub mod <dir>` 与 feature gate；
- provider implementation evidence：literal `impl LLMProvider`、`define_http_provider_with_hooks!`、
  `define_pooled_http_provider_with_hooks!` 等 macro invocation；
- capability evidence：`ProviderCapability::*` 或 model metadata，用于把 image/video/translation/search/vector/embedding-only
  provider 放入 non-llm-lane。
- internal dependency / metadata-use evidence：非 dispatch 运行时代码对 provider 目录内部类型的依赖
  （例如 pricing/cost metadata registry）必须单独记录；有内部依赖的目录不能仅凭无 factory route
  直接 delete/demote，需先迁移依赖或拆出 shared metadata。

文档、README、注释、tests、无关同名 struct（如 A2A 的 LangGraph 类型）只可作为参考，不可作为可达性判定。

**Phase 2 — 守护测试（先行合入）**

在 `registry` 增加 conformance 测试：扫描 `src/core/providers/*/` 的 literal `impl LLMProvider` 类型名、
`define_http_provider_with_hooks!` 与 `define_pooled_http_provider_with_hooks!` 等 macro-generated provider 名称，
与「native enum/factory/dispatch 构造点 + catalog-only 完成状态 + 维护者批复的临时 orphan baseline +
豁免清单」求差集。新增或未批准 orphan 非空即失败；已在 #837 批复矩阵中排入 delete/demote/non-LLM lane
的当前 orphan 可作为临时 baseline，带 issue、owner、期限和退出条件。关键规则：

- catalog 条目只在 native 目录不存在、或该目录被显式豁免时，才可满足该 provider 的最终可达状态；
- `custom_api`、pooled-hook provider（如 `ai21`、`amazon_nova`、`datarobot`、`empower`、`firecrawl`）
  等 macro provider 必须出现在扫描结果中，不能因无 literal impl 被漏掉；
- non-LLM provider 进入独立 lane，不得被 LLM delete guard 自动要求删除；
- 豁免清单与临时 baseline 为带 issue 引用、owner、期限和退出条件的常量表，CI 可见；T5/T6
  每删除或 demote 一个目录必须同步收缩 baseline，最终收尾时 baseline 为空。

**Phase 3 — 分批执行**

- delete lane：按目录家族分 tranche（每 PR 一个或数个小目录），纯删除 + `pub mod` 清理；每个 tranche
  先记录 public API/semver 影响，必要时使用 breaking-change commit 或 deprecation 过渡。
- demote lane：每 PR 一个 provider：确认已有 catalog route 或取得维护者对新增 catalog route 的产品批准 →
  证明 endpoint/auth/capability/non-chat endpoint 等价 → 使用 `def()` 或完整 `ProviderDefinition`
  （需要 alternate auth env vars 等时不得强行用 `def()`）→ 删 native 目录 → smoke 验证模型列表、
  鉴权头、capability 与 provider-specific endpoint 等价；若 native 目录暂留，必须在豁免清单登记，
  不能把 catalog 当 native 可达。新增 catalog-backed selector 属于 runtime behavior change，不能作为纯清理默认发生。
- wire lane（如维护者选择保留个别）：按 CLAUDE.md Tier-2 流程补 enum/factory/dispatch。

## Appendix A - Provider Disposition Matrix

This matrix covers every directory currently under `src/core/providers`. The lane is a proposed GH837 disposition before the T3 maintainer approval gate; it is not a deletion or demotion approval by itself.

| Directory | Proposed lane before T3 | Evidence summary | Follow-up |
| --- | --- | --- | --- |
| `ai21` | `delete-native` | `providers-extended` public module with `define_pooled_http_provider_with_hooks!`; no native `ProviderType`/factory dispatch. | Delete native directory and `pub mod`, or reclassify after T3. |
| `amazon_nova` | `demote-to-catalog` | Catalog-backed `ProviderType::AmazonNova`; native macro provider still exported, so catalog does not make the native module reachable. | Prove catalog equivalence, then delete native directory and shrink baseline. |
| `anthropic` | `wired-native` | Native `Provider` enum/factory dispatch and literal `LLMProvider` impl. | Keep wired module. |
| `azure` | `wired-native` | `providers-extra` native enum/factory dispatch and literal `LLMProvider` impl. | Keep gated native module. |
| `azure_ai` | `wired-native` | `providers-extra` native enum/factory dispatch and literal `LLMProvider` impl. | Keep gated native module. |
| `base` | `keep-infra` | Shared provider infrastructure; no provider implementation marker. | Keep. |
| `baseten` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch; dynamic deployment URL prevents plain catalog demote. | Delete or reclassify after T3. |
| `bedrock` | `wired-native` | Native enum/factory dispatch and literal `LLMProvider` impl. | Keep wired module. |
| `clarifai` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `cloudflare` | `wired-native` | Native enum/factory dispatch and literal `LLMProvider` impl. | Keep wired module. |
| `codestral` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch; native FIM/auth behavior prevents plain catalog demote. | Delete or reclassify after T3. |
| `cohere` | `wired-native` | `providers-extended` native dispatch and literal `LLMProvider` impl. | Keep gated native module. |
| `custom_api` | `exempt` | Macro-generated custom endpoint provider; not shared infra and not native-dispatched. | Requires explicit product/architecture decision before final lane. |
| `databricks` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `datarobot` | `delete-native` | `providers-extended` public module with `define_pooled_http_provider_with_hooks!`; no native dispatch. | Delete or reclassify after T3. |
| `deepgram` | `non-llm-lane` | Audio transcription provider; public module but no `LLMProvider` marker in the guard scan. | Decide non-LLM product support separately. |
| `deepl` | `delete-native` | Translation provider uses `define_http_provider_with_hooks!` and declares `ProviderCapability::ChatCompletion`, so it cannot use the non-LLM lane. | Delete or reclassify after T3. |
| `elevenlabs` | `non-llm-lane` | Text-to-speech/audio transcription provider; public module but no `LLMProvider` marker in the guard scan. | Decide non-LLM product support separately. |
| `empower` | `delete-native` | `providers-extended` public module with `define_pooled_http_provider_with_hooks!`; no native dispatch. | Delete or reclassify after T3. |
| `exa_ai` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `factory` | `keep-infra` | Provider construction infrastructure and tests. | Keep. |
| `fal_ai` | `wired-native` | `providers-extended` native dispatch for image generation and literal `LLMProvider` impl. | Keep gated native module. |
| `firecrawl` | `delete-native` | `providers-extended` public module with `define_pooled_http_provider_with_hooks!`; no native dispatch. | Delete or reclassify after T3. |
| `gemini` | `wired-native` | `providers-extended` native dispatch and literal `LLMProvider` impl. | Keep gated native module. |
| `gigachat` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `github` | `demote-to-catalog` | Catalog-backed `ProviderType::GitHub`; native provider still exported. | Prove catalog equivalence, then delete native directory and shrink baseline. |
| `github_copilot` | `wired-native` | `providers-extended` native dispatch and literal `LLMProvider` impl. | Keep gated native module. |
| `google_pse` | `delete-native` | Search provider declares `ProviderCapability::ChatCompletion` through an `LLMProvider` surface but has no native LLM dispatch. | Delete or reclassify after T3. |
| `gradient_ai` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `huggingface` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `jina` | `non-llm-lane` | Embeddings provider exposes literal `LLMProvider` impl. | Decide embedding product lane before delete/wire. |
| `langgraph` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `macros` | `keep-infra` | Macro definitions only; guard ignores definitions and scans invocations in provider directories. | Keep. |
| `manus` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `meta_llama` | `demote-to-catalog` | Catalog-backed `ProviderType::MetaLlama`; native provider still exported under `providers-extra`. | Prove catalog equivalence, then delete native directory and shrink baseline. |
| `milvus` | `non-llm-lane` | Vector-store provider exposes literal `LLMProvider` impl but is outside LLM factory dispatch. | Decide vector product lane before delete/wire. |
| `mistral` | `wired-native` | Native enum/factory dispatch and literal `LLMProvider` impl. | Keep wired module. |
| `morph` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `nlp_cloud` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `oci` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `ollama` | `demote-to-catalog` | OpenAI-compatible local provider candidate, but no current catalog-backed `ProviderType` route was found. | Requires T3 approval before adding catalog runtime behavior or deleting native code. |
| `openai` | `wired-native` | Native enum/factory dispatch and literal `LLMProvider` impl. | Keep wired module. |
| `openai_like` | `keep-infra` | Shared OpenAI-compatible runtime provider used by explicit and catalog paths. | Keep shared runtime module. |
| `petals` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `pg_vector` | `non-llm-lane` | Vector-store module outside LLM factory dispatch and no guard provider marker. | Decide vector product lane separately. |
| `predibase` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `ragflow` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `recraft` | `non-llm-lane` | Image provider exposes literal `LLMProvider` impl. | Decide image product lane before delete/wire. |
| `registry` | `keep-infra` | Catalog, support matrix, lifecycle, and registry metadata. | Keep. |
| `replicate` | `wired-native` | `providers-extended` native dispatch and literal `LLMProvider` impl. | Keep gated native module. |
| `runwayml` | `non-llm-lane` | Video/image provider exposes literal `LLMProvider` impl. | Decide video/image product lane before delete/wire. |
| `sagemaker` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `sap_ai` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `searxng` | `non-llm-lane` | Search provider exposes literal `LLMProvider` impl. | Decide search product lane before delete/wire. |
| `snowflake` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch; endpoint behavior needs more than plain catalog. | Delete or reclassify after T3. |
| `spark` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `stability` | `non-llm-lane` | Image provider exposes literal `LLMProvider` impl. | Decide image product lane before delete/wire. |
| `tavily` | `non-llm-lane` | Search provider exposes literal `LLMProvider` impl. | Decide search product lane before delete/wire. |
| `thinking` | `keep-infra` | Shared reasoning trait support. | Keep. |
| `topaz` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `triton` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |
| `v0` | `demote-to-catalog` | Catalog-backed `ProviderType::V0`; native provider still exported under `providers-extra`. | Prove catalog equivalence, then delete native directory and shrink baseline. |
| `vercel_ai` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch; auth/endpoint behavior requires explicit decision. | Delete or reclassify after T3. |
| `vertex_ai` | `wired-native` | `providers-extra` native dispatch and literal `LLMProvider` impl. | Keep gated native module. |
| `voyage` | `non-llm-lane` | Embedding provider exposes literal `LLMProvider` impl. | Decide embedding product lane before delete/wire. |
| `watsonx` | `delete-native` | `providers-extended` public module with literal `LLMProvider` impl; no native dispatch. | Delete or reclassify after T3. |

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P2 wire 可达 | factory/enum | conformance 测试 + 单测构造 |
| P3 delete 干净 | providers/mod.rs | `cargo check --all-features` + `rg` 无 dangling mod |
| P4 demote 等价 | catalog.rs + native dir removal | catalog smoke 测试（base_url/env key/alternate env/capability/non-chat endpoint）+ 无重复 native impl |
| P5 守护常驻 | registry conformance test | CI 上人为引入 literal impl 与 macro provider 孤儿目录的负测试 |
| P7 public API | providers/mod.rs / CHANGELOG | 删除导出模块前有 semver/compatibility 记录 |
| P9 non-LLM 范围 | capability scan / matrix | image/video/translation/embedding-only provider 未进入 LLM delete lane |

## 数据流

无运行时数据流变化；delete/demote 仅移除不可达代码路径。

## 备选方案

- 全部接线（wire all）：~41 个目录补 enum/factory/dispatch，扩大 #519 反对的封闭 enum 面，且多数无用户需求证据，拒绝。
- 保持现状仅加守护：阻止恶化但不解决存量数万行死代码，拒绝。
- 移入独立 `providers-graveyard` feature：仍参与编译与 review 面，拒绝。

## 风险

- Security: 无；删除减少攻击面。
- Compatibility: gateway routing 不受不可达 native module 删除影响；但 `pub mod` 导出的 provider 可能被下游 crate
  直接 import/instantiate，删除属于潜在 public API break，必须逐 tranche 记录 semver/compatibility 决策。
- Performance: 编译时间预期显著下降（删除数万行 + 各目录 tests）。
- Maintenance: 主要风险是误删 keep-infra 依赖，靠 `cargo check --all-features` 与全量测试兜底。

## 测试计划

- [ ] Unit tests: conformance 守护测试（含豁免清单机制、macro-generated provider fixture、
      catalog/native duplicate fixture）。
- [ ] Integration tests: demote 后 catalog smoke 测试。
- [ ] Manual verification: 处置矩阵逐行与 construction/dispatch/public-export/capability 证据核对；
      raw text hits 不作为通过条件。

## 回滚方案

删除类 PR 逐个 revert 即可恢复；git history 保留全部实现。守护测试可通过豁免清单临时放行。
