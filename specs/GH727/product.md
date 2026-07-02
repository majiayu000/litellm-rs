# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@0cd9ccd2fb6f` 仍有 25 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/core/providers/bedrock/provider_tests.rs`，它是一个 864 行 Bedrock provider unit test suite，
把 provider creation/capability、prompt conversion、OpenAI param mapping、request transform、
response transform、cost calculation、client accessor 和 debug/clone coverage 放在一个文件里。

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
| B | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | 建立 `types/` 子模块，root 继续 `pub use` 原有类型，禁止字段/别名重命名。 |
| C | Runtime orchestrators | OpenTelemetry, OAuth session, request validator, provider modules | 抽出 request mapping、response mapping、operation handlers、storage helpers 或 error mapper，保留外层入口和 trait surface。 |
| D | Shared utilities | config/net/sync helpers | 按功能域拆 util module，避免新增全局 prelude 或 Any-like public API。 |

## 本 tranche 目标

- 拆分 `src/core/providers/bedrock/provider_tests.rs`，它当前 864 行，是 #727 当前最大的 tracked Rust 文件。
- 保留 `src/core/providers/bedrock/mod.rs` 的 `#[cfg(test)] mod provider_tests;` 不变。
- 将 root `provider_tests.rs` 缩小为 shared test helpers 和 child module declarations。
- 按行为域移动测试到 `src/core/providers/bedrock/provider_tests/*.rs`：
  `creation_capability_tests.rs`、`prompt_param_tests.rs`、`request_transform_tests.rs`、
  `response_transform_tests.rs`、`cost_and_access_tests.rs`。
- 不改变任何测试断言、fixtures、model ids、request/response JSON 或 cost expectations。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 Bedrock provider runtime、client、config、model catalog、transformation logic、cost calculator 或 SigV4 behavior。
- 不改变 `BedrockProvider` public API、`BEDROCK_CAPABILITIES`、`BedrockConfig` validation、message prompt conversion、OpenAI param mapping、request/response transform semantics 或 cost calculation behavior。
- 不在本 PR 中处理其余 24 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `src/core/providers/bedrock/mod.rs` continues to load the test suite as `provider_tests`.
2. Root `provider_tests.rs` keeps only shared helpers and child test module declarations.
3. Existing Bedrock provider test assertions remain unchanged and still run under `core::providers::bedrock::provider_tests::*`.
4. No Bedrock production module is changed.
5. Every touched Rust file must be below U-16's 800-line ceiling.
6. `cargo test core::providers::bedrock::provider_tests --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/providers/bedrock/provider_tests.rs` keeps shared helpers and child module declarations only。
- [ ] Original Bedrock provider tests move to behavior-domain child modules without assertion changes。
- [ ] All touched Bedrock provider test files are below U-16's 800-line ceiling。
- [ ] Focused Bedrock provider test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a Bedrock provider test-suite split for U-16 compliance.
