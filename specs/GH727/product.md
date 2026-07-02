# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@f98985e1` 仍有 7 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/core/providers/anthropic/client/tests.rs`，它是一个 812 行 Anthropic client test suite；文件本身只承载
client creation、headers、error mapping、message/tool transforms、response transforms 和 request edge tests。

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

- 拆分 `src/core/providers/anthropic/client/tests.rs`，它当前 812 行，是 #727 当前最大的 tracked Rust 文件。
- 保留 `src/core/providers/anthropic/client/tests.rs` 作为 Anthropic client test facade 和 shared imports/helper scope。
- 将原测试按行为域移动到 `src/core/providers/anthropic/client/tests/*.rs` 子模块：setup/error/retry、message/tool transform、response transform、request edge behavior。
- 不改变 client creation、headers、HTTP error mapping、retry-after parsing、system message separation、tool transforms、response transforms、unknown-model policy, unsupported parameter, cache accounting, or test expectations。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 Anthropic production client, request, response, usage, config, registry, or provider behavior。
- 不合并或移动已有 `request_tests.rs`、`compatible_tests.rs`；本 tranche 只拆当前 oversized `tests.rs`。
- 不改变 unit test assertions, fixture models, fixture headers, error text checks, JSON shape checks, cache accounting expectations, or unknown-model policy expectations。
- 不在本 PR 中处理其余 6 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `src/core/providers/anthropic/client/tests.rs` keeps the original `client.rs` test module entrypoint and delegates to child test modules.
2. Child modules continue to access Anthropic client internals through the same `super::*` test-module scope.
3. Client creation, header construction, error mapping, retry-after parsing, message/tool transforms, response transforms, and request edge semantics stay unchanged.
4. Tests move without assertion or fixture changes.
5. No Anthropic production client, request, response, usage, config, registry, or provider behavior is changed.
6. Every touched Rust file must be below U-16's 800-line ceiling.
7. `cargo test core::providers::anthropic::client::tests --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/providers/anthropic/client/tests.rs` delegates to behavior-domain child test modules。
- [ ] Original Anthropic client tests move without assertion changes。
- [ ] Existing `client.rs` test module entrypoint and sibling test modules stay intact。
- [ ] All touched Anthropic client test files are below U-16's 800-line ceiling。
- [ ] Focused Anthropic client test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is an Anthropic client test-suite split for U-16 compliance.
