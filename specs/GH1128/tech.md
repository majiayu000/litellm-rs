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
| Guardrail engine | `src/core/guardrails/{traits,types,engine,openai_moderation,pii,prompt_injection}.rs` | 接收单个 `&str`；OpenAI moderation 每次调用发一个远程请求；一般执行错误可被 `fail_open` 忽略；`GuardrailError` 是公开 re-export、未标记 `non_exhaustive` 的 enum，custom guardrail 只需实现公开 `check_input(&str)`；公开 `add_guardrail` 也可手工注册 built-in | 需要 source-compatible 的默认 batch method 与独立 batch error 类型；扩大现有 enum、增加必需 trait method 或依赖 trait-object downcast 都不可接受 |
| Document MIME | `Cargo.toml`, `Cargo.lock`, `src/core/models/openai/messages.rs` | DTO 保留任意 `media_type` 字符串；`mime` 当前仅为 transitive dependency，不能直接依赖其 API | 需要直接声明标准 MIME parser 并审计全部 charset 参数，不能用字符串截断 |
| Chat execution boundary | `src/server/routes/ai/chat.rs` | 公开 route helper 总是在内部执行 input guardrail；直接暴露 unchecked internal handler 会形成旁路 | Background Responses 需在 queue 前检查一次、后台执行不重复，必须用窄作用域 audited entrypoint 和 caller-coverage test 固定 |
| Background Responses | `src/server/routes/ai/responses.rs`, `src/server/routes/ai/responses/lifecycle.rs`, `src/server/routes/ai/responses/lifecycle_tests.rs` | 先存储并返回 queued 200，再由后台 task 进入 chat input guardrail；deterministic normalization 400 只能晚到并变成 failed status | 必须把完整 input guardrail 移到 queue/persist/task 之前，并保证后台只消费已审核的原请求 |

## 设计方案

1. 用一个返回 owned `Result<Vec<GuardrailInputRecord>, GatewayError>` 的规范化函数替代
   `content_text -> Vec<&str>`。owned 结果允许安全容纳 JSON 序列化和 base64 解码
   后的正文；record 只在进程内携带 typed provenance 与原始扫描值，provenance 不编码
   进扫描文本，错误不能被 iterator/filter 静默丢弃。
2. builder 按稳定顺序生成互补记录：
   - 对每条 message，按原 part 顺序先收集全部普通 text leaves，过滤夹在其中的
     image/document/tool 等非 text parts；再依次生成三种 provider projection：
     `text_leaves.join("")`、`text_leaves.join(" ")`、`text_leaves.join("\n")`。
     这是仓库当前 provider 转换的完整边界集合：Gemini/Vertex 等直接拼接，Bedrock
     prompt transforms 先过滤非 text 再用单空格，Ollama 使用换行。三种 view 各自是
     独立 typed record；按上述固定顺序对完整字符串做局部去重，避免单 part 或已含
     边界时重复计入 records/bytes。不向扫描值写入标签，也不允许 guardrail 跨三种
     alternative records 匹配。
   - 另外按 message/part 原顺序收集全请求所有普通 text leaves，生成唯一的
     `request_text_leaves.join("\n")` legacy record。这精确保留当前
     `src/server/guardrails.rs` 的跨 message/newline 检测能力；只包含普通 message
     text，不把 message name、function/tool、document 或其他 typed records 混入，
     因此结构化字段仍保持 record isolation。该 record 与 message projections 一样
     参加局部值去重、256 records 与 2 MiB accounting。
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
   - 先定义 `ToolResult.content` 到 Bedrock textual leaves 的确定性 primitive。当前
     `tool_result_contents_from_value` 对数组直接按原顺序转换：
     `{"type":"text","text":<string>}` 贡献解码后的 `text`，未知 type 或无 type 的
     entry 贡献该 entry 的确定性 JSON string，image/image_url/document 等 provider
     会拒绝的 entry 不伪造 text；root string/non-array 只形成单 leaf。builder 镜像
     该 primitive 后，按每个实际 outgoing Bedrock `ToolResultBlock.content` 分组：
     普通 user/assistant message 中每个 `ContentPart::ToolResult` 独立形成一个组；
     tool/function-role `MessageContent::Parts` 则镜像
     `message_content_to_tool_result_contents`，按 part 顺序把普通 `Text` leaf 与每个
     ToolResult primitive 的 leaves 依次 extend 到同一组，因为 route 会把整条 message
     包成一个 ToolResult block。每组按固定顺序生成 `join("")`、`join(" ")`、
     `join("\n")` 三个 alternative records 并按完整值局部去重。不得跨不同 outgoing
     block 或 message 合并；尤其普通 user/assistant 的 Text 与 sibling ToolResult、
     或两个 sibling ToolResult blocks 仍隔离。raw JSON 与逐 string key/value
     semantic records 保持原字段粒度，provider views 是补充而非替代；每个去重后的
     view 都计入 256 records/2 MiB 上限。
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
4. 先用 `mime = "0.3"` 的直接依赖把 `Document.source.media_type` 完整解析为
   `mime::Mime`。因为锁定的 `mime 0.3.17` 会接受尾随空参数，在调用 parser 前必须用
   quote-aware 字节状态机检查原字符串的参数段：只把引号外分号视为 delimiter，拒绝
   任意 trim 后为空的参数段、连续分号和尾随分号（含尾随空白），不得把 quoted value
   内的分号误判为 delimiter。精确地说，`text/plain;` 与
   `text/plain; charset=utf-8;` 都必须稳定 400。随后 parser 语法失败同样返回稳定
   invalid media type 400，禁止按第一个分号截断或对 malformed 参数降级。按 parser
   的 essence 做 ASCII 大小写不敏感 allowlist
   比较，并遍历全部参数执行闭集策略：参数列表为空可继续；否则必须恰好只有一个
   `charset`，且其解引用后的值大小写不敏感地等于 `utf-8`。两个及以上 charset
   即使值相同也因歧义稳定 400；`utf8` 等别名、UTF-16/UTF-16LE/ISO-8859-1 等其他
   charset，以及任意 non-charset 参数一律拒绝，避免 `format` 等参数改变 provider
   文本语义后形成 scanner 未覆盖的视图。通过时原 `media_type` 不改写并继续交给
   provider。这样 gateway 只对无参数/明确 UTF-8 做 UTF-8 扫描，不能因
   UTF-16LE bytes 恰好是 NUL-interleaved 合法 UTF-8 而误放行。

   `Document.source.data` 随后按 base64 解码。仅接受明确文本媒体类型：
   `text/plain`、`text/csv`、`application/json` 和 `application/*+json`；解码结果
   必须是 UTF-8。`text/markdown`、`text/html`、XML/`+xml` 与其他 `text/*` 不在
   allowlist，因为本 tranche 不引入 Markdown/HTML/XML entity parser，必须
   fail-closed 而不是扫描 encoded entities。malformed base64、非 UTF-8 或
   其他媒体类型在 active-input predicate 为 true 时 fail-closed。
   malformed base64、非 UTF-8、非法/歧义/非 UTF-8 charset 和 unsupported
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
6. `check_chat_input` 在调用 fallible builder 前先查询 crate-private
   `GuardrailEngine::has_active_input_guardrails()`；其真值严格等于
   `config.enabled && config.check_input && guardrails.iter().any(|g| g.is_enabled())`。
   false 时立即沿用原请求 fast path，不执行任何 guardrail-specific JSON/MIME/base64/
   size normalization。现有 `GuardrailEngine::is_enabled()` 的公开语义保持不变，避免
   additive amendment 改变调用方；既有独立 request validator 也不因该 fast path
   关闭。engine unit test 与 route fixture 必须覆盖 global disabled、check_input
   disabled、空列表以及只有 disabled custom guardrail 四种 false case，并证明最后
   一种不会因合法 base64 中的 unsupported MIME、非 UTF-8 charset 或 invalid JSON
   触发 guardrail-specific 400。

   只有 active predicate 为 true 时才构造全部 records，成功后通过一次 batch 调用
   交给 engine。gateway 不循环调用现有单字符串入口，而是调用一次 crate-private
   `GuardrailEngine::check_input_records(&[GuardrailInputRecord])`。engine 继续保存
   现有 `Vec<BoxedGuardrail>` 并通过原 trait object 委托 `name`、`priority`、
   `is_enabled`、`check_output` 和新 input-batch 方法；不得改成按 concrete type
   枚举的 storage，也不得依赖不存在的 `Any` downcast。这样 custom output override、
   disabled 状态、排序、名称与 `guardrail_count` 保持原行为。

   `GuardrailInputRecord` 是 additive 的公开只读输入 view：从首次发布即
   `#[non_exhaustive]`、字段保持 private，并提供 `value(&self) -> &str`；typed
   provenance 只用于 gateway/built-in 路由，不拼入 value，外部实现不能伪造或修改
   gateway records。公开 `Guardrail` trait 增加一个有完整默认实现的
   `check_input_records(&[GuardrailInputRecord])`。默认实现按稳定 record 顺序调用
   现有必需方法 `check_input(&str)`，复用 merge/early-block/`Log` 语义；旧 custom
   implementation 不需要增加方法即可重新编译。PII 与 prompt-injection 可 override
   该方法以逐 record 做本地聚合，不能跨 record。`OpenAIModerationGuardrail`
   必须 override，因此无论由 config 创建还是作为 `Box<dyn Guardrail>` 经公开
   `add_guardrail` 手工注册，动态分派都进入同一个 single-request batch 路径。

   新增独立公开、从首次发布即标记 `#[non_exhaustive]` 的
   `GuardrailBatchError`，包含 `Guardrail(GuardrailError)`、
   `InputLimitExceeded` 与 `ResponseIntegrity`；它只服务新增默认 batch method，
   不给现有可穷举匹配的 `GuardrailError` 增加 variant。engine 对
   `GuardrailBatchError::Guardrail` 维持现有 `fail_open` 行为，对另外两类在
   `fail_open` 分支之前无条件传播。gateway 将 `InputLimitExceeded` 映射为安全稳定
   的 HTTP 400，并将 `ResponseIntegrity` 映射为不含响应正文的稳定 guardrail server
   error；二者都不得调用 downstream model provider。公开
   `GuardrailEngine::check_input(&str)` 保持既有签名，通过一元素 batch 执行，并把
   两个 batch-only fatal classes 映射为现有 `GuardrailError::Internal` 的固定安全
   消息。`GuardrailBatchError`、`GuardrailInputRecord` 与默认 method 是 additive
   API；现有方法签名、现有 enum variants 与 trait 必需方法集合不变。

   OpenAI moderation 的 batch override 按既有
   `!value.trim().is_empty()` 选择 eligible records，将其 values 作为 API 支持的
   string array 放进至多一次 `/moderations` 请求；全空白 batch 直接返回 pass，
   mixed batch 按原 record index 合并。发送前以 checked arithmetic 计算 eligible
   原始字符串（不是 trim 后副本）的 UTF-8 byte 总和，并要求
   `<= MAX_OPENAI_MODERATION_BATCH_BYTES = 32_768`。BPE token 数不会超过 UTF-8 byte
   数，因此该保守上限保证不超过仓库 catalog 中 moderation 模型的 32,768-token
   context，且不引入不确定 tokenizer fallback。32,769 bytes 及以上返回
   `GuardrailBatchError::InputLimitExceeded`；结果数量不匹配返回
   `GuardrailBatchError::ResponseIntegrity`。任一 guardrail 的聚合结果为
   block/error/modified 或 action-only `Mask` 时沿用上述 fail-closed `enforce`
   语义结束；`GuardrailAction::Log` 命中继续 merge 并执行后续 guardrails，最终保持
   非阻断。active-input predicate 为 false 时保持现有无检查行为。
7. builder 使用固定常量 `MAX_INPUT_GUARDRAIL_RECORDS = 256` 与
   `MAX_INPUT_GUARDRAIL_SCAN_BYTES = 2 * 1024 * 1024`。每次加入 record 前用 checked
   arithmetic 统计 records 数与所有 record values 的 UTF-8 byte 总和；任一超限
   返回 `GatewayError::validation`，公开固定 HTTP 400
   `invalid_request_error`/`invalid_request`，安全消息只说明 fragment/size limit。
   完整 batch 在任何 guardrail（尤其远程 moderation）调用前验证完毕。内置 remote
   guardrail 的外部请求次数必须按 guardrail 数量有界，而不是按 record 数量增长。
   2 MiB 是通用本地扫描载荷上限；32,768 bytes 是仅在 OpenAI moderation active
   时对 eligible batch 额外执行的、更严格的上游兼容上限，二者不得互相替代。
8. Background Responses 不得先 queue 再检查。`handle_background_response` 改为
   async，并在创建 response ID、写入 response store 或 spawn task 前调用完整
   `check_chat_input`。失败直接返回现有安全 error response；成功后只把同一个未修改
   `ChatCompletionRequest` 移交后台。`chat.rs` 提供一个 crate-private、仅允许此
   lifecycle 使用的 `handle_chat_completion_after_input_guardrail`，它跳过的只有已
   完成的 input check，仍执行 provider、output guardrail、budget/callback 等全部
   后续逻辑。source-boundary test 必须证明该 entrypoint 只有
   `responses/lifecycle.rs` 调用；lifecycle test 证明 check 在 queue/persist/spawn
   前恰好一次，失败时 response store、task registry 与 provider dispatch 都为零。
   这条路径不通过 clone 后修改请求，也不允许对其他 handler 公开 unchecked API。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | request/message/function/content 规范化 | 每种载体独立 allow/block fixture |
| B-002/B-003 | message/request text projections + typed independent records | 过滤 image 后的空串/单空格/换行 views、跨 message legacy newline view、view 局部去重、Bedrock/Ollama/Gemini-Vertex provider-transform 对照、单 ToolResult array 与 tool/function-role Text+ToolResult/多 ToolResult 的 outgoing-block-scoped split-pattern、普通 user/assistant sibling-block/message 隔离、顺序、结构化 record 隔离、JSON/document raw+semantic、Unicode escape、嵌套/escape-equivalent duplicate key 与 leading-BOM fail-closed snapshot |
| B-004 | `check_input` 调用顺序 | mock provider 未被调用 |
| B-005 | `enforce` modified 分支 | mask 仍 fail-closed 且请求 DTO 未改变 |
| B-006 | fallible builder + background pre-queue gate | malformed document/serialization 返回安全稳定 400；background 失败时 queue/store/task/provider zero-call，通过时 input check exactly-once |
| B-007 | document media gate | quote-aware 空参数/尾随分号检查 + MIME syntax/essence/charset table；无参数/唯一 UTF-8 正文解码，重复/非 UTF-8 charset 与 PDF/image/audio/URL 无网络且 fail-closed/保持范围 |
| B-008 | pre-builder active-input predicate | global/check_input/empty/all-custom-disabled fast paths；同一多模态请求不触发 guardrail-specific normalization 且保持兼容 |
| B-009 | default batch method + builder limits | 256/2 MiB 边界、越界 zero-call、legacy custom default adapter、现有公开 error enum exhaustive/旧 custom compile fixture、custom output/enable/priority/name regression、config-created 与 manually-added moderation 都走 single-batch、32,768/32,769-byte 边界，以及 batch count-mismatch/input-limit failures 在 `fail_open` true/false 下均不可吞掉 |

## 数据流

`ChatCompletionRequest.messages` 只有在 engine 的 active-input predicate 通过后才
按顺序进入 bounded fallible fragment builder；每条 message 过滤非 text parts 后
形成空串/单空格/换行三种去重后的 provider projection，全请求普通 text leaves 另
形成 legacy newline record，独立结构化字段形成 typed independent records；每个
实际 outgoing Bedrock ToolResult block 的 ordered text sequence 另形成 block-scoped
三投影并局部去重，tool/function-role message 的 Text 与多个 ToolResult 在同组，
普通 user/assistant sibling blocks 保持隔离。JSON 同时保留完整表示和解码 string nodes；document MIME 完整
解析且 charset absent/UTF-8 gate 通过后，base64 解码为 UTF-8，JSON MIME 进一步
生成 raw + semantic records；request-level `function_call` 最后加入。builder 完整
成功且通过 256/2 MiB 上限后，把 typed batch 一次交给 engine；现有
`Box<dyn Guardrail>` 通过默认/override batch method 动态分派，OpenAI moderation
使用一次 array 请求，fatal batch failures 由独立 `GuardrailBatchError` 在
`fail_open` 前分类。再由 `enforce` 统一映射 allow/block/modified/error。现有
`GuardrailError` variants、trait 必需方法、engine 单字符串签名与原请求对象不变。
Background Responses 在 queued response 产生前完成相同检查，并把已通过的同一请求
交给窄作用域 post-input-guardrail chat entrypoint；同步/stream 路径仍使用常规入口。

## 备选方案

- 对整个 `ChatCompletionRequest` 直接 JSON 序列化：拒绝，因为会扫描 image/audio
  base64、混入无关配置，并且 document 正文仍是编码数据。
- 只扫描 JSON/base64 原字符串：拒绝，因为不能识别 document 解码后的自然语言。
- 继续用 `split(';').next()` 剥离 MIME 参数：拒绝，因为会忽略 provider 可执行的
  charset 声明并放行 NUL-interleaved UTF-16 bytes。
- 对不支持 document 类型放行：拒绝，因为输入 guardrail 会继续存在公开 bypass。
- 自动提取 PDF/Office：超出范围且会新增复杂解析/资源消耗面。

## 风险

- Security: 支持媒体类型列表必须 fail-closed，不能被 MIME 大小写、parser 宽容接受的
  空参数/尾随分号、malformed/重复参数或非 UTF-8 charset 绕过；ToolResult provider
  views 必须覆盖 outgoing block 内实际相邻化，但不能跨 block/message 制造误匹配。
- Compatibility: 启用 input guardrail 的二进制 document 将被拒绝；发布说明需明确。
- Public API compatibility: `GuardrailError` variants、
  `GuardrailEngine::check_input` 签名与 `add_guardrail` 调用方式保持不变；
  `Guardrail` 只增加有默认实现的 additive batch method；新增只读 record 与 batch
  error 从首次发布即 `#[non_exhaustive]`。旧 custom implementation、custom output
  override 与手工注册 built-in 必须有 compile/runtime regression fixture。
- Performance: 文档解码、message/ToolResult projections 与 request legacy view 增加 owned 载荷；局部去重避免相同
  view 重复计数，所有派生值仍受 256 records/2 MiB checked 上限约束。
- Availability/Cost: records 有 256/2 MiB 硬上限；内置 OpenAI moderation 的
  eligible batch 另有保守 32,768-byte context 上限，必须 batch 为单次远程调用，
  并验证 response count，避免 JSON fan-out 或确定性的上游 context 拒绝。
- Maintenance: DTO 新增文本载体时应在 exhaustiveness test 中显式分类。

## 测试计划

- [ ] Unit tests: 全 variant、request/message legacy/modern function、JSON raw+semantic、record isolation、过滤 image 后的空串/单空格/换行 views、跨 message legacy newline view、single ToolResult array 与 tool/function-role Text+ToolResult/多 ToolResult outgoing-block views、普通 user/assistant sibling-block/message 隔离、局部去重、Unicode、嵌套与 escape-equivalent duplicate key、leading BOM。
- [ ] Document tests: plain/csv、JSON raw+semantic/invalid/duplicate-key/BOM/depth-limit、`+json`、完整 MIME syntax、无参数/大小写 UTF-8/quoted UTF-8、`text/plain;`、`text/plain; charset=utf-8;`、连续/空参数段、重复 charset、UTF-16LE/其他 charset、non-charset 参数 fail-closed、bad base64、bad UTF-8，以及 Markdown numeric/named entity、HTML/XML/`+xml`/其他 `text/*`/PDF fail-closed。
- [ ] Batch tests: 256/2 MiB 边界、checked overflow/越界 zero external calls、local record isolation、legacy custom default adapter、现有公开 error enum exhaustive/旧 custom compile fixture、custom input-allow/output-block + disabled + non-default-priority/name regression、config-created 与 manually-added OpenAI array single-call、32,768/32,769 eligible-byte 边界、mixed/all whitespace eligibility、`Log` 非阻断、action-only `Mask` fail-closed、batch response-integrity/input-limit failure 在 `fail_open` true/false 下 fail-closed。
- [ ] Integration tests: blocked 发生在 provider 前，400 error envelope 稳定；engine disabled、`check_input: false` 与仅有 disabled custom guardrail 都在 builder 前不增加 guardrail-specific 拒绝并保持 DTO，malformed base64 仍由前置 request validator 按既有行为拒绝。
- [ ] Provider-boundary tests: Bedrock 的 space-join、Ollama 的 newline-join 与
      Gemini/Vertex 的 empty-join 转换结果分别与 builder 对应 adjacency view 相等；
      image-separated 及 split-message `ignore` + `all previous instructions` 都在
      provider 调用前被拒绝；Bedrock ToolResult array、tool-role
      Text+ToolResult 与 tool-role 多 ToolResult 的相邻 `sec`/`ret` 由同 outgoing
      block view 命中，而普通 user/assistant sibling ToolResult blocks 与不同 message
      的两段保持隔离。
- [ ] Background Responses tests: normalization limit/invalid document/guardrail block 在
      queued persist/task spawn/200 前返回；失败时 store/task/provider zero-call，
      通过时 input guardrail exactly-once，post-input entrypoint caller 仅 lifecycle。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试、`cargo test`。

## 回滚方案

回滚规范化函数与测试即可，无持久化迁移。若二进制 document 兼容性需要恢复，
必须另行设计可审核的解析器或按端点禁用 input guardrail；不得重新静默放行。
