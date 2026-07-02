# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@c58aa820fead` 仍有 43 个 Rust 文件超过 U-16 的 800 行硬上限。前几个
tranche 已经证明“小 PR、单文件族、保持行为不变”的方式可行，但 spec packet 仍按上一轮
Cloudflare 单文件 tranche 书写，不能指导剩余队列的系统性解耦。

本轮目标是把 #727 升级为完整的大文件解耦计划：每个 PR 仍然小而可审，但所有 tranche 都必须
服从同一套架构边界，避免为了降行数而制造新的耦合、重导出混乱或行为漂移。

## 全量目标

- 把当前 43 个 over-800 Rust 文件逐步拆到 U-16 范围内。
- 每个 tranche 只拥有一个文件或一个紧密文件家族。
- 拆分必须沿现有架构边界进行：测试按行为域拆、类型按领域 DTO/状态/配置拆、运行时代码按
  provider/route/repository/validator/adapter 职责拆。
- 对 public API 类型文件使用 facade + `pub use` 保持现有导入路径兼容。
- 对运行时代码保留现有错误语义；不得用 warning、fallback 或 silently ignore 代替错误。
- #727 只在最后一次全量扫描确认没有 over-800 Rust 文件后才允许使用 closing keyword。

## 解耦分层

| Lane | 文件类型 | 代表文件 | 拆分策略 |
| --- | --- | --- | --- |
| A | Test-only suites | `src/core/cost/calculator/tests.rs`, router tests, utils/event tests, provider test files, integration route tests | 保持原测试断言和模块发现路径，按行为域拆成 child test modules。 |
| B | Public type facades | `src/sdk/types.rs`, `src/core/*/types.rs`, `src/config/models/server.rs` | 建立 `types/` 子模块，root 继续 `pub use` 原有类型，禁止字段/别名重命名。 |
| C | Runtime orchestrators | `vertex_ai/client.rs`, `unified_provider.rs`, `teams.rs`, repositories, validators, integrations | 抽出 request mapping、response mapping、operation handlers、storage helpers 或 error mapper，保留外层入口和 trait surface。 |
| D | Shared utilities | config/net/sync helpers | 按功能域拆 util module，避免新增全局 prelude 或 Any-like public API。 |

## 本 tranche 目标

- 拆分 `src/core/cost/calculator/tests.rs`，它当前 1025 行，是 #727 当前 top offenders 之一。
- 将单个 test module 解耦为职责明确的 child modules：
  - `pricing_lookup_tests.rs`
  - `component_cost_tests.rs`
  - `estimation_comparison_tests.rs`
  - `edge_case_tests.rs`
  - `workflow_tests.rs`
- `tests.rs` 只保留 shared helper 和 `mod` 声明。
- 保持所有测试断言、fixtures、production code 和 public API 不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 cost calculation runtime 行为。
- 不重构 pricing catalog、provider normalization、fallback pricing 或 `PricingService` authority。
- 不在本 PR 中处理其余 42 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. 原有 `core::cost::calculator::tests::*` 测试继续由 calculator test module 发现和运行。
2. 测试移动只能改变 file/module layout，不改变断言、输入数据或 production code。
3. `src/core/cost/calculator/tests.rs` 及新增 child test files 必须低于 800 行。
4. `cargo test core::cost::calculator --lib --all-features` 必须通过。

## 验收标准

- [ ] `src/core/cost/calculator/tests.rs` 成为 shared helper + child module coordinator。
- [ ] 原 pricing/provider alias/component/estimate/edge/workflow tests 分布到职责清晰的 child modules。
- [ ] Focused cost calculator tests 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a cost calculator test-suite decomposition tranche for U-16 compliance.
