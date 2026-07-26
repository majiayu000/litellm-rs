# Product Spec

## Linked Issue

GH-1128 / #1128

complexity: high

## 用户问题

输入 guardrail 当前只读取普通文本 content，忽略 document、tool result、
tool use input、tool-call arguments 和 message name。攻击者可以把同一提示注入或
敏感内容放进这些结构化字段，从而在 agent/tool 循环中绕过已配置的输入策略。

## 目标

- 覆盖聊天请求中所有由 Issue 明确列出的文本或结构化文本载体，以及同一公开
  DTO 中等价的 legacy function-call 载体。
- 对等价内容提供与普通文本相同的 guardrail 接受、修改或拒绝语义。
- 保持扫描输入确定、有限且可测试，不静默丢弃序列化失败。
- 兼容没有这些字段的现有请求。

## 非目标

- 下载 document URL、image URL 或其他远端资源后再扫描。
- 对图片、音频或任意二进制内容增加 OCR、转写或恶意软件检测。
- 改变 provider 请求转换、tool 执行或 guardrail provider 协议。
- 扩大到 Issue 未列出的响应输出面；流式输出由 GH-1127 处理。

## Behavior Invariants

1. B-001 普通文本、`Document.source`、`ToolResult.content`、`ToolUse.input`、`function_call`、`tool_calls[].function` 和 `ChatMessage.name` 都必须进入输入 guardrail。
2. B-002 多条 message、多段 content 和多个 tool call 必须按稳定顺序组合，并以明确边界分隔，不能因字符串直接拼接制造或消除敏感模式。
3. B-003 字符串字段保留原文本；结构化 JSON 字段使用确定性、完整的 JSON 表示，不得只扫描部分 key 或 value。文本型 document 必须扫描 base64 解码后的 UTF-8 正文，而不是无意义地只扫描编码字符串。
4. B-004 任一列入范围的字段包含被拒内容时，整个请求在调用 provider 前被拒绝。
5. B-005 guardrail 返回修改后文本时，不得把扁平扫描文本错误回写到原有多模态或 tool 结构；不支持安全回写的结构必须有明确契约而非静默改变请求含义。
6. B-006 无法构造完整扫描载荷时必须返回显式错误；禁止跳过失败字段后继续请求。
7. B-007 未列入范围的图片、音频和远端资源只保留现有行为，扫描过程不得发起网络访问；输入 guardrail 开启时，无法安全解码或不属于支持文本媒体类型的 document 必须 fail-closed，而不是作为“已扫描”放行。
8. B-008 没有配置输入 guardrail 时，请求结构和 provider 可见内容保持不变。

## 验收标准

- [ ] 每个列入范围的 content variant 都有接受与拒绝测试。
- [ ] modern/legacy function call 的 name/arguments、`ChatMessage.name` 各有独立覆盖。
- [ ] 文本 document fixture 证明扫描解码后正文；malformed base64、非 UTF-8 和不支持媒体类型 fail-closed。
- [ ] 多 message、多 content part、多 tool call 的稳定顺序与边界有测试。
- [ ] 嵌套 JSON、数组、空值、Unicode 和跨字段边界有测试。
- [ ] 测试证明 guardrail 拒绝发生在 provider 调用前。
- [ ] 测试证明扫描不会下载 URL 或改变未配置 guardrail 的请求。
- [ ] `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试及完整测试通过。

## 边界情况

- `ToolResult.content` 可能是字符串、对象、数组或空值。
- `ToolUse.input` 与 function arguments 可能包含深层嵌套 JSON。
- `Document.source.data` 是 base64；媒体类型可能是 `text/*`、JSON、PDF 或其他二进制。
- message name 或参数可能为空；空值不应制造虚假内容。
- 相邻字段的末尾和开头可能共同形成敏感词，边界策略必须固定并测试。
- guardrail provider 可能只支持纯文本修改，不能安全地重建任意结构化输入。

## 发布说明

这是安全收紧：过去可通过结构化 tool/document 字段发送的内容现在可能被已有
guardrail 拒绝。无需新增配置；未启用输入 guardrail 的部署保持兼容。
