# Product Spec

## Linked Issue

GH-957 / #957

## 用户问题

认证入口会以 `Debug` 格式记录 `AuthMethod`。当前 `AuthMethod` 直接派生 `Debug`，因此 JWT、API key
和 session identifier 的原始字符串可能进入应用日志、日志聚合系统或错误诊断附件。即使日志级别仅在
debug 环境启用，凭证一旦落盘就会扩大泄露面，并可能被具有日志读取权限的人重放。

## 目标

- 任何 `AuthMethod` 调试输出都不包含凭证原文或可重放片段。
- 日志仍能区分 JWT、API key、session 与无认证四种方法，保留必要的诊断价值。
- 任意使用 `AuthMethod` 调试输出的调用方都只能观察到稳定的 method kind 与固定脱敏值。

## 非目标

- 不修改 JWT、API key 或 session 的格式、解析和验证逻辑。
- 不改变认证成功、失败或 HTTP 状态码语义。
- 不引入凭证 hash、prefix、长度或其他可能形成侧信道的日志标识。
- 不处理 API-key owner、revocation cache 或认证基础设施错误；这些分别由 #958、#959、#960 跟踪。

## Behavior Invariants

1. `AuthMethod::Jwt` 对任意输入都产生同一个固定输出 `Jwt("[REDACTED]")`。
2. `AuthMethod::ApiKey` 对任意输入都产生同一个固定输出 `ApiKey("[REDACTED]")`。
3. `AuthMethod::Session` 对任意输入都产生同一个固定输出 `Session("[REDACTED]")`。
4. 脱敏输出仍能稳定区分 `Jwt`、`ApiKey`、`Session` 与 `None`。
5. 所有携带凭证的变体使用同一固定占位符 `[REDACTED]`；输出不包含 prefix、suffix、hash 或长度。
6. 认证入口保持当前日志事件和认证行为，仅改变敏感字段的格式化结果。

## 验收标准

- [ ] JWT、API key、session 三种携带凭证的变体均精确输出对应 method kind 与固定 `[REDACTED]`。
- [ ] 每个变体至少用两个不同输入证明输出完全相同，不依赖 secret 内容、长度或字符集。
- [ ] `AuthMethod::None` 仍可安全格式化并可识别。
- [ ] 任何日志调用都不能绕过安全 formatter 观察 `AuthMethod` 的内部凭证字段。
- [ ] 认证成功、失败、方法选择与 HTTP 行为保持不变；唯一可观察变化是 `AuthMethod` 调试文本被脱敏。

## 边界情况

- 空字符串、Unicode、换行符与类似日志控制字符的凭证都必须得到相同固定输出。
- 凭证内容恰好为 `[REDACTED]` 时，输出仍与所有其他输入完全相同，不能观察到输入差异。
- 其他直接输出 session identifier 的日志已独立拆到 #969，不属于本 issue 的完成声明。

## 发布说明

`AuthMethod` 调试输出不再包含 JWT、API key 或 session identifier 原文；认证协议与客户端行为不变。
