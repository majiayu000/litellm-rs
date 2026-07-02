# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@e7fd7a121a69` 仍有 29 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/core/router/tests/strategy_impl_tests.rs`，它是一个 880 行 test-only suite，把 routing context
construction、weighted random、least busy、lowest usage、lowest latency、lowest priority、rate-limit-aware、
round-robin 和 strategy consistency tests 全部放在一个文件里。

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
| A | Test-only suites | `src/utils/data/utils/tests.rs`, router tests, utils/event tests, provider test files, integration route tests | 保持原测试断言和模块发现路径，按行为域拆成 child test modules。 |
| B | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | 建立 `types/` 子模块，root 继续 `pub use` 原有类型，禁止字段/别名重命名。 |
| C | Runtime orchestrators | OpenTelemetry, OAuth session, request validator, provider modules | 抽出 request mapping、response mapping、operation handlers、storage helpers 或 error mapper，保留外层入口和 trait surface。 |
| D | Shared utilities | config/net/sync helpers | 按功能域拆 util module，避免新增全局 prelude 或 Any-like public API。 |

## 本 tranche 目标

- 拆分 `src/core/router/tests/strategy_impl_tests.rs`，它当前 880 行，是 #727 当前最大的 test-only suite。
- 保留 `src/core/router/tests/mod.rs` 的 `mod strategy_impl_tests;` 挂载方式不变。
- 将 root `strategy_impl_tests.rs` 缩小为 shared imports、provider/deployment helpers 和 strategy-domain child module declarations。
- 按策略域移动原测试断言到 `src/core/router/tests/strategy_impl_tests/*.rs`：
  context builder、weighted random、least busy、lowest usage、lowest latency、lowest priority、rate-limit-aware、round-robin、integration consistency。
- 保持非重复 strategy coverage、async behavior、atomic counter behavior 和 focused test command 覆盖不变。
- 已按 review 删除一个只重复首轮 round-robin 覆盖的 test；`test_round_robin_cycles_through_candidates`
  继续覆盖相同首轮顺序和下一轮 wrap 行为。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 `src/core/router/strategy_impl.rs`、deployment model 或 router runtime selection behavior。
- 不改变任何 routing strategy scoring、selection、counter、limit、latency 或 priority assertion。
- 不在本 PR 中处理其余 28 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `src/core/router/tests/mod.rs` continues to mount the suite as `core::router::tests::strategy_impl_tests`.
2. Shared provider and deployment helpers remain test-only helpers in the strategy test root.
3. Existing weighted random, least busy, lowest usage, lowest latency, lowest priority, rate-limit-aware, round-robin, and consistency coverage remains discoverable, except the review-confirmed duplicate round-robin first-cycle test is removed.
4. Round-robin tests keep the same `DashMap<String, AtomicUsize>` counter behavior.
5. Every touched Rust file must be below U-16's 800-line ceiling.
6. `cargo test core::router::tests::strategy_impl_tests --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/router/tests/strategy_impl_tests.rs` keeps only shared helpers and child module declarations。
- [ ] Original strategy implementation tests move under `src/core/router/tests/strategy_impl_tests/*.rs` without assertion changes。
- [ ] Strategy coverage remains present for routing contexts, weighted random, least busy, lowest usage, latency, priority, rate limits, round robin, and consistency。
- [ ] All touched Router strategy test files are below U-16's 800-line ceiling。
- [ ] Focused Router strategy test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a Router strategy test-suite split for U-16 compliance.
