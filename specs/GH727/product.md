# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@d45b427dfb7c` 仍有 39 个 Rust 文件超过 U-16 的 800 行硬上限。前几个
tranche 已经证明“小 PR、单文件族、保持行为不变”的方式可行，但 spec packet 仍按上一轮
Cloudflare 单文件 tranche 书写，不能指导剩余队列的系统性解耦。

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

- 拆分 `src/core/router/tests/concurrency_edge_case_tests.rs`，它当前 1079 行，是 #727 当前最大的 test-only suite。
- 将 router concurrency/edge tests 按行为域拆成 child modules：
  - concurrent selection and recording
  - model-list swap atomicity
  - weighted random distribution
  - EMA latency edge cases
  - cooldown expiry races
  - additional concurrency edge cases
- root test file 保留共享 imports 和 child `mod` declarations。
- 保持原测试断言、helper 使用、routing behavior coverage 和 focused test path 不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 router runtime logic、routing algorithms、deployment state handling 或测试断言。
- 不在本 PR 中处理其余 38 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. Existing test functions continue to run under `core::router::tests::concurrency_edge_case_tests::*`.
2. The split preserves all assertions and helper usage from the original file.
3. Shared imports stay centralized in the root module and child modules import through `use super::*`.
4. No router runtime code changes are included.
5. `src/core/router/tests/concurrency_edge_case_tests.rs` and its child files must be below 800 lines.
6. `cargo test core::router::tests::concurrency_edge_case_tests --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/router/tests/concurrency_edge_case_tests.rs` 声明 behavior-domain child modules。
- [ ] 原测试按 concurrent selection、model-list swap、weighted random、EMA、cooldown、additional edge cases 拆入 child files，断言不变。
- [ ] Focused router concurrency tests 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a router concurrency test-suite decomposition tranche for U-16 compliance.
