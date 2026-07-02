# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@4c278e8ab25e` 仍有 24 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/core/cache/types.rs`，它是一个 861 行 cache type module，其中 production cache key/entry/config/stats
definitions 约 550 行，其余是 inline unit tests。

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

- 拆分 `src/core/cache/types.rs`，它当前 861 行，是 #727 当前最大的 tracked Rust 文件。
- 保留 `core::cache::types::*` public cache type paths 不变。
- 将 root `types.rs` 保持为 production cache type definition file，并用 `#[path = "types_tests.rs"] mod tests;` 委托测试。
- 将原 inline tests 移动到 `src/core/cache/types_tests.rs`，不改变断言或 fixtures。
- 因 production definitions 本身低于 800 行，本 tranche 不拆 public cache type surface，避免无意义 facade churn。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 cache runtime behavior、storage backends、Redis integration、eviction behavior 或 public cache type fields。
- 不改变 `CacheKey` hash/Redis key behavior、`CacheEntry` TTL/access behavior、`SerializableCacheEntry` conversion behavior、`EvictionPolicy`/`CacheMode` serde behavior、`DualCacheConfig` defaults/builders、`AtomicCacheStats` counters 或 `CacheStatsSnapshot` hit-rate calculations。
- 不在本 PR 中处理其余 23 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `core::cache::types::*` public type names remain available from the original module path.
2. Cache type fields, derives, defaults, serde attributes, and display strings remain unchanged.
3. The root `types.rs` remains the production cache type definition file; tests are delegated only through `#[path = "types_tests.rs"]`.
4. Original inline tests remain under `core::cache::types::tests`.
5. No cache runtime modules are changed.
6. Every touched Rust file must be below U-16's 800-line ceiling.
7. `cargo test core::cache::types --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/cache/types.rs` keeps production cache type definitions and delegates tests with `#[path = "types_tests.rs"] mod tests;`。
- [ ] Original inline cache type tests move to `src/core/cache/types_tests.rs` without assertion changes。
- [ ] Public cache type definitions and behavior remain unchanged。
- [ ] Both touched cache type files are below U-16's 800-line ceiling。
- [ ] Focused cache types test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a cache types test extraction for U-16 compliance.
