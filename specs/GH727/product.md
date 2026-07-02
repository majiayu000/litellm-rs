# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@33028f3d` 仍有 11 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/core/virtual_keys/types.rs`，它是一个 825 行 virtual key DTO/type module；生产 DTO、permission、
rate-limit state 和 key generation settings 只到第 147 行，超标来源是 inline unit tests。

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

- 拆分 `src/core/virtual_keys/types.rs`，它当前 825 行，是 #727 当前最大的 tracked Rust 文件。
- 保留 `src/core/virtual_keys/types.rs` 作为 `VirtualKey`、`RateLimits`、`Permission`、`RateLimitState` 和 `KeyGenerationSettings` 的原始模块路径。
- 将原 inline unit tests 移动到 `src/core/virtual_keys/types_tests.rs`，并从 root 用 `#[path = "types_tests.rs"] mod tests;` 委托。
- 不改变 virtual key serde shape, permission variants/equality, rate limit field semantics, key generation defaults, budget/rate-limit validity simulations, or test expectations。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不拆分 virtual key production types 到 facade 子模块；本文件超标不是生产定义造成的。
- 不修改 virtual key manager, create/update request DTOs, auth, storage, budget runtime, or API behavior。
- 不改变 unit test assertions, fixture key IDs, serialized JSON expectations, permission variants, default rate limits, or validity simulation logic。
- 不在本 PR 中处理其余 10 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `src/core/virtual_keys/types.rs` keeps all existing public virtual key type, enum, field, and default names at the same module path.
2. Virtual key serialization, clone/debug expectations, metadata, budget, expiration, active-state, tag, and model-access semantics stay unchanged.
3. Permission variants/equality, rate-limit fields/state, and key generation defaults stay unchanged.
4. Inline tests move without assertion or fixture changes and continue to use `super::*`.
5. No virtual key manager, request DTO, auth, storage, budget runtime, or API behavior is changed.
6. Every touched Rust file must be below U-16's 800-line ceiling.
7. `cargo test core::virtual_keys::types --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/virtual_keys/types.rs` delegates tests to `src/core/virtual_keys/types_tests.rs`。
- [ ] Original virtual key type tests move without assertion changes。
- [ ] Virtual key DTOs, permissions, rate-limit state, and key generation defaults stay in the original module path。
- [ ] All touched virtual key type files are below U-16's 800-line ceiling。
- [ ] Focused virtual key type test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a virtual key type unit-test extraction for U-16 compliance.
