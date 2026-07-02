# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@27ac684370e1` 仍有 40 个 Rust 文件超过 U-16 的 800 行硬上限。前几个
tranche 已经证明“小 PR、单文件族、保持行为不变”的方式可行，但 spec packet 仍按上一轮
Cloudflare 单文件 tranche 书写，不能指导剩余队列的系统性解耦。

本轮目标是把 #727 升级为完整的大文件解耦计划：每个 PR 仍然小而可审，但所有 tranche 都必须
服从同一套架构边界，避免为了降行数而制造新的耦合、重导出混乱或行为漂移。

## 全量目标

- 把当前 40 个 over-800 Rust 文件逐步拆到 U-16 范围内。
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

- 拆分 `src/core/providers/bedrock/model_config.rs`，它当前 1118 行，是 #727 当前最大的 tracked Rust 文件。
- 将重复的 legacy `MODEL_CONFIGS` map 改为从现有 `bedrock/catalog` entries 投影生成。
- `model_config.rs` 保留 Bedrock family/API/config 类型和 public lookup facade：
  - `get_model_config`
  - `model_supports_capability`
  - `get_all_model_ids`
- 保持 Bedrock model routing public API、model IDs、capability flags、limits、pricing fields 和错误语义不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不添加、删除或重命名 Bedrock model IDs。
- 不修改 Bedrock request transformation、routing, pricing calculation, region validation, or model-id parsing behavior。
- 不在本 PR 中处理其余 39 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `BedrockModelFamily`, `BedrockApiType`, `ModelConfig`, `get_model_config`, `model_supports_capability`, and `get_all_model_ids` remain exported from `crate::core::providers::bedrock`.
2. `get_model_config` still returns `ProviderError::model_not_found("bedrock", ...)` for unknown model IDs.
3. Catalog projections must preserve model family, API type, streaming/tool/multimodal flags, context limits, output limits, and cost fields for every existing entry.
4. Existing Bedrock catalog cross-reference tests and model_config tests must pass.
5. `src/core/providers/bedrock/model_config.rs` and touched Bedrock catalog files must be below 800 lines.
6. `cargo test core::providers::bedrock::model_config --lib --all-features` and `cargo test core::providers::bedrock::catalog --lib --all-features` must pass.

## 验收标准

- [ ] `src/core/providers/bedrock/model_config.rs` 使用 catalog projection 构建 `MODEL_CONFIGS`，不再内联重复 model table。
- [ ] `bedrock/catalog` 文档从“legacy map 仍保留”更新为“legacy public facade projects from catalog”。
- [ ] Existing Bedrock model_config and catalog tests 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No intended runtime behavior change. This is a Bedrock model-config catalog projection tranche for U-16 compliance.
