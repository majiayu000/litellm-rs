# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@c0aa3a6f` 仍有 14 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/core/teams/manager.rs`，它是一个 830 行 team business-logic manager；生产方法到第 444 行，
超标来源是 inline async unit tests。

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

- 拆分 `src/core/teams/manager.rs`，它当前 830 行，是 #727 当前最大的 tracked Rust 文件。
- 保留 `src/core/teams/manager.rs` 作为 `TeamManager`、request DTOs、usage stats 和 validation helper 的原始模块路径。
- 将原 inline async unit tests 移动到 `src/core/teams/manager_tests.rs`，并从 root 用 `#[path = "manager_tests.rs"] mod tests;` 委托。
- 不改变 team creation/update/delete/member/usage/admin-check semantics、repository call order、validation error strings 或测试断言。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不拆分 `TeamManager` production methods into operation modules；本文件超标不是生产逻辑本身造成的。
- 不修改 team repository trait/implementation, team model types, storage, authorization, billing, or usage accounting runtime behavior。
- 不改变 async unit test assertions, in-memory repository fixture, member roles, UUID setup, last-owner checks, or expected errors。
- 不在本 PR 中处理其余 13 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `src/core/teams/manager.rs` keeps all existing public team manager type, request DTO, and method names at the same module path.
2. Team name validation, duplicate-name conflict detection, member role updates, last-owner protection, and usage stats stay unchanged.
3. Repository call order and error propagation stay unchanged.
4. Inline tests move without assertion or fixture changes and continue to use `super::*`.
5. No team repository, model, storage, auth, billing, or usage runtime behavior is changed.
6. Every touched Rust file must be below U-16's 800-line ceiling.
7. `cargo test core::teams::manager --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/teams/manager.rs` delegates tests to `src/core/teams/manager_tests.rs`。
- [ ] Original team manager tests move without assertion changes。
- [ ] Team manager production methods, request DTOs, and validation helper stay in the original module path。
- [ ] All touched team manager files are below U-16's 800-line ceiling。
- [ ] Focused team manager test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a team manager unit-test extraction for U-16 compliance.
