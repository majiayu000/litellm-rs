# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@63b6bf4e29bb` 仍有 41 个 Rust 文件超过 U-16 的 800 行硬上限。前几个
tranche 已经证明“小 PR、单文件族、保持行为不变”的方式可行，但 spec packet 仍按上一轮
Cloudflare 单文件 tranche 书写，不能指导剩余队列的系统性解耦。

本轮目标是把 #727 升级为完整的大文件解耦计划：每个 PR 仍然小而可审，但所有 tranche 都必须
服从同一套架构边界，避免为了降行数而制造新的耦合、重导出混乱或行为漂移。

## 全量目标

- 把当前 41 个 over-800 Rust 文件逐步拆到 U-16 范围内。
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

- 拆分 `src/core/providers/vertex_ai/client.rs`，它当前 1456 行，是 #727 当前 top offender。
- 将 Vertex AI client 的内部职责拆出为 focused child modules：
  - `client/error_mapper.rs`
  - `client/url.rs`
  - `client/health.rs`
  - `client_tests.rs`
- `client.rs` 保留 `VertexAIProvider` 主体、provider trait 实现和 request/response orchestration。
- 保持 `VertexAIProvider` public API、trait surface、错误语义、URL 字符串格式和测试断言不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 Vertex AI request/response transformation 行为。
- 不重构 auth、model parsing、Gemini/partner transformers、pricing 或 provider registry。
- 不在本 PR 中处理其余 40 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `VertexAIProvider` 仍从 `crate::core::providers::vertex_ai::client::VertexAIProvider` 导出。
2. `get_error_mapper` 仍返回 Vertex AI-specific mapper，HTTP/JSON/network error mapping 保持一致。
3. `build_url` 对 Gemini、partner、custom、global、streaming 和 custom API base 的输出保持一致。
4. 原 inline tests 继续由 `core::providers::vertex_ai::client::tests::*` 测试树运行。
5. `src/core/providers/vertex_ai/client.rs` 及新增 child files 必须低于 800 行。
6. `cargo test core::providers::vertex_ai::client --lib --all-features` 必须通过。

## 验收标准

- [ ] `src/core/providers/vertex_ai/client.rs` 声明 `error_mapper`、`url`、`health` 和 path-backed `tests` modules。
- [ ] `VertexAIErrorMapper` 移动到 `client/error_mapper.rs`，映射逻辑不变。
- [ ] URL builder 和 health check 移动到 child modules，调用点不变。
- [ ] 原 inline tests 移动到 `client_tests.rs`，断言不变。
- [ ] Focused Vertex AI client tests 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a Vertex AI client module decomposition tranche for U-16 compliance.
