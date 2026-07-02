# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@a42b908a3e26` 仍有 37 个 Rust 文件超过 U-16 的 800 行硬上限。前几个
tranche 已经证明“小 PR、单文件族、保持行为不变”的方式可行，但 spec packet 仍按上一轮
analytics types tranche 书写，需要继续滚动到当前最大文件的系统性解耦。

本轮目标是把 #727 升级为完整的大文件解耦计划：每个 PR 仍然小而可审，但所有 tranche 都必须
服从同一套架构边界，避免为了降行数而制造新的耦合、重导出混乱或行为漂移。

## 全量目标

- 把当前 39 个 over-800 Rust 文件逐步拆到 U-16 范围内。
- 每个 tranche 只拥有一个文件或一个紧密文件家族。
- 拆分必须沿现有架构边界进行：测试按行为域拆、类型按领域 DTO/状态/配置拆、运行时代码按
  provider/route/repository/validator/adapter 职责拆。
- 对 public API 类型文件使用 facade + `pub use` 保持现有导入路径兼容。
- 对运行时代码保留现有错误语义；不得用 warning、fallback 或 silently ignore 代替错误。
- #727 只在最后一次全量扫描确认没有 over-800 Rust 文件后才允许使用 closing keyword。

## 解耦分层

| Lane | 文件类型 | 代表文件 | 拆分策略 |
| --- | --- | --- | --- |
| A | Test-only suites | `src/core/cost/calculator/tests.rs`, router tests, utils/event tests, provider test files, integration route tests | 保持原测试断言和模块发现路径，按行为域拆成 child test modules。 |
| B | Public type facades | `src/sdk/types.rs`, `src/core/*/types.rs`, `src/config/models/server.rs` | 建立 `types/` 子模块，root 继续 `pub use` 原有类型，禁止字段/别名重命名。 |
| C | Runtime orchestrators | `vertex_ai/client.rs`, `unified_provider.rs`, `teams.rs`, repositories, validators, integrations | 抽出 request mapping、response mapping、operation handlers、storage helpers 或 error mapper，保留外层入口和 trait surface。 |
| D | Shared utilities | config/net/sync helpers | 按功能域拆 util module，避免新增全局 prelude 或 Any-like public API。 |

## 本 tranche 目标

- 拆分 `src/core/providers/unified_provider.rs`，它当前 1014 行，是 #727 当前最大的 runtime/provider error facade 文件。
- 将 unified provider error handling 按职责拆成 child modules：
  - `error.rs`: `ProviderError` enum and variant definitions
  - `methods.rs`: factory methods, retryability, retry delay, context attachment, HTTP status mapping
  - `http_mapping.rs`: shared default/extended status-code mappers and response-body parsing
  - `macros.rs`: exported provider error helper and mapper macros
- root `unified_provider.rs` 保持文档 facade，继续导出 `ProviderError`、`ContextualError`、
  `default_http_error_mapper`、`extended_http_error_mapper` 和 `parse_error_message_from_body`。
- 保持所有 error variants、factory method signatures、HTTP status/retry semantics 和 macro names 不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 provider runtime dispatch、factory wiring、provider implementations 或 error conversion traits。
- 不重命名 `ProviderError` variants、methods、macros 或 mapper function paths。
- 不在本 PR 中处理其余 36 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `crate::core::providers::unified_provider::ProviderError` remains the canonical type path.
2. Existing provider imports of `default_http_error_mapper`, `extended_http_error_mapper`, and
   `parse_error_message_from_body` continue to compile unchanged.
3. Exported macros `define_provider_error_helpers!`, `impl_provider_error_helpers!`,
   `define_standard_error_mapper!`, and `define_extended_error_mapper!` keep the same names and expansions.
4. Provider error factory, retryability, retry delay, HTTP status, and contextual error behavior remain unchanged.
5. `src/core/providers/unified_provider.rs` and `src/core/providers/unified_provider/*.rs` must be below 800 lines.
6. `cargo test core::providers::unified_provider_tests --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/providers/unified_provider.rs` declares focused child modules and re-exports the original public unified-provider surface。
- [ ] `ProviderError` moves to `src/core/providers/unified_provider/error.rs` without variant, derive, or error-display changes。
- [ ] ProviderError factory/status/retry/context methods move to `methods.rs` without signature or behavior changes。
- [ ] Default and extended HTTP mapper helpers move to `http_mapping.rs` without mapping semantic changes。
- [ ] Exported unified provider macros move to `macros.rs` without macro name or expansion changes。
- [ ] Focused unified provider error tests 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a unified provider error facade decomposition tranche for U-16 compliance.
