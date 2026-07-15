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
| Shared route HTTP adapter | `src/server/routes/ai/route_http.rs:10`, `src/server/routes/ai/route_http.rs:15`, `src/server/routes/ai/route_http.rs:36`, `src/server/routes/ai/route_http.rs:50` | `RouteHttpClient` 仍服务 batch、image、moderation 等配置型代理；Gemini 脱离后部分方法可能仅由测试覆盖 | Gemini route 必须停止使用它，但本 issue 不删除其他 route 的共享 adapter |
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
- OpenAI-like Gemini native sender 的 endpoint-policy 错误必须在 `reqwest::Error` 被 `Display`/字符串化之前保留结构化
  信号：直接 outbound URL policy 拒绝以及 source chain 中的 redirect-target/DNS-rebinding policy 拒绝映射为
  不可重试的 `Configuration`；redirect loop、普通 redirect failure 与其他 transport 错误仍是可 fallback 的
  `Network`，timeout 仍是 `Timeout`。只允许 Gemini native sender 显式选择该保真路径，不改变其他 provider
  或现有 connection-pool 公共执行方法的错误语义。
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

### 4. Bounded serial implementation plan

Fresh implementation diff 已证明完整接线会跨 11 个非文档文件且超过 500 changed lines；仅移动既有测试的
decomposition 已先行完成，继续把生产删除与回归测试压入单 PR 会违反 scope gate。实现因此拆为三个严格串行、
每个最多 10 个非文档文件且最多 500 changed lines 的 PR。每个 PR 都从前一阶段合并后的最新 `main` 开始，运行
scope/overlap、全特性构建、strict Clippy、全量测试与 current-head PR gate；前两阶段只使用 `Refs #966`，issue
保持 open，只有 Phase C 使用 `Fixes #966`。每阶段凡改变或删除 API key、endpoint policy、client 或错误脱敏
边界，必须在 exact head 获得独立 security review PASS；dependency audit 不能替代该审查。

#### Phase A — runtime dispatch and selected sender

允许修改以下 10 个文件：

1. `src/core/types/model.rs`
2. `src/core/providers/capability_dispatch.rs`
3. `src/core/providers/mod.rs`
4. `src/core/providers/gemini/client.rs`
5. `src/core/providers/gemini/provider.rs`
6. `src/core/providers/openai_like/provider.rs`
7. `src/server/routes/ai/gemini.rs`
8. `src/server/routes/ai/gemini/provider.rs`
9. `src/server/routes/ai/route_http.rs`
10. `tests/gemini_sdk_routes/runtime_provider_tests.rs`

该阶段增加 typed native capability/request/closed dispatch，并把 Gemini route 的实际 HTTP send 接到 selected
runtime `Provider`。native Gemini 与受限命名 OpenAI-like runtime 各自拥有 URL、key、headers、endpoint policy、
client、错误脱敏和 response-header timeout；名称规范化只接受大小写及 `_`/`-` 差异，不删除空格或任意标点。
为保持可构建的 500 行原子边界，route 暂时可以扫描 Gateway config 来形成候选 key 和只读 route identity，
`GeminiRouteProvider` 也可暂时保留旧敏感字段与不再发送请求的 `RouteHttpClient`；这些字段不得参与实际 send，
并必须在 Phase B 删除。`route_http.rs` 仅允许增加 Gemini 不再调用的方法所需 dead-code annotation，不改变其他
route 的共享 adapter 行为。Phase A 必须包含 config mutation 的 unary sender snapshot 回归、feature-matrix
构建、受限名称闭集与 named stream runtime timeout 测试，不得宣称满足 B-001/B-002/B-009 的最终形态。
Phase A 的原 PR 已在本 amendment 之前创建；amendment 合并后必须在原分支
`codex/gh966-runtime-dispatch` merge 最新 `origin/main`，禁止 force push或新建替代 PR，再对新的 exact head 重跑
CI、reviewThreads、implementation/security review 与 required gate。

#### Phase A regression follow-up — preserve typed endpoint-policy failures

Phase A 合并后的公开回归证明，仅在 OpenAI-like provider 层分析已字符串化的 `ProviderError::Network`
无法无损区分 endpoint policy 与普通 transport 错误：`reqwest::Error` 的 `Display` 不保留
redirect-target 与 DNS-rebinding 的 source chain，而宽泛匹配 `error following redirect` 会把 redirect loop
错判为不可重试配置错误。因此 Phase B 之前先在原 PR #1021、原分支
`codex/gh966-transport-classification` 完成一个严格串行的 regression follow-up，允许修改以下 6 个文件：

1. `src/utils/net/http.rs`
2. `src/utils/net/http/provider_tests.rs`
3. `src/core/providers/base/http.rs`
4. `src/core/providers/base/connection_pool.rs`
5. `src/core/providers/openai_like/provider.rs`
6. `src/core/providers/openai_like/provider/tests.rs`

`BaseHttpClient` 与 connection pool 必须保留现有 ordinary/streaming 执行方法与全局语义；
`BaseHttpClient` 仅可新增 crate-private typed request opt-in，connection pool 仅可新增显式 opt-in 路径；
OpenAI-like Gemini native sender 是唯一调用者。opt-in 路径在字符串化前检查直接 outbound URL policy
拒绝与 `reqwest::Error::source()` 链中的精确 redirect-target/DNS-rebinding policy 来源，并为 ordinary 与
streaming 返回结构化 `Configuration`。不得用宽泛 redirect 文本、URL、host 或 key 作为分类标记；
对外错误文本必须固定且不泄露 raw/URL-encoded key 或 endpoint。回归必须使用真实本地 redirect
链证明 policy redirect 为 `Configuration`、redirect loop 仍为 `Network`，并覆盖 DNS-rebinding source、直接
unsupported-scheme、timeout、ordinary 和 streaming。本 follow-up 最多 6 个非文档文件、500 changed lines，
使用 `Refs #966`；exact-head implementation/security review、CI、0 unresolved threads 与 required gate 通过后才可
合并，合并后 #966 仍保持 open。

#### Phase B prerequisite regression follow-up — align PublicOnly evidence with immutable runtime

Phase B 的 exact-head 回归暴露出 `tests/gemini_sdk_routes.rs` 中
`public_only_gemini_route_rejects_loopback_before_connect` 已不再能证明它声明的 route-time 行为：测试在
`AppState`/router bootstrap 之后发请求，但 immutable selected runtime provider 已在 bootstrap 时固定；旧 fixture
的 route 请求实际落到 bootstrap runtime 的 `example.com`，返回 405，而不是重新读取 Gateway config 中后来提供的
loopback endpoint 并返回 500。继续保留该断言会要求 route 恢复 post-selection config reconstruction，直接违反
B-001/B-002/B-009。

因此在 Phase B 前增加一个严格串行、独立的 regression follow-up，只允许修改
`tests/gemini_sdk_routes.rs`。它只能删除/替换上述这一条不可达的历史 route assertion：新断言必须在 runtime
provider bootstrap/configuration 阶段以 `PublicOnly` + loopback 构造 Gemini provider，证明构造以明确的
`Configuration`/SSRF 错误 fail closed，并证明 loopback listener 未收到连接。不得删除、跳过或放宽底层
`test_ssrf_validation_loopback`、`GeminiConfig::test_policy_client_settings_fail_closed`、
`base_http_client_rejects_public_loopback_base` 以及 factory endpoint-access 覆盖；不得把 405 当作安全成功，也不得
通过恢复 route config scan 让旧断言重新可达。

本 follow-up 最多 1 个非文档文件、500 changed lines，使用 `Refs #966`；exact-head focused test、全特性构建、
strict Clippy、全量测试、scope/overlap、independent implementation/security review、CI、0 unresolved threads 与
required gate 全部通过后才可合并，合并后 #966 保持 open。随后原 Phase B PR #1023 必须在原分支 merge 最新
`origin/main`（禁止 force push、禁止新建替代 PR），并在新的 exact head 重跑全部验证。该 follow-up 不进入 Phase B
diff，因此 Phase B 仍保持下述四文件 writable scope 和最多 500 changed lines。

#### Phase B — remove post-selection reconstruction

允许修改 `src/server/routes/ai/gemini.rs`、`src/server/routes/ai/gemini/provider.rs`、
`src/server/routes/ai/route_http.rs` 与 `tests/gemini_sdk_routes/runtime_provider_tests.rs`。该阶段删除 selected
deployment 到 `state.config().providers()` 的反查、route-owned client 构造、API key/base URL/headers/timeout
复制以及 Gemini adapter 的旧 send/error helpers。adapter 只保留 selected runtime provider name、pricing
identity 与客户端原始 requested Gemini model；URL/budget/spend 不得使用 named empty-model deployment 的
provider-name selection key。Phase B 结束时，pre-selection candidate-key 生成仍可读取 Gateway config，但选择
完成后不得再读取配置或重建执行器；issue 继续 open，PR 使用 `Refs #966`。合并后删除远端阶段分支并确认
#966 仍为 open，再从最新 main 创建 Phase C。Phase B 必须依赖上述 prerequisite regression follow-up 已合并，并从
该 merge 后的最新 `main` 重放；不得把 parent test 修改挤入已有 497-line Phase B diff。

#### Phase C — runtime-only discovery and final closure

允许修改 `src/server/routes/ai/gemini.rs`、`src/server/routes/ai/gemini/provider.rs`、
`tests/gemini_sdk_routes.rs` 与 `tests/gemini_sdk_routes/runtime_provider_tests.rs`。候选模型/别名只从 router 的
immutable runtime deployments 派生，删除 Gemini route 最后一处 `state.config().providers()`。补齐 unary/stream
双 snapshot、native + 三个命名兼容正例、任意名称拒绝、empty-model identity、fallback/budget/health/lease/spend、
client cancel neutral、upstream read failure 与 source guard。parent integration test 只在 immutable runtime 语义
改变既有断言时修改。最终 source guard 必须拒绝 Gemini route 的 config scan、`RouteHttpClient`、敏感 adapter
字段和 selected provider 之外的 sender；Phase C 通过 required gate 后使用 `Fixes #966` 关闭 issue。
旧 full checkpoint 只可作为实现参考，不可作为完成证据：不得恢复其过宽的标点归一化或过时 helper 布局，且
必须另行补齐 `googleaistudio` route 正例、cancel spend 与 upstream read-failure health assertions。

三个阶段的 writable union 为上述 11 个文件，不触碰 `src/server/routes/ai/execution.rs` 或 budget API。若任一阶段
fresh diff 超过自身 scope，不得削弱断言、压缩可读性或扩大 writable union；先重新切分该阶段并更新已合并规范。
Phase B prerequisite regression follow-up 是独立测试修正，不扩大 Phase B writable scope 或 changed-line budget。

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
- PublicOnly 回归在 runtime provider bootstrap/configuration 边界断言 loopback fail closed 且 listener 零连接；
  route integration 不以恢复 config rescan 来制造 500，底层 config/factory/Gemini client/Base HTTP SSRF 覆盖保持。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | route selected callback + closed provider dispatch | unary/stream snapshot executor tests；source guard 无 config rescan |
| B-002 | GeminiClient/OpenAILike runtime-owned config/client + typed policy opt-in | router 后 config mutation 双 listener/key/header/timeout tests；直接 URL 与 reqwest source 在 stringify 前保真 |
| B-003 | native Gemini passthrough | v1/v1beta unary/stream URL、query、headers、JSON/SSE integration matrix |
| B-004 | OpenAILike named compatibility dispatch | 三个名称正例、大小写规范化与任意名称拒绝 tests |
| B-005 | selection/unsupported error mapping | 无 deployment、模型不匹配、非 Gemini provider tests；无 fallback client source guard |
| B-006 | identity-only route adapter + execution helper | selected provider + requested Gemini model 的 URL/budget/spend；deployment-id fallback/health/lease；空 `models` named compatibility assertions |
| B-007 | existing unary retry helper | upstream 429/5xx、provider/model budget fallback tests；policy redirect 不重试且 redirect loop/普通 transport 仍 fallback |
| B-008 | existing stream execution lease + spend settlement | success=healthy、read failure=failed、cancel=neutral 的 lease/health/spend tests |
| B-009 | reduced `GeminiRouteProvider` | compile-time struct fields + source guard 禁止 key/url/headers/timeout/client |
| B-010 | focused source guard + full gates | guard red/green fixture、typed redirect/DNS source + ordinary/streaming regressions、bootstrap PublicOnly loopback fail-closed、strict Clippy、全量 test、PR gate |
| B-011 | provider-owned non-success response handling + fixed policy diagnostics | raw key、URL-encoded key、URI/endpoint echo 脱敏 tests；policy errors 不含敏感数据；route adapter 无 key source guard |

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
- [ ] Endpoint policy tests: PublicOnly loopback 在 config/factory/runtime client/Base HTTP 构造边界 fail closed；
  listener 零连接，且不依赖 route-time config reconstruction。
- [ ] Architecture tests: Gemini route config rescan/route client/敏感字段 guard 红绿与 production 零命中。
- [ ] Repository: `cargo fmt --all -- --check`、`cargo check --all-targets --all-features --locked`、
  `cargo clippy --all-targets --all-features --locked -- -D warnings`、
  `cargo test --all-features --locked -- --test-threads=1`、scope/overlap、SpecRail/PR gate。

## 回滚方案

若兼容性失败，按 Phase C → Phase B → Phase A 逆序整体 revert 已合并的 implementation PR，并在必要时重新打开
#966；不得只回滚 runtime dispatch 而留下声明但不可执行的 capability，也不得长期停留在仅 Phase A 的配置反查
状态或以 warning + config rescan 作为降级。修复应保持 GH968 endpoint policy，不重新引入普通 `reqwest` client。
