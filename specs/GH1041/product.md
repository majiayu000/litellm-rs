# Product Spec

## Linked Issue

GH-1041 / #1041

complexity: medium

## 用户问题

MCP 客户端声明使用 `2024-11-05` 协议，但初始化报文使用错误的 snake_case 字段，且会把缺失或损坏的初始化结果静默当作空 capabilities。客户端也未在成功协商后发送规范要求的 `notifications/initialized`，却直接把服务标记为已连接。

结果是协议不兼容或初始化不完整时仍可能呈现“连接成功”，而遵循规范的服务端可能一直等待初始化完成通知。

## 目标

- 初始化 API 边界使用 MCP 规范的 camelCase 字段。
- 严格验证服务端返回的协议版本、capabilities 和 server info。
- 仅在有效响应及 `notifications/initialized` 发送成功后进入 `Connected`。
- 初始化任一步骤失败时显式返回错误，不保留未完成协商的 capabilities。

## 非目标

- 不增加 `2024-11-05` 以外的 MCP 协议版本支持。
- 不实现 Stdio 或 WebSocket transport。
- 不修改 tools/resources/prompts 的业务语义。
- 不在本 issue 扩展通用 JSON-RPC response ID 校验。

## Behavior Invariants

1. B-001 initialize request 必须发送 `protocolVersion`、`clientInfo`，不得发送对应 snake_case 字段；capability 子字段必须使用 `listChanged`。
2. B-002 initialize response 必须包含可解析的 `protocolVersion`、`capabilities`、`serverInfo`；缺失、类型错误或空响应必须返回 `McpError::ProtocolError`，不得生成默认 capabilities。
3. B-003 服务端返回的 `protocolVersion` 必须等于客户端唯一支持的 `2024-11-05`；其他版本必须显式失败。
4. B-004 有效 initialize response 后，客户端必须发送无 JSON-RPC ID 的 `notifications/initialized`，且该通知成功前不得进入 `Connected`。
5. B-005 HTTP 与 SSE-over-HTTP notification 的成功响应允许为空 body；认证、授权、限流、网络和非成功 HTTP 状态必须保持显式错误。
6. B-006 initialize 解析、版本校验或 initialized notification 任一步骤失败时，server state 必须为 `Failed`，capabilities 必须保持 `None`。
7. B-007 完整成功路径只按顺序发送 initialize request 和 initialized notification，随后保存服务端 capabilities 并进入 `Connected`。

## 验收标准

- [ ] 序列化测试证明 canonical camelCase 字段存在且对应 snake_case 字段不存在。
- [ ] 缺失/损坏 required fields、空响应及版本不匹配均 fail closed。
- [ ] HTTP 生命周期测试证明 initialize 在前、initialized notification 在后，且 notification 没有 `id`。
- [ ] notification 失败测试证明 state 为 `Failed` 且 capabilities 为 `None`。
- [ ] 成功测试证明 notification 成功后才保存 capabilities 并进入 `Connected`。
- [ ] MCP focused tests、格式、全特性编译、strict Clippy 和全量测试通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-002；required result fields 和空 JSON-RPC response 均显式失败。 |
| 错误与失败路径 | covered: B-002, B-003, B-005, B-006；协议与 transport 错误不降级。 |
| 授权/权限 | covered: B-005；HTTP 401/403 保持现有 typed error。 |
| 并发/竞态 | N/A；本 issue 不改变 connect 调用的并发协调策略。 |
| 重试/幂等 | covered: B-006；失败不缓存部分协商结果，后续显式 connect 可重试。 |
| 非法状态转换 | covered: B-004, B-006, B-007；只有完整握手可进入 Connected。 |
| 兼容/迁移 | covered: B-001, B-003；修正为仓库声明支持的 2024-11-05 规范，不新增版本。 |
| 降级/回退 | covered: B-002, B-003, B-005；所有无效响应和非成功 transport 状态 fail closed。 |
| 证据与审计完整性 | covered: B-001 至 B-007；focused mock 与全量验证共同覆盖。 |
| 取消/中断 | covered: B-006；中断导致 transport error 时不发布部分状态。 |

## 发布说明

MCP `2024-11-05` 初始化现在使用规范字段并完成 initialized notification；无效或不兼容的握手将显式失败，不再伪装为已连接。
