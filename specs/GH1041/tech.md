# Tech Spec

## Linked Issue

GH-1041 / #1041

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Protocol DTOs | `src/core/mcp/protocol.rs` | initialization and nested capability structs inherit Rust snake_case field names | B-001 API-boundary mismatch |
| Connection lifecycle | `src/core/mcp/server.rs` | extracts optional `capabilities`, defaults parse failures, and marks Connected immediately | B-002/B-003/B-004/B-006/B-007 root cause |
| HTTP/SSE transport | `src/core/mcp/server.rs` | request helper always allocates an ID and always parses a JSON response | B-004/B-005 notification gap |
| Existing tests | `src/core/mcp/protocol.rs`, `src/core/mcp/server.rs` | basic message and registry tests; no initialization lifecycle coverage | Regression gap |

## 设计方案

1. 在 `protocol.rs` 定义唯一支持版本常量，并为 MCP boundary DTO 使用 `#[serde(rename_all = "camelCase")]`。复用现有 `ClientInfo` 的 name/version shape，新增 typed `InitializeResult`，要求 `protocol_version`、`capabilities`、`server_info` 全部存在且类型正确。
2. `initialize()` 将 success result 一次性反序列化为 `InitializeResult`。解析错误转换为包含上下文的 `ProtocolError`；协议版本必须精确匹配支持版本。
3. 把 HTTP/SSE 发送逻辑拆为共享的 typed message transport：request 仍分配 ID 并解析 JSON-RPC response；notification 使用 `JsonRpcRequest::notification()`，不分配 ID、不解析 response body，2xx/empty body 为成功。
4. 复用现有 HTTP error mapping，新增对其他非 2xx status 的显式 `TransportError`，避免错误页面被误报为 JSON parse error或 notification 成功。
5. `connect()` 的顺序固定为：state=Connecting → initialize request/validate → initialized notification → store capabilities → state=Connected。任一步骤错误进入 Failed；开始新连接前清空 capabilities，防止重连失败保留旧协商结果。
6. 使用 loopback HTTP mock 做真实报文与顺序断言；测试模块用 private-field access 注入允许 loopback 的 reqwest client，不放宽生产 SSRF client。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | protocol serde attributes | exact JSON serialization/deserialization unit tests |
| B-002 | typed `InitializeResult` parser | missing/malformed/empty result focused tests |
| B-003 | supported-version check | mismatched-version test returns ProtocolError |
| B-004 | notification send path + connect ordering | loopback server captures two ordered JSON messages and no notification id |
| B-005 | shared HTTP status mapping + notification body handling | empty 2xx notification succeeds; rejected notification returns typed error |
| B-006 | connect failure branch and cache ordering | parse/version/notification failures assert Failed + capabilities None |
| B-007 | complete loopback lifecycle | valid server data persisted only after both messages; state Connected |

## 数据流

`McpServer::connect` constructs typed initialize params → serde emits canonical MCP JSON → HTTP/SSE request receives JSON-RPC result → typed initialize parsing and version validation → notification transport emits `notifications/initialized` without ID → capabilities/state are committed together after success. Errors flow back as existing `McpError` variants without fallback values.

## 备选方案

- 继续使用 `Value::get` 并手工检查字段：容易遗漏 nested types，不能让 serde 集中执行 required-field validation，拒绝。
- 对 response version 自动回退到客户端版本：掩盖协商失败并违反 fail-closed 目标，拒绝。
- 让 notification 复用 request helper并忽略 response parse error：会错误携带 ID，且以静默吞错完成生命周期，拒绝。
- 为测试放宽生产 SSRF 规则：扩大安全边界且无必要，测试可在同模块注入 loopback client，拒绝。

## 风险

- Compatibility: 依赖当前错误 snake_case 或缺少 initialized notification 的非标准服务会从伪成功变为显式失败；这是协议修正。
- State correctness: capabilities 必须在 notification 成功后一次性发布，不能提前写入。
- Transport: notification 常见成功状态为空 body，不能调用 `.json()`；其他非 2xx 状态不能被接受。
- Security: production 构造仍必须使用 SSRF-safe client；loopback client 仅限 `#[cfg(test)]` 内部构造。

## 测试计划

- [ ] Protocol unit: initialize params 和 capability fields exact camelCase，snake_case absent。
- [ ] Parser unit: valid result、missing fields、wrong types、empty result、mismatched version。
- [ ] HTTP lifecycle: ordered initialize + initialized notification，ID presence/absence，empty success body。
- [ ] State failure: notification rejection produces Failed/None；valid lifecycle produces Connected/expected capabilities。
- [ ] Repository: `cargo fmt --all -- --check`、all-target/all-feature check、strict Clippy、full serial tests。

## 回滚方案

若 compatibility regression 被证实，保留 camelCase boundary 和 fail-closed validation，通过后续独立 issue 增加明确支持的协议版本或 transport compatibility；不得恢复默认 capabilities、跳过 initialized notification 或在失败后标记 Connected。
