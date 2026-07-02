# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@620c7a07` 仍有 12 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/core/budget/alerts.rs`，它是一个 828 行 budget alert manager module；生产 alert/storage/webhook/stat
逻辑保持在 530 行以内，超标来源是 inline async unit tests。

本轮目标继续执行完整的大文件解耦计划：每个 PR 仍然小而可审，但所有 tranche 都必须服从同一套
架构边界，避免制造新的耦合、重导出混乱或行为漂移。

## 全量目标

- 把当前 over-800 Rust 文件逐步拆到 U-16 范围内。
- 每个 tranche 只拥有一个文件或一个紧密文件家族。
- 拆分必须沿现有架构边界进行：测试按行为域拆、类型按领域 DTO/状态/配置拆、运行时代码按
  provider/route/repository/validator/adapter 职责拆。
- 对 public API 类型文件使用 facade + `pub use` 保持现有导入路径兼容。
- 对运行时代码保留现有错误语义；不得用 warning、fallback 或 silently ignore 代替错误。
- #727 只在最后一次全量扫描确认没有 over-800 Rust 文件后才允许使用 closing keyword。

## 解耦分层

| Lane | 文件类型 | 代表文件 | 拆分策略 |
| --- | --- | --- | --- |
| A | Test-only suites | `src/utils/data/utils/tests.rs`, router tests, utils/event tests, provider test files, integration route tests | 保持原测试断言和 focused test coverage，按行为域拆成 child test modules。 |
| B | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | 建立 `types/` 子模块或外置 tests；root 继续保留原有 public type paths。 |
| C | Runtime orchestrators | OpenTelemetry, OAuth session, request validator, provider modules | 抽出 request mapping、response mapping、operation handlers、storage helpers 或 error mapper，保留外层入口和 trait surface。 |
| D | Shared utilities | config/net/sync helpers | 按功能域拆 util module，避免新增全局 prelude 或 Any-like public API。 |

## 本 tranche 目标

- 拆分 `src/core/budget/alerts.rs`，它当前 828 行，是 #727 当前最大的 tracked Rust 文件。
- 保留 `src/core/budget/alerts.rs` 作为 `BudgetAlertManager`、`AlertConfig`、`WebhookConfig`、`AlertStats` 和 alert storage/webhook logic 的原始模块路径。
- 将原 inline async unit tests 移动到 `src/core/budget/alerts_tests.rs`，并从 root 用 `#[path = "alerts_tests.rs"] mod tests;` 委托。
- 不改变 budget threshold detection、alert severity mapping、acknowledgement, history/stat aggregation, webhook registration, or alerting enabled/disabled semantics。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不拆分 alert production logic 到 runtime 子模块；本文件超标不是生产实现造成的。
- 不修改 budget tracker, budget manager, middleware, provider limits, persistence, or spend calculation behavior。
- 不改变 unit test assertions, spend fixtures, budget status transitions, alert counts, severity expectations, webhook fixture URL, or config expectations。
- 不在本 PR 中处理其余 11 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `src/core/budget/alerts.rs` keeps all existing public alert manager, config, webhook, and stats names at the same module path.
2. Budget soft-limit, exceeded, reset, approaching-limit, severity, acknowledgement, history, and stats semantics stay unchanged.
3. Webhook registration, filtering by severity, retry behavior, and enabled/disabled alerting gates stay unchanged.
4. Inline tests move without assertion or fixture changes and continue to use `super::*`.
5. No budget runtime, tracker, manager, middleware, provider limit, persistence, or API behavior is changed.
6. Every touched Rust file must be below U-16's 800-line ceiling.
7. `cargo test core::budget::alerts --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/budget/alerts.rs` delegates tests to `src/core/budget/alerts_tests.rs`。
- [ ] Original budget alert tests move without assertion changes。
- [ ] Budget alert manager, storage, webhook config, and stats behavior stay in the original module path。
- [ ] All touched budget alert files are below U-16's 800-line ceiling。
- [ ] Focused budget alert test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a budget alert unit-test extraction for U-16 compliance.
