# Tech Spec

## Linked Issue

GH-963 / #963

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Provider config | `src/config/models/provider.rs` | `ProviderConfig.retry` 声明四个 schedule 字段并有默认值 | 用户输入与兼容边界 |
| Validation | `src/config/validation/config_validators.rs` | 校验 delay、cap、multiplier 和 jitter | 保证 runtime schedule 合法 |
| Provider factory | `src/core/providers/factory/mod.rs` | 只把 top-level `max_retries` 交给 provider client；nested retry 不属于 provider construction | 不应把 router schedule 塞进每个 provider builder |
| Gateway mapping | `src/core/router/gateway_config.rs` | provider 变成 deployment，但 `DeploymentConfig` 没有 retry schedule | 声明到执行的缺失边 |
| Deployment runtime | `src/core/router/deployment.rs` | 只保存 limits、timeout、weight、priority | 需要保存按 deployment 选择的 schedule |
| Retry engine | `src/core/router/retry_policy.rs`, `src/core/router/execution.rs` | `RetryPolicy` 对 global `RouterConfig` 使用 1s/2x/30s 固定退避 | 唯一 retry 决策和 delay 公式 |
| Live callers | `src/core/router/execute_impl.rs`, `src/server/routes/ai/execution.rs` | 选中 deployment 后仍把 global router config 传给 policy | unary 与 streaming 的实际消费点 |

## 设计方案

1. 在 `deployment.rs` 增加小型 runtime value object `RetrySchedule`，字段使用明确单位：
   `base_delay_ms`、`max_delay_ms`、`backoff_multiplier`、`jitter_ratio`。
   `DeploymentConfig.retry_schedule` 使用 `Option<RetrySchedule>`；`None` 表示保持旧的 global fallback。
2. 在 `gateway_config.rs` 提取纯映射 helper，把 `ProviderConfig.max_retries` 之外的 nested retry
   schedule 写入每个 deployment。Provider factory 继续只负责 provider construction，不复制 router retry 逻辑。
3. 在 `execution.rs` 增加 schedule delay 计算：
   - exponent 使用 retry attempt 的饱和减法；
   - 先计算指数退避，再应用对称 jitter，最后用 `max_delay_ms` 硬截断；
   - production helper 从 `rand` 取得 `[-1, 1]` sample；内部纯函数接受显式 sample 供测试使用；
   - 现有 `calculate_retry_delay(&RouterConfig, ...)` 保留，作为无 deployment schedule 时的兼容路径。
4. 在 `RetryPolicy` 增加 `decide_for_deployment`，与现有 `decide` 共用一个私有决策核心。
   retry-after hint 仍优先；只有缺少 hint 时才选择 deployment schedule 或 global fallback。
5. 只替换“已经选中 deployment 且 operation 返回错误”的四类调用：core unary/capability 和 server
   unary/streaming-pre-output。选择 deployment 之前的 router 错误仍用 global policy。
6. 不改变外层 `num_retries`/`max_attempts`。本 issue 修复 nested schedule 的执行，不混入 provider-client
   retry 次数与 router retry budget 的架构收敛。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 config 不丢失 | `gateway_config.rs`, `deployment.rs` | pure mapping test asserts all four fields |
| P2/P3 formula + jitter/cap | `execution.rs` | deterministic sample tests for 250/500/900 and jitter bounds |
| P4 retry hint precedence | `retry_policy.rs` | existing retry-after test plus deployment schedule regression |
| P5 compatibility fallback | `retry_policy.rs`, `DeploymentConfig::default` | `None` schedule matches existing global delay |
| P6 safety stops unchanged | existing retry policy/route tests | focused router and server execution suites |
| P7 one policy | `retry_policy.rs`, four live call sites | source inspection + selected-deployment regression tests |
| P8 validation unchanged | `config_validators.rs` | existing retry validation suite |

## 数据流

`gateway.yaml ProviderConfig.retry` → validation → `gateway_config` mapping →
`DeploymentConfig.retry_schedule` → selected deployment failure → `RetryPolicy::decide_for_deployment` →
retry-after hint or computed `Duration` → existing `tokio::time::sleep`.

没有新增持久化、网络 API 或外部数据格式。Machine-facing YAML keys 保持不变。

## 备选方案

- 把第一个 provider 的 retry 写入 global `RouterConfig`：多 provider 配置会互相覆盖，拒绝。
- 把 nested retry 传进所有 provider builder：会形成 provider 内部 retry 与 router retry 的重复执行，拒绝。
- 直接复用 `config/models/retry.rs::RetryConfig`：其字段语义（bool jitter、retryable error list、内含 max retries）
  与现有 provider YAML 不同，会扩大兼容面，拒绝本 issue 内合并。
- 为 unary/streaming 各写一套 backoff：违反唯一 policy，拒绝。

## 风险

- Security: 无新增输入面；继续依赖现有严格配置校验。
- Compatibility: gateway-config deployment 的默认 retry delay 从旧 global 1s/2x/30s 转为已声明的 100ms/2x/5s/10% jitter；这是预期修复。程序化 deployment 保持旧行为。
- Performance: 每次实际 retry 增加一次轻量随机采样；无 retry 时无额外 I/O。
- Maintenance: 新 runtime value object 只表达 normalized schedule，公式仍唯一存在于 router execution/policy。

## 测试计划

- [ ] Unit tests: provider-to-deployment mapping、指数公式、cap、jitter sample、global fallback、hint precedence。
- [ ] Integration tests: core router 与 server selected-deployment retry helper 使用自定义 schedule。
- [ ] Manual verification: `rg` 确认四个 selected-deployment failure call site 使用 `decide_for_deployment`，selection-error call site 保留 `decide`。
- [ ] Deterministic checks: `cargo fmt --all -- --check`; `cargo check --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked`。

## 回滚方案

回滚 GH963 PR 即恢复 global retry delay。没有 schema 或数据迁移；移除 `retry_schedule` 与新 policy overload 后，
现有 `RetryPolicy::decide` 和 global `RouterConfig` 兼容路径仍在。
