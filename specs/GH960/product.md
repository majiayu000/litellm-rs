# Product Spec

## Linked Issue

GH-960 / #960

## 用户问题

API-key 验证的数据库等基础设施故障当前被 `AuthSystem` 转换成失败的 `AuthResult`，随后 middleware 与 keys management 路由按无效凭证返回 401。部分路径还会把底层 `GatewayError` 文本拼进公开响应，使客户端无法区分凭证无效与认证服务故障，并可能获得数据库或缓存内部详情。

## 目标

- 保持真正无效、inactive、expired 或 owner-invalid API key 的稳定 401 语义。
- 让 API-key 验证返回的基础设施 `Err` 保持 typed error 边界，并由 HTTP 调用方映射为通用 500。
- middleware 与 keys management 认证路径使用同一个公开错误消息，且响应不包含底层错误字符串。
- 在服务端以 `error` 级别记录完整基础设施错误，保留运维诊断能力。

## 非目标

- 不改变 JWT 或 session 的验证与公开错误语义。
- 不改变 API-key active/expiry/owner 判定；#958 已定义 owner contract。
- 不改变 Redis snapshot 或 revoke 一致性；#959 已处理 lifecycle authority。
- 不改变 user 删除、外键或 owner provenance；该工作属于 #961。
- 不改变成功响应、公开 schema 或数据库 migration。

## Behavior Invariants

1. `P1`：`ApiKeyHandler::verify_key -> Ok(None)` 继续被 `AuthSystem` 表示为失败 `AuthResult`，公开 HTTP 状态为 401，消息保持 `Invalid API key`。
2. `P2`：`ApiKeyHandler::verify_key -> Err(error)` 必须从 `AuthSystem::authenticate` 传播为 `Err`；不得转换成 conclusively-invalid credential。
3. `P3`：auth middleware 收到该 `Err` 时释放预留资源、记录完整服务端错误，并返回 500 与固定通用消息；不得计为坏凭证或让重复 outage 触发 auth lockout，公开 body 不得包含数据库、Redis、SQL、连接串或原始 `GatewayError` 文本。
4. `P4`：keys management 的 direct authentication path 对同一 `Err` 使用相同固定消息和 500 状态，并只在服务端记录原始错误。
5. `P5`：固定通用消息只有一个 crate-internal 定义，避免 middleware 与 keys route 发生文案或状态语义漂移。
6. `P6`：OpenAI-compatible middleware 路径保留现有 OpenAI error envelope；非 OpenAI middleware 与 keys route 保留各自现有 envelope，但状态与公开消息一致。
7. `P7`：认证基础设施错误不得静默降级成 valid、invalid credential、ownerless 或 cache fallback。

## 验收标准

- [ ] API-key verifier 的基础设施错误从 `AuthSystem` 原样传播。
- [ ] invalid API key 继续返回 401 与稳定的无效凭证消息。
- [ ] middleware 与 keys route 都把基础设施错误映射为通用 500。
- [ ] 两个公共响应都使用共享消息且不包含内部错误详情。
- [ ] 重复认证基础设施故障保持 500，不被 auth failed-attempt limiter 转换成 429。
- [ ] 聚焦测试、格式、strict clippy、全量测试、scope/overlap guards 与 SpecRail gates 通过。

## 边界情况

- 当前 #959 路径不再读取 Redis，但数据库 key lookup、owner lookup 及未来认证依赖仍可能返回 `GatewayError`；统一按基础设施错误处理。
- HTTP 调用方必须先记录原始错误，再构造不接受内部详情参数的固定响应 helper。
- 本 issue 不把基础设施错误细分为 500/503；固定返回 500，避免未声明的可用性分类。

## 发布说明

无效 API key 仍返回 401。API-key 数据库等认证基础设施故障现在返回不含内部详情的通用 500，便于客户端安全地区分凭证问题与服务故障。
