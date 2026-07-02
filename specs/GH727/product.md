# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@37c31ff5b2d2` 仍有 34 个 Rust 文件超过 U-16 的 800 行硬上限。前几个
tranche 已经证明“小 PR、单文件族、保持行为不变”的方式可行，但 spec packet 仍按上一轮
teams route tranche 书写，需要继续滚动到当前最大文件的系统性解耦。

本轮目标是把 #727 升级为完整的大文件解耦计划：每个 PR 仍然小而可审，但所有 tranche 都必须
服从同一套架构边界，避免为了降行数而制造新的耦合、重导出混乱或行为漂移。

## 全量目标

- 把当前 39 个 over-800 Rust 文件逐步拆到 U-16 范围内。
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

- 拆分 `src/core/cost/types.rs`，它当前 934 行，是 #727 当前最大的 public cost type 文件。
- 保留 cost production types、impl blocks、error enum 和 `src/core/cost/mod.rs` re-exports 不变；
  生产类型抽出测试后约 420 行，低于 U-16。
- 将原 inline tests 整体移动到 `src/core/cost/types_tests.rs`，并通过
  `#[cfg(test)] #[path = "types_tests.rs"] mod tests;` 保持测试模块挂载路径。
- 保持所有 cost type names、fields、visibility、serde/error derives、impl method signatures 和 test assertions 不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不拆分或重命名 cost production types。
- 不修改 `src/core/cost/mod.rs` 的 public re-exports。
- 不修改 cost calculator、provider cost modules 或 pricing service behavior。
- 不在本 PR 中处理其余 33 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. Existing `crate::core::cost::types::*` type paths remain in the same module.
2. Existing top-level cost re-exports in `src/core/cost/mod.rs` continue to compile unchanged.
3. Original inline tests keep the same assertions after moving to `types_tests.rs`.
4. `src/core/cost/types.rs` and `src/core/cost/types_tests.rs` must be below 800 lines.
5. `cargo test core::cost::types --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/cost/types.rs` keeps production cost types and declares path-backed `types_tests.rs`。
- [ ] Original inline cost type tests move to `src/core/cost/types_tests.rs` without assertion changes。
- [ ] Both cost type files are below U-16's 800-line ceiling。
- [ ] Focused cost type tests 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a cost type test extraction tranche for U-16 compliance.
