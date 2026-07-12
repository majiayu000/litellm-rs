# Tech Spec

## Linked Issue

GH-968 / #968

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Provider trait switch | `src/core/traits/provider/config.rs:117` | `use_ssrf_safe_client()` 默认 `false`，只有个别 config override | 该布尔 hook 既非强类型也未覆盖其他 client 路径 |
| Runtime DNS/redirect guard | `src/utils/net/http.rs:39`, `src/utils/net/http.rs:76`, `src/utils/net/http.rs:249` | 已有 public-only DNS filter 与 redirect policy，但只由少量调用方选择，且 initial literal 依赖调用方先校验 | B-002 至 B-004 的可复用基础与缺口 |
| Config model/validation | `src/config/models/provider.rs:11`, `src/config/validation/config_validators.rs:141` | `base_url` 配置期运行 SSRF 校验，没有 endpoint access policy | B-001、B-005、B-006 的配置边界 |
| Environment config | `src/config/models/gateway.rs:133`, `src/config/models/gateway.rs:170` | provider env 可覆盖 `BASE_URL`，没有对应 policy 字段 | 环境 override 必须与 YAML 采用同一策略 |
| Factory propagation | `src/core/providers/factory/mod.rs:136`, `src/core/providers/factory/mod.rs:142`, `src/core/providers/factory/mod.rs:165` | 顶层 base URL 与 settings 被映射到各内部 config，没有安全策略传播 | 所有 provider 构造路径的汇合点 |
| Shared pool/streaming | `src/core/providers/base/connection_pool.rs:121`, `src/core/providers/base/connection_pool.rs:133`, `src/core/providers/base/connection_pool.rs:201`, `src/core/providers/base/connection_pool.rs:334` | 普通与流式 global client 无 endpoint policy | OpenAI/OpenAI-like 和 pooled provider 的主要旁路 |
| Native clients | `src/core/providers/anthropic/client.rs:45`, `src/core/providers/gemini/client.rs:42` | 原生 client 直接从 `ClientBuilder` 构造，并支持 proxy | B-007/B-008 的原生旁路 |
| Gemini SDK route | `src/server/routes/ai/gemini/provider.rs:23`, `src/server/routes/ai/gemini/provider.rs:341`, `src/server/routes/ai/gemini/provider.rs:446` | route 读取 provider base URL 后使用静态普通 client | issue 明确列出的 route 级 SSRF 缺口 |
| Shared route client | `src/server/routes/ai/provider_config.rs:9`, `src/server/routes/ai/provider_config.rs:38` | batches/images/moderations 等复用静态 `Client::new()` | 多条直接 route 的共同旁路 |

## 设计方案

### 1. Policy types and validation

- 在 `core::net` 定义 serde `snake_case` enum `ProviderEndpointAccess`，闭集为 `public_only` 与
  `private_network`，`Default` 为 `public_only`。
- 构造运行时 `ProviderEndpointPolicy` 时解析并固定配置 base URL 的 scheme、host、effective port；
  `private_network` policy 携带精确 authority，避免授权跨 provider 或目标复用。
- 将 IP 分类统一到 `core::net::ssrf_guard`：public-only 复用现有 private/reserved 判定；private-network 仅
  额外允许 loopback、RFC1918 和 ULA。metadata/link-local/CGNAT/unspecified/multicast/benchmark/
  documentation/reserved 在两种模式都拒绝。
- `config::validation::ssrf` 保留兼容入口但委托统一实现，避免两套范围漂移。

### 2. Policy-aware HTTP client

- 在 `utils::net::http` 新增不暴露裸 `reqwest::Client` 的 `ProviderHttpClient` newtype，分别持有普通与
  streaming client，并在 `get/post/request` 创建 builder 前校验 initial URL 与 private authority。
- resolver 包装可注入的 host resolver。生产实现使用 `ToSocketAddrs` + `spawn_blocking`，每次实际
  resolution 根据 policy 过滤全部返回地址；任一不允许地址使该 resolution 整体失败，不选择性放过混合答案。
- public-only redirect policy 最多 10 跳，每跳先做无 DNS URL/literal 校验，随后由连接 resolver 校验；
  private-network 使用 `Policy::none()`。
- 所有 policy client 使用 `.no_proxy()`。带 `proxy`/`proxy_url` 且目标 endpoint 可配置的 provider 在构造期
  fail closed；不保留普通 client fallback。
- cache key 包含 timeout、streaming/redirect mode、access mode；private client 还包含 authority。测试
  resolver/client 不进入全局 cache。

### 3. Configuration and propagation

- Gateway `ProviderConfig` 新增 `endpoint_access`，默认 `public_only`，Debug/Default/serde/builder/env loader
  全部传播；环境键为 `LITELLM_PROVIDER_<NAME>_ENDPOINT_ACCESS`。
- `CompletionOptions.api_base` 与其他请求级临时 override 强制构造 public-only policy，不读取 opt-in。
- `BaseConfig`、standalone config macro 和 factory JSON 显式携带 policy；provider-specific `settings` 不得
  覆盖顶层 policy。
- localhost/self-hosted catalog 默认不暗中获得私网权限；示例和错误信息要求用户显式选择。

### 4. Serial implementation slices

1. Policy foundation PR (`Refs #968`): endpoint access/policy、scheme/host/effective-port 私网绑定、完整公网 IP
   分类与 private-network 永久 metadata 拒绝。该 PR 不新增尚未接线的 Gateway config 字段或 HTTP client。
2. HTTP client foundation PR (`Refs #968`): 不暴露裸 client 的可用 request builder、普通/streaming/no-redirect
   policy client，以及 deterministic DNS rebinding/literal/redirect/private 未建连测试。
3. Shared-provider PR (`Refs #968`): Gateway config/default/env/validation、`ProviderConfig` trait、BaseConfig、
   factory、GlobalPoolManager、BaseHttpClient、provider macros、OpenAI/OpenAI-like 普通/流式/health 路径改用
   policy client。
4. Native-route PR (`Fixes #968`): Anthropic/Gemini/Azure/AzureAI/Vertex 及 Gemini/batches/images/moderations/
   fine-tuning/rerank route 旁路接入，拒绝不安全 proxy，并增加源码架构 guard。

每个 PR 独立满足 scope hard guard、current-head reviewer、CI 和 PR gate；只有第三段在全量矩阵通过后关闭 issue。

### 5. Architecture guard

- guard 扫描 Gateway provider/runtime route 的 production Rust 文件，禁止统一 HTTP 模块外新增
  `Client::new`、`Client::builder`、`ClientBuilder::new`、普通 `create_custom_client*` 和普通 streaming factory。
- allowlist 按精确 path + constructor purpose 管理，只允许统一 client 实现和明确不在 GH968 范围的模块；
  不使用可增长的命中数量 baseline。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | endpoint enum、Gateway config/default/env parser | serde/default/env unit tests；未知/空 access 配置启动失败 |
| B-002 | unified URL/IP classifier + initial request validation | literal table tests 覆盖 localhost/private/ULA/metadata/reserved；listener 无 accept |
| B-003 | policy DNS resolver + no fallback constructors | `SequenceResolver` public-at-validation -> private/metadata-at-connect tests；断言 request error 与 listener timeout |
| B-004 | public redirect policy + resolver | public redirect 到 private literal/hostname、多跳负例；每个目标 listener 均未 accept |
| B-005 | private policy authority + config propagation + cache key | 两 provider/两 authority 隔离测试；request override 无 opt-in 测试；cache key equality tests |
| B-006 | private-network address classifier + no-redirect client | loopback/RFC1918/ULA 正例，metadata/link-local/CGNAT/reserved 负例，302 不跟随 |
| B-007 | shared provider、native provider、health 和 route call sites | provider/factory/route matrix tests + architecture guard 零命中 + all-feature tests |
| B-008 | `.no_proxy()` 与 native proxy validation | 环境 proxy 不改变连接目标测试；Anthropic/Gemini 显式 proxy 非法组合测试 |
| B-009 | fallible constructors/error mapping | client build/resolution/config 失败测试；source search 确认没有 fallback 到普通 client |
| B-010 | source architecture guard + CI wiring | guard self-test 红绿 fixtures；PR/main CI exact step；production scan 为零 |
| B-011 | official/public 与 self-hosted compatibility | official config 构造成功；localhost 无 opt-in 失败、显式 private-network 成功；配置示例测试 |
| B-012 | test-only injectable resolver and loopback listeners | focused rebinding suite 明确断言 `accept()` timeout，不访问真实 DNS |

## 数据流

启动配置或环境变量产生 `ProviderEndpointAccess`，Gateway validation 对 configured URL 做第一轮校验；factory
将 access 与规范化 base authority 一起构造成不可变 policy。每次 route/provider 请求先由 newtype 校验 initial
URL/authority，再由 policy resolver 在 connector 实际解析时检查所有 IP；public redirect 重复相同流程，private
redirect 被禁用。错误向现有 provider/Gateway error 类型显式传播，不创建第二条普通 client 数据流。

## 备选方案

- 只把 `use_ssrf_safe_client()` 默认改为 true：该 hook 只被一个 macro 消费，且 initial IP literal、global pool、
  streaming、native client 和 route 静态 client 仍可绕过，拒绝。
- 只做配置期 DNS 校验：存在明确 TOCTOU/DNS rebinding 窗口，违反 B-003，拒绝。
- private-network 使用全局 bool：会通过 client cache/共享 pool 扩大授权，违反 B-005，拒绝。
- private-network 继续自动 redirect：redirect 可把已授权 authority 扩大到其他内网主机，拒绝。
- 使用真实 DNS 测试：不确定、不可证明请求期重绑定且可能受环境缓存影响，拒绝。

## 风险

- Security: wrapper 若暴露裸 client、混合 DNS 答案只过滤部分地址、proxy 仍启用或任一流式/route 路径遗漏，
  都会形成可利用旁路。
- Compatibility: localhost/self-hosted 和 proxy 用户需要显式迁移；错误必须在启动时清楚呈现。
- Performance: 每次新连接需要 DNS 分类；连接池仍复用已建立连接，cache key 扩展会增加有限 client 实例。
- Maintenance: 多套 provider client 构造长期易回归，必须以架构 guard 保持单入口。

## 测试计划

- [ ] Unit: IP/hostname/authority policy、serde/default/env、cache key、redirect mode、proxy 非法组合。
- [ ] Integration: scripted DNS rebinding、literal metadata、public redirect 到 private、private loopback 正例；所有
  负例用 listener timeout 证明未建连。
- [ ] Provider matrix: shared pool、streaming、health、native provider 和直接 route 各至少一个普通/负例，全部
  feature 组合编译。
- [ ] Architecture: guard self-test、production 零命中、PR/main CI wiring。
- [ ] Repository: `cargo fmt --all -- --check`、`cargo check --all-targets --all-features --locked`、strict Clippy、
  `cargo test --all-features --locked -- --test-threads=1`、scope/overlap、SpecRail packet/PR gate。

## 回滚方案

不得回滚到普通 client 或重新允许 metadata。若某一 provider 兼容性失败，保留 policy/client 基础并临时禁用该
provider 的可配置 endpoint，随后 forward-fix 其 propagation；分段 PR 可独立 revert 未接入的调用方，但最终
架构 guard 与已接入的安全路径不得用 warning-only fallback 替代。
