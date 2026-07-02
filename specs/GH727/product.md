# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@5a4c42c15b86` 仍有 31 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/core/integrations/observability/opentelemetry.rs`，它把配置、span 数据模型、OTLP payload/export、
`Integration` trait 实现和单元测试全部放在一个 921 行文件里。

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

- 拆分 `src/core/integrations/observability/opentelemetry.rs`，它当前 921 行，是 #727 当前最大的 runtime orchestrator。
- 保留原模块路径作为 facade：`opentelemetry.rs` 继续导出 `OpenTelemetryConfig`、
  `OpenTelemetryIntegration`、`Span`、`SpanKind`、`SpanStatus`、`SpanEvent` 和 `AttributeValue`。
- 按职责拆为 `config.rs`、`span.rs`、`exporter.rs`、`integration_impl.rs` 和 `tests.rs`。
- `integration_impl.rs` 继续拥有 `Integration for OpenTelemetryIntegration`，不得改变 trait surface、
  active/pending span 状态流转或 flush/shutdown 行为。
- 本 tranche 接受 review-driven sampling bug fix：`0.0 < sampling_ratio < 1.0` 不再因为
  `(now as f64) % 1.0` 永远为 `0.0` 而退化成 100% 采样。
- `exporter.rs` 只负责 OTLP payload 构造和 HTTP export；`span.rs` 只负责 span 数据模型、attribute conversion
  和 ID generation；`config.rs` 只负责 serde/default 配置。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不改变 OpenTelemetry endpoint、headers、resource/service payload shape 或 export error propagation。
- 不引入新的 observability abstraction、global prelude、dynamic `Any` API 或 warning-only fallback。
- 不在本 PR 中处理其余 30 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. Existing public imports through `crate::core::integrations::observability::opentelemetry::*` remain available.
2. `OpenTelemetryIntegration` continues to implement `Integration` with the same async handlers and return types.
3. Span lifecycle behavior remains unchanged: start stores active spans, end/error moves them to pending spans, cache hits create short completed spans, flush exports pending spans.
4. Partial sampling now uses a clock-derived fraction in `[0.0, 1.0)` instead of always sampling for any positive ratio.
5. OTLP payload construction remains test-covered through the moved `build_otlp_payload` test.
6. Every touched Rust file must be below U-16's 800-line ceiling.
7. `cargo test core::integrations::observability::opentelemetry --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/integrations/observability/opentelemetry.rs` is a small facade with focused child modules。
- [ ] Public OpenTelemetry config, integration, span, event, status, kind, and attribute names remain re-exported from the original module path。
- [ ] Config, span data model, OTLP exporter, integration implementation, and tests are separated by responsibility。
- [ ] Partial sampling fraction has focused test coverage。
- [ ] All touched OpenTelemetry files are below U-16's 800-line ceiling。
- [ ] Focused OpenTelemetry test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

This is an OpenTelemetry integration module split for U-16 compliance. It also fixes a review-identified
partial-sampling bug where any positive `sampling_ratio` sampled 100% of requests.
