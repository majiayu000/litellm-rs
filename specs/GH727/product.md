# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@2d35b72e1031` 仍有 27 个 tracked Rust 文件超过 U-16 的 800 行硬上限。当前最大文件是
`src/utils/data/validation/request_validator.rs`，它是一个 868 行 runtime validator module，把
chat request validation、message/content-part validation、name/media helper validation 和 inline tests
全部放在一个文件里。

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

- 拆分 `src/utils/data/validation/request_validator.rs`，它当前 868 行，是 #727 当前最大的 runtime validator module。
- 保留 `src/utils/data/validation/mod.rs` 的 `pub use request_validator::RequestValidator;` 和公开导入路径不变。
- 将 root `request_validator.rs` 缩小为 facade，只声明 child modules 并保留 public `RequestValidator` 类型。
- 按职责移动实现到 `src/utils/data/validation/request_validator/*.rs`：
  `chat.rs`、`names.rs`、`media.rs`、`tests.rs`。
- 保持 chat request、message role、message content、content part、model/function name、image URL/base64 和 audio validation 的错误语义不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 OpenAI request/response model definitions、route handlers 或 caller-facing validation contracts。
- 不改变 validator error strings、regex patterns、base64 decoding、URL parsing 或 supported audio formats。
- 不在本 PR 中处理其余 26 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `utils::data::validation::RequestValidator` remains available from the original public module path.
2. `validate_chat_completion_request` keeps the same public signature and validates model, messages, max tokens, and temperature in the same order.
3. Message role, content, content-part, model-name, function-name, image URL/base64, and audio validation errors remain `GatewayError::Validation` with unchanged messages.
4. Regex compilation failures remain `GatewayError::Internal`.
5. The moved inline tests remain under `utils::data::validation::request_validator::tests`.
6. Every touched Rust file must be below U-16's 800-line ceiling.
7. `cargo test utils::data::validation::request_validator --lib --all-features` must pass.

## 验收标准

- [ ] `src/utils/data/validation/request_validator.rs` is a small facade with module declarations and the original public `RequestValidator` type。
- [ ] Chat request/message/content-part validation moves to `request_validator/chat.rs` without signature or error-message changes。
- [ ] Model/function name and media/base64/audio validation move to focused child modules without behavior changes。
- [ ] Original inline request validator tests move to `request_validator/tests.rs` without assertion changes。
- [ ] All touched request validator files are below U-16's 800-line ceiling。
- [ ] Focused request validator test suite 通过。
- [ ] `cargo fmt --all -- --check`、`cargo check --lib --all-features`、`cargo check --all-features --locked` 和 `cargo check` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a request validator module split for U-16 compliance.
