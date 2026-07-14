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

## 设计方案

### 1. Crate-private native request contract

- 在 provider 层定义 crate-private、强类型的 Gemini native request（API version、model、method、stream、JSON）
  和执行结果；字段校验复用现有 route 校验，接口不使用 `Any`、downcast 或公开的无类型 payload。
- 为 closed `Provider` enum 增加 crate-private `supports_gemini_native_route()` 与异步执行 dispatch。仅 native
  `Provider::Gemini` 和满足名称闭集的 `Provider::OpenAILike` 返回支持；其他 variant 明确返回不支持错误。
- 该能力仅服务 SDK-compatible native wire route，不冒充普通 chat capability，也不扩大公共 API。

### 2. Runtime-owned HTTP execution

- `GeminiProvider` 委托其 `GeminiClient`，由 client 使用已构造的 ordinary/streaming `BaseHttpClient`、
  `GeminiConfig` endpoint/API key/custom headers/request timeout 构建 native URL 并返回原始 `reqwest::Response`。
- `OpenAILikeProvider` 只在规范化 `provider_name` 属于 `gemini|googleai|googleaistudio` 时执行该协议；使用自身
  `OpenAILikeConfig` base endpoint/API key/custom headers/timeout 与 pool-owned policy client，不读取 Gateway config。
- URL 只接受现有 `v1|v1beta` 与 `generateContent|streamGenerateContent` 组合；stream 添加 `alt=sse`，AI Studio
  key 继续使用 query。所有 client/build/send/timeout/HTTP 错误映射为现有 `ProviderError`，无 silent fallback。

### 3. Route selection and identity

- Gemini route 删除 `state.config().providers()` candidate scan、selected deployment 到 config 的匹配和
  route-owned client 构造。router key 由请求模型与受限兼容别名组成并去重；真正 eligibility 在 selected
  runtime provider 的 crate-private capability 上验证。
- 若同一 router key 下先选中不支持的 provider，execution helper 将其作为该 deployment 的明确不支持失败；
  route 不自行改用配置对象。为避免污染普通 provider，测试要求只把声明 native capability 的 deployment 纳入
  Gemini selection；若现有 capability selector 无法表达，新增内部 predicate selector，仍传完整 snapshot。
- `GeminiRouteProvider` 缩为 `provider_name`、`pricing_provider`、`model` 的只读 identity。预算预留在发送前仍用
  selected identity；真正 send 调用 selected `Provider`。普通 retry 与 stream lease 保持现有 helper 所有权。

### 4. Verification architecture

- integration fixture 先构造 router/runtime provider，再把 `AppState` 的 Gateway provider config 改为第二个
  listener/错误 key；普通与 stream 均必须只命中第一个 listener，第二个 listener 未接收连接。
- 扩展 fallback suite 覆盖上游错误、provider/model budget、health/cooldown、stream lease/spend identity；保留
  既有 named `gemini`/`googleai` OpenAI-compatible fixtures并增加 `googleaistudio`/非闭集拒绝。
- source guard 精确拒绝 Gemini route 中 `state.config().providers()`、`RouteHttpClient`、复制认证/endpoint 字段
  和 config-selection helper；不使用可增长的数量 baseline。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | route selected callback + closed provider dispatch | unary/stream snapshot executor tests；source guard 无 config rescan |
| B-002 | GeminiClient/OpenAILike runtime-owned config/client | router 后 config mutation 双 listener/key/header/timeout tests |
| B-003 | native Gemini passthrough | v1/v1beta unary/stream URL、query、headers、JSON/SSE integration matrix |
| B-004 | OpenAILike named compatibility dispatch | 三个名称正例、大小写规范化与任意名称拒绝 tests |
| B-005 | selection/unsupported error mapping | 无 deployment、模型不匹配、非 Gemini provider tests；无 fallback client source guard |
| B-006 | identity-only route adapter + execution helper | provider/model/deployment id 的 budget/health/fallback/spend assertions |
| B-007 | existing unary retry helper | upstream 429/5xx、provider/model budget fallback tests |
| B-008 | existing stream execution lease + spend settlement | stream success/read failure/cancel lease 与 spend identity tests |
| B-009 | reduced `GeminiRouteProvider` | compile-time struct fields + source guard 禁止 key/url/headers/timeout/client |
| B-010 | focused source guard + full gates | guard red/green fixture、strict Clippy、全量 test、PR gate |

## 数据流

请求经过鉴权与 payload 校验后，以请求模型进入统一路由器。路由器从同一 immutable snapshot 选出 deployment，
callback 获得 concrete `Provider`、deployment model 与 id；route 先以该身份预留预算，再把 native request 交给
该 `Provider` 的 crate-private dispatch。provider 自己的 runtime client 发出请求并返回原始 response。普通响应
与 SSE 仍由 route 透传/解析 usage；健康、fallback、lease 和 spend 全部使用同一 selected deployment identity。

## 备选方案

- 只让 route 按 selected provider name 继续扫描 config：同名/热替换仍产生第二执行器，拒绝。
- 把 endpoint/key/client 放进 callback 参数：仍复制 runtime state且扩大敏感数据边界，拒绝。
- 使用 `Any`/downcast 找 concrete provider：破坏 closed enum 的类型安全，拒绝。
- 让所有 OpenAI-compatible provider 都尝试 Gemini wire：会把不兼容 upstream 纳入路由，拒绝。
- 把 native request 转成普通 OpenAI chat：改变 SDK wire/响应透传与 SSE 语义，拒绝。

## 风险

- Security: 新 dispatch 若公开敏感 config、绕过 endpoint policy 或允许任意 OpenAI-compatible 名称，会扩大出站面。
- Compatibility: router key 变化可能漏掉现有显式别名；必须保留三个命名闭集与模型匹配测试。
- Performance: 复用 runtime client 减少 client 构造；额外 capability 判断为常数时间。
- Maintenance: native URL/header 逻辑若在 route 与 provider 两处保留会再次漂移，source guard 必须确保单一 owner。

## 测试计划

- [ ] Unit tests: capability/name 闭集、native URL/method/version、错误映射、identity-only adapter。
- [ ] Integration tests: unary/stream snapshot mutation、native与 named compatibility、fallback/budget/health/lease/spend。
- [ ] Architecture tests: Gemini route config rescan/route client/敏感字段 guard 红绿与 production 零命中。
- [ ] Repository: `cargo fmt --all -- --check`、`cargo check --all-targets --all-features --locked`、
  `cargo clippy --all-targets --all-features --locked -- -D warnings`、
  `cargo test --all-features --locked -- --test-threads=1`、scope/overlap、SpecRail/PR gate。

## 回滚方案

若兼容性失败，整体 revert 本 issue 的 implementation PR，恢复旧 route 行为并重新打开 #966；不得只回滚
runtime dispatch 而留下声明但不可执行的 capability，也不得以 warning + config rescan 作为降级。修复应保持
GH968 endpoint policy，不重新引入普通 `reqwest` client。
