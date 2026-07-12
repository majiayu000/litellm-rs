# Product Spec

## Linked Issue

GH-969 / #969

complexity: small

## 用户问题

认证与 OAuth 路径的三条日志直接输出 session identifier。该值可能是可重放 bearer credential，或至少是
可跨请求关联用户会话的敏感标识；一旦进入本地日志、聚合平台或诊断附件，就扩大凭证与隐私泄漏面。

## 目标

- 已知的 logout、invalid/expired session、session deletion 日志保留事件与结果，但不包含 session 值。
- 日志不以 prefix、suffix、hash、长度或其他输入派生值替代原文。
- 仓库 CI 能确定性阻断生产 Rust 日志重新直接引用 session credential 变量。
- session 验证、删除、HTTP 响应和 OAuth 协议行为保持不变。

## 非目标

- 不修改 `AuthMethod` 的 `Debug`；该边界已由 #957 关闭。
- 不修改 session token/ID 格式、解析、存储、过期、撤销或删除协议。
- 不移除 OAuth redirect/response 协议中客户端必须接收的 session 字段。
- 不把 email、path、provider、user ID 或错误对象一并定义为本 issue 的 session credential。

## Behavior Invariants

1. B-001 当普通 logout 成功解析出 session ID 时，日志只能表达“session 已失效”事件，不能包含该 ID 或
   任何由该 ID 派生的值。
2. B-002 当受保护 OAuth 请求携带的 session 无效或已过期时，warn 日志只能表达 invalid/expired 结果，
   不能包含 `sid` 或其派生值；原有 unauthorized 行为保持不变。
3. B-003 当 OAuth logout 成功删除 session 时，debug 日志只能表达删除成功，不能包含 `sid` 或其派生值；
   删除失败日志与 HTTP 成功响应语义保持不变。
4. B-004 三个日志点都不得输出 secret 的 prefix、suffix、hash、长度或其他可关联替代标识；事件类型和
   成功/失败状态是唯一保留信息。
5. B-005 session 验证、session store 调用、logout control flow、错误映射、HTTP status/body 与 redirect
   字段保持不变；唯一运行时变化是日志文本移除输入派生数据。
6. B-006 PR 与 main CI 必须运行 fail-closed source guard：任何生产 Rust log macro 在同一调用中直接引用
   `session_id`、`session_token` 或 `sid` 时，guard 以非零状态退出并报告命中；字符串中的分号、tail/match-arm
   表达式、嵌套调用或多行调用都不能绕过，缺少 `rg` 或 `python3` 也必须失败。
7. B-007 source guard 对 session identifier 使用独立的零基线；raw-body baseline override 不能放行 session
   credential 命中，正常 email/path/provider/error/protocol 字段不得被误判为该规则命中。

## 验收标准

- [ ] 三个已知日志点均只输出静态事件/结果文本，且没有 session 输入派生字段。
- [ ] 变更前 guard 精确报告 3 个 session identifier 日志命中并失败；变更后报告 0 并成功。
- [ ] PR CI 和 main full CI 都执行 `scripts/guards/check_log_pii.sh`。
- [ ] 全仓 auth/server 日志审计对每个 session 相关命中给出 credential/protocol/metadata/error disposition。
- [ ] 全量格式、编译、strict Clippy 和测试通过，认证与 logout 行为无回归。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-001, B-002, B-003；没有 session 时原控制流不新增日志值。 |
| 错误与失败路径 | covered: B-002, B-003, B-005；unauthorized、lookup/delete error 语义保持。 |
| 授权/权限 | covered: B-002, B-005；require-auth 判定不变，日志脱敏与权限结果正交。 |
| 并发/竞态 | N/A；静态日志文本与 source guard 无共享可变状态，session store 并发语义不变。 |
| 重试/幂等 | covered: B-004, B-005；重复事件仍只输出相同静态结果，不产生关联标识。 |
| 非法状态转换 | N/A；不新增或改变 session 状态转换。 |
| 兼容/迁移 | covered: B-005；无数据/API 迁移，仅日志字段减少。 |
| 降级/回退 | covered: B-005, B-006；guard 或工具缺失不得静默放行，运行时无新 fallback。 |
| 证据与审计完整性 | covered: B-006, B-007；红绿 guard、独立基线与 CI wiring 缺一即未完成。 |
| 取消/中断 | N/A；日志调用与离线 source guard 均可安全重试，不保存部分状态。 |

## 发布说明

认证与 OAuth session 事件日志不再包含 session token/ID；客户端协议、HTTP 行为和 session 生命周期不变。
