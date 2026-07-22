# Product Spec

## Linked Issue

GH-838 / #838

## 用户问题

在 `origin/main@c47596a4`，一批完整子系统「声明但未接线」：代码、类型、测试俱全，但 `src/server`、
`src/main.rs`、`GatewayConfig` 中零引用，任何请求路径都不会执行它们：

- `core/guardrails`（内容安全护栏）——安全控制不生效；
- `core/ip_access`——未注册为中间件；
- `core/mcp`、`core/a2a`、`core/realtime`——无路由挂载，但 CLAUDE.md、README、`docs/README.md`、
  `docs/protocols/{mcp,a2a}.md` 以 "MCP Gateway"、"A2A Protocol" 对外宣传；
- `core/observability` + `core/integrations`（Langfuse/OTel）——`main.rs:103-114` 只初始化
  `tracing_subscriber::fmt()`，整套导出器无可达路径，且近期仍有 commit 在重构它（`edec83d7`）；
- `core/webhooks`、`core/semantic_cache`、`core/analytics`、`core/virtual_keys`、`core/audit`；其中
  `virtual_keys` 已有迁移与 SeaORM CRUD，问题是 storage-backed 子系统已实现但未挂到 gateway 管理/API 路径；
- `/v1/batches` 纯透传，`core/batch::BatchProcessor` 持久化层从未被构造。

对用户的伤害：按文档宣传选型的用户拿到的是不存在的功能；对维护者的伤害：持续为不可达代码付出
重构、review、编译成本（U-26 declaration-execution gap）。

## 目标

- 每个子系统获得显式处置：接线（配置 + 启动初始化 + 路由/中间件挂载）或移出主干（删除 /
  experimental gate + 文档标注）。
- 文档（README、CLAUDE.md、`docs/README.md`、`docs/protocols/{mcp,a2a}.md`）能力描述与真实可达能力一致。
- 建立守护检查，防止新的「声明但未接线」子系统无声进入主干。

## 非目标

- 不在本 issue 内完成 MCP/A2A/realtime 的完整产品化（若维护者选择 wire，功能实现是后续独立 issue）。
- 不处理 provider 层的不可达目录（归 #837）。
- 不改变已接线子系统（router、budget、rate_limiter、cache 主路径等）的行为。

## Behavior Invariants

1. 处置矩阵批复前不删除任何子系统（U-05）。
2. 「wire」处置的子系统：存在配置项、启动初始化调用、以及至少一条端到端可达路径（路由或中间件），
   三者缺一即 CI 失败。
3. 「remove」处置的子系统：删除后 `cargo check --all-features` 与全量测试通过，文档同步更新。
4. 「experimental-gate」处置的子系统：模块被真实 default-off feature gate（默认不编译），README 标注
   experimental 且不再出现在能力列表主表；`storage`、`sqlite` 等默认/支持性 feature 不能冒充实验 gate。
5. 安全语义类子系统（guardrails、ip_access）若保留，必须默认接线或在配置显式关闭——不允许
   「代码在但从不执行」的中间态。
6. remove/gate 若影响 `src/lib.rs` 暴露的 `pub mod core` 下公共模块，必须按 public API 变更处理：
   明确 semver 影响、CHANGELOG 条目、deprecation/迁移说明，不能只按「文档收缩」处理。
7. 守护检查常驻：`core/` 下 gateway-facing 子系统必须被 server/main 引用或在豁免清单（带 issue 引用）中；
   纯库 API 模块（如 `completion`、`function_calling`、`traits`、`secret_managers`）不得因没有 server 路由而被阻断。
8. Batch 处置遵循兼容窗口：0.6.x 保留公开 `BatchProcessor` 及其既有行为并标记 deprecated，
   `/v1/batches` 继续保持现有 provider proxy 语义；只有在 0.6 deprecation 已有可验证 release、且版本工作流
   已能正确执行 0.x breaking release 后，才可在 0.7.0 删除 `BatchProcessor`。
9. Batch removal 仅覆盖从未接入 gateway 的 `BatchProcessor` 持久化入口；`AsyncBatchExecutor`、共享 batch 类型、
   database schema 与历史记录不随之删除，除非后续 spec 另行批准。
10. 0.7.0 的预定删除仅适用于已批准矩阵中标记为 `remove` 的行；`experimental-gate` 行在 0.7.0 之后仍保持
    default-off feature gate，除非后续独立 spec 再批准删除。
11. `user_management` 改为 default-off gate 之前，必须先迁移或重构 storage/SeaORM 对 legacy `User`/`Team`/
    `Organization` 类型的无条件依赖，且不得丢失现有 legacy/canonical 数据同步语义；默认 SQLite/storage build
    必须继续可编译。
12. `semantic_cache` 与 `analytics` 必须先经过 0.6.x compatibility/deprecation tranche：保留现有 public import、
    config 行为与 `analytics` Cargo feature/bundle/docs.rs 行为，同时增加 deprecation、CHANGELOG 与迁移证据。
    只有在 `SP838-T7v` 版本工作流验证通过且该 deprecation 已有可验证 0.6 release artifact 后，
    才可于 0.7.0 删除。

## 验收标准

- [x] 逐子系统处置矩阵（wire / remove / experimental-gate + 证据行）经维护者批复
     （[#838 comment 4982856136](https://github.com/majiayu000/litellm-rs/issues/838#issuecomment-4982856136)）。
- [ ] 被保留子系统满足 invariant 2 并有 smoke 测试。
- [ ] 被移除/降级子系统的文档同步完成（README、CLAUDE.md、`docs/README.md`、
      `docs/protocols/{mcp,a2a}.md`），并完成 public API 影响记录。
- [ ] 守护检查合入 CI。

## 边界情况

- 子系统之间的依赖（如 observability 依赖 integrations，audit logging 依赖 enterprise 配置/中间件）：
  处置必须按依赖拓扑成组决策。
- 半接线状态（batch）：维护者已批准移除从未接线的 `BatchProcessor` 持久化入口，但 public API 必须先经历
  0.6.x 保留行为的 deprecation 窗口；`/v1/batches` provider proxy 不属于 removal，必须保持现有行为。
- 0.7.0 removal 不得顺带删除 `AsyncBatchExecutor`、共享 batch 类型、database schema 或历史记录；这些对象若需处置，
  必须先有独立 spec 决策。
- `user_management` 并非孤立模块：默认 `sqlite` 通过 `storage` 无条件编译 SeaORM 兼容桥接。在将模块改为
  default-off 前，必须先解耦这些类型导入并保留数据同步行为。
- `analytics` removal 同时是 Cargo 公开 feature surface 变更：除了模块与配置 knob，还必须删除
  `analytics` feature、其在 `enterprise`/`full` 中的成员资格、docs.rs feature 发布面，并提供迁移说明。
- `semantic_cache` / `analytics` 的 0.6.x tranche 不得预先删除公开模块、改变现有 config rejection/runtime 行为，
  或让 `--features analytics`、`enterprise`、`full`、docs.rs 不再编译原有 analytics surface。
- `.specrail/runtime`、`docs/` 中引用这些子系统的历史文档：不追溯修改，只改能力宣传文档。

## 发布说明

若选择 remove/gate，CHANGELOG 需标注能力宣传的收缩；若被处理模块仍通过 `src/lib.rs` → `pub mod core`
对外可导入，还需记录 semver/deprecation/迁移影响。即使 gateway 运行时路径原本不可达，也可能破坏下游
库用户的 `litellm_rs::core::<module>` import。`BatchProcessor` 因此在 0.6.x 只做保留行为的 deprecation，
并在版本工作流 breaking-release gate 与已验证 0.6 release 均满足后，才于 0.7.0 删除。其他 0.7.0 removal
仅覆盖矩阵中的 `remove` 行，且同样以 `SP838-T7v` 与含相应 deprecation 的已验证 0.6 release artifact
为前置；`experimental-gate` 行保持 default-off，后续若删除需另立 spec。
