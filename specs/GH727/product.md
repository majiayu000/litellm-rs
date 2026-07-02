# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@2a20ad60fd7d` 仍有 32 个 Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/utils/data/utils/tests.rs`，它是一个纯测试 suite，把 base64、JSON conversion、cleanup、
merge/path/schema、string 和 serialization 行为都放在同一个文件里，维护时难以定位相关断言。

本轮目标继续执行完整的大文件解耦计划：每个 PR 仍然小而可审，但所有 tranche 都必须服从同一套
架构边界，避免制造新的耦合、重导出混乱或行为漂移。

## 全量目标

- 把当前 32 个 over-800 Rust 文件逐步拆到 U-16 范围内。
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

- 拆分 `src/utils/data/utils/tests.rs`，它当前 931 行，是 #727 当前最大的 test-only suite。
- 保留 `src/utils/data/utils/mod.rs` 的 `#[cfg(test)] mod tests;` 挂载方式不变。
- 将 root `tests.rs` 缩小为 child module declarations。
- 按行为域移动原测试断言到 `src/utils/data/utils/tests/*.rs`：
  base64、JSON conversion/tools、JSON cleanup、UUID、JSON merge/path、JSON flatten/schema、string utilities、string JSON extraction、serialization。
- 保持所有 test names、assertions、sample values 和 focused test command 覆盖不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 `DataUtils` production implementation。
- 不修改 `base64_ops.rs`、`json_ops.rs`、`serialization.rs`、`string_ops.rs` 或 `uuid_ops.rs`。
- 不改变任何 tested behavior 或 error expectation。
- 不在本 PR 中处理其余 31 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `src/utils/data/utils/mod.rs` continues to mount the test suite as `utils::data::utils::tests`.
2. Existing `DataUtils` tests keep their original test names and assertions after moving to child modules.
3. Base64, JSON conversion/tools, cleanup, UUID, merge/path, flatten/schema, string, URL extraction, JSON extraction, and serialization coverage all remain present.
4. Every touched test file must be below U-16's 800-line ceiling.
5. `cargo test utils::data::utils::tests --lib --all-features` must pass.

## 验收标准

- [ ] `src/utils/data/utils/tests.rs` keeps only behavior-domain child module declarations。
- [ ] Original DataUtils tests move under `src/utils/data/utils/tests/*.rs` without assertion changes。
- [ ] All touched DataUtils test files are below U-16's 800-line ceiling。
- [ ] Focused DataUtils test suite 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a DataUtils test-suite split for U-16 compliance.
