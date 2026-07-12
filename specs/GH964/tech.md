# Tech Spec

## Linked Issue

GH-964 / #964

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Provider config | `src/config/models/provider.rs` | 声明五个 health 字段；partial block 的 `expected_codes` 会 serde 成空数组 | 输入与默认值边界 |
| Validation | `src/config/validation/config_validators.rs` | 只校验非零/非空，不解析 endpoint、不校验 status 范围或跨字段组合 | 必须 fail closed |
| Provider probes | `src/core/providers/mod.rs`, provider implementations | `Provider::health_check()` 可调用各 provider 已认证的 native probe | 默认 probe 执行路径 |
| Gateway mapping | `src/core/router/gateway_config.rs` | provider 变成 deployment，但丢弃整个 health config | 声明到执行的缺失边 |
| Router runtime | `src/core/router/deployment.rs`, `unified.rs` | health 状态参与选择，但没有 active probe task | live 状态消费点 |
| Legacy health monitor | `src/core/health/*` | 未接入 router，实际 checker 使用随机模拟 | 明确不复用 |

## 设计方案

1. 在 provider config 中增加唯一 `default_health_expected_codes()`，让 serde partial block 与
   `Default` 都产生 `[200]`。增加结构化 endpoint resolver：绝对 URL 直接解析；相对 path 只允许
   基于显式 `base_url` 用 URL API 解析。`ProviderConfig::validate` 对最终 URL 执行既有 SSRF 校验，
   并校验 expected code 范围、重复项和 endpoint/expected_codes 组合。
2. 在 `deployment.rs` 增加 runtime `HealthCheckPolicy`：`provider_name`、`interval_secs`、
   `failure_threshold`、`recovery_timeout_secs`、normalized endpoint URL 和 expected codes。
   `DeploymentConfig.health_check_policy: Option<HealthCheckPolicy>` 默认 `None`；gateway 只在 provider
   health 配置偏离 schema 默认值时创建 policy，保证默认 gateway 与程序化构造都不新增主动流量。
3. `gateway_config.rs` 在 create-provider 前显式调用与完整 `ProviderConfig::validate` 共用的 health
   runtime validator，把 endpoint 解析错误转成 `RouterError::InvalidConfiguration`。direct factory
   不重跑 base URL 等已有全局校验，以保留自托管/private upstream 的程序化构造边界。每个由 provider
   config 创建的 deployment 保存相同 policy；完成所有 deployment 后调用
   `Router::start_configured_health_checks()`。
4. 新建 `src/core/router/health_probe.rs` 作为唯一 active probe engine：
   - 按 `provider_name` 分组 deployment，每个 provider 只 spawn 一个 task；
   - endpoint 缺失时调用组内 provider clone 的 native `health_check()`；
   - endpoint 存在时用按 deployment timeout 缓存的 SSRF-safe/no-redirect client 发无认证 GET，
     并按 expected codes 判定；该 client 在初始连接 DNS 解析时过滤 private/reserved IP，且不请求
     redirect target，因此 3xx 状态可被直接消费；
   - 用 deployment `timeout_secs` 包裹单次 probe；
   - 首次立即执行，普通结果后 sleep interval，达到失败阈值后 sleep recovery timeout；
   - task handle 保存在 Router，`Drop` 中 abort，错误仅影响当前 provider loop。
5. probe task 局部保存连续失败计数，结果原子更新同 provider 的全部 deployment：
   - success: 清零并将非 Cooldown 状态设为 Healthy；
   - failure below threshold: 将非 Cooldown/Unhealthy 状态设为 Degraded；
   - failure at/above threshold: 将非 Cooldown 状态设为 Unhealthy；Cooldown 期间记录 probe-unhealthy
     标记，使 cooldown 到期后恢复为 Unhealthy 而不是提前进入 Degraded。
   请求路径 `Deployment::record_failure` 改为只把 Healthy/Unknown 转为 Degraded，不覆盖 Unhealthy/Cooldown。
6. 全局 `RouterConfig.enable_pre_call_checks=false` 时 start 方法直接返回且不 spawn task。动态
   `add_deployment`/`set_model_list` 不自动重建任务，作为本 issue 明确非目标；gateway startup 是唯一
   自动启动边界。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1/P2 one task + interval/global switch | `health_probe.rs`, `gateway_config.rs` | grouping/start tests and deterministic next-delay assertions |
| P3 native path/programmatic compatibility | `HealthCheckPolicy::None`, native probe helper | native-provider mock HTTP test; default deployment has no task |
| P4/P5 endpoint + expected codes | config resolver/validator, custom HTTP probe | partial YAML, URL validation, local HTTP 204/302/500 tests |
| P6/P7 threshold + recovery | probe transition helper | table tests for Degraded/Unhealthy/Healthy and next delay |
| P8 cooldown/request race | deployment transition helpers | tests prove Cooldown/Unhealthy are not overwritten |
| P9 lifecycle/error isolation | Router task registry + Drop | abort/drop and failed-probe loop tests |
| P10 direct factory validation | `from_gateway_config` | invalid config rejected before provider creation |

## 数据流

`gateway.yaml ProviderConfig.health_check` -> full provider validation -> normalized
`HealthCheckPolicy` -> deployment group -> one Tokio probe loop -> provider-native status or custom HTTP status ->
atomic router `HealthStatus` transition -> existing deployment selection excludes `Unhealthy`.

没有新增持久化或外部 API。自定义 endpoint 不携带 provider credential；URL 和错误日志不得包含密钥。

## 备选方案

- 接入 `core::health::HealthMonitor`：其 checker 仍是随机模拟且状态不驱动 UnifiedRouter，拒绝。
- 每个 model deployment 启动 task：会对同一 provider 重复探测，拒绝。
- 把 endpoint 注入所有 provider trait 实现：需要跨全部 provider 复制认证与 URL 逻辑，超出本 issue。
- 对自定义 endpoint 自动附加 `api_key`：不同 provider 的认证协议不同且可能泄密，拒绝。
- 只映射 threshold 到全局 RouterConfig：多 provider 策略互相覆盖，拒绝。

## 风险

- Security: endpoint 是新的出站 URL 面；必须复用 SSRF 校验，禁止隐式凭据，并避免 URL/secret 日志。
- Compatibility: 默认 gateway health checks 从声明但无效变成真实主动请求；这是预期修复。
- Performance: 每 provider 一个低频 task；多模型不放大请求数。
- Maintenance: 新 engine 只负责 UnifiedRouter，不与未接线的 legacy HealthMonitor 共享状态。

## 测试计划

- Unit tests: 默认值、URL resolver、status code/跨字段校验、mapping、状态迁移、delay 选择。
- Integration tests: native provider probe、自定义 endpoint expected code、全局开关、provider 分组与 task 取消。
- Manual verification: `rg` 确认五字段 mapping 与唯一 startup 调用，legacy random checker 未被引用。
- Deterministic checks: fmt、all-features check、strict clippy、focused router/config tests、串行全量 tests。

## 回滚方案

回滚 GH964 PR 即恢复无 active provider probe 的旧行为。没有 schema migration 或持久化状态；删除
`HealthCheckPolicy`、probe module 与 gateway startup 调用后，现有请求级 health/cooldown 路径保持可用。
