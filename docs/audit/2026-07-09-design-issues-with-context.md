# litellm-rs 设计问题审计报告（带完整上下文）

| 字段 | 值 |
|------|-----|
| **审计日期** | 2026-07-09 |
| **仓库** | litellm-rs（Rust 版 AI Gateway / OpenAI-compatible multi-provider proxy） |
| **原报告分支** | `codex/issue-567-mistral-nova-xai-llama`（仅作来源记录，不代表当前远端指针） |
| **HEAD** | `8a9690ff`（原报告记录；当前仓库与远端均无法解析该对象） |
| **相对 main** | 原报告称分支与 `origin` 已 diverged（本地 2 ahead / 64 behind）；当前无法独立复原该快照 |
| **代码规模** | ~1091 个 `.rs` 文件，`src/` 约 12MB 源码树，合计约 **32.8 万行** |
| **技术栈** | Rust · Tokio · Actix-web · Sea-ORM · Redis · S3 ·（可选）Qdrant · Prometheus/tracing |
| **方法** | 4 个只读 explore 子 agent 并行 + 主 agent 对 Critical 证据二次直读复核 |
| **性质** | **设计 / 架构 / 契约 / 安全** 审计；**非**完整功能测试或性能基准 |
| **可用关联文档** | `docs/analysis/vibe-coding-postmortem.md`、`CLAUDE.md` Provider Tiers；原报告引用的两份 2026-06-02 文档未归档到当前仓库 |

---

## 0. 阅读指引

### 0.0 归档状态与来源限制

- 本文是对一份 2026-07-09 报告的**历史归档**，不是当前 remediation 清单。
- 原报告记录的 snapshot `8a9690ff`、`docs/audit/2026-06-02-codebase-audit.md` 和
  `docs/audit/2026-06-02-phase0-spec.md` 在当前仓库与远端均不可解析，因此相关沿革只能作为原报告陈述保留，
  不能作为可复现证据或链接目标。
- 正文第 1-13 节的文件行号、数量和“当前”字样均指原报告快照。当前状态只看 §0.4 的复验表和 GitHub 实时队列。

### 0.1 这份文档解决什么问题

回答三个问题：

1. **现状是什么**：网关真实请求路径上，什么是活的、什么只是“写在目录里”。
2. **设计问题是什么**：不是“少个函数”，而是身份矩阵、声明–执行裂缝、契约漏斗、安全默认路径等**结构性**问题。
3. **当时建议先修什么**：保留原报告的收敛顺序，用于解释历史决策，不用于直接领取当前工作。

### 0.2 如何解读“事实 / 推断 / 建议”

| 标签 | 含义 |
|------|------|
| **事实** | 原报告声称可在其 snapshot 用文件路径/行号/符号复核；因 snapshot 不可解析，当前只能在 §0.4 的明确基线上重新验证 |
| **推断** | 由事实推导的运行时影响；标注置信度（高/中/低） |
| **建议** | 修复方向；默认“接线 OR 删除”，不做无主路径价值的抽象层 |

### 0.3 方法局限（必读）

- 结论来自**静态阅读**与构造点 grep，**本次未运行** `cargo test` / e2e / 真实上游调用。
- “死代码 / 未接线”类：对 factory、`AppState`、HTTP route、middleware 的引用关系置信度**高**；未对每一个 Stub provider 逐字节证明“永远不可达”。
- 落代码修复时必须按项目纪律（W-03 / W-16）用**本会话命令输出**再声明完成。
- 本报告侧重**设计问题**；安全细节以“影响设计决策”为准，完整漏洞利用链不在此展开。
- 原始 snapshot 与两份上游审计文档未归档；凡依赖它们的历史断言都不得提升为当前事实。

### 0.4 ⚠️ 2026-07-11 main 复验状态（必读）

原报告称 snapshot `8a9690ff` 落后 `origin/main` 64 个提交，但该对象现已不可解析。2026-07-11 以
`origin/main@12a30d7a` 为明确基线重新检查 9 条 Critical；后续 `fbac1a4c` 又合入了 GH837 的一个 partial
删除 tranche。阅读本文时请以下表为准，正文各 CR 描述仅保留为历史陈述：

| CR | main 状态（2026-07-11，`12a30d7a`） | 证据锚点 |
|----|--------------------------------------|----------|
| CR-1 / CR-2 | ✅ **已修复** — 两处 transform 均通过 `insert_optional_param!` 序列化 typed 字段（frequency_penalty / presence_penalty / logit_bias 等）；`12a30d7a` 进一步保留 legacy functions | `openai/client.rs:276-278`、`openai_like/provider.rs:278-280` |
| CR-3 | 🔶 **部分修复** — 原生 enum/factory 已扩展；Bedrock 本身并非 feature-gated。不可达目录由 #837 继续处置，#967 只跟踪 capability 与可执行 surface 一致性 | `providers/mod.rs`；issue #837、#967 |
| CR-4 | 🔶 **部分修复，残余由 #838 跟踪** — deterministic `response_cache` 已接线；`cache.semantic_cache` 仍明确无运行时效果，`semantic_cache_enabled` 仍为 `false` | `config/models/cache.rs:45-48`、`server/state.rs:127`；issue #838 |
| CR-5 | ✅ **已修复** — `StorageLayer::new` 支持 `auto_migrate`，关闭时 schema 检查 fail-closed 报错 | `storage/mod.rs:178-213` |
| CR-6 | 🔶 **部分修复** — 主 missing-price 路径已改为 `require_pricing_field` 返回 `Err`；原报告所述多套 pricing SSOT 与所有调用路径尚未在本次复验中证明收敛 | `pricing_service/service.rs:166-179` |
| CR-7 | ✅ **已修复** — issue-840 战役（PR #908–#917）将 chat/embeddings/images/audio/gemini 等全部路由经 budgeted executor，记账 + 强制执行 | `server/routes/ai/chat.rs:25,120-153` |
| CR-8 | ❌ **原 finding 仍未解决且未跟踪** — #968 只覆盖可配置 provider `base_url` 的运行时 SSRF-safe client；webhook 注册仍主要做 scheme 检查，投递仍使用普通 client | `webhooks/manager.rs:31-32,61-72`；issue #968（仅 provider base URL） |
| CR-9 | 🔶 **行为已修复，机制描述需区分** — production auth fail-closed 行为已存在；`is_production_ready()` 用于 warning/readiness 路径，并非配置验证本身的强制入口 | `config/models/auth.rs`、配置验证调用点 |

截至 2026-07-11，现役队列需按主题读取：#837/#838（provider 与子系统处置，PR #971 已合入 #837 的一个
partial tranche）、#953 与 #957-#961/#969（auth 生命周期与日志，其中 PR #970 是 #957 的 draft spec）、
#963-#967（retry/health/router/Gemini/capability）、#968（provider base URL SSRF）、#519（架构路线图）。
#962 已由 PR #972 关闭。原 webhook SSRF finding 尚无对应 open issue。

**结论**：本文档只作为**历史快照 + 根因分析 + 修复方法论**归档；新的修复工作必须先刷新 GitHub
issues/PRs，再按对应 spec 领取。不得按本文 §11 路线图或编号区间重复开工。

---

## 1. 项目上下文

### 1.1 产品定位（文档宣称）

`CLAUDE.md` 将本项目描述为：

- 高性能 **AI Gateway**（Rust 实现的 Python LiteLLM 精神续作）
- OpenAI 兼容 HTTP API
- **100+ provider** 智能路由
- 附带 MCP Gateway、A2A Protocol、预算、缓存、teams、virtual keys 等企业能力

### 1.2 设计原则（仓库自述）

| 原则 | 来源 | 对审计的含义 |
|------|------|----------------|
| Tier1 = catalog-only，零代码 | `CLAUDE.md` Provider Tiers | catalog 条目应只走 `OpenAILikeProvider` |
| Tier2 = 非 OpenAI / 需签名 / 特殊流 | 同上 | 必须有专用实现且 **factory 可达** |
| 无向后兼容 | `CLAUDE.md:119` | 允许删半成品，禁止“兼容层堆叠” |
| 一 issue 一分支一 PR；≤10 文件 / 500 行 | Agent rules | 修复必须切片，禁止巨型“大扫除 PR” |
| U-26 声明必须接线 | VibeGuard | 配置/模块存在却不执行 = Critical 级设计债 |
| U-29 禁止静默降级 | VibeGuard | 配置生效失败必须 error/可观测，不能 warn 当成功 |

### 1.3 真实请求热路径（事实）

```
CLI Serve
  → config load (file | env) + validate
  → StorageLayer::new (DB / Redis-or-noop / files / optional vector)
  → AuthSystem
  → PricingService (失败仅 warn)
  → UnifiedRouter::from_gateway_config
       → 对每个 enabled provider: create_provider(...)
  → AppState {
        config, auth, unified_router, storage,
        pricing, budget_limits, team_manager, key_manager
      }
  → Actix: Auth → RateLimit → Security → routes
  → /v1/chat/completions:
        build_core_chat_request
        → unified_router 选 deployment
        → Provider enum dispatch
        → transform_chat_request / 上游 HTTP
        → 响应转换
```

**未进入上述热路径的代表能力（事实）**：

- `LLMCache` / `SemanticCache`（`AppState` 无字段；server 无 get/set）
- `BudgetMiddleware` / `BudgetAwareRouter`（未 wrap HttpServer）
- `VirtualKeyManager`（DB ops 在，产品路径不在）
- MCP / A2A gateway 路由
- 约 40+ 个实现了 `LLMProvider` 但未进入 `Provider` 枚举的目录模块

### 1.4 Provider 运行时真相对照

| 层 | 数量 / 形态 | 文件锚点 |
|----|-------------|----------|
| 磁盘 `src/core/providers/*` | ~70+ 目录（含 base/factory/registry） | `src/core/providers/` |
| `impl LLMProvider for` | **52**（含宏展开） | 各 provider `*.rs` |
| 运行时 `enum Provider` | **仅 5 变体** | `providers/mod.rs` ≈328–335 |
| Factory 对 Bedrock/Vertex/Azure… | 建成 **OpenAILike** | `factory/registry.rs` ≈103–123 |
| Lifecycle 清单 | Wire / Stub / CatalogOnly / Internal / Delete | `registry/lifecycle.rs` |

```rust
// src/core/providers/mod.rs（事实摘录）
pub enum Provider {
    OpenAI(openai::OpenAIProvider),
    Anthropic(anthropic::AnthropicProvider),
    Mistral(mistral::MistralProvider),
    Cloudflare(cloudflare::CloudflareProvider),
    OpenAILike(openai_like::OpenAILikeProvider),
}
```

**推断（高）**：网关视角下“原生实现目录很多”≠“生产路径用到了原生实现”。

---

## 2. 审计方法与上下文

### 2.1 并行 agent 分工

| Agent | 视角 | 产出重点 |
|-------|------|----------|
| A · Architecture | 模块边界、Provider 身份、分层、Router 双轨 | 假接线、类型双轨、enum vs trait |
| B · Error & Security | 错误统一性、静默降级、鉴权、SSRF、日志 | webhook SSRF、auth 全开、错误泄露 |
| C · Duplication & Integrity | 重复实现、API 字段、主路径集成 | 参数丢弃、budget/cache 未挂、定价多源 |
| D · Config & Startup | 启动链、DB/migrate、feature、半成品产品面 | 不 migrate、多 key 体系、feature 错位 |

主 agent 对以下证据做了**二次直读**：

- `openai/client.rs` `transform_chat_request`
- `openai_like/provider.rs` `transform_chat_request`
- `factory/mod.rs` + `factory/registry.rs`
- `server/state.rs` / `server/routes/ai/chat.rs`（`thinking: None`）
- `storage/mod.rs` migrate 仅独立方法
- `types/chat.rs` 字段全集
- `registry/lifecycle.rs` Stub/Wire 语义

### 2.2 与历史审计的关系

| 文档 | 日期 | 关系 |
|------|------|------|
| `docs/audit-2026-05-01/*` | 2026-05 | 77→72 条 remediations 战役证据 |
| `2026-06-02-codebase-audit.md`（未归档） | 2026-06-02 | 原报告称其为上一份 Critical 清单；当前不可独立复核 |
| `2026-06-02-phase0-spec.md`（未归档） | 2026-06-02 | 原报告称其为 Phase 0 规格；当前不可独立复核 |
| **本文** | 2026-07-09 | 原报告称在不可解析的 `8a9690ff` 上复验并扩展设计语境 |

#### 相对 2026-06-02 的复验表

| 2026-06 强信号 | 2026-07-09 | 说明 |
|----------------|------------|------|
| CR-1/CR-2 参数静默丢弃 | ✅ **仍在** | 手搓 transform 仍缺 typed 字段；`extra_params` 注释仍误导 |
| CR-3 Provider 假接线 | ✅ **仍在** | enum 仍 5 变体；Bedrock/Vertex → OpenAILike |
| CR-4 缓存未接线 | ✅ **仍在** | `AppState` 仍无 cache；server 无 LLMCache 引用 |
| CR-5 默认 DB / migrate | ✅ **仍在**（略细化） | `StorageLayer` 有 `migrate()` 方法，但 Serve **不调用** |
| CR-6 成本多源 + 静默 $0 | ✅ **仍在** | 多路径 + `unwrap_or(0.0)` |
| U-26 MCP/A2A/Retry/监控等 | ✅ **仍在** | 与 6 月结论一致 |
| SSE 错误后仍发 `[DONE]` | 2026-06 已证伪 | 本次未重复深挖 streaming；默认继承“当前正确” |
| Webhook SSRF / 授权未闭环 | 🆕 **本次补强** | 6 月报告触达较少，本次 security agent 重点命中 |
| Budget 管理 API 与 enforce 脱节 | 🆕 **写清** | 有 `budget_limits` 与 API，无主路径 middleware |
| `thinking: None` 硬清零 | 🆕 **写清** | `chat.rs:419` 与消息 From 转换 |

**推断（中）**：`8a9690ff` 相对 6 月审计 HEAD 主要新增是 **Phase 0 规格文档**，Critical 代码债大部未落地修复。

---

## 3. 系统“宣称 vs 实现”总览

```
宣称面                          实现面（热路径）
─────────────────────────────   ─────────────────────────────
100+ providers                  5-variant enum + OpenAILike 海量分支
完整 OpenAI chat 参数           transform 漏斗，typed 字段静默丢
cache.enabled / semantic        配置解析 + 启动日志；请求 0 效果
budget / teams / virtual keys   teams/keys 部分可用；virtual key 半成品；
                                budget API 可写，completion 不 enforce
MCP 90 tests / A2A 48 tests     模块在；gateway 路由未挂
定价 / 计费                     多套 SSOT；主 chat 不统一记 cost
企业观测 / metrics 端口         硬编码与配置脱节（历史+启动链）
```

**根因一句话（推断，高）**：  
按 Python LiteLLM 的**产品面**铺了模块与单测，Rust 网关只完成了 **auth → select → call → respond** 细管道；其余能力停在“库代码完备、集成未完成”。

---

## 4. 设计问题根因分析

### 4.1 声明–执行裂缝（U-26）是主轴

| 层 | 现象 |
|----|------|
| 配置 | YAML 字段存在且 `deny_unknown_fields` 严格，但运行时不读 |
| 模块 | `core/*` 完整实现 + 单元测试，server 不构造 |
| Trait | `LLMProvider` 实现了 = 看起来可插拔；运行时封闭 `enum Provider` |
| 文档 | AGENTS 写 Tier2 要专目录；factory 却用 OpenAILike 顶替 |

### 4.2 enum 扩展税导致“假兼容”

扩展 `Provider` 变体需要同步 match / macro / `name()` / `provider_type()` 多处（M-7 类问题）。  
**结构性诱因**：把复杂 provider 塞进 `OpenAILike`，编译成本低，**语义正确性成本外溢给用户**。

### 4.3 多 SSOT 身份矩阵

同一 provider 名字可能同时出现在：

1. `PROVIDER_CATALOG`（Tier1 数据）
2. `ProviderType` 枚举变体
3. `PROVIDER_MODULE_LIFECYCLE`（Stub/Wire…）
4. `src/core/providers/<name>/` 完整实现

→ “该不该写代码”没有单一答案；半迁移持续增殖。

### 4.4 类型漏斗（API 契约腐蚀）

```
HTTP ChatCompletionRequest
    → build_core_chat_request  (thinking 强制 None；字段映射)
    → ChatRequest (typed 完整)
    → transform_chat_request   (再次漏斗：只序列化子集)
    → 上游 JSON
```

每一跳都可以静默丢字段；测试若只测组件，测不到整条漏斗。

### 4.5 安全能力“有实现未默认”

SSRF-safe client、redaction、`is_admin_route`、`check_permission` 等**写了**但未成为默认强制路径——与 U-26 同构，只是领域换成安全。

### 4.6 组织/过程上下文（历史）

`docs/analysis/vibe-coding-postmortem.md` 记录：多 agent 并行 PR 共享提交、原子性不足、分支爆炸。  
**推断（中）**：当前“半接线模块堆叠”与长期 AI 辅助铺面、缺少“接线门禁”有关，而不只是单次设计失误。

---

## 5. Critical 设计问题（P0）

编号沿用原报告所称的 2026-06 Phase 0 体系；该上游规格未归档，编号仅用于本文内部对照。

---

### CR-1 — 原生 OpenAI 请求序列化静默丢字段

| 项 | 内容 |
|----|------|
| **类别** | API 契约 / 数据完整性 |
| **严重度** | Critical |
| **事实** | `OpenAIProvider::transform_chat_request` 手搓 JSON，只写 model/messages/temperature/max_tokens/tools… 等子集；注释写 Skip extra_params（`openai/client.rs` ≈195–274） |
| **事实** | `ChatRequest` 定义了 `frequency_penalty`、`presence_penalty`、`logit_bias`、`logprobs`、`top_logprobs`、`reasoning_effort`、`store`、`metadata`、`service_tier`、`parallel_tool_calls`、`extra_params`（`types/chat.rs` ≈84–176） |
| **事实** | 仓库另有较完整的 `OpenAIRequestTransformer`，主路径未统一走它 |
| **推断（高）** | 客户端合法参数到达 OpenAI 时被忽略；无 4xx、无 warn → 极难排查 |
| **建议** | 主路径只保留一套序列化器；补 round-trip 测试（先红后绿） |

---

### CR-2 — 全部 Tier1 OpenAI-like 同样丢 typed 字段 + 误导注释

| 项 | 内容 |
|----|------|
| **类别** | API 契约 · 波及 catalog 全目录 |
| **严重度** | Critical |
| **事实** | `OpenAILikeProvider::transform_chat_request` 同样只映射子集（`openai_like/provider.rs` ≈198–310） |
| **事实** | 注释写 frequency_penalty 等经 `extra_params` 转发（≈279）；但 gateway 把这些放在 **typed 字段**，不进 flatten map |
| **事实** | factory 对 catalog 名一律 `Provider::OpenAILike`（`factory/mod.rs` ≈48–89） |
| **推断（高）** | groq/together/fireworks/deepseek/openrouter… 全部受影响 |
| **建议** | 与 CR-1 合并一个 PR：共享序列化器；删误导注释 |

---

### CR-3 — Provider 身份与运行时脱节（假接线）

| 项 | 内容 |
|----|------|
| **类别** | 架构 · Provider 系统 |
| **严重度** | Critical |
| **事实** | 运行时 enum 仅 5 变体；52 个 `LLMProvider` 实现 |
| **事实** | factory 对 `Bedrock` / `VertexAI` / `Azure` 等构建 OpenAILike（`factory/registry.rs`） |
| **事实** | 专用 `BedrockProvider`（含 SigV4、Converse）等存在且 `impl LLMProvider` |
| **事实** | lifecycle 把部分路径标为 “wire … OpenAI-compatible factory branch”——**Wire ≠ 原生协议** |
| **推断（高）** | 对非 OpenAI 协议 provider，配置成功但协议错误 → 运行时失败或错误行为 |
| **推断（中）** | 若某云厂商提供“OpenAI 兼容端点”，OpenAILike 可能“碰巧可用”，但仍掩盖原生实现债务 |
| **建议** | (a) 原生协议进 enum 并接线；原生 Bedrock 当前要求 AWS SigV4，未来 API-key 支持必须作为独立配置契约；记录模型支持时使用仓库当期明确支持的 exact model ID（例如 scope 中的 Claude Opus 4.7，且仅在上游 API 支持时），并在启用 live runtime 前验证账户与 region 可用性；(b) 或删除/隔离死实现；(c) **先**做 dispatch 契约测试：每个 `ProviderType` 分类为 Native / CatalogOpenAiLike / ExplicitOpenAiLike / Unsupported |

> Phase 0 约束：CR-3 **先核实分类**，不要立刻开全局原生 rewrite。

---

### CR-4 — Cache 配置完整但运行时零效果

| 项 | 内容 |
|----|------|
| **类别** | U-26 · 声明–执行 |
| **严重度** | Critical |
| **事实** | `GatewayConfig.cache: CacheConfig` 存在 |
| **事实** | `AppState` 无 cache 字段（`server/state.rs`） |
| **事实** | `src/server` 无 `LLMCache`/`DualCache` 引用；chat 无 get/set |
| **推断（高）** | `cache.enabled: true` / semantic_cache 只换日志/状态展示，不省上游调用 |
| **建议** | **接线**（AppState + chat/embed 前后）**或** 配置校验拒绝 / 删除字段（二选一，禁止 inert） |

---

### CR-5 — 默认存储路径：不 migrate + 非持久

| 项 | 内容 |
|----|------|
| **类别** | 启动 / 持久化 |
| **严重度** | Critical |
| **事实** | `DatabaseConfig` 默认 `enabled: false`（`config/models/storage.rs`） |
| **事实** | `StorageLayer::migrate` 存在，但 `HttpServer::new` 只 `StorageLayer::new`，Serve 不调用 migrate（`server/http.rs`） |
| **事实** | migrate 主要挂在 CLI `database migrate` |
| **推断（高）** | 默认/未迁移环境：keys/teams/budget 可能缺表失败或行为异常；内存库重启全丢 |
| **建议** | Serve 可配 auto-migrate；内存 SQLite 至少 `Migrator::up`；非持久模式用 warn/error 级可见信号 |

---

### CR-6 — 成本 / 定价多 SSOT + 缺价静默 $0

| 项 | 内容 |
|----|------|
| **类别** | 计费正确性 · 重复设计 |
| **严重度** | Critical |
| **事实** | 并存：`calculate_for_provider`、provider-blind `calculate`、`create_cost_calculator`（几乎无调用方）、`PricingService`、hardcode 表、DB pricing 表未统一读写 |
| **事实** | `pricing_service/service.rs` 对 missing unit price `unwrap_or(0.0)` |
| **事实** | chat 主路径并不统一走 `AppState.pricing` 记费（budget/cache 同批未集成） |
| **推断（中）** | 一旦 budget 接入，错误成本会直接导致错误配额；缺价 $0 可能绕过预算 |
| **建议** | 单一 Pricing SSOT；缺价 ≠ 免费；删死代码；completion 后统一 calculate → record |

---

### CR-7 — Budget 管理面与强制执行脱节（本次补强）

| 项 | 内容 |
|----|------|
| **类别** | U-26 · 产品半成品 |
| **严重度** | Critical（对“计费网关”叙事） |
| **事实** | `AppState.budget_limits` 存在；`/v1/budget/*` 可操作 `UnifiedBudgetLimits` |
| **事实** | `BudgetMiddleware` / `BudgetAwareRouter` / `BudgetManager` 未进入 HttpServer 主链 |
| **事实** | 内部至少两套预算模型（作用域 budget vs provider/model limits） |
| **推断（高）** | 管理员“设了限额”，completion 仍不 pre-check / 不 record spend |
| **建议** | 合并领域模型；在 provider selection 与任何上游 I/O **之前**完成 route authorization、权限与预算检查，只在上游成功响应后记录 usage/spend；或下线预算 API 直至接线 |

---

### CR-8 — Webhook 出站几乎无 SSRF 防护（本次补强）

| 项 | 内容 |
|----|------|
| **类别** | 安全设计 |
| **严重度** | Critical |
| **事实** | webhook 注册/投递侧主要校验非空 + `http(s)`；投递用默认 outbound client |
| **事实** | 仓库已有 `ssrf_guard` / SSRF-safe client，但默认未强制用于 webhook |
| **推断（高）** | 若 webhook URL 可被租户或配置写入，可打内网 / 云 metadata |
| **建议** | 注册与投递双重 SSRF 校验；webhook 专用 safe client；限制 redirect |

---

### CR-9 — 鉴权可双关导致管理面+AI 全开放（本次补强）

| 项 | 内容 |
|----|------|
| **类别** | 安全默认 |
| **严重度** | Critical（误配场景） |
| **事实** | JWT 与 API key 均关闭时中间件放行；部分 handler 的 `is_auth_enabled` 同步跳过 |
| **事实** | 默认配置通常 enable_api_key=true，但可被配置成双关 |
| **推断（中–高）** | 生产误配 = chat + keys + teams 裸奔 |
| **建议** | 生产 profile 强制至少一种 auth；禁止无 auth 的管理 API；启动 `is_production_ready` 硬失败 |

---

## 6. High 设计问题（P1）

### H-1 · U-26 死配置 / 死模块群

| 项 | 证据方向 | 处置 |
|----|----------|------|
| MCP / A2A | 模块+测试；server 无挂载 | 接线 OR 删除文档承诺 |
| 监控 metrics 端口/路径 | 配置与硬编码不一致 | 消费配置 OR 删字段 |
| 限流字段 TPM/burst 等 | 仅部分字段被 limiter 使用 | 接线 OR 删 schema |
| per-provider `RetryConfig` | factory `..` 丢弃 | 绑定到客户端/router |
| enterprise/analytics 空 feature | Cargo 空壳 | 删或真门控 |

### H-2 · 双轨类型系统 + thinking 硬清零

- HTTP DTO：`core/models/openai/*`
- Core：`core/types/*`
- SDK / completion 另有 Message/Tool 副本
- **事实**：`build_core_chat_request` 写 `thinking: None`（`chat.rs` ≈419）
- **建议**：core 为唯一内部模型；边界一次映射；thinking round-trip 或显式 400

### H-3 · 双 Router + 双 Registry

- Gateway：`UnifiedRouter` + `create_provider`
- SDK：`DefaultRouter` + `ProviderRegistry`
- 命名都叫 registry/router，认知与修复分叉
- **建议**：单一 `RuntimeProviderStore` + 单一执行引擎

### H-4 · 运行时封闭 enum vs 开放 trait 叙事

- 文档/技能仍描述“实现 LLMProvider 即可接入”
- 实际必须进 `Provider` enum 或被 factory 映射到 5 变体之一
- 子 trait（LLMChat 等）标注 deprecated 未接线
- **建议**：`Arc<dyn LLMProvider>` **或** 严格门禁：未接线实现不得 public 为“支持”

### H-5 · 出站 HTTP / SSRF 未统一

- 两套 client 工厂（`core/http/outbound`、`utils/net/http`）
- SSRF 配置期校验与运行时 guard 能力不一致（DNS rebinding 风险）
- `use_ssrf_safe_client` 默认 false，全仓极少返回 true
- **建议**：用户可控 URL 强制 safe DNS client；合并实现

### H-6 · 错误信息回传客户端（CWE-209 类）

- Internal/Config/Storage/HttpClient 等 `to_string()` 进入 JSON
- provider body 进入 InvalidRequest/ApiError
- stream error 事件同样字符串化
- **建议**：对外 fixed code + safe message；细节只打服务端日志

### H-7 · 认证 ≠ 授权

- `is_admin_route` / `check_permission` 主要在 helpers/tests
- AI 路由取 context 后不强制 permission
- Bearer 一律当 JWT 的路径与 API key 风格冲突风险
- **建议**：中间件接 admin 闸；AI 路由强制 check_permission；Bearer 分流

### H-8 · 密钥体系三轨

| 体系 | 路径 | 状态 |
|------|------|------|
| Auth API key | `auth/api_key` → `api_keys` 表 | 鉴权在用 |
| KeyManager | `/v1/keys` → 同表映射 | CRUD 在用 |
| VirtualKey | `virtual_keys` 表 + manager | **未进 AppState / 无产品路由** |

- **建议**：单一密钥领域模型；virtual key 合并或归档

### H-9 · 分层边界模糊

- `core` 塞满 keys/audit/guardrails/rate_limit/ip_access…
- `utils/error` 依赖 `ProviderError`（utils → core）
- 观测三栈、SSRF 双实现、pricing 多模块
- **建议**：依赖方向：`types ← providers ← router ← server`；utils 禁止依赖 providers

### H-10 · Feature flags 与运行表面错位

- default feature 与可配置 backend 不一致（如 postgres/s3/vector）
- 空 feature（analytics/websockets 等）制造“已支持”错觉
- **建议**：配置校验交叉检查 Cargo feature；空 feature 删除

### H-11 · 预算/文件/定价刷新的静默降级

- 预算快照失败 warn 继续
- 文件 store 非原子（内容写了 metadata 失败留孤儿）
- pricing 刷新失败用旧/空数据
- **建议**：财务/安全路径 fail 可见；原子写；定价失败策略显式

---

## 7. Medium / Low（P2 / P3）摘要

| ID | 问题 | 级别 |
|----|------|------|
| M-1 | embeddings 显式 `null` optional 字段，严格后端可能 400 | P2 |
| M-2 | 流式/非流式 cache token / reasoning 字段不对称（历史） | P2 |
| M-3 | 死的 stream transformer 硬编码 `thinking: None` | P2 |
| M-4 | 模型元数据 3+ 源漂移（JSON / static_models / provider models） | P2 |
| M-5 | DI 混用：AppState 注入 + 大量 LazyLock 全局单例 | P2 |
| M-6 | stringly `Result<_, String>` 集中在 config/transformer | P2 |
| M-7 | 默认 `database.url` 像真 PG 但 `enabled=false` | P2 |
| M-8 | pricing JSON 路径 cwd 相对，非仓库根启动易失败 | P2 |
| M-9 | `gateway.yaml` 仅 example，quick-start 文档易踩空 | P2 |
| M-10 | 热加载叙事：`AtomicValue` 可 swap 但无 watcher；OptimizedConfig 名不副实 | P2 |
| M-11 | SSE/HTTP client 多实现行为不一致 | P2 |
| M-12 | 恒真测试（`is_ok() \|\| is_err()`）掩盖主路径洞 | P2 |
| M-13 | 巨型文件超 U-16（800 行）——尤其 Stub 原生 provider | P2 |
| L-1 | Redis 限流失败 → 进程内 DashMap（多实例语义变） | P3 |
| L-2 | API key 默认无盐 SHA-256（HMAC 可选） | P3 |
| L-3 | health 暴露 auth/features 等探测面 | P3 |

---

## 8. 已核验为相对健康的部分（避免误报）

以下继承 2026-06 核验 + 本次抽查，**不要当成当前债**：

| 主题 | 状态 |
|------|------|
| SSE 错误终止（不发错误后的伪成功 `[DONE]` 一类问题） | 2026-06 证伪为当前正确；本次未反证 |
| SQL 注入面 | 参数化查询为主；标识符 quoting 有注意 |
| 密码哈希 | Argon2 |
| JWT audience 隔离 | 有设计 |
| CORS `*` + credentials 拒绝 | 有防护 |
| 本地文件 `file_id` 路径约束 | 有基础防 traversal |
| 生产路径 unwrap 主体在 test/启动 expect | 相对可控 |
| catalog ↔ factory round-trip 测试 | 有守护 Tier1 可创建性 |
| 路由策略 enum 与 selection match | 历史审计认为一致 |

---

## 9. 架构简图（设计债标注）

```
                    ┌─────────────────────────────┐
  Client            │  Actix routes (OpenAI DTO)  │
                    └─────────────┬───────────────┘
                                  │ build_core_chat_request
                                  │  ⚠ thinking=None (H-2)
                                  ▼
                    ┌─────────────────────────────┐
                    │     UnifiedRouter           │
                    │  (SDK DefaultRouter 另轨)   │  ⚠ H-3
                    └─────────────┬───────────────┘
                                  │
          ┌───────────────────────┼───────────────────────┐
          ▼                       ▼                       ▼
   Provider::OpenAI      Provider::Anthropic      Provider::OpenAILike
   ⚠ CR-1 丢字段           (native)               ⚠ CR-2 丢字段
                                                      ▲
                         大量“原生”目录 ──假接线──┘  CR-3
                         (Bedrock/Vertex/Azure…)

   旁路声明但未挂主链：
   · Cache (CR-4)  · Budget enforce (CR-7)  · MCP/A2A (H-1)
   · VirtualKey (H-8) · 统一 Pricing (CR-6)
```

---

## 10. 跨 agent 共识排序（置信度）

| 排名 | 信号 | 命中 agent | 置信度 |
|------|------|------------|--------|
| 1 | U-26 声明–执行裂缝（cache/budget/mcp/retry…） | A/C/D（+历史 3 agent） | **极高** |
| 2 | OpenAI 参数漏斗静默丢弃 | C + 主 agent 直读 | **极高** |
| 3 | Provider 假接线 / 身份多表 | A + 主 agent 直读 | **极高** |
| 4 | 定价多 SSOT + 静默 $0 | A/C/D + 历史 | **高** |
| 5 | 安全默认路径未强制（webhook SSRF / auth 双关 / 错误泄露） | B | **高** |
| 6 | 类型/Router 双轨 | A/C | **高** |
| 7 | 启动不 migrate + 默认内存库 | D + 历史 | **高** |

---

## 11. 历史修复路线图（不可直接执行）

> 本节保留 2026-07-09 原报告的计划语境。CR-1/2、CR-5、CR-7 等已有后续实现，其他条目的范围也已拆分；
> 这里的顺序、PR 建议和“当前缺口”均不是 2026-07-11 的实时队列。执行前必须从 GitHub issue/PR 与对应 spec 重新取证。

### 11.1 项目硬约束（修复时必须遵守）

- 一 issue → 一 branch → 一 PR；从 **最新 main** 拉分支
- 单 PR ≤ 10 文件 / 500 行（Cargo.lock、docs 除外）
- 每个修复带失败再通过的测试
- **接线 OR 删除**，禁止再留 inert 配置
- 禁止静默降级（U-29）

### 11.2 历史 Phase 0（原上游规格未归档）

| 顺序 | 切片 | 覆盖 |
|------|------|------|
| 1 | `fix(provider): preserve typed OpenAI chat parameters` | CR-1 + CR-2 |
| 2 | `fix(storage): migrate default SQLite path` | CR-5 |
| 3 | `fix(pricing): authoritative provider-scoped cost` | CR-6 |
| 4 | `fix(cache): no inert cache config` | CR-4 |
| 5 | `test(provider): dispatch contract classification` | CR-3 前置 |

### 11.3 历史 Phase 0.5（原报告新增 Critical）

| 切片 | 覆盖 |
|------|------|
| `fix(budget): enforce or unpublish budget API` | CR-7 |
| `fix(security): webhook SSRF + production auth gate` | CR-8 / CR-9 |
| `fix(api): preserve or reject thinking explicitly` | H-2 子集 |

### 11.4 Phase 1（本周级）

- U-26 死配置群逐块决策（H-1）
- 错误对外脱敏（H-6）
- 授权闭环（H-7）
- 出站 HTTP 统一（H-5）

### 11.5 Phase 2（结构收敛）

- Provider 运行时模型（boxed dyn 或严格 enum 扩展策略）
- 类型单轨、Router 单轨
- Stub 目录清理（lifecycle 驱动：wire / catalog-only / delete）
- 巨型文件拆分（仅**在用**路径优先）

### 11.6 验收门禁（每个 PR）

```bash
cargo fmt --all -- --check
cargo check --all-features
cargo test <targeted> --all-features
# merge 前
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
bash scripts/guards/check_pr_scope.sh
bash scripts/guards/check_pr_overlap.sh
```

**原报告建议的集成级契约测试**（历史清单，不代表当前缺口）：

1. OpenAI 全 typed 字段 round-trip 到 outbound JSON（可用 mock）
2. `cache.enabled=true` → 第二次请求不打 mock upstream **或** 启动拒绝
3. budget 超限 → 429/402 **或** API 不存在
4. 每个 `ProviderType` 的 dispatch 分类断言
5. Serve 默认路径下 keys 表可用（migrate）

---

## 12. 给后续 Agent / 人类的操作上下文

### 12.1 不要做的事

- 不要再新增“完整 provider 目录 + 单测”而不改 factory/`Provider` enum
- 不要再加 YAML 字段而不改热路径
- 不要用 OpenAILike 顶替非 OpenAI 协议却标为 Wire
- 不要在同一 PR 修 CR-1 又做 Bedrock 原生大迁移
- 不要用弱化断言让测试变绿（W-12）

### 12.2 做任何修复前先回答

1. 这条路径是否在 **Serve → chat/embed** 上可达？
2. 失败时是 **error 可见** 还是 warn 继续？
3. 是否存在第二套实现会漂移？
4. 测试是否覆盖 **主路径** 而非仅模块内？

### 12.3 关键文件速查

| 主题 | 路径 |
|------|------|
| 运行时 Provider enum | `src/core/providers/mod.rs` |
| Factory | `src/core/providers/factory/{mod,registry,builder}.rs` |
| Lifecycle | `src/core/providers/registry/lifecycle.rs` |
| Catalog | `src/core/providers/registry/catalog.rs` |
| OpenAI transform | `src/core/providers/openai/client.rs` |
| OpenAI-like transform | `src/core/providers/openai_like/provider.rs` |
| Chat 边界转换 | `src/server/routes/ai/chat.rs` |
| AppState | `src/server/state.rs` |
| 启动 | `src/server/http.rs`, `src/server/builder.rs`, `src/main.rs` |
| Chat 类型 | `src/core/types/chat.rs` |
| 配置入口 | `src/config/models/gateway.rs` |
| Phase 0 规格 | 原报告引用 `docs/audit/2026-06-02-phase0-spec.md`；当前仓库未归档 |
| 上份审计 | 原报告引用 `docs/audit/2026-06-02-codebase-audit.md`；当前仓库未归档 |

---

## 13. 附录

### A. 关键数字快照（2026-07-09）

| 指标 | 值 |
|------|-----|
| `.rs` 文件数（约） | 1091 |
| `impl LLMProvider for` | 52 |
| `Provider` enum 变体 | 5 |
| 最大单文件（示例） | `gemini/models.rs` 1596 行 |
| 历史审计 Critical（2026-06） | 6 |
| 本文 Critical 扩展后 | 9（原 6 + budget/SSRF/auth 显式化） |

### B. 词汇表

| 词 | 含义 |
|----|------|
| **Tier1** | OpenAI 兼容、catalog 数据驱动、零专用代码 |
| **Tier2** | 需要专用协议/签名/流的 provider |
| **Wire** | lifecycle：声称从 factory 可达（**不保证原生协议**） |
| **Stub** | 目录保留、未 runtime-wired |
| **U-26** | 声明了 Config/Trait/模块但启动未集成 |
| **U-29** | 禁止对用户可见失败做静默降级 |
| **假接线** | factory 成功创建但实现类型/协议与名字暗示的不一致 |

### C. 文档维护

| 变更 | 动作 |
|------|------|
| Phase 0 某条代码已合 main | 在本文件对应 CR 标注 **已修复 + commit**，并更新 2026-06 复验表 |
| 证伪某条 finding | 写入第 8 节，勿静默删除历史 |
| 新开审计 | 新建 `docs/audit/YYYY-MM-DD-*.md`，在此文顶部加 “被取代/被补充” 链接 |

### D. 一页纸结论（历史快照，不可作为当前状态转发）

> 在原报告 snapshot 中，产品声明与网关接线面存在明显差距；该判断用于解释后续修复战役。
> 截至 2026-07-11，typed 参数、deterministic cache、migrate 与 budget 主路径已有修复，不能再称为全部未接线。
> 仍需实时核对的残余包括 semantic cache、webhook SSRF、provider disposition、auth 生命周期和结构收敛。
> 当前修复顺序与完成状态只以 GitHub open queue、当前代码和 fresh verification 为准。

---

## 14. 变更记录

| 日期 | 版本 | 说明 |
|------|------|------|
| 2026-07-09 | v1 | 初版：4 agent 并行探索 + 主 agent 复核；对 2026-06-02 全量复验并补安全/预算/thinking 语境 |
| 2026-07-11 | v1.1 | 在 `origin/main@12a30d7a` 复验 Critical，并将文档定位转为历史快照 + 方法论 |
| 2026-07-11 | v1.2 | 补 provenance 限制；纠正 CR-3/4/6/8/9 与 live queue 状态；将 §11 和转发摘要标为不可执行历史内容 |

---

*本文为设计审计产物，不构成已修复声明。任何 “fixed” 必须附带本会话 `cargo test` 输出。*
