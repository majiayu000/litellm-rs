# Tech Spec

## Linked Issue

GH-1128 / #1128

## Product Spec

见 `product.md`（B-001 ～ B-009）。

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Guardrail adapter | `src/server/guardrails.rs` | `content_text` 只借用普通 text part | 根因与唯一 enforcement boundary |
| Chat DTO | `src/core/models/openai/messages.rs` | `ContentPart` 包含 document/tool result/tool use | 列出必须规范化的载体 |
| Function DTO | `src/core/models/openai/tools.rs` | legacy/modern function arguments 都会发给 provider | 等价结构化输入不能继续遗漏 |
| Guardrail engine | `src/core/guardrails/{traits,types,engine,openai_moderation,pii,prompt_injection}.rs` | 接收单个 `&str`；OpenAI moderation 每次调用发一个远程请求；一般执行错误可被 `fail_open` 忽略；mask 在 gateway boundary fail-closed | 需要 record-aware batch、不可吞掉的 response-integrity error，避免字段拼接、缺结果绕过和外部调用放大 |

## 设计方案

1. 用一个返回 owned `Result<Vec<GuardrailInputRecord>, GatewayError>` 的规范化函数替代
   `content_text -> Vec<&str>`。owned 结果允许安全容纳 JSON 序列化和 base64 解码
   后的正文；record 只在进程内携带 typed provenance 与原始扫描值，provenance 不编码
   进扫描文本，错误不能被 iterator/filter 静默丢弃。
2. builder 按稳定顺序生成互补记录：
   - 对每条 message 中语义连续的 text content parts 依次生成三种 adjacency view：
     `parts.join("")`、`parts.join(" ")`、`parts.join("\n")`。这是仓库当前 provider
     转换的完整边界集合：Gemini/Vertex 等直接拼接，Bedrock prompt transforms 使用
     单空格，Ollama 使用换行。三种 view 各自是独立 typed record；按上述固定顺序
     对完整字符串做局部去重，避免单 part 或已含边界时重复计入 records/bytes。
     不向扫描值写入标签，也不允许 guardrail 跨三种 alternative records 匹配。
     document/tool 等不连续 surface 不加入这些 adjacency views。
   - 对 message name、普通 content、message-level legacy `function_call`
     name/arguments、modern `tool_calls[].function` name/arguments 和
     `ToolUse.name` 生成独立 typed records。每个 record 的值单独交给 engine，
     禁止把 kind、长度、换行或其他人工标签拼入扫描值，禁止 regex 跨 record 匹配。
   - request-level `ChatCompletionRequest.function_call.name/arguments` 在 messages
     之后按固定 typed kind 加入；顺序与 request DTO 一致。这里描述的是本仓库现有
     compatibility DTO，而不是宣称标准 OpenAI request-level selection 新增了
     arguments：`src/core/models/openai/requests.rs` 当前复用
     `tools::FunctionCall { name, arguments }`，`chat.rs` 会把整个值序列化到内部
     provider request。只要该输入仍被接受并转发，两个字符串都必须在 provider 前
     受 guardrail 审核。
3. `ToolResult.content` 与 `ToolUse.input` 先使用 `serde_json::to_string` 加入完整、
   确定性表示，再深度遍历 `serde_json::Value`，按稳定对象 key 顺序分别加入解码
   后的 string keys/values。对原始 JSON string 不能直接反序列化成会覆盖重复 key
   的 `serde_json::Value`；使用 bounded serde visitor 在构造每层 map 时按解码后的
   key 检测重复项，任意层级的重复 key（包括 `"\u0063md"` 与 `"cmd"`）都返回稳定
   invalid JSON 400。合法 JSON function arguments 使用该 visitor 和同一语义遍历，
   并同时保留原 argument string。先跳过 JSON 允许的 leading whitespace；若下一
   scalar 是 U+FEFF BOM，则稳定 400，不能把 BOM-prefixed payload 降级为普通文本。
   首个非空白字符不是 `{`/`[` 且解析失败的普通非 JSON argument 仍只扫描原字符串；
   若首个非空白字符为 `{`/`[`，任何解析失败（包括 serde
   recursion/depth/resource limit）都返回稳定 400，禁止把超深但有效 JSON 降级成
   raw-only 扫描。序列化错误同样 fail-closed。
4. `Document.source.data` 先按 base64 解码。仅接受明确文本媒体类型：
   `text/plain`、`text/csv`、`application/json` 和 `application/*+json`；解码结果
   必须是 UTF-8。`text/markdown`、`text/html`、XML/`+xml` 与其他 `text/*` 不在
   allowlist，因为本 tranche 不引入 Markdown/HTML/XML entity parser，必须
   fail-closed 而不是扫描 encoded entities。malformed base64、非 UTF-8 或
   其他媒体类型在 input guardrail 开启且 `check_input` 为 true 时 fail-closed。
   MIME 比较忽略 ASCII 大小写并剥离参数。malformed base64、非 UTF-8 和 unsupported
   MIME 均返回 `GatewayError::validation`，外部固定为 HTTP 400、
   `type=invalid_request_error`、`code=invalid_request`；安全消息只说明
   base64/UTF-8/media-type 类别，不回显输入。
   `application/json` 与 `application/*+json` 的解码正文必须作为 raw record
   扫描，并解析成 `serde_json::Value` 后按第 3 条生成 semantic records；声明为
   JSON 但语法、重复 key、BOM、recursion/depth 或资源限制失败时返回同一稳定
   400（消息类别为 invalid JSON，不回显正文），
   不得仅扫描带 `\uXXXX` 转义的 raw 表示后放行。
   不读取 URL、不解析 PDF/Office、不扫描 image/audio base64。
5. 新规范化只改变传给 guardrail 的扫描字符串，绝不重写原 DTO。gateway
   `enforce` 必须在 `result.action == GuardrailAction::Mask` 或
   `modified_content.is_some()` 任一成立时显式失败；这覆盖 OpenAI moderation
   action-only `Mask`，避免缺少 modified content 时继续请求，也避免把扁平文本
   错误回写到结构。
6. 在 `check_input` 中先构造全部 records，成功后通过一次 batch 调用交给 engine。
   gateway 不循环调用现有单字符串入口，而是调用一次新增的
   `GuardrailEngine::check_input_records(&[GuardrailInputRecord])`。engine 按优先级
   对每个 guardrail 调用一次 batch 方法。`Guardrail` trait 上新增的 batch 方法必须
   有默认实现：按稳定 record 顺序调用既有必需方法 `check_input(&str)` 并使用现有
   merge/early-block 语义聚合，因此通过公开 `add_guardrail` 注册、只实现旧单字符串
   方法的 custom guardrail 不需要改源码，也不会被跳过。PII/prompt-injection
   override 该方法并在方法内逐 record
   做本地匹配并聚合，不能跨 record；OpenAI moderation 按既有
   `!value.trim().is_empty()` 选择 eligible records，将其 values 作为 API 支持的
   string array 放进至多一次 `/moderations` 请求；全空白 batch 直接返回 pass，
   mixed batch 按原 record index 合并。发送前以 checked arithmetic 计算 eligible
   原始字符串（不是 trim 后副本）的 UTF-8 byte 总和，并要求
   `<= MAX_OPENAI_MODERATION_BATCH_BYTES = 32_768`。BPE token 数不会超过 UTF-8 byte
   数，因此该保守上限保证不超过仓库 catalog 中 moderation 模型的 32,768-token
   context，且不引入不确定 tokenizer fallback。32,769 bytes 及以上使用 typed
   `GuardrailError::InputLimitExceeded`，由 gateway 映射为安全稳定的 HTTP 400；
   engine 必须在 `fail_open` 判断前传播，且不得调用 moderation 或 downstream
   model provider。`GuardrailError` 另增加明确的 batch
   response-integrity variant；结果数量
   不匹配返回该 variant，engine 在检查 `fail_open` 之前无条件传播它，只有 network/
   provider availability 等既有可降级错误仍受 `fail_open` 控制。任一 guardrail
   的聚合结果为 block/error/modified 或 action-only `Mask` 时沿用上述 fail-closed
   `enforce` 语义结束；
   `GuardrailAction::Log` 命中继续 merge 并执行后续 guardrails，最终保持非阻断，
   与现有 engine 契约一致。
   单字符串 `check_input` 通过一元素 batch 保持兼容。engine disabled 或
   `check_input: false` 时保持现有无检查行为。
7. builder 使用固定常量 `MAX_INPUT_GUARDRAIL_RECORDS = 256` 与
   `MAX_INPUT_GUARDRAIL_SCAN_BYTES = 2 * 1024 * 1024`。每次加入 record 前用 checked
   arithmetic 统计 records 数与所有 record values 的 UTF-8 byte 总和；任一超限
   返回 `GatewayError::validation`，公开固定 HTTP 400
   `invalid_request_error`/`invalid_request`，安全消息只说明 fragment/size limit。
   完整 batch 在任何 guardrail（尤其远程 moderation）调用前验证完毕。内置 remote
   guardrail 的外部请求次数必须按 guardrail 数量有界，而不是按 record 数量增长。
   2 MiB 是通用本地扫描载荷上限；32,768 bytes 是仅在 OpenAI moderation active
   时对 eligible batch 额外执行的、更严格的上游兼容上限，二者不得互相替代。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | request/message/function/content 规范化 | 每种载体独立 allow/block fixture |
| B-002/B-003 | adjacency + typed independent records | 跨 content part 的空串/单空格/换行 views、view 局部去重、Bedrock/Ollama/Gemini-Vertex provider-transform 对照、顺序、record 隔离、JSON/document raw+semantic、Unicode escape、嵌套/escape-equivalent duplicate key 与 leading-BOM fail-closed snapshot |
| B-004 | `check_input` 调用顺序 | mock provider 未被调用 |
| B-005 | `enforce` modified 分支 | mask 仍 fail-closed 且请求 DTO 未改变 |
| B-006 | fallible builder | malformed document/serialization 返回安全稳定 400，且不调用 engine/provider 后续 |
| B-007 | document media gate | 文本正文解码；PDF/image/audio/URL 无网络且 fail-closed/保持范围 |
| B-008 | disabled fast path | 同一多模态请求在未启用 guardrail 时保持兼容 |
| B-009 | batch engine + builder limits | 256/2 MiB 边界、越界 zero-call、custom guardrail 默认 batch adapter、moderation 32,768/32,769-byte 边界与 single-batch，以及 count mismatch/input-limit 在 `fail_open` true/false 下均不可吞掉 |

## 数据流

`ChatCompletionRequest.messages` 按顺序进入 bounded fallible fragment builder；连续 text
content 形成空串/单空格/换行三种去重后的 adjacency records，独立字段形成 typed independent records。JSON 同时保留完整
表示和解码 string nodes；允许的 document base64 解码为 UTF-8，JSON MIME 进一步
生成 raw + semantic records；request-level `function_call` 最后加入。builder 完整
成功且通过 256/2 MiB 上限后，把 typed batch 一次交给 engine；engine 让每个
guardrail 在保持 record 边界的前提下处理该 batch，OpenAI moderation 使用一次 array
请求。再由 `enforce` 统一映射 allow/block/modified/error。原请求对象始终不变。

## 备选方案

- 对整个 `ChatCompletionRequest` 直接 JSON 序列化：拒绝，因为会扫描 image/audio
  base64、混入无关配置，并且 document 正文仍是编码数据。
- 只扫描 JSON/base64 原字符串：拒绝，因为不能识别 document 解码后的自然语言。
- 对不支持 document 类型放行：拒绝，因为输入 guardrail 会继续存在公开 bypass。
- 自动提取 PDF/Office：超出范围且会新增复杂解析/资源消耗面。

## 风险

- Security: 支持媒体类型列表必须 fail-closed，不能被 MIME 大小写/参数绕过。
- Compatibility: 启用 input guardrail 的二进制 document 将被拒绝；发布说明需明确。
- Performance: 文档解码和三种 adjacency views 增加 owned 载荷；局部去重避免相同
  view 重复计数，所有派生值仍受 256 records/2 MiB checked 上限约束。
- Availability/Cost: records 有 256/2 MiB 硬上限；内置 OpenAI moderation 的
  eligible batch 另有保守 32,768-byte context 上限，必须 batch 为单次远程调用，
  并验证 response count，避免 JSON fan-out 或确定性的上游 context 拒绝。
- Maintenance: DTO 新增文本载体时应在 exhaustiveness test 中显式分类。

## 测试计划

- [ ] Unit tests: 全 variant、request/message legacy/modern function、JSON raw+semantic、record isolation、跨 part 空串/单空格/换行 views 与局部去重、Unicode、嵌套与 escape-equivalent duplicate key、leading BOM。
- [ ] Document tests: plain/csv、JSON raw+semantic/invalid/duplicate-key/BOM/depth-limit、`+json`、MIME 参数、bad base64、bad UTF-8，以及 Markdown numeric/named entity、HTML/XML/`+xml`/其他 `text/*`/PDF fail-closed。
- [ ] Batch tests: 256/2 MiB 边界、checked overflow/越界 zero external calls、local record isolation、legacy custom guardrail 默认 adapter、OpenAI array single-call、32,768/32,769 eligible-byte 边界、mixed/all whitespace eligibility、`Log` 非阻断、action-only `Mask` fail-closed、response count/input-limit 在 `fail_open` true/false 下 fail-closed。
- [ ] Integration tests: blocked 发生在 provider 前，400 error envelope 稳定；engine disabled 与 `check_input: false` 不增加 guardrail-specific 拒绝并保持 DTO，malformed base64 仍由前置 request validator 按既有行为拒绝。
- [ ] Provider-boundary tests: Bedrock 的 space-join、Ollama 的 newline-join 与
      Gemini/Vertex 的 empty-join 转换结果分别与 builder 对应 adjacency view 相等；
      `ignore` + `all previous instructions` 在 provider 调用前被拒绝。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试、`cargo test`。

## 回滚方案

回滚规范化函数与测试即可，无持久化迁移。若二进制 document 兼容性需要恢复，
必须另行设计可审核的解析器或按端点禁用 input guardrail；不得重新静默放行。
