# Tech Spec

## Linked Issue

GH-838 / #838

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| 启动入口 | `src/main.rs:103-114`、`src/server/builder.rs` | 仅 `tracing_subscriber::fmt()`；无 observability/langfuse/otel 初始化 | wire 的落点 |
| HTTP 装配 | `src/server/http.rs:198-223` | 中间件与路由注册全集；无 ip_access/guardrails/mcp/a2a/realtime | 可达性判定依据 |
| 配置根 | `src/config/models/gateway.rs:351-375` | `GatewayConfig` 字段全集；上述子系统无配置项 | invariant 2 的第一要件 |
| 未接线子系统 | `src/core/{guardrails,ip_access,mcp,a2a,realtime,webhooks,semantic_cache,analytics,virtual_keys,observability,integrations,audit}` | 完整实现 + 测试，server/main 零引用 | 处置对象 |
| 合法库 API | `src/lib.rs`、`src/core/{completion,function_calling,traits,secret_managers}` | 通过 `pub mod core`、prelude 或 provider 内部 trait 使用暴露，不需要 server 路由 | guard 必须区分 library-only 与 gateway-facing |
| 公共模块导出 | `src/lib.rs`、`src/core/mod.rs` | `pub mod core` 暴露 `mcp`、`a2a`、`realtime` 等候选模块 | remove/gate 会影响下游 import，需 semver/CHANGELOG/deprecation |
| 子系统 registry | `src/core/subsystem_registry.rs`、`src/core/subsystem_registry/tests.rs` | 登记 gateway-facing 决策、runtime path 与豁免/guard 期望 | gate/remove 后必须防止 stale registry/export claims |
| Batch 半接线 | `src/server/routes/ai/batches.rs:41-95` vs `src/core/batch/processor/core.rs:71,143,181` | 路由纯透传；`BatchProcessor` 从未构造 | 半接线样本 |
| 版本工作流 | `.github/workflows/version-bump.yml` | breaking commit 统一执行 major bump，0.x 会错误跨到 1.0.0；`git log --oneline` 不能可靠检测 commit body 中的 `BREAKING CHANGE:` | 所有 0.7.0 removal 的强制前置 |
| virtual_keys | `src/core/virtual_keys/*`、`src/storage/database/migration/m20240301_000003_create_virtual_keys_table.rs`、`src/storage/database/seaorm_db/virtual_key_ops.rs` | 有迁移、manager 与 SeaORM CRUD；`src/core/mod.rs:49` 的 stub 注释已过期 | 应按「已实现但未挂 gateway」处置 |
| 自认证据 | `src/server/http.rs:660-669`（cache admin 501 "not wired"） | 代码自认部分管理面未接线 | 佐证 |

## 设计方案

**Phase 1 — 可达性证据表（附录）**

对每个 gateway-facing 子系统运行固定判定：
`rg "core::<name>|<MainType>" src/server src/main.rs src/bin src/config`，零命中即未接线。
表格记录：子系统、主类型、命中数、依赖的其他子系统、测试规模、最近 90 天 churn。另设
`library_only` 分类：若模块通过 `src/lib.rs`、prelude、provider trait 或 crate 内部 API 合法暴露
（例如 `completion`、`function_calling`、`traits`、`secret_managers`），不得仅因 server/main 零命中判为违规。

### Appendix A — SP838-T2 可达性证据表（2026-07-05）

判定命令模板：

```bash
rg -n "core::<module>|<PrimaryType>" src/server src/main.rs src/bin
rg -n "core::<module>|<PrimaryType>" src/config config
rg -n "<config.path>|<knob_name>" src/config config/gateway*.yaml.example
rg -n "#\[(tokio::)?test" src/core/<module>
git log --since=2026-04-06 --format='%h' -- src/core/<module>
```

说明：`src/config`、`config/`、admin/status 响应中的配置字段或说明文案不算运行时接线证据；
只有在 `src/server`、`src/main.rs`、`src/bin` 中构造主类型、注册中间件/路由、或挂入后台任务才算可达。
下表的 `runtime refs` 使用 `core::<module>|<PrimaryType>` 查询 `src/server src/main.rs src/bin`，
结果均为 0，表示当前没有启动装配或请求路径引用这些主类型。`primary config refs` 只统计主类型查询在
`src/config config` 的命中；`config knob refs` 单独统计已解析/校验/示例化但不能作为可达证据的配置开关命中。

| 子系统 | 主类型查询 | runtime refs | primary config refs | config knob refs | 依赖/耦合 | Rust 文件数 | 测试数 | 90 天 churn | 判定 |
| --- | --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | --- |
| `guardrails` | `GuardrailEngine` | 0 | 0 | 0 | completion 热路径、安全策略 | 10 | 94 | 2 | 未接线；若保留需默认接线或显式配置关闭 |
| `ip_access` | `IpAccessMiddleware` | 0 | 0 | 0 | Actix middleware、CIDR/IP 规则 | 6 | 52 | 1 | 未接线；若保留需中间件短路测试 |
| `mcp` | `McpGateway` | 0 | 0 | 0 | SSRF 校验、tool schema、transport | 10 | 116 | 2 | 未接线；产品化应独立 gate/wire 决策 |
| `a2a` | `A2AGateway` | 0 | 0 | 0 | SSRF 校验、agent registry/provider | 7 | 132 | 2 | 未接线；产品化应独立 gate/wire 决策 |
| `realtime` | `RealtimeSession` | 0 | 0 | 0 | WebSocket/realtime session state | 4 | 17 | 1 | 未接线；仅 feature-gated module API 存在 |
| `observability` | `MetricsCollector` | 0 | 0 | 0 | tracing/metrics/export destinations | 11 | 124 | 3 | 未接线；现有基础 tracing/metrics 在其他模块 |
| `integrations` | `IntegrationManager` | 0 | 0 | 0 | Langfuse/OTel/Helicone/Arize clients | 21 | 110 | 4 | 未接线；未进入真实 LLM request lifecycle |
| `webhooks` | `WebhookManager` | 0 | 0 | 0 | delivery processor、signing、HTTP POST | 6 | 36 | 1 | 未接线；不是 stub-only，需 wire 或 gate |
| `semantic_cache` | `SemanticCache` | 0 | 0 | 38 | storage/vector、embedding provider | 6 | 83 | 1 | 配置拒绝；`cache.semantic_cache` 已解析/校验但不能作为可达证据 |
| `analytics` | `AnalyticsEngine` | 0 | 0 | 27 | storage-backed metrics/reporting | 16 | 75 | 3 | feature-gated 但无 runtime collector/route；`enterprise.advanced_analytics` 已解析/校验 |
| `virtual_keys` | `VirtualKeyManager` | 0 | 0 | 0 | SeaORM CRUD、key policy/rate limits | 6 | 83 | 2 | 未接线；不是 stub-only，需 gateway/API 决策 |
| `audit` | `AuditMiddleware` | 0 | 0 | 29 | enterprise config、audit logger/output | 8 | 52 | 1 | 配置拒绝；`enterprise.audit_logging` 已解析/校验但不能 no-op |
| `batch` | `BatchProcessor` | 0 | 0 | 0 | storage/database、batch route/proxy | 9 | 70 | 1 | 半接线；HTTP route 存在但 processor 未构造 |
| `user_management` | `UserManager` | 0 | 0 | 0 | storage-backed user/team/org domain | 9 | 69 | 2 | 未接线；GH838 豁免清单记录其未由 gateway 构造 |

**Phase 2 — 已批准处置矩阵**

维护者已在
[#838 comment 4982856136](https://github.com/majiayu000/litellm-rs/issues/838#issuecomment-4982856136)
批复以下矩阵：

| 子系统 | 批准处置 | 理由/约束 |
| --- | --- | --- |
| guardrails | wire（默认开，配置可关） | 安全语义，必须证明 `GuardrailEngine::check_input` / `check_output` 在真实请求路径执行并能阻断恶意内容 |
| ip_access | wire（中间件 + 配置；默认 allow-all） | 安全语义，必须证明 denied IP 在 handler/provider 前短路，不能只返回最终 403 |
| observability + integrations | wire（启动初始化 + 配置） | 近期仍在投入（edec83d7），删除损失最大 |
| batch 持久化 | remove `BatchProcessor`（0.6.x deprecate → 0.7.0 remove） | `/v1/batches` 保留现有 provider proxy；0.6.x 保留 public API 与行为，0.7.0 removal 受 release gates 约束；`AsyncBatchExecutor`、共享类型、schema/history 不在本 removal scope |
| mcp / a2a / realtime | experimental-gate（default-off） | 实现大、无路由，产品化是独立决策；realtime 在 feature 启用时保留 websockets |
| webhooks | experimental-gate（default-off） | 已有 subscription / delivery processor / signing / HTTP POST 能力，但尚未挂 gateway event path |
| semantic_cache | remove（0.6.x deprecate → 0.7.0 remove） | 0.6.x 保留 public import 与当前 `cache.semantic_cache` config 行为；0.7.0 才删除模块与 knob |
| analytics | remove（0.6.x deprecate → 0.7.0 remove） | 0.6.x 保留 public import/config/现有 Cargo feature 行为；0.7.0 成组清理模块、knob、`analytics` feature、`enterprise`/`full` 成员与 docs.rs surface |
| virtual_keys | wire | 已有迁移与 storage-backed CRUD，接入 gateway 管理/API 路径 |
| audit logging | wire（默认关） | `enterprise.audit_logging` 是 runtime enablement knob；启用时必须真实挂入 `AuditLogger` / `AuditMiddleware`，默认关闭时不执行 |
| user_management | experimental-gate（default-off） | gate 前先迁移/重构 SeaORM 对 legacy `User`/`Team`/`Organization` 的无条件导入，保留 legacy/canonical 同步并确保默认 SQLite/storage build |

`remove` 行影响的 public surface 在 0.6.0 deprecated、0.7.0 removed；0.7.0 breaking removal 必须先通过
version-workflow gate，并以已验证的 0.6 release/deprecation artifact 为前置证据。`experimental-gate` 行不在
0.7.0 removal scope，它们保持 default-off gate，直到后续独立 spec 批准删除。

**Phase 3 — 执行**

- wire lane：每子系统一个 PR：`GatewayConfig` 字段 + `Default` + 校验 → 启动初始化（builder.rs）→
  中间件/路由挂载 → 行为测试（U-26 checklist 全项 + 子系统真实执行）。guardrails PR 必须用恶意输入/输出证明
  engine 被调用并 enforcement；ip_access PR 必须用 sentinel handler/provider 证明 denied IP 不会执行下游副作用；
  audit logging PR 必须证明 `enterprise.audit_logging=true` 会构造并执行 `AuditLogger`/`AuditMiddleware`，false 仍是默认且不执行。
- gate lane：`Cargo.toml` 真 default-off feature（`mcp = []` → gate `core/mcp` 的 `pub mod`，且不被 default
  features 间接启用；`storage` / `sqlite` 等默认或支持性 feature 不算 experimental gate）+ README/docs
  experimental 段；相关 config schema/env/example 同步 gate 或返回显式 validation error，避免用户配置 no-op knob；
  对 README.md、CLAUDE.md、`docs/README.md`、现存 `docs/protocols/**` 与 config surfaces 同时扫描
  `mcp|a2a|realtime|webhooks|user_management`，每个命中都必须与 default-off experimental 处置一致，
  不得继续宣称为默认或稳定能力；
  若 public import 改变，同步 semver、CHANGELOG、deprecation/迁移说明。`user_management` gate 开始前必须先解耦
  `src/storage/database/seaorm_db/{user_ops.rs,user_management_ops.rs,team_repository/**}` 及相关测试对 legacy 域类型的
  无条件导入，迁移或重构兼容桥接而不得静默丢弃数据同步，并在 gate 关闭时保持默认 SQLite/storage build。
- remove lane：删除模块 + `core/mod.rs` 清理 + README/CLAUDE.md/`docs/` 同步；若 public import 改变，
  同步删除/拒绝相关 config knobs（如 `cache.semantic_cache`、`enterprise.advanced_analytics`）并更新 examples，
  对 batch、`semantic_cache`、`analytics` 同步更新 `src/core/subsystem_registry.rs` 及对应 tests/exemptions，
  删除或改写已不真实的 decision/runtime/export claims，并用 guard regression 证明无 stale registry entry；
  同步 semver、CHANGELOG、deprecation/迁移说明。`analytics` removal 还必须删除 `Cargo.toml` 中的 `analytics`
  feature，将其从 `enterprise`、`full`、`package.metadata.docs.rs.features` 移除，并在迁移说明中告知下游
  用户停止传入 `--features analytics`。
- semantic_cache/analytics compatibility lane：0.6.x 仅增加 public API deprecation、CHANGELOG 与迁移说明，
  保留 `litellm_rs::core::{semantic_cache,analytics}` import、当前 config parse/rejection/runtime 行为，并保留
  `--features analytics`、`enterprise`、`full` 与 docs.rs 现有编译 surface。两者的 0.7.0 removal 都必须依赖
  `SP838-T7v` 通过且含该 deprecation 的 0.6 release artifact 已验证。
- version-workflow lane：修改 `.github/workflows/version-bump.yml`，并新增 `checks/version_bump_policy.py` 与
  `checks/fixtures/version_bump_cases.json` deterministic policy fixture，
  证明 breaking 版本策略为 `0.5.0 → 0.6.0`、`0.6.0 → 0.7.0`、`1.2.3 → 2.0.0`；fixture 必须覆盖
  `feat!:`、`fix!:`、`refactor!:` subject 以及 commit body/footer 的 `BREAKING CHANGE:` 检测。该 lane 为
  `SP838-T7v`，不再作为仅存于 prose 的依赖。
- batch remove lane 严格拆分：0.6.x tranche 只为公开 `BatchProcessor` 增加 deprecation/migration 说明，保持
  签名与行为不变，且不改 `/v1/batches` provider proxy；0.7.0 tranche 仅删除 `BatchProcessor` 公开入口与其
  专属实现，并以 `SP838-T7v` breaking-release fixture 和已验证 0.6 release 为硬依赖。
  0.7.0 tranche 还必须同步移除/改写 subsystem registry 中 `BatchProcessor` 的 stale runtime/export claim 并更新 guard tests。
  `AsyncBatchExecutor`、共享 batch 类型、database schema/history 不在该删除范围，除非独立 spec 另行批准。

**Phase 4 — 守护检查**

脚本或测试：解析 `src/core/mod.rs` 的顶层 `pub mod` 清单，先分类为 `gateway_facing`、`library_only`、
`internal_support`、`feature_gated`。`feature_gated` 只接受 default-off experimental feature；被 default
features 启用的支持 feature（例如 storage-backed cfg）仍按 gateway-facing 判断。仅 `gateway_facing` 模块断言运行时可达：
「被启动装配实际构造并挂入请求路径/中间件/路由/后台任务 ∨ 在带 issue 的豁免清单 ∨ 被真 feature gate」。
单纯存在 `GatewayConfig` 字段、admin/status 展示、validation 文案或 `src/config`/`src/server` 文本引用不算可达性证据；
例如 `semantic_cache` 的配置与 admin flag 不能替代真实请求处理接线。CI 负测试必须证明新增 config-only
或 admin-only 的 gateway 子系统会失败，正测试必须证明 `completion`、`function_calling`、`traits`、
`secret_managers` 不会被误拦。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P2 wire 三要件 | config + builder + http.rs + request path | U-26 checklist 单测 + 子系统真实行为测试 |
| P3 remove 干净 | core/mod.rs + subsystem registry/guards + Cargo.toml + README/CLAUDE.md/`docs/` + CHANGELOG + version workflow | 0.6 public/config/feature compatibility fixtures + 0.6 release artifact + `SP838-T7v` + 0.7 removal/registry 回归；analytics feature/bundle/docs.rs 清理 |
| P4 gate 真实 | Cargo.toml + cfg + docs.rs feature 列表 + CHANGELOG | default-off feature 组合验证 + deprecation/迁移说明 + user_management 默认 SQLite/storage 兼容测试 |
| P5 安全默认 | guardrails/ip_access 配置 | 默认配置下中间件生效的集成测试 |
| P6 守护常驻 | CI 检查 | 人为添加未接线模块的负测试 + library-only 模块正测试 |

## 数据流

wire lane 引入新的启动初始化顺序：config load → storage → 各子系统 init → middleware 注册 → 路由。
observability 初始化必须在 server 启动前完成（tracing 全局注册的一次性约束），且配置感知 tracing/OTel/Langfuse
初始化必须发生在当前 fallback `tracing_subscriber::fmt().init()` 之前或替代它；不能先注册全局 fallback subscriber
再在 builder 中尝试安装配置化 subscriber。observability+integrations 还必须进入真实 LLM request 生命周期：
请求开始时触发 `IntegrationManager::on_llm_start`，成功/失败结束时触发 `on_llm_end`/`on_llm_error`，
不能只依赖既有 `/metrics` HTTP middleware。

## 备选方案

- 全部接线：mcp/a2a/realtime 产品化工作量数周起，且无用户需求证据，拒绝一刀切。
- 全部删除：observability/guardrails 是网关刚需能力且近期有投入，拒绝一刀切。
- 只改文档不动代码：消除宣传落差但维护成本继续发生，作为最低限度 fallback 记录。

## 风险

- Security: guardrails/ip_access 接线后默认开启可能改变现有部署行为——配置逃生门 + CHANGELOG。
- Compatibility: gate/remove 改变 `--all-features` 的模块集合或 `litellm_rs::core::<module>` public import，
  需同步 semver、CHANGELOG、docs.rs feature 列表（Cargo.toml 已有先例）与 deprecation/迁移说明。
- Compatibility: `user_management` 被 storage/SeaORM 兼容桥接无条件引用；直接 gate 会破坏默认 SQLite build
  或丢失 legacy/canonical 数据同步，必须先迁移依赖。
- Performance: wire lane 新增中间件在热路径上，需按 #842 的分配纪律实现。
- Maintenance: 处置矩阵是一次性决策，守护检查防回归。

## 测试计划

- [ ] Unit tests: 各 wire 子系统的 U-26 三要件（config load 被调用、init 被调用、路由可达）和真实执行断言。
- [ ] Integration tests: guardrails 恶意输入/输出被拦截；ip_access denied IP 不到达 sentinel handler/provider；
      batch 0.6 compatibility fixture 证明 `BatchProcessor` 行为不变，proxy fixture 证明 `/v1/batches` 仍走既有 provider upstream。
- [ ] Observability integration tests: 对一条真实 chat/completion 请求注入 test integration，断言
      `on_llm_start` 与 `on_llm_end`/`on_llm_error` 被调用；`/metrics` 只能作为 HTTP middleware 辅助检查。
- [ ] Audit logging integration tests: `enterprise.audit_logging=true` 时请求真实经过 audit logger/middleware，false 时不执行。
- [ ] Gate/remove compatibility tests: `user_management` 关闭时默认 SQLite/storage 组合可编译且兼容桥接行为不丢失；
      analytics removal 后 Cargo 不再宣告 `analytics` feature，`enterprise`/`full`/docs.rs 不再间接启用它。
- [ ] 0.6 remove-row compatibility tests: semantic_cache/analytics public import assertions 在
      `cargo test --test public_api_compat --features "providers-extended,analytics"` 下必须真实编译并执行，
      不得再被模块级或测试级其他 `cfg` 剔除；config 行为不变，
      `cargo check --no-default-features --features analytics`、`cargo check --features enterprise`、`cargo check --features full` 通过。
- [ ] Version policy fixtures: breaking `0.5.0 → 0.6.0`、`0.6.0 → 0.7.0`、`1.2.3 → 2.0.0`，并覆盖
      bang subject 与 `BREAKING CHANGE:` footer detection。
- [ ] Manual verification: `curl` 冒烟被 wire 的路由；Langfuse/OTel 或 test integration 记录请求生命周期事件。

## 回滚方案

wire lane 每子系统有独立配置开关，可运行时关闭；gate/remove lane 按 PR revert。
