# Tech Spec

## Linked Issue

GH-966 / #966

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Gemini route entry | `src/server/routes/ai/gemini.rs:135`, `src/server/routes/ai/gemini.rs:148`, `src/server/routes/ai/gemini.rs:165`, `src/server/routes/ai/gemini.rs:231`, `src/server/routes/ai/gemini.rs:246` | route 从 Gateway config 预检/构造 router keys，selected callback 后再次反查配置 | B-001、B-002、B-005 的漂移入口 |
| Route provider adapter | `src/server/routes/ai/gemini/provider.rs:24`, `src/server/routes/ai/gemini/provider.rs:61`, `src/server/routes/ai/gemini/provider.rs:80`, `src/server/routes/ai/gemini/provider.rs:118`, `src/server/routes/ai/gemini/provider.rs:178`, `src/server/routes/ai/gemini/provider.rs:316` | adapter 复制 key/base URL/headers/timeout 并构造 `RouteHttpClient` 发送 | issue 指定的第二执行器 |
| Runtime snapshot | `src/core/router/deployment.rs:295`, `src/server/routes/ai/execution.rs:60`, `src/server/routes/ai/execution.rs:157` | deployment 已持有 concrete `Provider`、model、id，执行 helper 将 snapshot clone 交给 callback | 可直接满足 B-001/B-006 |
| Closed provider dispatch | `src/core/providers/mod.rs:326`, `src/core/providers/mod.rs:352` | `Provider` enum 已集中执行 concrete provider 方法，但无 Gemini native passthrough dispatch | B-003/B-004 的类型安全入口 |
| Native Gemini client | `src/core/providers/gemini/client.rs:35`, `src/core/providers/gemini/client.rs:92`, `src/core/providers/gemini/client.rs:125`, `src/core/providers/gemini/provider.rs:34` | client 已拥有不可变 config 与 policy-aware ordinary/streaming clients，私有发送只服务转换后的 chat API | 应复用的 runtime executor |
| Named compatibility runtime | `src/core/providers/openai_like/provider.rs:53`, `src/core/providers/openai_like/provider.rs:66`, `src/core/providers/factory/registry.rs:104` | OpenAI-like runtime 已拥有 config/pool/name，但没有受限的 Gemini native passthrough | B-004 兼容闭集 |
| Spend/lease | `src/server/routes/ai/gemini/spend.rs:24`, `src/server/routes/ai/gemini/spend.rs:118`, `src/server/routes/ai/gemini.rs:280` | spend 依赖 route adapter 身份；stream lease 由统一 execution helper管理 | adapter 应缩为身份数据而非 executor |
| Error redaction | `src/server/routes/ai/gemini/provider.rs:406`, `src/server/routes/ai/gemini/provider.rs:486` | route adapter 持有 API key，并在映射 upstream error 前替换 raw/URL-encoded key | 去掉 adapter key 后必须把 B-011 迁移到 runtime owner |

## 设计方案

### 1. Typed native capability and crate-private request contract

- 在 provider 层定义 crate-private、强类型的 Gemini native request（API version、model、method、stream、JSON）
  和执行结果；字段校验复用现有 route 校验，接口不使用 `Any`、downcast 或公开的无类型 payload。
- 在现有 `ProviderCapability` typed enum 增加一个 additive Gemini native route marker，使现有 router capability
  selector 可在 lease 选择前排除无关 deployment；执行 request/response 方法仍为 crate-private。仅 native
  `Provider::Gemini` 和满足名称闭集的 `Provider::OpenAILike` 声明支持，其他 variant 明确返回不支持错误。
- 该 marker 仅服务 SDK-compatible native wire route，不冒充普通 chat capability，也不提供公开的执行 payload。

### 2. Runtime-owned HTTP execution

- `GeminiProvider` 委托其 `GeminiClient`，由 client 使用已构造的 ordinary/streaming `BaseHttpClient`、
  `GeminiConfig` endpoint/API key/custom headers/request timeout 构建 native URL 并返回原始 `reqwest::Response`。
- `OpenAILikeProvider` 只在规范化 `provider_name` 属于 `gemini|googleai|googleaistudio` 时执行该协议；使用自身
  `OpenAILikeConfig` base endpoint/API key/custom headers/timeout 与 pool-owned policy client，不读取 Gateway config。
- URL 只接受现有 `v1|v1beta` 与 `generateContent|streamGenerateContent` 组合；stream 添加 `alt=sse`，AI Studio
  key 继续使用 query。所有 client/build/send/timeout/HTTP 错误映射为现有 `ProviderError`，无 silent fallback。
- provider-owned execute contract 在读取任何非成功 upstream body 后、返回/记录错误前，同时替换 runtime
  config 中 API key 的 raw 与 `application/x-www-form-urlencoded` 形式；route adapter 不重新获取或保存 key。

### 3. Route selection and identity

- Gemini route 删除 `state.config().providers()` candidate scan、selected deployment 到 config 的匹配和
  route-owned client 构造。router key 由请求模型与受限兼容别名组成并去重；`run_unary`/`run_stream` 继续使用
  现有 `select_deployment_lease_for_capability_matching`，但传入 Gemini native marker，使不支持的 deployment
  在 lease 获取前被排除，不修改 799 行的 execution helper。
- `GeminiRouteProvider` 缩为 `provider_name`、`pricing_provider`、`requested_model` 的只读 identity。native URL、
  预算与 spend 使用 selected provider name + 客户端原始 requested Gemini model；不得用 callback 的
  `selected_model` 替换请求 model，因为空 `models` 的 named compatibility deployment 可能以 provider name
  作为 deployment model。fallback/health/lease 继续由 helper 持有 selected deployment id，普通 retry 与
  stream lease 保持现有 helper 所有权。
- client channel 断开时保持现有 neutral health 语义：结算取消前已观察 usage/输出，随后 drop 同一 lease 仅释放
  并发计数，不调用 `finish_success` 或 `finish_failure`；上游读取失败仍显式 `finish_failure`。

### 4. Bounded implementation file plan

完整接线预计修改下列 10 个非文档文件，不触碰 `src/server/routes/ai/execution.rs` 或 budget API：

1. `src/core/types/model.rs`：新增 typed Gemini native capability marker。
2. `src/core/providers/capability_dispatch.rs`：限制 named OpenAI-like capability 闭集。
3. `src/core/providers/mod.rs`：crate-private request/execute closed-enum dispatch。
4. `src/core/providers/gemini/client.rs`：复用现有 config/client 的 raw native send；修改后保持 800 行以内。
5. `src/core/providers/gemini/provider.rs`：声明 capability 并委托现有 client。
6. `src/core/providers/openai_like/provider.rs`：受限名称的 runtime-owned native send；修改后保持 800 行以内。
7. `src/server/routes/ai/gemini.rs`：selected provider execution、native marker 与 neutral cancel 断言。
8. `src/server/routes/ai/gemini/provider.rs`：删除 config scan/client rebuild，保留 identity/budget/error adapter。
9. `tests/gemini_sdk_routes.rs`：只登记新的拆分测试子模块，不继续膨胀现有 698 行主体。
10. `tests/gemini_sdk_routes/runtime_provider_tests.rs`：snapshot mutation、兼容闭集与 cancel identity 回归测试。

实现前后都运行 `check_pr_scope.sh`；若 fresh diff 仍超过 500 changed lines，则不得削弱测试或挤压文件，而是先
提交仅移动既有测试、集合/断言 byte-equivalent 的 `Refs #966` fixture decomposition PR，再从最新 main 执行上述
接线并在最终 `Fixes #966` PR 关闭 issue。任何中间 PR 不声明未接线 capability。

### 5. Verification architecture

- integration fixture 先构造 router/runtime provider，再把 `AppState` 的 Gateway provider config 改为第二个
  listener/错误 key；普通与 stream 均必须只命中第一个 listener，第二个 listener 未接收连接。
- named compatibility fixture 覆盖 `models=[]`：router deployment model/selection key 可为 `googleai` 等 provider
  name，但 upstream URL、pricing lookup、model budget 与 spend 必须仍使用客户端请求的 Gemini model。
- 扩展 fallback suite 覆盖上游错误、provider/model budget、health/cooldown、stream lease/spend identity；保留
  既有 named `gemini`/`googleai` OpenAI-compatible fixtures并增加 `googleaistudio`/非闭集拒绝。
- cancel 测试断言 selected lease active count 回到零、健康成功/失败计数均不增加，并按取消前已观察数据处理
  spend；upstream read failure 对照组断言健康失败增加。
- source guard 精确拒绝 Gemini route 中 `state.config().providers()`、`RouteHttpClient`、复制认证/endpoint 字段
  和 config-selection helper；不使用可增长的数量 baseline。
- provider 单测让 upstream error body/URI 分别回显 raw key 与 URL-encoded key，断言返回的 typed error/body、
  `ProviderError` display/debug 候选文本均只含 `[REDACTED]`，且不含两种原值。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | route selected callback + closed provider dispatch | unary/stream snapshot executor tests；source guard 无 config rescan |
| B-002 | GeminiClient/OpenAILike runtime-owned config/client | router 后 config mutation 双 listener/key/header/timeout tests |
| B-003 | native Gemini passthrough | v1/v1beta unary/stream URL、query、headers、JSON/SSE integration matrix |
| B-004 | OpenAILike named compatibility dispatch | 三个名称正例、大小写规范化与任意名称拒绝 tests |
| B-005 | selection/unsupported error mapping | 无 deployment、模型不匹配、非 Gemini provider tests；无 fallback client source guard |
| B-006 | identity-only route adapter + execution helper | selected provider + requested Gemini model 的 URL/budget/spend；deployment-id fallback/health/lease；空 `models` named compatibility assertions |
| B-007 | existing unary retry helper | upstream 429/5xx、provider/model budget fallback tests |
| B-008 | existing stream execution lease + spend settlement | success=healthy、read failure=failed、cancel=neutral 的 lease/health/spend tests |
| B-009 | reduced `GeminiRouteProvider` | compile-time struct fields + source guard 禁止 key/url/headers/timeout/client |
| B-010 | focused source guard + full gates | guard red/green fixture、strict Clippy、全量 test、PR gate |
| B-011 | provider-owned non-success response handling | raw key、URL-encoded key 与 URI echo 脱敏 tests；route adapter 无 key source guard |

## 数据流

请求经过鉴权与 payload 校验后，以请求模型进入统一路由器。路由器从同一 immutable snapshot 选出 deployment，
callback 获得 concrete `Provider`、deployment model 与 id，同时保留原始 requested Gemini model；route 以
selected provider name + requested model 预留预算并构造 native request，再交给该 `Provider` 的 crate-private
dispatch。provider runtime client 发出请求并返回 typed response/error。普通响应与 SSE 仍由 route 透传/解析
usage；spend 使用相同 provider/requested-model，健康、fallback 与 lease 使用相同 selected deployment id。

## 备选方案

- 只让 route 按 selected provider name 继续扫描 config：同名/热替换仍产生第二执行器，拒绝。
- 把 endpoint/key/client 放进 callback 参数：仍复制 runtime state且扩大敏感数据边界，拒绝。
- 使用 `Any`/downcast 找 concrete provider：破坏 closed enum 的类型安全，拒绝。
- 让所有 OpenAI-compatible provider 都尝试 Gemini wire：会把不兼容 upstream 纳入路由，拒绝。
- 把 native request 转成普通 OpenAI chat：改变 SDK wire/响应透传与 SSE 语义，拒绝。

## 风险

- Security: 新 dispatch 若公开敏感 config、绕过 endpoint policy、遗漏 raw/encoded key 脱敏或允许任意
  OpenAI-compatible 名称，会扩大出站面或泄漏凭据。
- Compatibility: additive capability marker 与 router key 变化可能漏掉现有显式别名；必须保留三个命名闭集与模型匹配测试。
- Performance: 复用 runtime client 减少 client 构造；额外 capability 判断为常数时间。
- Maintenance: native URL/header 逻辑若在 route 与 provider 两处保留会再次漂移，source guard 必须确保单一 owner。

## 测试计划

- [ ] Unit tests: capability/name 闭集、native URL/method/version、错误映射、identity-only adapter。
- [ ] Integration tests: unary/stream snapshot mutation、native与 named compatibility、fallback/budget/health/lease/spend。
- [ ] Cancellation tests: client cancel health neutral/lease release/spend settlement，并以上游 read failure 为失败对照。
- [ ] Security tests: upstream error/URI 中 raw 与 URL-encoded API key 均在 provider 边界内脱敏。
- [ ] Architecture tests: Gemini route config rescan/route client/敏感字段 guard 红绿与 production 零命中。
- [ ] Repository: `cargo fmt --all -- --check`、`cargo check --all-targets --all-features --locked`、
  `cargo clippy --all-targets --all-features --locked -- -D warnings`、
  `cargo test --all-features --locked -- --test-threads=1`、scope/overlap、SpecRail/PR gate。

## 回滚方案

若兼容性失败，整体 revert 本 issue 的 implementation PR，恢复旧 route 行为并重新打开 #966；不得只回滚
runtime dispatch 而留下声明但不可执行的 capability，也不得以 warning + config rescan 作为降级。修复应保持
GH968 endpoint policy，不重新引入普通 `reqwest` client。
