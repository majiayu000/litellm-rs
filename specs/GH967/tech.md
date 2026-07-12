# Tech Spec

## Linked Issue

GH-967 / #967

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Required boundary |
| --- | --- | --- | --- |
| Tier-1 catalog | `src/core/providers/registry/definition.rs`, `catalog.rs` | Definition has URL/auth/model metadata but no capability data | Definition owns an explicit static capability slice |
| OpenAI-like runtime | `src/core/providers/openai_like/provider.rs` | Every instance returns one overbroad static list | Instance stores the selected static slice and validates it |
| Provider factories | `src/core/providers/factory/mod.rs`, `factory/registry.rs` | Catalog metadata becomes config, then capability identity is lost | Both catalog paths pass definition capabilities into the instance |
| Exported completion router | `src/core/completion/default_router/mod.rs` | Environment registration uses the default profile and silently ignores construction errors | Pass the definition slice and propagate profile errors |
| Capability selection | `src/core/providers/capability_dispatch.rs`, `src/core/router/selection.rs` | Existing canonical predicate already filters deployments | Preserve this path; correct the instance truth it consumes |
| Explicit proxy routes | `src/server/routes/ai/images.rs`, `moderations.rs` | `openai_compatible` has callable image/moderation proxy paths outside `LLMProvider` | Preserve these capabilities only for the explicit selector profile |
| Public support matrix | `src/core/providers/registry/support_matrix.rs` | Generic Tier-1 is HTTP chat/stream only | Keep unchanged and add consistency coverage where useful |

## 设计方案

1. 在 `ProviderDefinition` 增加 `capabilities: &'static [ProviderCapability]`。catalog 使用命名明确的
   `def_chat` / `def_local_chat` profile helper，使每个 entry 的选择在调用点可见。当前 profile 包含
   `ChatCompletion`、`ChatCompletionStream`、`ToolCalling` 和 `FunctionCalling`；不声明 image 或
   moderation capability。
2. 在 `OpenAILikeProvider` 定义 `OPENAI_LIKE_CATALOG_CAPABILITIES` 与
   `OPENAI_COMPATIBLE_PROXY_CAPABILITIES` 两个命名 profile，并增加 instance 字段
   `capabilities: &'static [ProviderCapability]`。现有 `new(config)` 使用保守 catalog profile；crate
   内部 catalog constructor 接收 definition slice；显式 `openai_compatible` constructor 使用 proxy profile。
3. constructor 在建立 HTTP pool 前验证：slice 非空、无重复、每个值都属于该 constructor 的允许集合。
   catalog constructor 因此拒绝 image/moderation；失败返回 `OpenAILikeError::configuration`，不 fallback。
4. `LLMProvider::capabilities()` 返回 instance 字段。主 gateway factory、registry factory 与 exported
   completion `DefaultRouter` 的 catalog 分支都调用 catalog constructor；显式 `openai_compatible` factory
   调用 proxy constructor。`DefaultRouter` 只对缺少 env key 跳过，profile 构造错误向启动调用者传播。
5. 不修改 router selection 算法。`Provider::supports_capability_for_model()` 已经委托 canonical
   `supports_capability()`；instance 真值修正后，unsupported deployment 会在 reservation/执行前被排除。
6. conformance tests：
   - 遍历 `PROVIDER_CATALOG`，校验每个 slice 非空、无重复且属于 executable surface；
   - 通过 factory 构造代表性 canonical/alias provider，断言 instance 与 definition exact match；
   - 注入静态非法 profile，断言 constructor configuration error；
   - router 同时验证 Tier-1 chat 可选择、`ImageEdit`/`ImageVariation`/`Moderation` 返回
     `UnsupportedCapability`；既有 explicit proxy integration tests 必须继续通过。

## Executable Surface Mapping

| Capability | Runtime method | Status |
| --- | --- | --- |
| `ChatCompletion` | `OpenAILikeProvider::chat_completion` | implemented |
| `ChatCompletionStream` | `OpenAILikeProvider::chat_completion_stream` | implemented |
| `ToolCalling` | chat request/response passthrough | implemented through chat |
| `FunctionCalling` | legacy function fields through chat passthrough | implemented through chat |
| `ImageEdit` | explicit `openai_compatible` image proxy route | explicit proxy profile only |
| `ImageVariation` | explicit `openai_compatible` image proxy route | explicit proxy profile only |
| `Moderation` | explicit `openai_compatible` moderation proxy route | explicit proxy profile only |

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1/P2/P9 catalog ownership | definition, catalog, all three catalog factories | full-catalog and factory/default-router identity tests |
| P3/P4 executable surface | OpenAI-like provider and explicit proxy routes | exact catalog/proxy profiles plus existing integration tests |
| P5 fail closed | capability-aware constructor | invalid static profile returns configuration error |
| P6 route exclusion | existing router selection | catalog deployment chat succeeds; image/moderation unsupported |
| P7 direct construction | `OpenAILikeProvider::new` | default instance exact capability test |
| P8 conformance | catalog/provider tests | iteration over every catalog definition |

## 数据流

`ProviderDefinition.capabilities` -> catalog factory -> `OpenAILikeProvider.capabilities` ->
`Provider::supports_capability_for_model` -> router candidate filter -> selected executable deployment。

没有新增持久化、外部 API、配置字段或动态网络探测。capability slice 全部是编译期静态数据。

## 受影响文件与规模

- `src/core/providers/registry/definition.rs`
- `src/core/providers/registry/catalog.rs`
- `src/core/providers/openai_like/provider.rs`
- `src/core/providers/openai_like/provider/tests.rs`
- `src/core/providers/factory/mod.rs`
- `src/core/providers/factory/registry.rs`
- `src/core/completion/default_router/mod.rs`
- `src/core/router/tests/selection_tests.rs`
- `CLAUDE.md`
- `specs/GH967/*`

预计不超过 10 个文件、500 行 production/test diff（docs 不计），满足仓库 PR scope guard。

## 备选方案

- 只删除三个 capability：能修眼前症状，但 catalog 仍没有真值，未来会再次漂移，拒绝。
- 修改 trait 为 instance-borrowed `&[ProviderCapability]`：会触及全部 provider 实现，超出本 issue。
- 从 support matrix 运行时反推 capability：surface matrix 与 provider method surface 语义不同，拒绝。
- 用 `Any`/downcast 检查方法：违反 provider dispatch contract，拒绝。

## 风险

- Compatibility: Tier-1 的错误 image/moderation 宣称会更早返回 unsupported；显式 proxy route 必须保持。
- Logic: 三条 catalog 构造路径都必须传递 capability，否则行为会因入口不同而漂移。
- Data integrity: static slice 的重复/越界声明必须 fail closed，不能 warning 后继续。
- Performance: capability 查询仍是很短的静态 slice scan，无额外分配。
- Security: 不改变 URL、认证、header 或请求执行。

## 测试计划

- Focused: catalog definition、OpenAI-like provider、三条 factory/router capability tests、现有 image/moderation proxy integration tests。
- Deterministic: `cargo fmt --all -- --check`、`git diff --check`、all-features check、strict clippy。
- Repository: `cargo test --all-features --locked -- --test-threads=1`。
- Guards: `bash scripts/guards/check_pr_scope.sh`、`bash scripts/guards/check_pr_overlap.sh`。
- SpecRail: GH967 packet check、implementation route gate、current-head PR gate。

## 回滚方案

回滚 GH967 PR 即恢复旧静态 capability 列表。没有 migration 或持久化状态；但回滚会重新允许 route
选择没有执行方法的 Tier-1 provider，因此只应在发现真实 chat/stream 回归时执行。
