# Product Spec

## Linked Issue

GH-963 / #963

## 用户问题

网关接受并校验每个 provider 的 `retry.base_delay`、`retry.max_delay`、
`retry.backoff_multiplier` 和 `retry.jitter`，但 live router 重试仍使用全局固定退避。
用户以为配置已生效，实际请求延迟和退避节奏却与配置不同，形成声明与执行不一致。

## 目标

- 让每个由 gateway provider 配置创建的 deployment 携带其 retry schedule。
- 让 unary 请求和首个 chunk 前的 streaming 请求都通过现有 `RetryPolicy` 使用选中 deployment 的 schedule。
- 用确定性测试证明自定义 base delay、multiplier、cap 和关闭 jitter 后的准确行为。
- 保持 retry hint、非重试错误、budget fallback、deadline 和已输出 streaming 数据的现有安全语义。

## 非目标

- 不统一或删除 provider 内部 HTTP client 的 retry 配置。
- 不改变现有 router 总尝试次数或 `ProviderConfig.max_retries` 的 provider-client 语义。
- 不实现 #964 的 health-check runtime wiring。
- 不重构 Router、provider factory 或 retry 配置的全部历史类型。

## Behavior Invariants

1. 通过 `GatewayConfig.providers` 创建的每个 deployment 都保留对应 provider 的四个 nested retry 字段，任何字段都不能在 factory/router 边界被静默丢弃。
2. 选中 deployment 发生可重试错误时，第 1 次 retry 使用 `base_delay`；第 N 次 retry 使用 `base_delay * backoff_multiplier^(N-1)`，结果不得超过 `max_delay`。
3. `jitter` 表示相对抖动比例；`0.0` 必须产生确定性 delay，非零值只能在未抖动 delay 的 `±jitter` 范围内变化，最终仍受 `max_delay` 硬上限约束。
4. 上游明确返回 retry-after hint 时，hint 继续优先于配置 schedule。
5. deployment 没有 provider schedule（例如程序化构造的现有 deployment）时，继续使用现有 `RouterConfig` 全局退避，避免改变 SDK/测试构造行为。
6. deployment 选择失败、非重试错误、budget/unpriced fallback、deadline 不足，以及 streaming 已输出 chunk 后失败的行为保持不变。
7. unary 与 streaming 首包前路径必须调用同一个 `RetryPolicy` 决策实现，不允许各自复制 backoff 公式。
8. 现有配置校验继续拒绝 zero delay、base 大于 max、非正 multiplier 和范围外 jitter。

## 验收标准

- [ ] provider nested retry 配置映射到 deployment runtime schedule。
- [ ] core router 与 server unary/streaming 的选中 deployment 失败路径都消费该 schedule。
- [ ] 测试证明自定义 `base_delay=250`、`backoff_multiplier=2`、`max_delay=900` 产生 `250/500/900` 毫秒退避。
- [ ] 测试证明 `jitter=0` 完全确定，非零 jitter 不突破比例范围和 `max_delay`。
- [ ] retry-after hint 与现有安全停止条件的测试保持通过。
- [ ] `cargo fmt --all -- --check`、严格 clippy、全量测试和 SpecRail packet 校验通过。

## 边界情况

- 极大 attempt 不能发生整数溢出或 panic；计算结果必须被 `max_delay` 截断。
- `backoff_multiplier < 1.0` 按已通过校验的配置执行递减退避。
- `jitter=1.0` 允许 delay 降到零，但不能超过 `max_delay`。
- 多 deployment 重试时，每次失败使用实际被选中 deployment 的 schedule。

## 发布说明

这是修复“配置接受但不生效”的行为变更。Gateway YAML 未显式设置 nested retry 时会启用其既有默认值
（100ms base、5s cap、2x、10% jitter）；程序化构造且未设置 deployment schedule 的调用仍保持旧的全局退避。
