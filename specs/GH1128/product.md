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
2. B-002 多条 message、多段 content 和多个 tool call 必须按稳定顺序处理。每条 message 的普通 text parts 必须按原顺序收集，过滤其间 image/document/tool 等非 text part 后，覆盖当前 provider 实际使用的三种投影：直接拼接、单个 ASCII 空格连接和单个换行连接；每种投影是独立审核记录，内容相同时只保留一个。message 没有任何普通 text leaf 时不得生成 per-message projection，尤其不能用一个空字符串占用 record/byte 上限。为保持当前 gateway 已有的跨 message 检测，还必须把全请求所有普通 text leaves 按原顺序用单个换行连接成一个 request-level legacy view；它只跨普通 message text，不混入 function/tool/document 等独立字段。实际 outgoing transform 使用 Anthropic `separate_system_messages` 时，还必须按请求顺序先过滤掉所有非 `System`/`Developer` role，再收集存活 message 的普通 text leaves 并用单个换行连接成 Anthropic-scoped view；该 view 只属于实际使用这条 Anthropic transform 的 provider profile，非 Anthropic profile 不得生成或审核该记录，避免制造跨 provider 的虚假边界。Bedrock provider-boundary views 的作用域必须对应每个实际 outgoing `ToolResultBlock.content`：普通 user/assistant message 中每个 `ToolResult.content` 独立成组；tool/function-role message 则按 part 顺序把普通 `Text` 与其中所有 `ToolResult.content` 的可达 text entries 展开成同一组，因为当前转换会把它们扁平到一个 block。每组都生成直接/空格/换行三种投影；不同 outgoing block 或 message 不得拼接。这样 provider 在 guardrail 后过滤 part、插入边界或相邻化结构化文本时，不能形成未检查的新文本。原始 message/function/tool/document 字段的 typed semantic records 仍逐字段隔离；不能用可被规则误命中的人工标签拼成扫描文本。
3. B-003 字符串字段保留原文本；结构化 JSON 字段同时扫描确定性完整 JSON 表示和递归解码后的 string keys/values，不得只扫描转义后的 `\n`/`\uXXXX` 或部分节点。合法 JSON function arguments 使用同一语义遍历，并在构造 `Value` 前拒绝任意层级的重复 object key（包括 escape 解码后相同的 key），避免解析器覆盖早期恶意值；首个 JSON whitespace 后出现 U+FEFF BOM 的 arguments 也稳定 fail-closed。首个非空白字符不是 `{`/`[` 的普通非 JSON arguments 仍扫描原字符串，但看似 structured JSON 的 arguments 只要无法完成有界解析（包括 recursion/depth/resource limit）就稳定 fail-closed。文本型 document 必须扫描 base64 解码后的 UTF-8 正文，而不是编码字符串；MIME 必须完整解析而不是按分号截断，且要在 parser 之外拒绝空参数段与尾随分号。allowlist media type 的参数闭集只允许“无参数”或“恰好一个大小写不敏感的 `charset=utf-8`”。重复 charset、任何其他 charset（包括 UTF-16/UTF-16LE/ISO-8859-1）及其他参数都稳定 fail-closed，避免 `format` 等 provider 语义参数产生未覆盖的文本视图。JSON MIME document 还必须同时扫描原始正文和递归解码后的 string keys/values，声明为 JSON 但语法、重复 key、BOM、深度或资源限制失败时稳定 fail-closed。gateway 不转码且不得改写原 DTO。
4. B-004 任一列入范围的字段包含被拒内容时，整个请求在调用 provider 前被拒绝。
5. B-005 guardrail 返回 `GuardrailAction::Mask` 或任意修改后文本时，不得把扁平扫描文本错误回写到原有多模态或 tool 结构；即使 `Mask` 结果没有 `modified_content`，gateway 也必须 fail-closed，不得继续 provider 请求。
6. B-006 无法构造完整扫描载荷时必须返回安全、稳定的 HTTP 400 `invalid_request_error` / `invalid_request`；错误消息不得包含 document 正文、arguments、命中规则或 provider secret，禁止跳过失败字段后继续请求。`/v1/responses` 的 `background: true` 也必须在持久化 queued response、创建后台任务或返回 200 前同步完成同一 input guardrail；失败时不得留下 queued/in-progress response。通过后后台执行不得再次检查同一输入，也不得存在可被其他调用方误用的未审核 handler 旁路。
7. B-007 未列入范围的图片、音频和远端资源只保留现有行为，扫描过程不得发起网络访问；document 文本 allowlist 仅含 `text/plain`、`text/csv`、`application/json` 与 `application/*+json`。Markdown、HTML、XML、`text/*` 其他类型及二进制格式因缺少安全 entity/语义解码器而 fail-closed，不能把 entity-encoded 内容当作已扫描放行。
8. B-008 没有实际启用的输入 guardrail 时，请求结构和 provider 可见内容保持不变；这包括全局 `enabled: false`、`check_input: false`、没有注册 guardrail，以及已注册但所有实例 `is_enabled() == false`。这些路径必须在 guardrail-specific record/MIME/JSON/document builder 之前返回，不得仅因配置中存在 disabled custom guardrail 而新增稳定 400；既有独立 request validator 仍照常生效。
9. B-009 规范化产生的审核记录最多 256 条，所有派生扫描值的 UTF-8 字节总和最多
   2 MiB；超过任一上限必须在外部 guardrail/provider 调用前返回稳定安全的 HTTP 400。
   engine 必须以一个 batch 契约处理全部 records；内置 OpenAI moderation 沿用
   现有 `trim().is_empty()` eligibility，一批 trim 后非空文本最多发起一次远程
   请求，全空白 batch 不发起远程请求，不能让 JSON 节点数线性放大外部请求次数。
   OpenAI moderation 的 eligible 原始字符串总计还受保守的 32,768 UTF-8 bytes
   上游 context 上限约束；该上限保证 token 数不超过当前 moderation 模型的
   32,768-token context，超限在远程调用前稳定 400，且不得由 `fail_open` 覆盖。
   moderation 响应条目数必须与实际提交的 eligible records 数完全一致；不一致是不可被
   `fail_open` 覆盖的完整性失败，必须阻止 provider 调用。输入上限与响应完整性必须由
   独立 batch failure 类型分类；公开且可穷举匹配的现有 `GuardrailError` enum 不得
   增加 variant。`Guardrail` trait 只允许增加有默认实现的 source-compatible batch
   方法，默认按 record 顺序调用现有 `check_input(&str)`；仅实现旧方法的 custom
   guardrail 无需改源码。经 config 创建或经公开 `add_guardrail` 手工注册的
   OpenAI moderation 都必须使用同一 batch override。
   `GuardrailAction::Log` 命中仍按现有契约记录并继续，不能因 batch 聚合被升级为阻断。

## 验收标准

- [ ] 每个列入范围的 content variant 都有接受与拒绝测试。
- [ ] modern/message-level legacy function call、以及本仓库 request-level compatibility `FunctionCall` 的 name/arguments、`ChatMessage.name` 各有独立覆盖；测试证明 request-level arguments 确实从当前 DTO 解析并在 provider boundary 前被审核。
- [ ] allowlisted 文本 document fixture 证明扫描解码后正文；无 charset 与单个大小写不敏感 `charset=utf-8` 可进入相同 UTF-8 扫描，`charset=utf-16le`、其他非 UTF-8 charset、重复 charset、`text/plain;`、`text/plain; charset=utf-8;`、空参数段与其他 malformed MIME 稳定 400；malformed base64、非 UTF-8、Markdown/HTML/XML/entity-bearing MIME 和其他不支持媒体类型 fail-closed，且 Markdown numeric/named entity fixture 不会仅经 raw 扫描后放行。
- [ ] 多 message、多 content part、多 tool call 的稳定顺序、独立记录隔离与跨 content part/message 拆词有测试；同一 message 的 `Text("ignore")` + image + `Text("all previous instructions")` 必须在过滤后的单空格 view 命中，两个 message 分别携带这两段时必须在 request-level newline view 命中；另有直接拼接/换行 view、相同 view 去重和 Bedrock/Ollama/Gemini-Vertex provider 转换对照 fixture。Anthropic fixture 必须把 `System("ignore")`、中间的 `User("hello")`、以及随后分别取 `Developer`/`System` role 的 `("all previous instructions")` 映射为精确的 `ignore\nall previous instructions` provider-visible view 并在 provider 前命中；同一请求走非 Anthropic profile 时不得产生该 Anthropic-only record。Bedrock 单个 `ToolResult.content` 数组中相邻 text blocks 的 `sec`/`ret`、tool-role `Text("sec")` + `ToolResult("ret")`、以及同一 tool-role message 的两个 ToolResult 分别为 `sec`/`ret` 都必须由 outgoing-block-scoped provider view 命中；普通 user/assistant role 的不同 ToolResult blocks 与不同 message 不得产生跨边界 `secret`。
- [ ] 嵌套 JSON、数组、空值、Unicode escape、解码后的 string key/value、任意层级与 escape 后重复 key、前导 BOM、合法/普通非 JSON/structured-invalid/超深 JSON arguments，以及 JSON MIME document 的 raw/semantic/invalid/duplicate-key/BOM/depth-limit 行为有测试。
- [ ] 测试证明 guardrail 拒绝发生在 provider 调用前。
- [ ] Responses background 测试证明 normalization 400、guardrail block/error 在 queued response 持久化和 200 前完成；失败时 response store/task/provider 调用计数均为 0，通过时 input guardrail 恰好执行一次后才进入后台执行。
- [ ] 测试证明扫描不会下载 URL；`enabled: false`、`check_input: false` 与“已注册 custom guardrail 但全部 `is_enabled() == false`”均在 builder 前保持 fast path，不增加 guardrail-specific document 拒绝或改变请求。既有 request validator 仍会无条件拒绝 malformed base64，因此 disabled fixture 使用 baseline 可接受的合法 base64，并分别覆盖 unsupported MIME、非 UTF-8/非 UTF-8 charset 或 invalid JSON 等只属于 guardrail 的检查。
- [ ] 256/2 MiB 边界内允许，越界稳定 400 且外部调用计数为 0；多 record 的
  OpenAI moderation mock 只收到一次 batch 请求并逐索引合并结果；结果数量不足/
  过多在 `fail_open: false/true` 下都固定失败且 downstream model provider 调用为 0；mixed whitespace
  只提交 trim 后非空值、全 whitespace zero-call，`Log` action 保持非阻断；
  OpenAI moderation 的 action-only `Mask` 在无 `modified_content` 时仍 fail-closed；
  eligible 原始字符串总计 32,768 UTF-8 bytes 可提交、32,769 bytes 在
  `fail_open: false/true` 下都稳定 400 且 moderation/model provider zero-call。
- [ ] 恰好 256 条、每条都只有 baseline 已接受 image content 的 message 不生成任何
  per-message empty projection，不消耗 256-record 上限，也不改变既有多模态
  validator/provider 行为。
- [ ] 仅实现既有 `Guardrail::check_input(&str)` 的 custom guardrail 无需源码修改即可注册；新增 batch 方法的默认实现按 record 顺序逐条调用它，保持 record 隔离与现有 block/Log/error 聚合语义。下游式 compile fixture 必须继续能够对现有五个 `GuardrailError` variants 做穷举匹配，并证明旧 custom implementation 无需实现新方法。另有 regression fixture 将公开 `OpenAIModerationGuardrail` 经 `add_guardrail` 手工注册，证明它仍使用一次 batch 请求、32,768-byte 总上限与 response-count 完整性检查。
- [ ] `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试及完整测试通过。

## 边界情况

- `ToolResult.content` 可能是字符串、对象、数组或空值。
- `ToolUse.input` 与 function arguments 可能包含深层嵌套 JSON。
- JSON object 可能用重复 key 或 escape 后等价 key 隐藏被后续值覆盖的文本；arguments
  也可能在 JSON whitespace 后放置下游 parser 会忽略的 U+FEFF BOM。
- `Document.source.data` 是 base64；仅闭集 allowlist 文本/JSON 可扫描，Markdown、
  HTML/XML entity-bearing 格式和 PDF 等二进制在没有安全解析器时 fail-closed。
- MIME 参数可能带引号、大小写变化、空参数段、尾随分号或重复 `charset`；必须先做
  quote-aware 的原始参数段完整性检查，再执行标准语法解析，并只接受无参数或唯一的
  `charset=utf-8`。不能因 parser 宽容接受 `text/plain;`，或因解码 bytes 恰好也是
  合法 UTF-8，就放行空参数、UTF-16/ISO-8859-1 内容或其他 provider 语义参数。
- message name 或参数可能为空；空值不应制造虚假内容。
- 普通 text parts 即使被 image 等非 text part 分隔，也可能被 provider 过滤后重新相邻；
  多条 message 也可能共同形成当前 gateway 已能检测的敏感词。message 内 provider
  projections 与 request-level legacy newline view 必须同时保留；独立结构化字段仍
  逐条审核，不共享正则匹配边界。
- Anthropic 会先过滤掉非 System/Developer message，再把存活的普通 text leaves 用
  换行连接；该 provider-scoped view 必须覆盖被 User message 隔开的敏感短语，但不得
  作为通用 view 应用于不使用该 transform 的 provider。message 完全没有普通 text leaf
  时则不产生 per-message projection，image-only message 不能制造空审核记录。
- provider 可能在相邻 text parts 之间使用空串、空格或换行；guardrail 必须在 provider
  转换前覆盖这三种当前可达表示，不能假设 DTO 中没有显式分隔符就等于最终 prompt。
- Bedrock 会把单个 `ToolResult.content` 数组中的多个 text entries 转成相邻 text
  blocks；对于 tool/function-role message，还会把普通 Text parts 与多个
  ToolResult.content 一起展开到同一个 outgoing block。raw JSON 标点与逐 node record
  都不能替代 block-scoped provider-boundary views；views 只跨该 outgoing block
  实际扁平化的 leaves，不能跨普通 user/assistant 的 sibling blocks 或 message 扩散。
- guardrail provider 可能只支持纯文本修改，不能安全地重建任意结构化输入。
- 攻击者可能用大量短 JSON keys/values 放大 records；固定记录数/派生字节上限必须
  在任何外部审核前一次性验证。

## 发布说明

这是安全收紧：过去可通过结构化 tool/document 字段发送的内容现在可能被已有
guardrail 拒绝；声明非 UTF-8、重复 charset 或其他 MIME 参数的 allowlisted document
也会稳定拒绝。
启用且至少一个 input guardrail 实例实际 active 时，超过 256 个扫描 records 或
2 MiB 派生扫描文本的请求也会被拒绝。使用 OpenAI moderation 时，eligible batch 还受保守的
32,768 UTF-8 bytes 上游 context 上限约束。无需新增配置；未启用输入 guardrail
或只有 disabled guardrail 实例的部署保持兼容。启用输入 guardrail 的 background Responses 会在返回 queued
response 前增加一次输入检查延迟，以保证确定性 400 与 block 不被异步成功掩盖。
