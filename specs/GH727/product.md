# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@68a17074` 仍有 6 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件之一是
`tests/moderations_routes.rs`，它是一个 809 行 moderation route integration test suite；文件本身只承载
mock moderation upstream、gateway app-state builders、provider/auth helpers 和 route behavior tests。

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

- 拆分 `tests/moderations_routes.rs`，它当前 809 行，是 #727 当前最大的 tracked Rust 文件之一。
- 保留 `tests/moderations_routes.rs` 作为 gated integration-test facade、mock moderation upstream、app-state builders、provider helpers 和 auth helper owner。
- 将原 route behavior tests 按行为域移动到 `tests/tests/moderations_routes_*.rs` 子模块：proxy/default selection、auth/validation、budget/fallback。
- 不改变 route URIs、request bodies、mock upstream capture, provider header assertions, auth rejection, validation rejection, budget rejection/fallback, wildcard/default-model behavior, or test expectations。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 moderation route runtime, gateway server state, provider router, auth middleware, budget limits, or upstream client behavior。
- 不改变 mock server response shape、captured header/body fields、fixture provider names、fixture model names、or expected status codes。
- 不在本 PR 中处理其余 5 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `tests/moderations_routes.rs` keeps the original `#[cfg(all(test, feature = "gateway", feature = "storage"))]` integration-test gate.
2. Parent module keeps mock upstream/server helpers, app-state builders, provider helpers, and authenticated API key helper.
3. Child modules continue to access shared helpers through `super::*`.
4. Proxy behavior, auth/validation behavior, budget rejection/fallback behavior, wildcard/default-model behavior, and upstream request capture semantics stay unchanged.
5. Tests move without assertion or fixture changes.
6. No moderation runtime route, router, auth, budget, storage, or upstream client behavior is changed.
7. Every touched Rust file must be below U-16's 800-line ceiling.
8. `cargo test --test moderations_routes --all-features` must pass.

## 验收标准

- [ ] `tests/moderations_routes.rs` delegates route behavior tests to child modules。
- [ ] Original moderation route tests move without assertion changes。
- [ ] Shared mock server, app-state, provider, and auth helpers stay in the original gated test facade。
- [ ] All touched moderation route test files are below U-16's 800-line ceiling。
- [ ] Focused moderation route integration test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a moderation route integration-test suite split for U-16 compliance.
