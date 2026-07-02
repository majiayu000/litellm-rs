# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@e98c0357` 仍有 17 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`tests/integration/auth_middleware_tests.rs`，它是一个 844 行 auth middleware integration test suite，其中
shared fixtures/helpers 约 190 行，其余是 behavior tests。

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

- 拆分 `tests/integration/auth_middleware_tests.rs`，它当前 844 行，是 #727 当前最大的 tracked Rust 文件。
- 将 root `auth_middleware_tests.rs` 保持为 test-suite entry point，并用 `#[path = "auth_middleware_tests_parts/mod.rs"] mod tests;` 委托 suite module。
- 将 shared fixtures/helpers 保留在 `tests/integration/auth_middleware_tests_parts/mod.rs`。
- 将原 auth middleware integration tests 按行为域移动到 child modules：rejected/rate-limit paths、authenticated permission/context paths、disabled-auth paths。
- 不改变断言、fixtures、seeded principal setup、route paths、status-code expectations、rate-limit configuration 或 request context checks。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 AuthMiddleware、RateLimitMiddleware、HttpServer、AppState、storage, auth, permission, rate-limit, or request-context production code。
- 不改变 integration test assertions, seeded API key/user setup, fixture routes, request headers, peer addresses, or status-code expectations。
- 不在本 PR 中处理其余 16 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `tests/integration/auth_middleware_tests.rs` remains the integration test-suite entry point referenced by `tests/integration/mod.rs`.
2. Shared fixtures and helpers remain available to all moved child test modules through `super::*`.
3. Rejected auth/rate-limit tests, authenticated permission/context tests, and disabled-auth tests keep their original assertions and setup.
4. No auth, rate-limit, storage, HTTP server, or request context production behavior is changed.
5. Test files are split only by behavior domain.
6. Every touched Rust file must be below U-16's 800-line ceiling.
7. `cargo test --all-features auth_middleware_tests` must pass.

## 验收标准

- [ ] `tests/integration/auth_middleware_tests.rs` delegates to `tests/integration/auth_middleware_tests_parts/mod.rs`。
- [ ] Shared auth middleware fixtures/helpers remain in `tests/integration/auth_middleware_tests_parts/mod.rs`。
- [ ] Original auth middleware integration tests move into behavior-domain child modules without assertion changes。
- [ ] All touched auth middleware test files are below U-16's 800-line ceiling。
- [ ] Focused auth middleware integration test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is an auth middleware integration test-suite split for U-16 compliance.
