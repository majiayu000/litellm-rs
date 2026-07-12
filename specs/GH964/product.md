# Product Spec

## Linked Issue

GH-964 / #964

## 用户问题

网关接受并校验每个 provider 的 `health_check.interval`、`failure_threshold`、
`recovery_timeout`、`endpoint` 和 `expected_codes`，但 live router 不调度这些检查，
也不根据结果更新 deployment 健康状态。用户配置看似有效，实际路由行为完全不变。

## 目标

- 让 gateway provider 配置创建的 deployment 参与真实、持续的健康探测。
- 让五个公开字段都具有明确、可测试的运行时或拒绝语义。
- 让探测结果直接影响 live router 的 deployment 可选状态。
- 保持 provider-native health check、请求失败 circuit breaker 与程序化 Router 的兼容边界。

## 非目标

- 不接入当前使用随机模拟结果的 `core::health::HealthMonitor`。
- 不重写各 provider 已有的 native `health_check()` 实现。
- 不为自定义 health endpoint 推断或注入 provider 密钥、签名或私有请求头。
- 不实现 gateway 配置热重载后的动态 probe 重建。
- 不改变请求错误触发的全局 circuit-breaker 阈值与 cooldown 配置。

## Behavior Invariants

1. 当全局 `router.load_balancer.health_check_enabled=true` 且 provider health 配置偏离 schema
   默认值时，该 `ProviderConfig` 必须启动一个 live probe loop；任一公开 health 字段 override
   都会激活 policy。同一 provider 的多个 model deployment 共享一次 probe 结果，不得重复发起
   每模型探测。完全默认的 health 配置保持既有无主动流量行为。
2. probe loop 启动后立即执行首次检查，之后按该 provider 的 `interval` 秒调度；关闭全局
   health check 时不得启动后台 probe。
3. active policy 的 `endpoint=None` 时必须调用该 provider 现有的 native `health_check()`；完全
   默认的 gateway health 配置以及程序化构造且没有 provider health policy 的 deployment 不得自动
   启动 probe。
4. 配置 `endpoint` 时，绝对 URL 或基于显式 `base_url` 解析出的相对 path 必须执行无认证
   HTTP GET；只有响应状态位于 `expected_codes` 时才算成功。相对 endpoint 缺少 `base_url`、
   不安全 URL、空 endpoint 或非法 status code 必须在配置校验阶段拒绝。probe 不跟随 redirect，
   因此配置的 3xx expected code 必须按原始响应判断，且不得请求 `Location` target。
5. 未配置 `endpoint` 时只允许默认 `expected_codes=[200]`；自定义 expected codes 必须同时
   配置 endpoint，避免接受后忽略。partial `health_check:` 配置省略 expected codes 时仍默认为
   `[200]`。
6. 连续失败数小于 `failure_threshold` 时，相关 deployment 变为 `Degraded`；达到阈值时变为
   `Unhealthy` 并从路由选择中排除。一次成功 probe 必须清零连续失败计数。
7. deployment 达到失败阈值后，下一次 probe 必须等待 `recovery_timeout` 秒；恢复窗口后的成功
   probe 将其恢复为 `Healthy`，失败则保持 `Unhealthy` 并再次等待恢复窗口。
8. health probe 不得覆盖仍有效的请求级 `Cooldown`。并发中的请求失败也不得把 probe 已标记的
   `Unhealthy` deployment 降级回可路由的 `Degraded`。
9. provider probe task 必须随 Router drop 被取消；probe 错误不得 panic、泄漏密钥或静默停止
   整个 loop。
10. provider 配置在进入 runtime mapping 前必须再次通过现有 `Validate` 契约，直接调用
    `Router::from_gateway_config` 也不能绕过字段校验。

## 验收标准

- [ ] 五个 per-provider health-check 字段均有 live consumer 或显式拒绝规则。
- [ ] 自定义 interval/threshold/recovery 改变 probe 调度和 deployment 状态迁移。
- [ ] provider-native 与自定义 endpoint 两条 probe 路径都有确定性测试。
- [ ] 多模型 provider 只运行一个 probe，结果同步到其全部 deployment。
- [ ] 全局关闭、程序化 deployment、active cooldown 与 Router drop 的兼容行为有测试。
- [ ] 格式、严格 clippy、全量测试与 SpecRail packet 校验通过。

## 边界情况

- `failure_threshold=1` 时首次失败立即标记 `Unhealthy`。
- `expected_codes` 可以包含多个不同的合法 HTTP 状态，但不得为空、重复或超出 `100..=599`。
- endpoint 返回 3xx 时直接按该状态判断，不跟随 redirect。
- endpoint 返回非预期状态、连接失败或超过 deployment timeout 都按一次失败处理。
- provider-native 返回 `Degraded`、`Unhealthy` 或 `Unknown` 都按失败处理。
- probe 执行时间不计入 interval；下一次延迟从本次结果处理完成后开始。

## 发布说明

这是修复“自定义配置接受但不生效”的行为变更。默认开启 health check 且 provider 配置了任一
health override 的 gateway 会开始主动探测；完全默认配置不新增上游流量。自定义 endpoint 是
无认证 GET，需认证的 provider 应保留 `endpoint=None` 并通过 interval/threshold/recovery override
激活 native health check。程序化 Router 不受影响，除非 deployment 显式携带 health policy 并启动检查。
