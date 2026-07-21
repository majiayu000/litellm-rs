# Tech Spec

## Linked Issue

GH-965 / #965

## Product Spec

见 `product.md`。

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Canonical router candidate | `src/core/router/unified.rs:206`, `src/core/router/gateway_config.rs:37` | `UnifiedRouter` 持有 immutable deployment snapshots；gateway config 通过 `create_provider` 构造 provider 并加入 deployment。 | 已包含 provider instance、selection state、health/retry/fallback 的主要 runtime ownership。 |
| HTTP binding | `src/server/http.rs:131`, `src/server/state.rs:35` | server 启动时构造一个 `UnifiedRouter` 并以 `Arc` 放入 `AppState`。 | HTTP 已最接近目标 adapter 形态，应作为兼容基线而非重写目标。 |
| High-level completion | `src/core/completion/default_router/mod.rs:22`, `src/core/completion/default_router/mod.rs:205`, `src/core/completion/default_router/router_impl.rs:174` | `DefaultRouter` 持有独立 `ProviderRegistry`，全局 `OnceCell<Box<dyn Router>>` 从环境变量初始化并执行独立 selection/send。 | 是第二套 provider store 和 execution engine。 |
| Completion trait/export | `src/core/completion/router_trait.rs:14`, `src/core/completion/mod.rs:35` | 独立 `Router` trait、`DefaultRouter` 与 free functions 都从 completion module 导出。 | 兼容命运需要 `HD-003`，不能直接删除。 |
| Runtime registry | `src/core/providers/provider_registry.rs:9`, `src/lib.rs:145` | `ProviderRegistry` 是公开的独立 `HashMap<String, Provider>`；completion 与 embedding router 使用它。 | 必须删除 runtime ownership 或降级为无状态/兼容 facade，并处理 embedding 调用方。 |
| SDK ownership | `src/sdk/client/llm_client.rs:16`, `src/sdk/client/routing.rs:109`, `src/sdk/client/completions.rs:92` | `LLMClient` 持有 SDK config、两个 HTTP client、stats 与 load balancer；selection 只复用 `UnifiedRouter` 的静态 strategy helper，执行仍按 SDK config 分支发送。 | 是第三套 provider map、selection state 与 sender。 |
| Support truth | `src/core/providers/registry/support_matrix.rs:12`, `src/core/providers/registry/support_matrix.rs:285` | #728 的 matrix 判断入口 surface 是否可用，但不创建 provider 或执行请求。 | GH-965 必须消费它，不能把声明误当 runtime。 |
| Error surfaces | `src/core/providers/unified_provider_error.rs:4`, `src/utils/error/gateway_error/types.rs:14`, `src/sdk/errors.rs:7` | core provider、gateway 和 SDK 分别公开 error enum，当前映射并非单一闭集。 | B-006 与 `HD-004` 需要完整映射。 |
| Release policy | `.github/workflows/version-bump.yml:50`, `.github/workflows/version-bump.yml:63` | conventional breaking commit 固定选择 `major`，0.5.x 会计算为 1.0.0。 | 已批准 0.6.0 deprecation → 0.7.0 removal；removal 前必须先修订并 fixture 验证。 |
| Existing architecture note | `docs/refactor/01-architecture.md:126` | 已建议把 `DefaultRouter` 改为 `UnifiedRouter` adapter。 | 支持方向，但不是维护者对生命周期/API 的正式选择。 |

## Duplicate / Overlap Boundary

- #725 / merged PR #731 已收敛 registry metadata、factory constructibility 与 canonical keying；GH-965
  复用 `create_provider`/`Provider`，不重写 catalog、factory 或 enum dispatch。
- #728 / merged PR #734 已建立 HTTP/SDK/`completion()` support matrix；GH-965 只让实际执行消费同一
  runtime，不扩大或重算 matrix。
- #966 已由 merged PR #1026 收敛 Gemini SDK-compatible HTTP route 的 selected runtime identity；
  `tests/gemini_sdk_routes/runtime_provider_tests.rs` 保持只读并必须继续通过。D7b/D7c 仅可对
  `src/server/routes/ai/gemini.rs`、`src/server/routes/ai/gemini/provider.rs` 做 RuntimeHandle binding plumbing，
  不得改变 Gemini wire、endpoint policy 或 selected identity；这是对已实现调用链的机械迁移，不重开 #1026。
- #968 负责 provider endpoint access 与 SSRF；GH-965 不创建普通 client 旁路，adapter 只能使用 selected
  provider 已持有的 secure client。
- #519 保持 umbrella；#727 继续处理通用 U-16 文件拆分。GH-965 的拆分只服务本 issue 的 500-line gate。
- 2026-07-15 重复检查：`gh pr list --repo majiayu000/litellm-rs --state all --search
  '965 in:body'` 返回空列表；GitHub issue 搜索 `DefaultRouter UnifiedRouter`、
  `ProviderRegistry completion router`、`canonical runtime router` 只命中 #965、umbrella #519 及已关闭
  #725 实现。未发现覆盖本 acceptance criteria 的重复工作或 `Refs/Fixes #965` PR。

## Human Decision Gate

维护者 issue comment `4982855807` 已将 `HD-001` 至 `HD-004` 全部标为 `resolved`：采用显式 per-instance
runtime + replaceable process default、受限 request context、0.6.0 deprecation → 0.7.0 removal，以及
`ProviderError` typed source。实现门仍需本 D0 amendment 经独立 review 并合并；`SP965-T002` 不得仅凭 issue
comment 越过 spec gate。另有 release-policy 子门：`SP965-T010` 只建立并链接 durable follow-up；
0.7.0 typed replacement/removal 由该 follow-up 实施，并把当前 version-bump workflow 的显式修订与 fixture
证据设为 removal 前硬依赖。

## 设计方案

### 1. Canonical runtime contract and exact binding API

以现有 `UnifiedRouter` deployment snapshot 为唯一 owner。D1 新增以下 exact public API（名称、返回错误和
replacement 语义是 contract；实现可在预算内拆模块，但不得增加另一种 binding）：

```rust
#[derive(Clone)]
pub struct RuntimeBinding {
    router: Arc<UnifiedRouter>,
}

impl RuntimeBinding {
    pub fn new(router: Arc<UnifiedRouter>) -> Self;
    pub fn bind(&self) -> RuntimeHandle;
}

#[derive(Clone)]
pub struct RuntimeHandle {
    binding: RuntimeBinding,
    /// Pinned at bind time; never re-loaded from the router's `ArcSwap`.
    snapshot: Arc<RoutingSnapshot>,
}

impl RuntimeHandle {
    pub fn generation(&self) -> u64;
    /// Read-only view of this generation. Intentionally not `&Arc<UnifiedRouter>`.
    pub fn snapshot(&self) -> &RoutingSnapshot;
}

// Public because it appears in `RuntimeHandle::snapshot`; fields stay private
// outside the router module, so callers cannot mutate or republish it.
pub struct RoutingSnapshot { /* generation + immutable routing metadata */ }

pub struct DefaultRuntimeBinding { /* reuses AtomicValue<RuntimeBinding> */ }
impl DefaultRuntimeBinding {
    pub fn new(initial: RuntimeBinding) -> Self;
    pub fn load(&self) -> RuntimeHandle;
    pub fn replace(&self, next: RuntimeBinding) -> RuntimeBinding;
}

pub fn install_default_runtime(runtime: RuntimeBinding)
    -> Result<RuntimeHandle, ProviderError>;
pub fn default_runtime() -> Result<RuntimeHandle, ProviderError>;
pub fn replace_default_runtime(runtime: RuntimeBinding)
    -> Result<RuntimeBinding, ProviderError>;
```

**Generation identity 必须由 pinned snapshot 承担，而不是由"只传 `Arc` 即只读"的假设承担。** `UnifiedRouter`
现有的 `add_deployment`（`unified.rs:287`）、`remove_deployment`（`:292`）、`set_model_list`（`:309`）都是
`pub fn(&self, ..)`，经 `update_routing_snapshot`（`:279`）对 `ArcSwap<RoutingSnapshot>`（`:208`）做
copy-on-write；`&`/`Arc` 借用检查挡不住它们。同时 `selection.rs:213` 与 `:439` 每次都 `routing_snapshot.load()`
读取**当前**值，因此只持有 `Arc<UnifiedRouter>` 的 handle 会在请求中途观察到 deployment 变化，直接违反 B-009。

D1 的解法是让 `RuntimeHandle` 在 bind 时 pin 住 `Arc<RoutingSnapshot>`，selection/execution 全程只读该 pinned
snapshot，不再在请求路径上 `load()` router 的 ArcSwap。由此：

- 已绑定请求对任何 in-place mutation 免疫，B-009 由结构保证，而非靠调用方自律；
- `add_deployment`/`remove_deployment`/`set_model_list` 可**保持现有公开签名不变**，无需降级可见性——
  这一点是刻意的：这三个方法不在 `HD-003`/B-011 批准的 deprecation/removal 清单内，把它们降为
  `pub(crate)` 会引入批准窗口之外的 breaking change（`UnifiedRouter` 由 `src/lib.rs:151` 公开导出），
  违反 product.md 的"不扩大已批准 HD 决策"。

`RoutingSnapshot` 自身携带 generation；所有成功发布 snapshot 的路径必须收敛到同一个持有
`routing_snapshot_write_lock` 的 publication helper，并在 store 前从进程级 monotonic counter 分配新 generation。
这包括 `replace_default_runtime`、legacy `add_deployment`/`remove_deployment`/`set_model_list`，也包括当前绕过
`update_routing_snapshot`、自行 lock/clone/store 的 `add_model_alias`（`unified.rs:321-329`）。alias 校验失败不得
发布 snapshot 或消耗 generation。否则 alias resolution 可以在 generation 不变时改变，或两个 handle 共享
generation 编号却持有不同 snapshot，conformance fixture 比较的 generation 就不再是有效 binding identity。

`RuntimeHandle` 不得暴露 `&Arc<UnifiedRouter>`；需要跨 handle 共享同一 pinned generation 的调用方传递
`RuntimeHandle` 本身。`RuntimeBinding` 是唯一可安装的不透明 owner token：它可以由 `Arc<UnifiedRouter>` 构造并
在内部访问 router，但不公开 router accessor。`DefaultRuntimeBinding` 原子存储 token，**不得存储已 pin 的
`RuntimeHandle`**；`load()` 每次从当前 token 的 router 读取一次最新 snapshot 并绑定新 handle。这样 legacy
mutation 后，旧 handle 仍看到旧 snapshot，而后续 `default_runtime()` 会看到新 snapshot/new generation，
无需 mutation 回调去重写 process default。

`install_default_runtime` 只允许首次安装；重复安装返回 `ProviderError::Configuration`，调用方需要替换时必须
显式使用 `replace_default_runtime`。每次 install/replace 在安装 token 时为其当前 snapshot 发布新的、单调递增且
进程内不复用的 generation；`replace` 返回旧 `RuntimeBinding` token，rollback 通过把该 token 直接传回
`replace_default_runtime` 完成，禁止从 handle 取回 router 或用 sentinel 重建。free function 每次调用开始只
load 一次；HTTP startup 从 `Arc<UnifiedRouter>` 构造 `RuntimeBinding` 并由 `AppState` 长期持有 binding，
**每个请求入口**只调用一次 `bind()` 得到 request-scoped handle；`LLMClient` 等长期 facade 同样只创建或接收
binding，并在每个公开 operation 入口 bind 一次。任何长期对象都不得缓存已 pin 的 `RuntimeHandle`。禁止 free
function 的 env lazy bootstrap；无 default 时返回 `ProviderError::Configuration`。

runtime contract 必须同时拥有：

- 通过 #725 canonical factory 构造的 `Provider` instance；
- model/capability/alias selection、health/cooldown、lease、rate/budget state；
- retry/fallback 与 selected-deployment execution；
- immutable generation replacement；
- 由 `HD-004` 选择的 typed error。

adapter 只提交 canonical request + surface context 并接收 selected identity/result。不得新增 `Any`/
downcast、第二 provider enum、字符串 provider dispatch 或 adapter-owned sender。

### 2. Request context and completion migration

D1 新增唯一 request-scoped input：

```rust
pub struct RuntimeRequestContext {
    headers: HeaderMap,
    timeout: Option<Duration>,
    legacy_selector: Option<LegacyRuntimeSelector>, // 0.6.0 only
}

pub struct LegacyRuntimeSelector {
    api_key: Option<String>,
    api_base: Option<Url>,
    api_version: Option<String>,
    organization: Option<String>,
}
```

构造只能经 `RuntimeRequestContext::validate(options, policy) -> Result<Self, ProviderError>`。`headers` 的 name/
value 必须可解析且通过 runtime header allow/deny policy；`authorization`、`proxy-authorization`、`cookie`、
`set-cookie`、`host`、`content-length` 与 hop-by-hop headers 永远禁止覆盖。`timeout` 必须大于零且不超过
runtime policy 上限；缺失时使用 selected deployment timeout。验证失败为 `InvalidRequest`，不得丢弃字段后
继续。context 是 immutable、只供 selected provider 的既有 secure sender 使用，不参与 config publication。

`CompletionOptions`（`src/core/completion/types.rs`）的 request override 字段必须逐个分类，不得遗漏；
`HD-002` 只点名了 `headers`/`timeout`/`api_key`/`api_base`，但同一 struct 还公开 `api_version`（`types.rs:71`）
与 `organization`（`types.rs:73`），且 `default_router/router_impl.rs:215-225` 当前确实把它们记为
`organization_override` / `api_version_override` 并参与 dynamic provider config。D3 若不分类，这两个字段
将没有被批准的行为：保留即继续 request-scoped provider config mutation（违反 B-003），丢弃即静默降级
（违反 B-004/U-29）。分类如下：

| `CompletionOptions` 字段 | 0.6.0 处置 |
| --- | --- |
| `headers`, `timeout` | 保留为 validated request context（`HD-002`）。 |
| `api_key`, `api_base` | 0.6.0 `#[deprecated]` legacy selector，0.7.0 删除（`HD-002`）。 |
| `api_version`, `organization` | 与 `api_key`/`api_base` **同类处理**：0.6.0 `#[deprecated]`，只作为 legacy selector 的 match 维度参与 deployment 唯一匹配，绝不构造或改写 request-scoped provider config；0.7.0 与 selector 一并删除。 |
| `extra_params`, `metadata` 及其余 model 参数 | 不是 provider selection/config override，按现状继续随 canonical request 传给 selected provider。 |

`api_version`/`organization` 归入 legacy selector 是本 amendment 对 `HD-002` 的自然延伸，不改变 0.6→0.7 窗口；若维护者希望长期保留为 validated context，必须先显式修订 `HD-002`。0.6.0 中 `api_key`/`api_base`/`api_version`/`organization` 均带 `#[deprecated(since = "0.6.0", note = "configure a canonical runtime deployment")]`，0.7.0 与 selector 一并删除。

legacy selector 只在当前 handle 的 immutable deployment snapshot 中做 policy match：所有已提供字段必须匹配同一 deployment 的 canonical config，且结果必须恰好一个；零/多匹配分别返回 typed not-found/invalid-configuration。`LegacyRuntimeSelector` 不实现 `Display`，手写 `Debug` 始终把 `api_key`/`api_base` present value 输出为 `[REDACTED]`，不得保留 URL userinfo、path、query 或 fragment。raw secret、signed query 与 private endpoint 只存在于 request-local memory，不进入 identity、trace、error 或 log；resolver 不得调用 factory、`add_deployment`、client constructor 或 env。

credential match **不得直接使用现有 `utils::auth::crypto::hmac::constant_time_eq`**（`hmac.rs:26`），因为该函数对不等长输入提前 `return false`（`:27-29`），会泄漏候选 credential 长度。D3C 必须在 canonical deployment config 归一化/发布时，将 stored credential **预计算**为不透明、不可序列化且 `Debug` 恒为 `[REDACTED]` 的 `[u8; 32]` SHA-256 digest，并随 immutable routing snapshot 的 legacy-selector metadata 保存；raw stored credential 不进入 metadata。request selector 的 raw key 每请求只计算一次 digest，候选循环只比较两个定长 32 字节值；禁止在循环中重新 hash stored credential。`sha2` 已在 `Cargo.toml:97`，不引入依赖，也不改变 `verify_hmac_signature` 等固定长度 HMAC 调用方。

先让现有 free functions 和 `HD-003` 保留的 trait/type 委托给批准的 runtime binding，再删除 `DefaultRouter` 的 env bootstrap、static prefix selection、dynamic provider construction 和直接 provider execution。按 `HD-002`，`headers`/`timeout` 只能经 `RuntimeRequestContext::validate` 进入 selected provider；`api_key`/`api_base` 仅存在于 0.6.0 legacy selector 窗口。迁移期间 facade 不得在 runtime 失败后回到旧 registry。

2026-07-21 的 D2 exact-head independent review 发现两条不可延期的违规路径：`RuntimeHandle::execute_with_selected_deployment` 虽 pin 住 snapshot，却在 fallback 终止时把 terminal `ProviderError` 转成 `RouterError`，completion adapter 再转回 `ProviderError`，导致 authentication、timeout、API status、unsupported、cancel、parsing、content-filter 等被压扁为 `ProviderUnavailable`（违反 B-006）；adapter 在 runtime alias 解析前调用 `unsupported_explicit_completion_selector`，会按文本前缀拒绝形如 `google/...` 的合法 alias（违反 B-002/B-005）。

D2 因此允许在 `src/core/router/execute_impl.rs` 做最小 typed-boundary refactor，exact contract 如下：

- 新增 crate-private `RuntimeHandle::execute_with_selected_deployment_typed`：只消费 handle 已 pin 的 `RoutingSnapshot`，返回 `Result<ExecutionResult<T>, ProviderError>`；不得重新 load snapshot 或暴露 binding/router accessor。
- 抽取唯一的 in-snapshot typed implementation，复用 selection、retry/fallback、lease settlement 与 attempt accounting。现有 `RouterError` compatibility API 仅在最外层委托后做一次 `provider_error_to_router_error`；禁止复制 routing loop、增加平行 error enum/classifier 或 adapter side-channel。
- completion unary 只调用 typed handle API，将 terminal `ProviderError` 直接映射为 `GatewayError`；禁止 `ProviderError -> RouterError -> ProviderError` 往返。
- completion adapter 删除 unary textual provider-prefix gate；alias 必须先由 pinned snapshot 解析，surface unsupported 由 selected canonical provider 返回 typed `NotSupported`/`NotImplemented`，不得伪装为 `InvalidRequest`。
- focused fixtures 覆盖 prefix-looking runtime alias 成功、至少一个非 rate-limit/model-not-found terminal provider error 保持原 variant/canonical code 且 redaction 生效，并用 source guard 拒绝 unary helper 中的 `unsupported_explicit_completion_selector`、`router_error_to_provider_error` 和 legacy fallback。

该 amendment 仅修复已批准的 `HD-003/004` 执行边界：不新增公开 API、不改变 retry/fallback policy、不把 D3 validated headers/timeout 或 legacy selector 提前到 D2。D2 仍限最多 8 个实际非文档文件/500 changed lines；候选 allowlist 增加一个 router 文件，但实现 PR 只能使用完成 typed boundary 所需的子集。

### 3. SDK migration and retained facade API

`LLMClient::new(config: ClientConfig)` 保持；它把 config 归一化并构造一个 canonical router，随后只持有
`RuntimeBinding` 与 immutable compatibility config view。新增
`LLMClient::from_runtime(runtime: RuntimeBinding) -> Self` 供显式共享 HTTP/SDK runtime owner；
`LLMClient::runtime(&self) -> RuntimeBinding` 返回同一 refreshable binding token。签名收 `RuntimeBinding` 而非
`Arc<UnifiedRouter>` 是 contract 的一部分：后者绕过唯一 binding owner，也会诱使实现为 SDK generation 填
sentinel。原有 chat/stream/embedding 方法签名保持；每个公开 operation 在入口只调用一次 `bind()`，随后把得到的
`RuntimeHandle` 传完整个 selection/execution/stream 生命周期，不读取 process default，也不在 operation 中二次
load。

HTTP `AppState` 与 SDK 共享 runtime 时传递同一个 `RuntimeBinding`，因此新请求都能观察后续 snapshot publication；
已开始的请求仍由各自的 handle 保持 pinned。只有需要证明跨 surface **同一 generation** 的 conformance fixture
可以先 bind 一个 handle，再调用显式命名的 request-scoped internal adapter（`*_with_runtime_handle`）；该入口不得
成为长期 client constructor、不得把 handle 写回 `LLMClient`，operation 结束即释放。这样 fixed-generation 是刻意且
局部的测试/adapter 路径，不会让普通 SDK client 永久停留在构造时 generation。`ClientConfig` 保持配置 DTO，不持有
provider/client/state。

`ClientConfig` 先归一成 canonical provider/deployment config 并构造 runtime。SDK routing 不再把本地
`ProviderStats` 转成临时 `RoutingContext`，SDK execution 也不再按 `SdkProviderConfig` 创建请求 client。
`LLMClient` 保留的 DTO convenience、stats view 与错误外观都从 canonical execution result/state 派生，
不能反馈为第二套 selection truth。

### 4. Registry demotion

0.6.0 对 `DefaultRouter`、completion `Router` trait 与 `ProviderRegistry` mutation/ownership surface 添加
`#[deprecated(since = "0.6.0", ...)]`。`DefaultRouter` 只包装 refreshable binding source，不保存
`RuntimeHandle`：显式 runtime 模式持有 `RuntimeBinding` 并在每次 trait call 入口 bind 一次；process-default 模式
在每次 trait call 入口调用一次 `default_runtime()`，从而观察 `replace_default_runtime`，随后只委托该 handle。
embedding router 必须迁移到 canonical runtime。本 issue 的 D6 只完成 demotion/deprecation，不提前删除
public surface；0.7.0 删除由 D8 durable follow-up 执行。
`LLMClient`、`ClientConfig`、`completion`/`acompletion`/`completion_stream` 永久保留。

**`ProviderRegistry` 在 0.6.0 保持现有签名与现有行为，不改造成 stateless facade。** 现有公开 mutation 是
`register(&mut self, provider: Provider)`（`provider_registry.rs:22`）、
`register_with_key(&mut self, ..)`（`:32`）、`remove(&mut self, name: &str) -> Option<Provider>`（`:52`）、
`clear(&mut self)`（`:72`）。这些签名没有错误通道，因此"mutation 返回 configuration error"在 0.6.0 无法实现：
`-> ()` 只剩静默 no-op（用户 mutation 被丢弃且无任何信号，违反 U-29 no-silent-degradation）或 panic 两条路，
而改签名本身就是 `HD-003` 明确推迟到 0.7.0 的 breaking change。product.md 的 release policy 另有硬约束——
当前 `.github/workflows/version-bump.yml:54` 把 0.x breaking commit 计算为 **1.0.0**，在 workflow 修订并有
fixture 证据前任何 breaking tranche 都不得合并，所以"0.6.0 直接 break registry"在现有自动化下也发不出去。

D6 因此按如下方式收敛，且不触碰 `HD-003` 的 0.6→0.7 窗口：

- `ProviderRegistry` 在 0.6.0 **继续是一个可独立使用、行为与 0.5.x 一致的数据结构**，整体标注
  `#[deprecated(since = "0.6.0", note = "construct a canonical runtime deployment instead")]`；
  其 `&mut self` mutation 照常改自身 map，语义诚实，无需错误通道。
- 关键变化是**所有权而非签名**：canonical runtime 与全部 production 调用方（completion、embedding router）
  不再读写 `ProviderRegistry`，它因此不再是任何执行路径上的 provider store，也不再是第二真值源。
  B-003/B-011 由"runtime 不消费它"满足，而不是由"它自己变空壳"满足。
- source guard 断言 production 代码（`src/` 去除 registry 自身定义、`src/core/providers/mod.rs` 与
  `src/lib.rs` 的兼容 re-export、以及 tests）零命中 `ProviderRegistry`，这是 D6 的可验证完成信号；re-export
  allowlist 只维持 0.6.0 public source compatibility，不得构造、读取或 mutate registry。
- 0.7.0 由 D8 follow-up 整体删除该类型，届时的 breaking change 与 workflow 修订一并执行。

若维护者后续决定改为在 0.6.0 就引入 `Result` 签名，必须先修订 `HD-003` 与 release workflow 并接受版本会被
计算为 1.0.0；实现者不得自行选择该路径。

### 5. Canonical error API and exhaustive mapping

D1E-a1/D1E-a2a/D1E-a2b/D1E-b 以 `ProviderError` 为 source，并复用现有 `src/utils/error/canonical.rs` 的 `ErrorCode` / `CanonicalError`
以及 `src/core/providers/unified_provider_http_mapping.rs` 的 `ProviderHttpErrorFacts` /
`provider_http_error_facts`；不得新增平行的 provider error class 或 HTTP facts 类型。`ErrorCode` 是公开且未标
`non_exhaustive` 的 enum，0.6.0 **不得新增 `Cancelled` 等 variant**；cancellation 由
`ProviderFailureFacts::cancelled` 等非 taxonomy typed facts 保留。class/retry 和 HTTP facts 两个现有 exhaustive
mapping 必须由同一 table-driven fixture 锁定；HTTP、SDK、Gateway 和 retry policy 只能消费这些现有 API，
不得各写一份 `ProviderError` match 或解析 `Display`：

```rust
impl ProviderError {
    pub fn canonical_code(&self) -> ErrorCode; // delegates CanonicalError
    pub fn http_facts(&self) -> ProviderHttpErrorFacts; // delegates existing mapping
    pub fn redacted(&self) -> ProviderError;
}
```

**不得新增 context-free 的 `canonical_retryable(&self) -> bool`。** 下表中 `ContentFiltered`、
`DeploymentError`、`Streaming` 的 retryability 依赖"runtime 是否已发出可见输出"，无上下文的 bool 无法表达：
它要么让 adapter 在已输出后重试而重复用户可见内容（违反 B-010），要么一律不重试而压掉 pre-output 的
合法 fallback（违反 B-008）。

retry 决策唯一入口是**已存在**的 `src/core/router/retry_policy.rs::RetryPolicy::decide(&RouterConfig,
&ProviderError, RetryContext) -> RetryDecision`。该文件已提供 `RetryContext { operation, stream_stage,
idempotency, attempt, max_attempts, retry_budget_remaining, deadline_remaining }` 与
`StreamRetryStage::{NotStreaming, BeforeFirstChunk, AfterChunksEmitted}`，且已在
`StreamAlreadyEmitted` 处停止重试——本表的 pre/post-output 语义正是它，不要另建平行 API。

D1E-a1 的收敛工作因此不是"加 retryable 方法"，而是删除既有的平行 taxonomy：
`retry_policy.rs:7` 从 `src/core/providers/failure.rs` 导入 `ProviderFailureFacts`/`ProviderFailureKind`，
而 `ProviderFailureKind`（`failure.rs:11`）是 `ProviderError` 变体的 1:1 镜像，与 `canonical.rs:13` 的
class 级 `ErrorCode` 构成第二套分类。`ProviderFailureKind` 必须删除；但 retry 需要的 typed facts 不能只从
`ErrorCode` 反推，因为 class 会合并 `ContentFiltered`、`DeploymentError`、`Streaming` 等具有不同 pre/post-output
语义的 variant。`ProviderFailureFacts::from_error` 是全仓**唯一**允许对 `ProviderError` 做 exhaustive retry-fact
match 的位置，输出 `canonical_code: ErrorCode` 加非 taxonomy 的原始事实（至少包含 API status/`retry_after`、
content-filter potentially-retryable、pre-output-only、cancellation）；`RetryPolicy::decide` 只消费这些 facts 与
`RetryContext`。不得再建 mirrored enum、第二个 variant classifier，或从 `ErrorCode` 丢失上述事实。

D1E-c 处理已经公开的 provider-specific context-free compatibility helpers：0.6.0 保持其当前返回行为并统一标注
deprecated，但 production provider routing/retry source guard 必须零消费；不得以它们代替
`RetryPolicy::decide`。该集合包括 `ProviderError::is_retryable`、
`ContextualProviderError::is_retryable`、`ProviderErrorTrait::is_retryable`、`SDKError::is_retryable`、
`core::router::execution::is_retryable_error` 与 `ErrorUtils::should_retry`，并全部进入 0.7.0 removal handoff。
`ErrorCode::is_retryable` 与 `CanonicalError::canonical_retryable` 则明确 grandfather：它们继续作为 A2A/MCP/
HTTP presentation 的 coarse compatibility fact，0.6/0.7 不删除，但不得参与 provider runtime retry/fallback。
二者若未来要删除，必须另立 public-API decision，不得借 GH-965 顺带移除。

`SDKError` 同样是公开且未标 `non_exhaustive` 的 enum，0.6.0 **不得新增 `Provider(ProviderError)` variant**。
现有 `ProviderError(String)` 在 0.6.0 deprecated 并保持源码/运行行为；0.7.0 在 release-workflow prerequisite 完成后
再引入 typed replacement 并删除 string variant。0.6 SDK conversion 先对 `ProviderError` 调 `redacted()`，再只按
`canonical_code()` 映射到现有 variant：Authentication/Authorization → `AuthError`，RateLimited/QuotaExceeded →
`RateLimitError`，InvalidRequest/Conflict → `InvalidRequest`，NotFound → `ModelNotFound`，Timeout/Network →
`NetworkError`，Unavailable → deprecated `ProviderError`，Configuration → `ConfigError`，Parsing → `ParseError`，
NotImplemented → `NotSupported`，Internal → `Internal`。variant selection 不得解析 redacted string。
D1E-a1 结束时，`src/sdk/errors.rs` 中现有、未修改的 exhaustive SDK category match 仅作为严格串行过渡：
a1 source guard 只禁止 provider/retry production scope 中新增或第二个 retry-fact classifier，不得扩大该
SDK match；D1E-a2a 必须用 `canonical_code()` mapping 删除该 exhaustive/string classifier。D1E-a2a 的
writable scope **恰为** `src/core/providers/unified_provider_methods.rs` 与 `src/sdk/errors.rs`，只实现
保留原 variant/category 的 `redacted()` copy、SDK canonical mapping 及其 negative/category fixtures，
≤500 changed lines。为让该 tranche 在不引入 lint 例外时独立通过 strict Clippy，D1E-a2a **不得**给 legacy
`SDKError::ProviderError(String)` 增加真实 `#[deprecated]` 属性，也不得增加任何 `allow/expect(deprecated)`；
0.6 deprecation metadata 延后到紧随其后的 D1E-a2b。D1E-a2a 的 classifier guard 是以下**确定性验证命令**，
不是待加入 `src/` 的测试或 production diff：

```bash
python3 - <<'PY'
import hashlib
import json
import re
from pathlib import Path

MARKER = "SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError"
ATTRIBUTE = "#[allow(deprecated)]"
OUTSIDE_SHA256 = {
    "T023a": "721c4930700167ebf9e9172f31d5f38f4e65d5dac6811c1cee973c3810ec380b",
    "T023b": "cc062b0bdc847ee3033fb75b50e7f315e8d61b691ef8e82d0b4f2456a031a053",
}
PUNCT = set("{}()[].,:;|=<>?!&+-*/%^#@~$'")
MULTI = ("::", "=>", "->", "..=", "...", "..", "&&", "||", "==", "!=", "<=", ">=",
         "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<", ">>")
def rust_lex(text, keep_comments=False):
    tokens = []
    offset = 0
    while offset < len(text):
        if text[offset].isspace():
            offset += 1; continue
        start = offset
        if text.startswith("//", offset):
            offset = text.find("\n", offset)
            if offset < 0:
                offset = len(text)
            if keep_comments:
                tokens.append(("COMMENT:" + text[start:offset], start, offset))
            continue
        if text.startswith("/*", offset):
            depth = 1; offset += 2
            while offset < len(text) and depth:
                if text.startswith("/*", offset):
                    depth += 1; offset += 2
                elif text.startswith("*/", offset):
                    depth -= 1; offset += 2
                else:
                    offset += 1
            assert depth == 0, "unterminated block comment"
            if keep_comments:
                tokens.append(("COMMENT:" + text[start:offset], start, offset))
            continue
        raw = re.match(r'(?:br|cr|r)(?P<h>#{0,255})"', text[offset:])
        if raw:
            closing = '"' + raw.group("h")
            offset += raw.end()
            end = text.find(closing, offset)
            assert end >= 0, "unterminated raw string"
            offset = end + len(closing)
            tokens.append(((text[start:offset] if keep_comments else "LITERAL"), start, offset)); continue
        prefix = 1 if text.startswith(('b"', 'c"'), offset) else 0
        if text[offset + prefix:offset + prefix + 1] == '"':
            offset += prefix + 1
            while offset < len(text):
                if text[offset] == "\\":
                    offset += 2
                elif text[offset] == '"':
                    offset += 1
                    break
                else:
                    offset += 1
            assert offset <= len(text) and text[offset - 1] == '"', "unterminated string"
            tokens.append(((text[start:offset] if keep_comments else "LITERAL"), start, offset)); continue
        if text[offset] == "'" and offset + 2 < len(text) and (
            text[offset + 1] == "\\" or text[offset + 2] == "'"
        ):
            offset += 1
            offset += 2 if text[offset] == "\\" else 1
            assert offset < len(text) and text[offset] == "'", "unterminated char"
            offset += 1
            tokens.append(((text[start:offset] if keep_comments else "LITERAL"), start, offset)); continue
        match = re.match(r"[A-Za-z_][A-Za-z0-9_]*", text[offset:])
        if not match:
            match = re.match(r"[0-9][A-Za-z0-9_.]*", text[offset:])
        if match:
            offset += match.end(); tokens.append((text[start:offset], start, offset)); continue
        operator = next((item for item in MULTI if text.startswith(item, offset)), None)
        if operator:
            offset += len(operator); tokens.append((operator, start, offset)); continue
        assert text[offset] in PUNCT, f"unrecognized Rust syntax at {offset}: {text[offset:offset + 20]!r}"
        offset += 1; tokens.append((text[start:offset], start, offset))
    return tokens
def values(text, keep_comments=False):
    return [value for value, _, _ in rust_lex(text, keep_comments)]
def subsequences(haystack, needle):
    return [index for index in range(len(haystack) - len(needle) + 1)
            if haystack[index:index + len(needle)] == needle]
def matching_brace(tokens, open_index):
    assert tokens[open_index][0] == "{"
    depth = 0
    for index in range(open_index, len(tokens)):
        value = tokens[index][0]
        if value == "{": depth += 1
        elif value == "}":
            depth -= 1
            if depth == 0: return index
    raise AssertionError("unclosed Rust block")
def conversion_span(production):
    tokens = rust_lex(production)
    token_values = [value for value, _, _ in tokens]
    header = values("impl From<crate::core::providers::ProviderError> for SDKError {")
    starts = subsequences(token_values, header)
    assert len(starts) == 1, "expected exactly one ProviderError-to-SDKError conversion"
    start = starts[0]; close = matching_brace(tokens, start + len(header) - 1)
    return tokens[start][1], tokens[close][2]
def normalize_conversion(block):
    marker_line = "// " + MARKER
    counts = block.count(marker_line), block.count(ATTRIBUTE)
    if counts == (0, 0):
        return block, "T023a"
    assert counts == (1, 1), "T023b decoration count mismatch"
    decoration = re.compile(
        r"(?m)^(?P<indent>[ \t]*)" + re.escape(marker_line) + r"\r?\n"
        r"(?P=indent)" + re.escape(ATTRIBUTE) + r"\r?\n"
        r"(?P=indent)ErrorCode::Unavailable => SDKError::ProviderError\(message\),$"
    )
    matches = list(decoration.finditer(block))
    assert len(matches) == 1, "T023b decoration is not on the exact Unavailable arm"
    match = matches[0]
    plain_arm = match.group("indent") + "ErrorCode::Unavailable => SDKError::ProviderError(message),"
    return block[:match.start()] + plain_arm + block[match.end():], "T023b"
EXPECTED = """
impl From<crate::core::providers::ProviderError> for SDKError {
    fn from(error: crate::core::providers::ProviderError) -> Self {
        let redacted = error.redacted();
        let code = redacted.canonical_code();
        let message = redacted.to_string();
        match code {
            ErrorCode::Authentication | ErrorCode::Authorization => SDKError::AuthError(message),
            ErrorCode::RateLimited | ErrorCode::QuotaExceeded => SDKError::RateLimitError(message),
            ErrorCode::InvalidRequest | ErrorCode::Conflict => SDKError::InvalidRequest(message),
            ErrorCode::NotFound => SDKError::ModelNotFound(message),
            ErrorCode::Timeout | ErrorCode::Network => SDKError::NetworkError(message),
            ErrorCode::Unavailable => SDKError::ProviderError(message),
            ErrorCode::Configuration => SDKError::ConfigError(message),
            ErrorCode::Parsing => SDKError::ParseError(message),
            ErrorCode::NotImplemented => SDKError::NotSupported(message),
            ErrorCode::Internal => SDKError::Internal(message),
        }
    }
}
"""
def production_before_tests(source):
    tokens = rust_lex(source)
    token_values = [value for value, _, _ in tokens]
    boundary = values("#[cfg(test)] mod tests {")
    depth = 0
    starts = []
    for index, value in enumerate(token_values):
        if depth == 0 and token_values[index:index + len(boundary)] == boundary:
            starts.append(index)
        if value == "{": depth += 1
        elif value == "}":
            depth -= 1
            assert depth >= 0, "unbalanced Rust braces"
    assert depth == 0, "unbalanced Rust braces"
    assert len(starts) == 1, "expected one top-level #[cfg(test)] mod tests item"
    start = starts[0]
    close = matching_brace(tokens, start + len(boundary) - 1)
    assert not source[tokens[close][2]:].strip(), (
        "test module must be the final item; trailing comment/item forbidden"
    )
    return source[:tokens[start][1]]
def verify_source(source):
    production = production_before_tests(source)
    start, end = conversion_span(production)
    normalized, phase = normalize_conversion(production[start:end])
    assert values(normalized, True) == values(EXPECTED, True), "conversion token shape changed"
    outside = production[:start] + production[end:]
    payload = json.dumps(values(outside, True), ensure_ascii=True, separators=(",", ":")).encode()
    fingerprint = hashlib.sha256(payload).hexdigest()
    assert fingerprint == OUTSIDE_SHA256[phase], (
        f"outside-production fingerprint changed for {phase}: {fingerprint}"
    )
    return phase
phase = verify_source(Path("src/sdk/errors.rs").read_text(encoding="utf-8"))
print(f"ProviderError classifier guard passed: phase={phase}")
PY
```

该命令不是 whitespace/text-count heuristic。它先 fail closed 词法化并定位完整 conversion：T023a 只接受无
decoration 的批准 token shape；T023b 只接受在 `ErrorCode::Unavailable` arm 紧邻、依次出现且各恰为一次的固定
marker 与 `#[allow(deprecated)]`，精确剥离这两行后再要求**整个 token 序列逐项相等**。任何额外、错位或错误
attribute/comment，以及换行形式的 `match\n redacted.to_string()`、第二个 `match`/`if`、字符串 helper、
`ProviderError::Variant` arm、额外 assignment/call 或 variant classifier 都失败；纯格式换行/缩进仍可通过。

同一命令还由该 fail-closed lexer 定位唯一顶层 `#[cfg(test)] mod tests` item 的 opening brace 与 matching
close，要求 closing brace 后只有 whitespace（尾随 comment/item 均失败），并只把该 item 前缀视为 production；
conversion 外的完整 comment/literal-preserving token stream 经无歧义 JSON 编码后必须命中 phase-specific 固定 SHA-256。
T023a digest 由 immutable `origin/main@2ff9bb2066adfb04d67b2e692ae9fbd9968fa9b5` 的
`src/sdk/errors.rs` 去掉 conversion/tests 后、只加入编译所需精确
`use crate::utils::error::ErrorCode;` 生成，禁止 outside decoration。T023b synthetic baseline 只再加入
legacy variant 的 exact `#[deprecated(since = "0.6.0", note = "use the existing typed SDK categories returned by ProviderError conversion")]`
以及 Gateway Unavailable arm、`SDKError::is_retryable` 紧邻的固定 marker +局部 allow；tests/completions 的
其余 6 个 marker/allow（5 tests + 1 completions）继续由后文 9-site all-target guard 精确锁定。固定值只在 spec authoring 时以上述
immutable baseline 和同一 lexer/JSON 算法生成；运行命令不调用 `git show`，不依赖 mutable/shallow Git 状态。
因此任何 cfg/inline/custom attribute、body/helper/const/static/macro、comment/literal 或其他 production 变化
都会改变 digest 并失败。该命令不增加 production/test changed line；若 D1E-a2a 超过 500 changed lines，必须
在不删除/压缩安全 fixture 的前提下减少实现 diff，不得把验证命令伪装成 source 文件或放宽预算。

2026-07-18 在原 D1E-a2 实现 worktree 上重新验证发现：给 legacy
`SDKError::ProviderError(String)` 加真实 `#[deprecated(since = "0.6.0", ...)]` 后，strict Clippy 会把同一
0.6 compatibility surface 的既有 construction/match 升级为 `-D deprecated` 错误。除
`src/sdk/errors.rs` 内部的兼容映射、match 与测试外，`src/sdk/client/completions.rs` 的
`LLMClient::execute_chat_request` 还在 unsupported provider-type fallback 中构造该 variant；它不在原两文件
scope 内，因此把 redaction/mapping、true deprecation、全仓 guard 与兼容 allow 挤进同一个 ≤500-line tranche
不可满足。

D1E-a2b 因此成为独立、严格串行的 deprecation-only tranche：只可从已合并 D1E-a2a 的
`origin/main` 开始，非文档 writable scope **恰为** `src/sdk/errors.rs`、
`src/sdk/client/completions.rs` 与新文件 `src/sdk/provider_error_deprecation_guard_tests.rs`，
保持 ≤350 changed lines。它只给 legacy variant 增加真实
`#[deprecated(since = "0.6.0", ...)]`、下列局部兼容 lint 标记和 source guard；`completions.rs` 只允许在
现有 fallback arm 上增加局部 lint 属性与 `SP965-T010` 所链接 0.7 follow-up 的 removal marker。不得改 control flow、错误文本、
provider selection、canonical mapping、redaction 或 sender 行为。0.6 legacy variant 的公开签名、
`Display`、retryability 与所有既有构造/匹配行为保持不变。

仅下列 legacy compatibility 站点可使用**紧邻目标表达式或单个函数/测试函数**的
`#[allow(deprecated)]`，且每个属性必须带固定注释
`SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError`：

1. `src/sdk/errors.rs` 的 `From<GatewayError>` 中 `GatewayError::Unavailable` construction arm；
2. `src/sdk/errors.rs` 的 `From<ProviderError>` 中 `ErrorCode::Unavailable` construction arm；
3. `src/sdk/errors.rs` 的 `SDKError::is_retryable` legacy match；
4. `src/sdk/client/completions.rs` 的 `LLMClient::execute_chat_request` 既有 unsupported provider-type
   fallback arm；
5. `src/sdk/errors.rs` 测试中的单个 `sdk_variant` category helper，以及
   `test_sdk_error_provider_error`、`test_is_retryable_provider_error`、
   `test_from_gateway_error_provider_unavailable`、`test_sdk_error_empty_message` 四个既有兼容测试。

上述清单恰为 9 个局部 allow 站点（`src/sdk/errors.rs` 8 个、
`src/sdk/client/completions.rs` 1 个），不得按文件或 module 合并计数。
禁止 module/crate-wide `allow(deprecated)`、`expect(deprecated)`、command-line lint 降级、未列名的新
`SDKError::ProviderError` callsite，或把多个非兼容路径包进同一个 allow。D1E-a2b 必须在
`src/sdk/errors.rs` 的 test scope 增加
`legacy_provider_error_deprecation_allowlist_does_not_grow` source guard。该测试在现有文件末尾的
`#[cfg(test)] mod tests` 内 `include!` 一个 Rust test module；实现放在
`src/sdk/provider_error_deprecation_guard_tests.rs`（<300 行），使用现有 dev-dependency `syn` 的
`Visit` 与 `syn::ext::IdentExt::unraw` 做 owner/role/path 分类。禁止嵌入 Python、启动 `python3` 或
依赖非 Rust 运行时；普通 `cargo test` 在 `PATH` 无 `python3` 时仍必须通过。guard 必须从以下来源取并集、
canonicalize、去重、排序且读取失败时 fail closed，覆盖**每一个 Rust target/source**：

- `git ls-files '*.rs'` 与 filesystem discovery 下 `src/**/*.rs`、`tests/**/*.rs`、`examples/**/*.rs`、
  `benches/**/*.rs` 以及 workspace/package root 的 `build.rs`；
- `cargo metadata --no-deps --format-version 1` 返回的每个 package target `src_path`，以及每个 package root
  的上述 Rust 目录/`build.rs`；metadata 声明在常规目录之外的 target 也必须扫描。

inventory 必须区分当前 checkout 实际随 package 发布的文件：package/source archive 无 `.git` 时不得要求
未随包发布的 `tests/**/*` 等基线文件存在；`cargo package --list` 可用时以其清单约束 package-owned baseline，
解包后的 package fixture 必须仅靠 shipped source + Cargo metadata 运行 focused guard。Git 仅作为可选并集来源，
缺失 `.git`/`git` 不得令测试失败。

guard 对该全集中的 legacy variant qualified reference、显式 variant import/alias 与 wildcard variant import
做 exact allowlist，只接受上列 9 个 path + enclosing function/arm/test 站点；任何新增 production/test/example/
bench/build-script/Cargo-target callsite、改名绕过或数量增长都 fail closed。仓库已存在、与本 legacy variant
无关的 broad `allow/expect(deprecated)` 可仅按 path + anchor +既有计数基线保留，但新增/移动/扩大的 broad
attribute 必须失败；即使某文件命中该 unrelated baseline，只要新增 `SDKError::ProviderError` reference/import/
alias/wildcard，仍必须失败，除非它就是上述 9 个带 T010 durable-handoff marker 的局部站点。T023b 自身禁止新增 crate/module-wide
allow/expect，所有 9 个站点仍须逐一验证紧邻的 T010-linked follow-up marker 与局部 allow。

guard 还必须扫描相关 lint/config/command surfaces：workspace/package `Cargo.toml`、`.cargo/config*`、
`.github/workflows/**`、`Makefile*`、`justfile*`、`scripts/**`、`checks/**`、`xtask/**`、`clippy.toml`、
`rust-toolchain*` 及 Cargo metadata 暴露的 manifest/target config 路径，拒绝 `-A deprecated`、
`--allow deprecated` 或等价 `RUSTFLAGS`/command-line lint 降级。strict verification 必须使用原样
`cargo clippy --all-targets --all-features --locked -- -D warnings`；guard 自身不得成为 allowlisted
deprecated use。`SP965-T010` 只负责创建并链接 durable handoff；所有这些例外与 markers 由该 0.7 follow-up
在 typed replacement/removal 落地时一并删除，不形成长期 lint policy。

`GatewayError::Provider(ProviderError)` 已存在并保持；跨 Gateway 边界构造
`GatewayError::Provider(e.redacted())`，只有该既有 wrapper 持有脱敏 typed copy。原始 runtime `ProviderError` 只在
request-local retry/observability 路径流转。outer adapter 只用 canonical/http facts 生成 status/code/retry hint；
`completion()` 继续返回 `GatewayError` 外观。
表中 `R` 表示输出前必须调用 `redacted()`：credential、
authorization/cookie header、signed query value 和已知 secret pattern 替换为 `[REDACTED]`；provider/model/
deployment identity 与非秘密限额保留。原始 typed error 只在 request-local memory 中流转，不进入 log/response。

| `ProviderError` variant | Existing `ErrorCode` | HTTP | Gateway / SDK 0.6 | Retryability | Redaction / cancellation |
| --- | --- | --- | --- | --- | --- |
| `Authentication` | Authentication | 401 | redacted Gateway typed / existing SDK category | no | R |
| `RateLimit` | RateLimited | 429 + limit headers | redacted Gateway typed / existing SDK category | yes; honor `retry_after` | R |
| `QuotaExceeded` | QuotaExceeded | 402 | redacted Gateway typed / existing SDK category | no | R |
| `ModelNotFound` | NotFound | 404 | redacted Gateway typed / existing SDK category | no | R |
| `InvalidRequest` | InvalidRequest | 400 | redacted Gateway typed / existing SDK category | no | R |
| `Network` | Network | 502 | redacted Gateway typed / existing SDK category | yes | R |
| `ProviderUnavailable` | Unavailable | 503 | redacted Gateway typed / existing SDK category | yes | R |
| `NotSupported` | NotImplemented | 501 | redacted Gateway typed / existing SDK category | no | R |
| `NotImplemented` | NotImplemented | 501 | redacted Gateway typed / existing SDK category | no | R |
| `Configuration` | Configuration | 500 | redacted Gateway typed / existing SDK category | no | R |
| `Serialization` | Parsing | 500 | redacted Gateway typed / existing SDK category | no | R |
| `Timeout` | Timeout | 504 | redacted Gateway typed / existing SDK category | yes | R |
| `ContextLengthExceeded` | InvalidRequest | 400 | redacted Gateway typed / existing SDK category | no | R |
| `ContentFiltered` | InvalidRequest | 400 | redacted Gateway typed / existing SDK category | only when `potentially_retryable == Some(true)` and runtime has not emitted output | R |
| `ApiError` | status-derived | preserve only 400..=599; otherwise 502 | redacted Gateway typed / existing SDK category | yes only for 408, 429, 5xx, or modeled Bedrock 424; otherwise no | R; 401→Authentication, 403→Authorization, 404→NotFound, 408/504→Timeout, 409→Conflict, other 4xx→InvalidRequest, 429→RateLimit, 5xx→Unavailable, 1xx/2xx/3xx/nonstandard→Internal |
| `TokenLimitExceeded` | InvalidRequest | 400 | redacted Gateway typed / existing SDK category | no | R |
| `FeatureDisabled` | NotImplemented | 501 | redacted Gateway typed / existing SDK category | no | R |
| `DeploymentError` | NotFound | 404 | redacted Gateway typed / existing SDK category | yes before first output only | R |
| `ResponseParsing` | Parsing | 502 | redacted Gateway typed / existing SDK category | no | R |
| `RoutingError` | Unavailable | 503 | redacted Gateway typed / existing SDK category | no; it is already terminal aggregate failure | R |
| `TransformationError` | Parsing | 500 | redacted Gateway typed / existing SDK category | no | R |
| `Cancelled` | InvalidRequest（0.6 public taxonomy compatibility） | 499 | redacted Gateway typed / existing SDK category | never | R; separate `cancelled` fact makes lease settlement neutral |
| `Streaming` | Internal | 502 | redacted Gateway typed / existing SDK category | yes only before first output; never after output | R |
| `Other` | Internal | 502 | redacted Gateway typed / existing SDK category | no | R |

The `ApiError` row is exhaustive by status partition；stored status 只有 400..=599 可透传，其他值（包括当前
`gemini/error.rs:108` 构造的 200）一律产出 `ErrorCode::Internal` + HTTP 502，绝不能把 provider failure 发成
success/redirect。`CanonicalError for ProviderError` 与现有
`provider_http_error_facts()` 都不得有 wildcard arm；table fixture 必须逐 variant 断言 `ErrorCode`、HTTP facts、
retryability、redaction 与 cancellation settlement，新增 variant 会编译失败直到两个现有 mapping 与本表同步。
Cancellation never increments failure/cooldown and never triggers fallback; transport disconnect is converted to
`Cancelled` only when the caller cancellation token fired, otherwise it remains `Network`/`Streaming`.

### 6. HTTP binding and conformance

HTTP 继续从 server startup 获取 `Arc<UnifiedRouter>` 并在 startup 注入边界构造 `RuntimeBinding`，每个请求在
进入执行 helper 前 bind 一个 pinned `RuntimeHandle`。现有 `execution.rs::{execute_with_selected_deployment,
execute_stream_with_selected_deployment}` 与 `budgeted.rs::{run_unary,run_stream}` 当前仍接收
`&UnifiedRouter`/`Arc<UnifiedRouter>`，因此迁移必须按 D7a-D7i 严格串行：D7a 先增加 handle-aware helper 且暂留
legacy wrapper；D7b 迁移 unary callers；D7c 迁移 stream/query/Gemini callers 并删除旧 router field/wrapper；
D7d 在接收 `background=true` response 时就 bind handle 并把它传入 spawned task；D7e 建 capability-only runtime
attempt plan，D7f 加 closed provider management dispatch，D7g/D7h 再分别迁移 batch 与 fine-tuning route。route
wire 不变，任何中间 tranche 都不得重新 load 当前 snapshot 替代 pinned handle。

management operation 是 crate-private closed enum（batch create/list/get/cancel 与 fine-tuning lifecycle），ID 作为
单独字段由 provider 做 URL-segment encoding；禁止 adapter 传任意 URL/path/method 或取得 raw client。create request
若有 model，使用既有 model + capability selection；list/get/cancel 等无 model 操作由 pinned handle 建立
capability-only attempt plan：只遍历该 snapshot 中声明相应 executable capability 的 canonical deployments，复用
相同 health/cooldown/budget/routing strategy、`RetryPolicy::decide` 与 exactly-once lease settlement，稳定的内部
routing key 是 operation capability 而非空字符串/sentinel model。route 不得自行循环 provider，ID 操作的跨
deployment fallback 也只能由该 runtime plan 决定。只有每个 attempt 实际 selected `Provider` 的
OpenAI/OpenAI-compatible 实现用其已构造的 auth、endpoint policy 与 pool 发送；unsupported provider 返回 typed
`NotSupported`。这样 lifecycle route 不扫描 `config.gateway.providers`、不构造
`RouteHttpClient`/`OpenAIFineTuningProvider`，也不引入 OS/URL injection surface。

D7i 新增 `tests/integration/router_runtime_conformance.rs` 前已搜索现有 tests，无同名/同职责 fixture；该 module
使用本地 listener 和 deterministic providers，令同一 runtime generation 依次由三入口触发并比较 selected
identity、typed category、attempt trace 与 state delta。source guard 扫描 production adapter，拒绝第二
map/config scan/local routing counters/client construction。

## Tranche Plan and File Budgets

所有 tranche 严格串行；每个 PR 最多 10 个非文档文件、500 changed lines，实际 scope 必须是下列集合的
子集。若任一 tranche 预计超限，先拆成新的 spec amendment，禁止挤压测试或临时扩大。

| Tranche | Candidate writable scope（已核实存在；新 conformance 文件除外） | Budget / close semantics |
| --- | --- | --- |
| D0 decision amendment | `specs/GH965/{product,tech,tasks}.md` | docs-only；`Refs #965`；不实现。 |
| D1 runtime contract | `src/core/router/mod.rs`, `unified.rs`, `gateway_config.rs`, `deployment.rs`, `selection.rs`, `execute_impl.rs`, `execution.rs`, `error.rs`, `src/core/router/tests/router_tests.rs` | 9 files / ≤500；`Refs #965`。 |
| D1E-a1 canonical taxonomy + retry convergence | `src/core/providers/unified_provider_http_mapping.rs`, `src/core/providers/unified_provider_methods.rs`, `src/core/providers/failure.rs`, `src/core/providers/mod.rs`, `src/utils/error/canonical.rs`, `src/core/router/retry_policy.rs` | 6 files / ≤500；删除 `ProviderFailureKind` 及 re-export，在唯一 exhaustive match 中保留 typed retry facts，并锁定 canonical/HTTP/retry table；`Refs #965`。 |
| D1E-a2a provider redaction + SDK mapping | `src/core/providers/unified_provider_methods.rs`, `src/sdk/errors.rs` | 2 files / ≤500；增加保留原 variant 的 `redacted()` copy，0.6 SDK 只按 canonical code 映射到既有 variant；不得给 legacy string variant 加真实 deprecation 属性或 lint allow；`Refs #965`。 |
| D1E-a2b legacy SDK error deprecation | `src/sdk/errors.rs`, `src/sdk/client/completions.rs`, new `src/sdk/provider_error_deprecation_guard_tests.rs` | 3 files / ≤350；依赖 D1E-a2a merged；只增加 true deprecation、9 个局部 allow + T010-linked 0.7 follow-up marker，以及 Rust-only `syn::Visit` owner/role exact-allowlist guard；支持 package checkout 且不依赖 Git/Python，无运行行为改动；`Refs #965`。 |
| D1E-b response emitters + redaction | `src/utils/error/gateway_error/response.rs`, `src/utils/error/gateway_error/conversions.rs`, `src/server/routes/ai/openai_errors.rs`, `src/utils/error/gateway_error/response_tests.rs` | 4 files / ≤500；Gateway wrapper 与真实响应出口都只携带 `redacted()` copy；`Refs #965`。 |
| D1E-c legacy retry helper deprecation | `src/core/providers/contextual_error.rs`, `src/core/providers/unified_provider_methods.rs`, `src/core/types/errors/traits.rs`, `src/core/router/execution.rs`, `src/utils/error/utils/retry.rs`, `src/sdk/errors.rs`, `src/server/routes/ai/batches.rs`, `src/server/routes/ai/fine_tuning.rs` | 8 files / ≤500；六个 provider-specific helper 保留 0.6 行为、deprecated、production 零消费；canonical coarse helpers 明确 grandfather；`Refs #965`。 |
| D2 completion facade | `src/core/router/execute_impl.rs`, `src/core/completion/mod.rs`, `router_trait.rs`, `types.rs`, `conversion.rs`, `default_router/mod.rs`, `default_router/router_impl.rs`, `src/core/completion/tests.rs`, `tests/e2e/chat_completion.rs` | 实际最多 8 files / ≤500；只迁移 pinned typed execution boundary + binding + unary；候选路径 9 个但 PR 必须取所需子集；`Refs #965`。 |
| D3C credential compare hardening | `src/core/router/unified.rs`, `deployment.rs`, `gateway_config.rs`, `src/utils/auth/crypto/hmac.rs`, `src/utils/auth/crypto/tests.rs` | 5 files / ≤500；deployment publication 预计算并存储定长 digest，request path 每请求只 hash 一次后定长比较；`Refs #965`。 |
| D3 completion stream/override cleanup | `src/core/completion/stream.rs`, `src/core/completion/types.rs`, `default_router/mod.rs`, `default_router/router_impl.rs`, `default_router/dynamic_providers.rs`, `default_router/dynamic_providers/routes.rs`, `default_router/dynamic_providers/tests.rs`, `tests/e2e/chat_completion.rs` | 8 files / ≤500；依 `HD-002/003`；含全部 override 字段分类与 `#[deprecated]`；`Refs #965`。 |
| D4 SDK runtime binding | `src/sdk/config.rs`, `errors.rs`, `client/llm_client.rs`, `client/routing.rs`, `client/types.rs`, `client/tests.rs`, `src/sdk/mod.rs` | 7 files / ≤500；仅 construction/selection；`Refs #965`。 |
| D5 SDK execution cleanup | `src/sdk/client/completions.rs`, `embeddings.rs`, `provider_payloads.rs`, `stats.rs`, `llm_client.rs`, `routing.rs`, `tests/integration/router_tests.rs` | 7 files / ≤500；sender/state/error mapping；`Refs #965`。 |
| D6 registry demotion | `src/core/providers/provider_registry.rs`, `src/core/providers/mod.rs`, `src/core/embedding/router.rs`, `src/core/completion/mod.rs`, `src/core/completion/router_trait.rs`, `src/core/completion/default_router/mod.rs`, `src/lib.rs` | 7 files / ≤500；依 `HD-003`；`router_trait.rs` 是 completion `Router` trait 的定义处，必须在此打 `#[deprecated]`；`Refs #965`。 |
| D7a HTTP handle-aware helpers | `src/server/state.rs`, `http.rs`, `src/server/routes/ai/execution.rs`, `budgeted.rs`, `execution_retry_delay_tests.rs`, `provider_selection.rs` | 6 files / ≤500；AppState/runtime token 与 handle-aware helper plumbing，暂留 build-safe legacy wrapper；`Refs #965`。 |
| D7b HTTP unary callers | `src/server/routes/ai/audio/speech.rs`, `audio/transcriptions.rs`, `audio/translations.rs`, `chat.rs`, `embeddings.rs`, `images.rs`, `images/generation.rs`, `moderations.rs`, `rerank.rs`, `gemini.rs` | 10 files / ≤500；所有 unary callers 改传 pinned handle；Gemini 仅 binding plumbing；`Refs #965`。 |
| D7c HTTP stream/query cleanup | `src/server/routes/ai/chat_streaming.rs`, `completions_streaming.rs`, `responses_stream.rs`, `gemini.rs`, `gemini/provider.rs`, `models.rs`, `response_cache.rs`, `budgeted.rs`, `execution.rs`, `src/server/state.rs` | 10 files / ≤500；迁移余下 caller，删除 AppState router field 与 legacy helper；Gemini 行为不变；`Refs #965`。 |
| D7d background response pinning | `src/server/routes/ai/responses.rs`, `responses/lifecycle.rs`, `responses/lifecycle_tests.rs`, `chat.rs` | 4 files / ≤500；acceptance 时 bind，spawned task 只消费该 handle；`Refs #965`。 |
| D7e capability-only runtime attempt plan | `src/core/router/selection.rs`, `execute_impl.rs`, `tests/execution_tests.rs`, `tests/router_tests.rs` | 4 files / ≤500；pinned snapshot + capability routing key + runtime-owned retry/settlement，无 sentinel model；`Refs #965`。 |
| D7f closed provider management dispatch | new `src/core/providers/management_dispatch.rs`, `src/core/providers/mod.rs`, `openai/client.rs`, `openai/api_methods.rs`, new `openai_like/management.rs`, `openai_like/mod.rs`, `openai_like/provider.rs`, `openai_like/provider/tests.rs`, `openai/client_tests/provider_support_tests.rs` | 9 files / ≤500；只暴露 closed batch/fine operations，复用 provider auth/endpoint/pool 并补齐 executable capability；`Refs #965`。 |
| D7g batch route ownership | `src/server/routes/ai/batches.rs`, `tests/batches_routes.rs` | 2 files / ≤500；route 只组装 typed operation 并委托 D7e/D7f runtime plan；`Refs #965`。 |
| D7h fine-tuning route ownership + sender cleanup | `src/server/routes/ai/fine_tuning.rs`, `route_http.rs`, `provider_config.rs`, `src/server/routes/ai/mod.rs`, `tests/fine_tuning_routes.rs` | 5 files / ≤500；route 只委托 runtime plan；删除最后的 route-owned client/config helpers；`Refs #965`。 |
| D7i HTTP conformance | `tests/integration/mod.rs`, `tests/integration/router_tests.rs`, new `tests/integration/router_runtime_conformance.rs` | 3 files / ≤500；三入口 deterministic conformance + 全 production route source guard；`Refs #965`。 |
| D8 release handoff | GitHub follow-up issue + linked SpecRail packet（不改 production） | durable 0.7.0 removal scope；closure 前完成；`Refs #965`。 |

`src/server/routes/ai/execution.rs` 当前接近 U-16 hard ceiling，D7a/D7c 只允许机械签名迁移与删除 legacy wrapper，
不得在其中扩展业务逻辑；若该机械改动令文件超过 800 行，先在 D7a 内抽取既有 helper，不得扩大行为。
D2/D3、D4/D5、D2/D6 虽有同路径（`types.rs`、`router_trait.rs`、
`default_router/mod.rs`），但为严格串行且各自从前一 merged SHA 开始，不并行写同一文件。

D1E 拆成 D1E-a1/D1E-a2a/D1E-a2b/D1E-b/D1E-c 是预算修正，不是范围扩大：原 D1E 列的 `utils/retry.rs` **不存在**
（repo 中无该文件），而真正的 retry/响应出口是 `src/core/router/retry_policy.rs`、
`src/core/providers/failure.rs`、`src/utils/error/gateway_error/response.rs`、
`src/server/routes/ai/openai_errors.rs`，且六个 provider-specific context-free compatibility helpers 及其两个
production callers 分散在八个真实文件（canonical coarse helpers 明确 grandfather，不计 removal scope）。
单一 tranche 需同时改动 taxonomy、序列化出口与 deprecated compatibility surface，500-line 预算下必然挤压测试，
故按本节"超限先拆 tranche"的规则拆分。2026-07-18 在 merged `origin/main@8d57e42b` 上进行的实现测量进一步证明，
原 D1E-a 的 canonical/HTTP/retry table 与 SDK mapping/redaction implementation 已达到 598 changed lines，
且此时 SDK negative fixture 尚未加入；压缩断言或删测试才能回到 500，明确违反 task guard。
因此 D1E-a 先严格串行拆为 a1（typed facts/retry/HTTP）与 a2a（redaction/SDK canonical mapping），共享的
`unified_provider_methods.rs` 只允许前一 tranche 合并后由后一 tranche 接续修改。2026-07-18 对实现 draft 的
fresh measurement 是 **491 changed lines = 439 additions + 52 deletions**；439 additions 已包含 legacy
`#[deprecated]` attribute 的 4 行。D1E-a2a 精确移除这 4 行后为
**487 changed lines = 435 additions + 52 deletions**。上文 deterministic classifier command 当时尚未存在于
draft，按 contract 只在 verification 中执行且占 0 production/test diff line。若实现偏离该测量或新增行使
a2a 超过 500，必须先减少实现且不得删除/压缩安全 fixture；不能把 guard 预算记为 0 后继续超限。
true deprecation、`completions.rs` 兼容 lint 与全 Rust-target source guard 独立进入 a2b；独立 P1 review
因此要求该 tranche 严格依赖 a2a merged，且不再写 `unified_provider_methods.rs`。同理，credential 修复
独立成 D3C，避免 D3 触及 10 文件上限。D3C 为满足 length-independent match 必须同时拥有 digest helper 与
canonical deployment/snapshot metadata publication；只改 `hmac.rs`/tests 会在 request candidate loop 内重复
hash 变长 stored secret，不能满足本 contract。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | D1 canonical construction/runtime registration | `cargo test --all-features --locked core::router`；factory-count/source guard fixture。 |
| B-002 | D2/D4/D7a-D7i adapters + shared fixture | `cargo test --all-features --locked --test lib integration::router_runtime_conformance::selection_identity`。 |
| B-003 | D2-D7 adapter cleanup | `cargo test --all-features --locked --test lib integration::router_runtime_conformance::single_sender`；production source guard。 |
| B-004 | D1/D2/D4 config normalization | `cargo test --all-features --locked --test lib integration::router_runtime_conformance::invalid_and_empty_config`。 |
| B-005 | canonical alias/surface selection | `cargo test --all-features --locked support_matrix` 加 conformance `alias_and_unsupported` fixture。 |
| B-006 | D1E-a1 删除 `ProviderFailureKind` 并保留 typed facts；D1E-a2a 收敛 provider redaction/SDK existing-category mapping；D1E-a2b 增加 legacy SDK error deprecation 与兼容 guard；D1E-b 收敛 Gateway wrapper/响应出口；D1E-c 隔离旧 bool helpers；D2 以 pinned typed handle boundary 把 terminal `ProviderError` 原样交给 completion adapter | conformance `error_class_mapping` table覆盖全部 `ProviderError` variants，并检查 0.6 existing SDK category/Gateway typed wrapper、secret redaction/retryability/cancellation；`RetryPolicy::decide` 按 `RetryContext` 逐 variant 断言 pre/post-output；tech §5 deterministic command 证明 a2a canonical-code-only conversion；`legacy_provider_error_deprecation_allowlist_does_not_grow` 扫描所有 Rust target/source，锁定 D1E-a2b 的局部兼容站点并拒绝 lint downgrade 或 allow/callsite 增长；D2 focused completion fixture 证明 terminal provider variant/canonical code 不经 `RouterError` 往返。 |
| B-007 | deployment lease/state + SDK stats view | conformance `exactly_once_state` fixture比较 attempt trace 与 counter delta。 |
| B-008 | runtime retry/fallback | conformance `retry_and_fallback` fixture证明 adapter request count 与 runtime attempts 相等。 |
| B-009 | immutable generation replacement | conformance `snapshot_replacement` 并发双 listener/key fixture。 |
| B-010 | runtime streaming lease | conformance `stream_failure_cancel_and_success` fixture；`cargo test --all-features --locked streaming`。 |
| B-011 | D2-D6 facades/deprecations | compile fixtures + `cargo test --all-features --locked --doc`；release-note/API diff 人工复核。 |
| B-012 | D1E-a2a SDK mapping classifier command、D1E-a2b all-target deprecation allowlist guard；D7i final evidence architecture | D1E-a2a 的 tech §5 command 拒绝 SDK exhaustive/string classifier且不占 implementation diff；D1E-a2b 的 Rust-only `syn::Visit` guard 对 `src`、实际存在/随包发布的 `tests`、`examples`、`benches`、`build.rs` 与 Cargo metadata target `src_path` 做 owner/role/path exact-allowlist，拒绝 qualified/value/type/import alias、wildcard、qself/raw/`Self`/split path 与 macro relocation，并在无 Git/无 Python的 package checkout 运行；最终全部 `router_runtime_conformance` tests + source guard red/green fixture 扫描所有 production AI routes，`config.gateway.providers` selection scan、`RouteHttpClient`、`OpenAIFineTuningProvider` 与 adapter-owned sender 零命中；仅 matrix tests 不计完成。 |

## 数据流

入口配置先按批准的 compatibility rules 归一成 canonical provider/deployment config；canonical factory 创建
provider，runtime 原子发布含 deployment/state 的 immutable generation。请求 adapter 将 DTO、surface、
model 与批准的 request context 交给 runtime；runtime 选择 deployment、取得 lease、执行 selected provider、
决定 retry/fallback 并 exactly-once 结算。adapter 只把 canonical response/error 映射为 HTTP、SDK 或
completion 外观，不持久化第二份 routing state，也不执行额外外部调用。

无数据库迁移。外部调用只允许 selected `Provider` 内已受 #968 policy 约束的 client 发出。

## 备选方案

- 只共享 `UnifiedRouter::select_from_routing_contexts`：SDK 仍拥有 config、stats 和 sender，无法满足 B-001/
  B-003/B-007，拒绝。
- 保留 `DefaultRouter` 作为 fallback engine：runtime 故障会静默切到不同 provider/config，违反 B-004，拒绝。
- 用 support matrix 生成三套 adapter dispatch：matrix 是声明而非 provider instance，不满足 issue acceptance，拒绝。
- 一次 PR 删除全部 legacy：超过 review/file/line budget，且会绕过 `HD-002/003`，拒绝。
- 用 `Any`/downcast 或字符串名称恢复 concrete provider：破坏闭集类型安全并复制 dispatch，拒绝。

## 风险

- Security: request override 或 SDK client 迁移若创建普通 sender，会绕过 #968；source guard 与
  `HD-002` 必须 fail closed。
- Compatibility: public completion/SDK/registry APIs 已导出；`HD-003` 和 compile fixtures 是硬门槛。
- Performance: shared ArcSwap snapshot 应避免额外锁；adapter 不再复制 provider client。需比较 selection
  allocation/latency，禁止为兼容保留双执行。
- Maintenance: staged PR 若停在 facade 已接入但旧 engine 仍可回退，会形成更隐蔽双路径；每 tranche 必须
  有 source guard 和明确 rollback point。
- Concurrency: generation replacement、stream lease 与 SDK stats view 容易重复结算；B-007/B-009/B-010
  fixtures 必须使用可观察 counter/listener，不只检查返回值。

## 测试计划

- [ ] Decision gate：`HD-001` 至 `HD-004` resolved，spec amendment 通过 write/implement route gate。
- [ ] Unit：router construction/selection/retry/state、completion/SDK adapter mapping、support matrix regression。
- [ ] Integration：三入口 selection/error/retry/fallback/snapshot/stream/cancel/exactly-once fixtures。
- [ ] Architecture：production source guard 对第二 map/config scan/local router/client 为零命中，red fixture 必须失败。
- [ ] Repository：`cargo fmt --all -- --check`。
- [ ] Repository：`cargo check --all-targets --all-features --locked`。
- [ ] Repository：`cargo clippy --all-targets --all-features --locked -- -D warnings`。
- [ ] Repository：`cargo test --all-features --locked -- --test-threads=1`。
- [ ] PR：每 tranche scope/overlap、CI、0 unresolved review threads、independent review 与 required gate。

## 回滚方案

按 D7i → D7h → D7g → D7f → D7e → D7d → D7c → D7b → D7a → D6 → D5 → D4 → D3 → D3C → D2 → D1E-c → D1E-b → D1E-a2b → D1E-a2a → D1E-a1 → D1 逆序整体 revert 已合并 tranche；每个中间点必须仍有一个明确可用
的 canonical runtime，不得只恢复 adapter fallback。若 closure audit 已关闭 #965，回滚后重新打开 issue 并在
release note 标明被恢复的 `HD-003` compatibility surface。无持久化迁移；runtime generation replacement
通过进程重启/重新构造恢复。若安全回归涉及 sender/override，首先回滚对应 D3/D5，同时保持 #968 policy。
