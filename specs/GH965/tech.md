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
comment 越过 spec gate。另有 release-policy 子门：0.7.0 removal 由 `SP965-T010` 建立的 durable follow-up
管理，并把当前 version-bump workflow 的显式修订与 fixture 证据设为 removal 前硬依赖。

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

`api_version`/`organization` 归入 legacy selector 是本 amendment 的新增判定（`HD-002` 的自然延伸，不改变其
0.6→0.7 窗口）。若维护者认为两者应长期保留为 validated context，需要显式修订 `HD-002` 后再实现。

0.6.0 中 `api_key`/`api_base`/`api_version`/`organization` 带 `#[deprecated(since = "0.6.0", note = "configure a canonical runtime deployment")]`。
legacy selector 只在当前 handle 的 immutable deployment snapshot 中做 policy match：提供的每个字段都必须与
同一 deployment 的 canonical config 相符，且结果必须恰好一个；零/多匹配分别返回 typed not-found/
invalid-configuration。`LegacyRuntimeSelector` 不实现 `Display`，其手写 `Debug` 永远把 `api_key` 与
`api_base` 的 present value 输出为 `[REDACTED]`（不得保留 URL userinfo、path、query 或 fragment）；raw secret、
signed query 与 private endpoint 只存在于 request-local memory，不进入 identity、trace、error 或 log；
resolver 不得调用 factory、`add_deployment`、client constructor 或 env。0.7.0 删除字段与 selector。

credential match **不得直接使用现有 `utils::auth::crypto::hmac::constant_time_eq`**（`hmac.rs:26`）：该函数
对不等长输入提前 `return false`（`:27-29`），泄漏候选 credential 的长度，且比较耗时随长度变化。API key 熵较高
使其可利用性有限，但 legacy selector 恰好是把用户提供的 raw secret 与 deployment 配置逐个比对的放大场景。
D3C 的做法是在 canonical deployment config 归一化/发布时，把每个 stored credential **预计算**为不透明、
不可序列化且 `Debug` 恒为 `[REDACTED]` 的 `[u8; 32]` SHA-256 digest，并随 immutable routing snapshot 的
legacy-selector metadata 保存；raw stored credential 不进入该 metadata。request selector 的 raw key 每请求只
计算一次 digest，随后对每个候选只做两个定长 32 字节值的 constant-time 比较。禁止在 candidate loop 内重新
hash stored credential；否则耗时仍随每个候选 secret 长度变化，不能满足 length-independent match。
`sha2` 已在 `Cargo.toml:97`，因此不引入新依赖，也不改动 `verify_hmac_signature` 等既有 HMAC 调用方的行为
（那些输入长度本就由算法固定）。

先让现有 free functions 和经 `HD-003` 保留的 trait/type 委托给批准的 runtime binding，再逐段删除
`DefaultRouter` 的 env bootstrap、static prefix selection、dynamic provider construction 和直接 provider
execution。按已解决的 `HD-002`，`headers`/`timeout` 只能通过 `RuntimeRequestContext::validate` 进入 selected
provider 的安全 execution API；`api_key`/`api_base` 只在 0.6.0 legacy selector 窗口存在并按 0.7.0 删除，
不得由实现者重新选择保留或弃用。迁移期间 facade 不能在 runtime 失败时回到旧 registry。

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

D1E-a/D1E-b 以 `ProviderError` 为 source，并复用现有 `src/utils/error/canonical.rs` 的 `ErrorCode` / `CanonicalError`
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

D1E-a 的收敛工作因此不是"加 retryable 方法"，而是删除既有的平行 taxonomy：
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
| D1E-a canonical taxonomy + retry convergence | `src/core/providers/unified_provider_http_mapping.rs`, `unified_provider_methods.rs`, `src/core/providers/failure.rs`, `src/core/providers/mod.rs`, `src/utils/error/canonical.rs`, `src/core/router/retry_policy.rs`, `gateway_error/http_mapping.rs`, `src/sdk/errors.rs` | 8 files / ≤500；删除 `ProviderFailureKind` 及 re-export，在唯一 exhaustive match 中保留 typed retry facts；0.6 SDK 映射到现有 variant；`Refs #965`。 |
| D1E-b response emitters + redaction | `src/utils/error/gateway_error/response.rs`, `src/utils/error/gateway_error/conversions.rs`, `src/server/routes/ai/openai_errors.rs`, `src/utils/error/gateway_error/response_tests.rs` | 4 files / ≤500；Gateway wrapper 与真实响应出口都只携带 `redacted()` copy；`Refs #965`。 |
| D1E-c legacy retry helper deprecation | `src/core/providers/contextual_error.rs`, `src/core/providers/unified_provider_methods.rs`, `src/core/types/errors/traits.rs`, `src/core/router/execution.rs`, `src/utils/error/utils/retry.rs`, `src/sdk/errors.rs`, `src/server/routes/ai/batches.rs`, `src/server/routes/ai/fine_tuning.rs` | 8 files / ≤500；六个 provider-specific helper 保留 0.6 行为、deprecated、production 零消费；canonical coarse helpers 明确 grandfather；`Refs #965`。 |
| D2 completion facade | `src/core/completion/mod.rs`, `router_trait.rs`, `types.rs`, `conversion.rs`, `default_router/mod.rs`, `default_router/router_impl.rs`, `src/core/completion/tests.rs`, `tests/e2e/chat_completion.rs` | 8 files / ≤500；只迁移 binding + unary；`Refs #965`。 |
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

D1E 拆成 D1E-a/D1E-b/D1E-c 是本 amendment 的预算修正，不是范围扩大：原 D1E 列的 `utils/retry.rs` **不存在**
（repo 中无该文件），而真正的 retry/响应出口是 `src/core/router/retry_policy.rs`、
`src/core/providers/failure.rs`、`src/utils/error/gateway_error/response.rs`、
`src/server/routes/ai/openai_errors.rs`，且六个 provider-specific context-free compatibility helpers 及其两个
production callers 分散在八个真实文件（canonical coarse helpers 明确 grandfather，不计 removal scope）。
单一 tranche 需同时改动 taxonomy、序列化出口与 deprecated compatibility surface，500-line 预算下必然挤压测试，
故按本节"超限先拆 tranche"的规则拆分。同理，credential 修复
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
| B-006 | D1E-a 删除 `ProviderFailureKind` 并保留 typed facts；D1E-b 收敛脱敏 wrapper/响应出口；D1E-c 隔离旧 bool helpers | conformance `error_class_mapping` table覆盖全部 `ProviderError` variants，并检查 0.6 existing SDK category/Gateway typed wrapper、secret redaction/retryability/cancellation；`RetryPolicy::decide` 按 `RetryContext` 逐 variant 断言 pre/post-output。 |
| B-007 | deployment lease/state + SDK stats view | conformance `exactly_once_state` fixture比较 attempt trace 与 counter delta。 |
| B-008 | runtime retry/fallback | conformance `retry_and_fallback` fixture证明 adapter request count 与 runtime attempts 相等。 |
| B-009 | immutable generation replacement | conformance `snapshot_replacement` 并发双 listener/key fixture。 |
| B-010 | runtime streaming lease | conformance `stream_failure_cancel_and_success` fixture；`cargo test --all-features --locked streaming`。 |
| B-011 | D2-D6 facades/deprecations | compile fixtures + `cargo test --all-features --locked --doc`；release-note/API diff 人工复核。 |
| B-012 | D7i evidence architecture | 全部 `router_runtime_conformance` tests + source guard red/green fixture；guard 扫描所有 production AI routes，`config.gateway.providers` selection scan、`RouteHttpClient`、`OpenAIFineTuningProvider` 与 adapter-owned sender 零命中；仅 matrix tests 不计完成。 |

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

按 D7i → D7h → D7g → D7f → D7e → D7d → D7c → D7b → D7a → D6 → D5 → D4 → D3 → D3C → D2 → D1E-c → D1E-b → D1E-a → D1 逆序整体 revert 已合并 tranche；每个中间点必须仍有一个明确可用
的 canonical runtime，不得只恢复 adapter fallback。若 closure audit 已关闭 #965，回滚后重新打开 issue 并在
release note 标明被恢复的 `HD-003` compatibility surface。无持久化迁移；runtime generation replacement
通过进程重启/重新构造恢复。若安全回归涉及 sender/override，首先回滚对应 D3/D5，同时保持 #968 policy。
