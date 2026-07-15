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
| Existing architecture note | `docs/refactor/01-architecture.md:126` | 已建议把 `DefaultRouter` 改为 `UnifiedRouter` adapter。 | 支持方向，但不是维护者对生命周期/API 的正式选择。 |

## Duplicate / Overlap Boundary

- #725 / merged PR #731 已收敛 registry metadata、factory constructibility 与 canonical keying；GH-965
  复用 `create_provider`/`Provider`，不重写 catalog、factory 或 enum dispatch。
- #728 / merged PR #734 已建立 HTTP/SDK/`completion()` support matrix；GH-965 只让实际执行消费同一
  runtime，不扩大或重算 matrix。
- #966 已由 merged PR #1026 收敛 Gemini SDK-compatible HTTP route 的 selected runtime identity；
  该 PR 修改的 `src/server/routes/ai/gemini.rs`、`src/server/routes/ai/gemini/provider.rs`、
  `tests/gemini_sdk_routes/runtime_provider_tests.rs` 是 GH-965 的已满足前置与非重叠边界。GH-965 不修改
  Gemini wire/policy/selection。
- #968 负责 provider endpoint access 与 SSRF；GH-965 不创建普通 client 旁路，adapter 只能使用 selected
  provider 已持有的 secure client。
- #519 保持 umbrella；#727 继续处理通用 U-16 文件拆分。GH-965 的拆分只服务本 issue 的 500-line gate。
- 2026-07-15 重复检查：`gh pr list --repo majiayu000/litellm-rs --state all --search
  '965 in:body'` 返回空列表；GitHub issue 搜索 `DefaultRouter UnifiedRouter`、
  `ProviderRegistry completion router`、`canonical runtime router` 只命中 #965、umbrella #519 及已关闭
  #725 实现。未发现覆盖本 acceptance criteria 的重复工作或 `Refs/Fixes #965` PR。

## Human Decision Gate

实现前必须由维护者在 spec amendment 中逐项给出选择、理由、迁移版本与回滚语义。当前均为
`unresolved`，因此本 packet 可审查但不可进入 `ready_to_implement`：

| ID | 必须选择的内容 | 不得默认的原因 |
| --- | --- | --- |
| HD-001 | runtime 是显式 per-instance、进程级 default，还是明确组合；HTTP/SDK/free functions 的注入与 replacement API。 | 生命周期选择决定是否真正共享 generation，也决定测试隔离和并发替换语义。 |
| HD-002 | request-level `CompletionOptions` credential/endpoint/headers/timeout override 的保留或弃用，以及 canonical request context 表示。 | 临时 provider/client 会违反 B-003，并可能绕过 #968。 |
| HD-003 | 五组公共 facade 的保留、deprecation window 或 breaking removal。 | 当前 crate 公开导出相关类型；实现者无权猜 semver policy。 |
| HD-004 | canonical typed runtime error 与 HTTP/SDK/completion/cancellation 映射表。 | 只统一字符串会继续造成 retryability/status 漂移。 |

Spec amendment 必须更新 `product.md`、本节、下方确切 file scopes 与测试 fixtures；四项未全部 resolved 时
`SP965-T002` 及后续任务保持 blocked。

## 设计方案

### 1. Canonical runtime contract

以现有 `UnifiedRouter` deployment snapshot 为唯一候选 owner，但公开构造/注入形态由 `HD-001` 决定。
批准后的 runtime contract 必须同时拥有：

- 通过 #725 canonical factory 构造的 `Provider` instance；
- model/capability/alias selection、health/cooldown、lease、rate/budget state；
- retry/fallback 与 selected-deployment execution；
- immutable generation replacement；
- 由 `HD-004` 选择的 typed error。

adapter 只提交 canonical request + surface context 并接收 selected identity/result。不得新增 `Any`/
downcast、第二 provider enum、字符串 provider dispatch 或 adapter-owned sender。

### 2. Completion migration

先让现有 free functions 和经 `HD-003` 保留的 trait/type 委托给批准的 runtime binding，再逐段删除
`DefaultRouter` 的 env bootstrap、static prefix selection、dynamic provider construction 和直接 provider
execution。`HD-002` 若保留 overrides，它们进入 canonical request context，由 selected provider 的安全
execution API 解释；若弃用，则按批准窗口返回明确 typed error。迁移期间 facade 不能在 runtime 失败时回到
旧 registry。

### 3. SDK migration

`ClientConfig` 先归一成 canonical provider/deployment config 并构造 runtime。SDK routing 不再把本地
`ProviderStats` 转成临时 `RoutingContext`，SDK execution 也不再按 `SdkProviderConfig` 创建请求 client。
`LLMClient` 保留的 DTO convenience、stats view 与错误外观都从 canonical execution result/state 派生，
不能反馈为第二套 selection truth。

### 4. Registry demotion

完成 completion/SDK 切换后，搜索所有 `ProviderRegistry` 调用方。embedding router 必须迁移到同一 runtime
contract 或由批准的兼容 facade 委托；公开导出按 `HD-003` 处理。任何保留的 `ProviderRegistry` 都不得拥有
独立 mutable `HashMap<String, Provider>` 或被 production execution 读取。

### 5. HTTP binding and conformance

HTTP 继续从 server startup 获取 `Arc<UnifiedRouter>`；若 `HD-001` 要求共享/替换 API，只在
`http.rs`/`state.rs` 的注入边界调整，不改 route wire。新增 `tests/integration/router_runtime_conformance.rs`
前已搜索现有 tests，无同名/同职责 fixture；该 module 使用本地 listener 和 deterministic providers，令同一
runtime generation 依次由三入口触发并比较 selected identity、typed category、attempt trace 与 state delta。
source guard 扫描 production adapter，拒绝第二 map/config scan/local routing counters/client construction。

## Tranche Plan and File Budgets

所有 tranche 严格串行；每个 PR 最多 10 个非文档文件、500 changed lines，实际 scope 必须是下列集合的
子集。若任一 tranche 预计超限，先拆成新的 spec amendment，禁止挤压测试或临时扩大。

| Tranche | Candidate writable scope（已核实存在；新 conformance 文件除外） | Budget / close semantics |
| --- | --- | --- |
| D0 decision amendment | `specs/GH965/{product,tech,tasks}.md` | docs-only；`Refs #965`；不实现。 |
| D1 runtime contract | `src/core/router/mod.rs`, `unified.rs`, `gateway_config.rs`, `deployment.rs`, `selection.rs`, `execute_impl.rs`, `execution.rs`, `error.rs`, `src/core/router/tests/router_tests.rs` | 9 files / ≤500；`Refs #965`。 |
| D2 completion facade | `src/core/completion/mod.rs`, `router_trait.rs`, `types.rs`, `conversion.rs`, `default_router/mod.rs`, `default_router/router_impl.rs`, `src/core/completion/tests.rs`, `tests/e2e/chat_completion.rs` | 8 files / ≤500；只迁移 binding + unary；`Refs #965`。 |
| D3 completion stream/override cleanup | `src/core/completion/stream.rs`, `default_router/mod.rs`, `default_router/router_impl.rs`, `default_router/dynamic_providers.rs`, `default_router/dynamic_providers/routes.rs`, `default_router/dynamic_providers/tests.rs`, `tests/e2e/chat_completion.rs` | 7 files / ≤500；依 `HD-002/003`；`Refs #965`。 |
| D4 SDK runtime binding | `src/sdk/config.rs`, `errors.rs`, `client/llm_client.rs`, `client/routing.rs`, `client/types.rs`, `client/tests.rs`, `src/sdk/mod.rs` | 7 files / ≤500；仅 construction/selection；`Refs #965`。 |
| D5 SDK execution cleanup | `src/sdk/client/completions.rs`, `embeddings.rs`, `provider_payloads.rs`, `stats.rs`, `llm_client.rs`, `routing.rs`, `tests/integration/router_tests.rs` | 7 files / ≤500；sender/state/error mapping；`Refs #965`。 |
| D6 registry demotion | `src/core/providers/provider_registry.rs`, `src/core/providers/mod.rs`, `src/core/embedding/router.rs`, `src/core/completion/mod.rs`, `src/core/completion/default_router/mod.rs`, `src/lib.rs` | 6 files / ≤500；依 `HD-003`；`Refs #965`。 |
| D7 binding + conformance | `src/server/http.rs`, `src/server/state.rs`, `tests/integration/mod.rs`, `tests/integration/router_tests.rs`, new `tests/integration/router_runtime_conformance.rs` | 5 files / ≤500；不触碰 `execution.rs` 或 #1026 三文件；final PR 才 `Fixes #965`。 |

`src/server/routes/ai/execution.rs` 当前接近 U-16 hard ceiling，不在 writable scope；D7 复用其已存在的
selected-deployment helper。D2/D3、D4/D5 虽有同路径，但为严格串行且各自从前一 merged SHA 开始，不并行
写同一文件。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | D1 canonical construction/runtime registration | `cargo test --all-features --locked core::router`；factory-count/source guard fixture。 |
| B-002 | D2/D4/D7 adapters + shared fixture | `cargo test --all-features --locked --test lib integration::router_runtime_conformance::selection_identity`。 |
| B-003 | D2-D7 adapter cleanup | `cargo test --all-features --locked --test lib integration::router_runtime_conformance::single_sender`；production source guard。 |
| B-004 | D1/D2/D4 config normalization | `cargo test --all-features --locked --test lib integration::router_runtime_conformance::invalid_and_empty_config`。 |
| B-005 | canonical alias/surface selection | `cargo test --all-features --locked support_matrix` 加 conformance `alias_and_unsupported` fixture。 |
| B-006 | `HD-004` mapping + adapters | conformance `error_class_mapping` table覆盖全部闭集并检查 secret redaction/retryability。 |
| B-007 | deployment lease/state + SDK stats view | conformance `exactly_once_state` fixture比较 attempt trace 与 counter delta。 |
| B-008 | runtime retry/fallback | conformance `retry_and_fallback` fixture证明 adapter request count 与 runtime attempts 相等。 |
| B-009 | immutable generation replacement | conformance `snapshot_replacement` 并发双 listener/key fixture。 |
| B-010 | runtime streaming lease | conformance `stream_failure_cancel_and_success` fixture；`cargo test --all-features --locked streaming`。 |
| B-011 | D2-D6 facades/deprecations | compile fixtures + `cargo test --all-features --locked --doc`；release-note/API diff 人工复核。 |
| B-012 | D7 evidence architecture | 全部 `router_runtime_conformance` tests + source guard red/green fixture；仅 matrix tests 不计完成。 |

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

按 D7 → D6 → D5 → D4 → D3 → D2 → D1 逆序整体 revert 已合并 tranche；每个中间点必须仍有一个明确可用
的 canonical runtime，不得只恢复 adapter fallback。若 final PR 已关闭 #965，回滚后重新打开 issue 并在
release note 标明被恢复的 `HD-003` compatibility surface。无持久化迁移；runtime generation replacement
通过进程重启/重新构造恢复。若安全回归涉及 sender/override，首先回滚对应 D3/D5，同时保持 #968 policy。
