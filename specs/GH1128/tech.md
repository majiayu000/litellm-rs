# Tech Spec

## Linked Issue

GH-1128 / #1128

## Product Spec

见 `product.md`（B-001 ～ B-008）。

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Guardrail adapter | `src/server/guardrails.rs` | `content_text` 只借用普通 text part | 根因与唯一 enforcement boundary |
| Chat DTO | `src/core/models/openai/messages.rs` | `ContentPart` 包含 document/tool result/tool use | 列出必须规范化的载体 |
| Function DTO | `src/core/models/openai/tools.rs` | legacy/modern function arguments 都会发给 provider | 等价结构化输入不能继续遗漏 |
| Guardrail engine | `src/core/guardrails/*` | 接收 `&str`，mask 在 gateway boundary fail-closed | 新适配器必须保留错误语义 |

## 设计方案

1. 用一个返回 owned `Result<Vec<String>, GatewayError>` 的规范化函数替代
   `content_text -> Vec<&str>`。owned 结果允许安全容纳 JSON 序列化和 base64 解码
   后的正文，错误不能被 iterator/filter 静默丢弃。
2. 每条 message 以带稳定标签的片段加入扫描载荷：message name、普通 content、
   legacy `function_call` name/arguments、modern `tool_calls[].function`
   name/arguments。标签与换行分隔避免字段直接拼接，迭代顺序严格跟随请求顺序。
3. `ToolResult.content` 与 `ToolUse.input` 使用 `serde_json::to_string` 的完整 JSON
   表示；同时扫描 `ToolUse.name`。序列化错误映射为显式 gateway internal error。
4. `Document.source.data` 先按 base64 解码。仅接受明确文本媒体类型：
   `text/*`、`application/json`、`application/*+json`、`application/xml` 和
   `application/*+xml`；解码结果必须是 UTF-8。malformed base64、非 UTF-8 或
   其他媒体类型在 input guardrail 开启且 `check_input` 为 true 时 fail-closed。
   不读取 URL、不解析 PDF/Office、不扫描 image/audio base64。
5. 新规范化只改变传给 guardrail 的扫描字符串，绝不重写原 DTO。现有
   `GuardrailAction::Mask`/modified 结果继续由 `enforce` 显式失败，避免把扁平文本
   错误回写到结构。
6. 在 `check_input` 中先构造全部载荷，成功后调用一次 engine；任何字段失败都在
   provider 选择/调用前结束。engine disabled 或 `check_input: false` 时可直接
   保持现有无检查行为，避免因未启用策略拒绝二进制 document。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | message/function/content 规范化 | 每种载体独立 blocked fixture |
| B-002/B-003 | labelled fragment builder | 顺序、边界、嵌套 JSON 与 Unicode snapshot |
| B-004 | `check_input` 调用顺序 | mock provider 未被调用 |
| B-005 | `enforce` modified 分支 | mask 仍 fail-closed 且请求 DTO 未改变 |
| B-006 | fallible builder | malformed document/serialization 错误不调用 engine/provider 后续 |
| B-007 | document media gate | 文本正文解码；PDF/image/audio/URL 无网络且 fail-closed/保持范围 |
| B-008 | disabled fast path | 同一多模态请求在未启用 guardrail 时保持兼容 |

## 数据流

`ChatCompletionRequest.messages` 按顺序进入 fallible fragment builder；文本字段原样
复制，JSON 字段确定性序列化，允许的 document base64 解码为 UTF-8。片段带标签与
边界拼成单个扫描载荷，交给现有 `GuardrailEngine::check_input`，再由 `enforce`
统一映射 allow/block/modified/error。原请求对象始终不变。

## 备选方案

- 对整个 `ChatCompletionRequest` 直接 JSON 序列化：拒绝，因为会扫描 image/audio
  base64、混入无关配置，并且 document 正文仍是编码数据。
- 只扫描 JSON/base64 原字符串：拒绝，因为不能识别 document 解码后的自然语言。
- 对不支持 document 类型放行：拒绝，因为输入 guardrail 会继续存在公开 bypass。
- 自动提取 PDF/Office：超出范围且会新增复杂解析/资源消耗面。

## 风险

- Security: 支持媒体类型列表必须 fail-closed，不能被 MIME 大小写/参数绕过。
- Compatibility: 启用 input guardrail 的二进制 document 将被拒绝；发布说明需明确。
- Performance: 文档解码和 owned 载荷增加内存；沿用请求大小限制并避免重复复制。
- Maintenance: DTO 新增文本载体时应在 exhaustiveness test 中显式分类。

## 测试计划

- [ ] Unit tests: 全 variant、legacy/modern function、嵌套 JSON、标签边界、Unicode。
- [ ] Document tests: textual MIME、`+json/+xml`、MIME 参数、bad base64、bad UTF-8、PDF。
- [ ] Integration tests: blocked 发生在 provider 前，disabled 配置保持 DTO。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试、`cargo test`。

## 回滚方案

回滚规范化函数与测试即可，无持久化迁移。若二进制 document 兼容性需要恢复，
必须另行设计可审核的解析器或按端点禁用 input guardrail；不得重新静默放行。
