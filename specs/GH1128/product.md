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

1. B-001 普通文本、`Document.source`、`ToolResult.content`、`ToolUse.name/input`、message-level legacy `function_call`、`tool_calls[].function`、request-level `ChatCompletionRequest.function_call` 和 `ChatMessage.name` 都必须进入输入 guardrail。本仓库当前 request-level compatibility DTO 复用 `FunctionCall { name, arguments }` 并把两者序列化转发给 provider；即使标准 OpenAI selection 语义通常只指定 name，该已接受并转发的 `arguments` 仍属于必须扫描的现有公开输入面。
2. B-002 多条 message、多段 content 和多个 tool call 必须按稳定顺序处理。语义上连续的 message content parts 必须保留原邻接关系以检测跨 part 拆词；message/function/tool 等独立字段必须作为独立审核记录通过一次 engine batch 调用处理，由各 guardrail 在 batch 内逐 record 隔离检查，不能用可被规则误命中的标签或分隔符拼成一个扫描字符串，也不能允许正则跨独立记录匹配。
3. B-003 字符串字段保留原文本；结构化 JSON 字段同时扫描确定性完整 JSON 表示和递归解码后的 string keys/values，不得只扫描转义后的 `\n`/`\uXXXX` 或部分节点。合法 JSON function arguments 使用同一语义遍历，非 JSON arguments 仍扫描原字符串。文本型 document 必须扫描 base64 解码后的 UTF-8 正文，而不是编码字符串；JSON MIME document 还必须同时扫描原始正文和递归解码后的 string keys/values，声明为 JSON 但语法错误时稳定 fail-closed。
4. B-004 任一列入范围的字段包含被拒内容时，整个请求在调用 provider 前被拒绝。
5. B-005 guardrail 返回修改后文本时，不得把扁平扫描文本错误回写到原有多模态或 tool 结构；不支持安全回写的结构必须有明确契约而非静默改变请求含义。
6. B-006 无法构造完整扫描载荷时必须返回安全、稳定的 HTTP 400 `invalid_request_error` / `invalid_request`；错误消息不得包含 document 正文、arguments、命中规则或 provider secret，禁止跳过失败字段后继续请求。
7. B-007 未列入范围的图片、音频和远端资源只保留现有行为，扫描过程不得发起网络访问；输入 guardrail 开启时，无法安全解码或不属于支持文本媒体类型的 document 必须 fail-closed，而不是作为“已扫描”放行。
8. B-008 没有配置输入 guardrail 时，请求结构和 provider 可见内容保持不变。
9. B-009 规范化产生的审核记录最多 256 条，所有派生扫描值的 UTF-8 字节总和最多
   2 MiB；超过任一上限必须在外部 guardrail/provider 调用前返回稳定安全的 HTTP 400。
   engine 必须以一个 batch 契约处理全部 records；内置 OpenAI moderation 沿用
   现有 `trim().is_empty()` eligibility，一批 trim 后非空文本最多发起一次远程
   请求，全空白 batch 不发起远程请求，不能让 JSON 节点数线性放大外部请求次数。
   moderation 响应条目数必须与实际提交的 eligible records 数完全一致；不一致是不可被
   `fail_open` 覆盖的完整性失败，必须阻止 provider 调用。
   `GuardrailAction::Log` 命中仍按现有契约记录并继续，不能因 batch 聚合被升级为阻断。

## 验收标准

- [ ] 每个列入范围的 content variant 都有接受与拒绝测试。
- [ ] modern/message-level legacy function call、以及本仓库 request-level compatibility `FunctionCall` 的 name/arguments、`ChatMessage.name` 各有独立覆盖；测试证明 request-level arguments 确实从当前 DTO 解析并在 provider boundary 前被审核。
- [ ] 文本 document fixture 证明扫描解码后正文；malformed base64、非 UTF-8 和不支持媒体类型 fail-closed。
- [ ] 多 message、多 content part、多 tool call 的稳定顺序、独立记录隔离与跨 content part 拆词有测试。
- [ ] 嵌套 JSON、数组、空值、Unicode escape、解码后的 string key/value、合法/非法 JSON arguments，以及 JSON MIME document 的 raw/semantic/invalid 三类行为有测试。
- [ ] 测试证明 guardrail 拒绝发生在 provider 调用前。
- [ ] 测试证明扫描不会下载 URL；`enabled: false` 与 `check_input: false` 不增加 guardrail-specific document 拒绝或改变请求。既有 request validator 仍会无条件拒绝 malformed base64，因此 disabled fixture 使用 baseline 可接受的合法 base64，并分别覆盖 unsupported MIME、非 UTF-8 或 invalid JSON 等只属于 guardrail 的检查。
- [ ] 256/2 MiB 边界内允许，越界稳定 400 且外部调用计数为 0；多 record 的
  OpenAI moderation mock 只收到一次 batch 请求并逐索引合并结果；结果数量不足/
  过多在 `fail_open: false/true` 下都固定失败且 provider 调用为 0；mixed whitespace
  只提交 trim 后非空值、全 whitespace zero-call，`Log` action 保持非阻断。
- [ ] `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试及完整测试通过。

## 边界情况

- `ToolResult.content` 可能是字符串、对象、数组或空值。
- `ToolUse.input` 与 function arguments 可能包含深层嵌套 JSON。
- `Document.source.data` 是 base64；媒体类型可能是 `text/*`、JSON、PDF 或其他二进制。
- message name 或参数可能为空；空值不应制造虚假内容。
- 相邻 content parts 的末尾和开头可能共同形成敏感词，连续视图必须保留该邻接；独立字段逐条审核，不共享正则匹配边界。
- guardrail provider 可能只支持纯文本修改，不能安全地重建任意结构化输入。
- 攻击者可能用大量短 JSON keys/values 放大 records；固定记录数/派生字节上限必须
  在任何外部审核前一次性验证。

## 发布说明

这是安全收紧：过去可通过结构化 tool/document 字段发送的内容现在可能被已有
guardrail 拒绝。启用输入 guardrail 时，超过 256 个扫描 records 或 2 MiB 派生扫描
文本的请求也会被拒绝。无需新增配置；未启用输入 guardrail 的部署保持兼容。
