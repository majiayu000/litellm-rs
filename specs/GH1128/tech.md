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
| Anthropic system projection | `src/core/providers/anthropic/client.rs:290` | `separate_system_messages` 先过滤为 `System`/`Developer`，再把存活的普通 text leaves 用换行连接；中间的 User message 不进入该 provider-visible string | input guardrail 必须镜像这条 provider-scoped 边界，但不能把它扩散到非 Anthropic profile |
| Gemini system projection | `src/core/providers/gemini/client.rs:252` | `GeminiClient` 对 Google AI 与自身 Vertex endpoint 使用同一 transform，只保留 `System`，输出有序 `systemInstruction.parts`；独立 `Provider::VertexAI` 使用另一 transformer | profile 必须按实际 provider transform 区分，不能把 `Developer` 或独立 VertexAI 误归入 native Gemini |
| Bedrock system projection | `src/core/providers/bedrock/{chat/mod.rs,chat/converse.rs,model_id.rs}` | provider 会按选中 model 的 `BedrockApiType` 在 Converse/ConverseStream 与 Invoke 系列间分流；只有非 prompt-management Converse 接受 request-level `System` 并输出 `system[]`，`Developer` 被过滤 | profile 必须按 deployment 的选中 model/API transform 分类，不能只按 `Provider::Bedrock` enum |
| Guardrail engine | `src/core/guardrails/{traits,types,engine,openai_moderation,pii,prompt_injection}.rs` | 接收单个 `&str`；OpenAI moderation 每次调用发一个远程请求；一般执行错误可被 `fail_open` 忽略；`GuardrailError` 是公开 re-export、未标记 `non_exhaustive` 的 enum，custom guardrail 只需实现公开 `check_input(&str)`；公开 `add_guardrail` 也可手工注册 built-in | 需要 source-compatible 的默认 batch method 与独立 batch error 类型；扩大现有 enum、增加必需 trait method 或依赖 trait-object downcast 都不可接受 |
| Document MIME | `Cargo.toml`, `Cargo.lock`, `src/core/models/openai/messages.rs` | DTO 保留任意 `media_type` 字符串；`mime` 当前仅为 transitive dependency，不能直接依赖其 API | 需要直接声明标准 MIME parser 并审计全部 charset 参数，不能用字符串截断 |
| Routing snapshot | `src/core/router/{mod.rs,selection.rs,unified.rs}` | `RuntimeHandle` 已持有 immutable `RoutingSnapshot`/generation，但 server chat selection 仍直接调用 `UnifiedRouter`，每次选择都可 load 新 snapshot；现有 selector 还会应用 health/cooldown/concurrency/rate-limit 等瞬时过滤 | audit profiles 必须从一个 snapshot 的稳定候选集推导，后续 selection/retry 必须复用同一 handle，不能只审核 audit 时瞬时可选者 |
| Chat execution boundary | `src/server/routes/ai/{chat.rs,chat_streaming.rs,budgeted.rs,execution.rs,response_cache.rs}` | input guardrail 后，cache pricing、unary/stream 与每次 retry 都可重新读取 current router；公开 route helper 总是在内部执行 input guardrail | active-input lifecycle 必须把 audited routing handle 传入 cache/pricing 与最终 dispatch；disabled lifecycle 保留动态 routing，窄作用域 API 与 caller tests 防止丢失 audit |
| Background Responses | `src/server/routes/ai/responses.rs`, `src/server/routes/ai/responses/lifecycle.rs`, `src/server/routes/ai/responses/lifecycle_tests.rs` | 先存储并返回 queued 200，再由后台 task 进入 chat input guardrail；deterministic normalization 400 只能晚到并变成 failed status | 必须把完整 input guardrail 移到 queue/persist/task 之前，并保证后台只消费已审核的原请求 |
| Streaming Responses | `src/server/routes/ai/responses_stream.rs` | 独立执行 input check 后调用动态 `run_stream` | stream 路径也必须携带 audited handle，不能成为 route-swap 旁路 |

## 设计方案

1. 用一个返回 owned `Result<Vec<GuardrailInputRecord>, GatewayError>` 的规范化函数替代
   `content_text -> Vec<&str>`。owned 结果允许安全容纳 JSON 序列化和 base64 解码
   后的正文；record 只在进程内携带 typed provenance 与原始扫描值，provenance 不编码
   进扫描文本，错误不能被 iterator/filter 静默丢弃。
2. active-input predicate 为 true 时，`check_chat_input` 先通过现有
   `RuntimeBinding`/`RuntimeHandle` surface 捕获一个 request-scoped immutable
   `RoutingSnapshot`，并把 handle/generation 保存在 crate-private
   `InputGuardrailAudit` 中。router snapshot surface 增加一个只读的 stable-candidate
   枚举：按请求 model group、endpoint capability 与 deployment lifecycle eligibility
   返回该 snapshot 内所有可能参与本请求 cache pricing、unary/stream、retry 和
   background dispatch 的 deployment/provider/model，不调用 selector，也不读取
   health、cooldown、active requests、RPM/TPM 或预算等瞬时状态。这样 audit 时暂时
   unhealthy/in-cooldown、但同 generation 内稍后恢复的 deployment 也已经被覆盖。
   profile derivation 与 record builder 都只读该 handle；审核完成后发布的新 snapshot
   不得进入本请求。

   `check_chat_input` 返回显式的 `Disabled` 或 `Audited(InputGuardrailAudit)` outcome，
   不能用可被调用方静默丢弃的裸 `Option` 表示。`Disabled` 只来自 active predicate
   false，并保留现有 dynamic selection；`Audited` 必须传入 cache pricing、
   `run_unary`/`run_stream`、每次 retry 与 background task。router/selection 增加
   capability-matching in-snapshot lease 方法；`budgeted.rs`/`execution.rs` 使用同一个
   handle 完成所有 attempts，不再调用会 load current snapshot 的 selector。

   builder 随后按稳定顺序生成互补记录：
   - 对每条 message，按原 part 顺序先收集全部普通 text leaves，过滤夹在其中的
     image/document/tool 等非 text parts；再依次生成三种 provider projection：
     `text_leaves.join("")`、`text_leaves.join(" ")`、`text_leaves.join("\n")`。
     `text_leaves.is_empty()` 时直接跳过这三种 projection，不创建空 record；只有至少
     一个 leaf 时才执行 join 与局部去重。
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
   - system-container projections 独立于上述通用 legacy record，并从 stable
     candidates 的实际 provider/model transform 分类：
     - 原生 Anthropic `separate_system_messages` 按请求顺序保留
       `System`/`Developer` role 的普通 text leaves，生成唯一且精确的 newline
       record；fixture 覆盖 System/User/Developer 与 System/User/System。
     - `Provider::Gemini` 的 `GeminiClient` 对 Google AI 与自身 Vertex endpoint
       使用同一 profile：只按请求顺序保留 `System` role，并对 outgoing
       `systemInstruction.parts[].text` 有序值生成 `join("")`、`join(" ")`、
       `join("\n")` 三种局部去重 records。`Developer` 不进入；feature-gated 的独立
       `Provider::VertexAI` 不是 `GeminiClient`，不得生成该 profile。
     - `Provider::Bedrock` 必须以 stable candidate 的 selected model 调用与 dispatch
       共用的 Bedrock model-ID/API-transform classifier。只有实际分类为
       `Converse`/`ConverseStream` 且不是 prompt-management ARN 时，按请求顺序保留
       `System` role；每条 System message 的 Text 原样保留，Parts 则镜像现有
       transform 过滤普通 Text 后以单 ASCII 空格连接，再对 outgoing `system[]`
       有序 text values 生成 direct/space/newline 三种局部去重 records。
       `Developer`、Invoke/InvokeStream 与会拒绝 request-level system 的
       prompt-management ARN 不生成。禁止仅按 provider enum 把所有 Bedrock model
       归为 Converse。
     不存在存活 leaf 时不生成 record；nonmatching profile 必须证明 zero-record，
     避免跨 provider 或跨 transform 误匹配。profile 集合在 input check 开始时固定，
     provider 调用后不得补检。
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
   false 时返回显式 `Disabled` outcome，立即沿用原请求 fast path，不创建
   `RuntimeHandle`、不固定 routing generation，也不执行任何 guardrail-specific
   JSON/MIME/base64/size normalization。现有 `GuardrailEngine::is_enabled()` 的公开
   语义保持不变，避免 additive amendment 改变调用方；既有独立 request validator 也
   不因该 fast path 关闭。engine unit test 与 route fixture 必须覆盖 global
   disabled、check_input disabled、空列表以及只有 disabled custom guardrail 四种
   false case，并证明最后一种不会因合法 base64 中的 unsupported MIME、非 UTF-8
   charset 或 invalid JSON 触发 guardrail-specific 400，且 cache pricing/dispatch
   仍可选择检查开始后发布的 current generation。

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
8. chat cache-hit pricing gate、cache-miss unary、chat streaming 和 Responses
   streaming 都必须接收第 2 条的完整 outcome。`Audited` 路径只能在 handle snapshot
   内执行 capability selection；unary/stream 的 budget/unpriced exclusion 与 provider
   retry 只改变同一 snapshot 内候选，所有 attempts 的 selected generation 都必须等于
   audited generation。cache hit 也不是例外：`ensure_chat_cache_pricing_gate` 必须
   接收 audited handle 并在同一 snapshot 内选择 priced deployment，不能重新读取
   `AppState.unified_router`。`Disabled` 路径才可调用现有 dynamic selection。
   source-boundary tests 必须枚举 outcome 的全部消费者，证明 active audit 不能被转换成
   dynamic/disabled path。
9. Background Responses 不得先 queue 再检查。`handle_background_response` 改为
   async，并在创建 response ID、写入 response store 或 spawn task 前调用完整
   `check_chat_input`。失败直接返回现有安全 error response；成功后把同一个未修改
   `ChatCompletionRequest` 与完整 `Audited(InputGuardrailAudit)` outcome 一起移交
   后台。`chat.rs` 提供一个 crate-private、仅允许此 lifecycle 使用的
   `handle_chat_completion_after_input_guardrail`；它跳过的只有已完成的 input
   check，仍执行 pinned-snapshot provider/retry、output guardrail、budget/callback
   等全部后续逻辑。source-boundary test 必须证明该 entrypoint 只有
   `responses/lifecycle.rs` 调用；lifecycle test 证明 check 在 queue/persist/spawn
   前恰好一次，失败时 response store、task registry 与 provider dispatch 都为零。
   通过后即使 barrier 期间发布新 deployment，background task 仍不得读取 current
   router，新 deployment zero-call，chosen generation 等于 audited generation。
   这条路径不通过 clone 后修改请求，也不允许对其他 handler 公开 unchecked API。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | request/message/function/content 规范化 | 每种载体独立 allow/block fixture |
| B-002/B-003 | message/request/provider-scoped projections + immutable audited routing context + typed records | 空 text-leaf/256 image-only、通用三投影、legacy newline、Anthropic exact-newline、GeminiClient/Bedrock Converse System-only 三投影、Developer/independent-VertexAI/Bedrock-Invoke/prompt-ARN zero-record、Bedrock 按 selected-model API transform 分类、ToolResult block views、record isolation、JSON/document raw+semantic，以及 route-swap/same-generation transient-state 下 chosen generation 等于 audited generation |
| B-004 | `check_input` 调用顺序 | mock provider 未被调用 |
| B-005 | `enforce` modified 分支 | mask 仍 fail-closed 且请求 DTO 未改变 |
| B-006 | fallible builder + audited handle handoff + background pre-queue gate | malformed document/serialization 返回安全稳定 400；cache hit pricing/background 不重读 current snapshot；background 失败时 queue/store/task/provider zero-call，通过时 request 与同一 audited handle 入 task、input check exactly-once |
| B-007 | document media gate | quote-aware 空参数/尾随分号检查 + MIME syntax/essence/charset table；无参数/唯一 UTF-8 正文解码，重复/非 UTF-8 charset 与 PDF/image/audio/URL 无网络且 fail-closed/保持范围 |
| B-008 | pre-builder active-input predicate | global/check_input/empty/all-custom-disabled fast paths 不创建 audited binding；同一多模态请求不触发 guardrail-specific normalization，provider content 与动态 routing 保持兼容 |
| B-009 | default batch method + builder limits | 256/2 MiB 边界、越界 zero-call、legacy custom default adapter、现有公开 error enum exhaustive/旧 custom compile fixture、custom output/enable/priority/name regression、config-created 与 manually-added moderation 都走 single-batch、32,768/32,769-byte 边界，以及 batch count-mismatch/input-limit failures 在 `fail_open` true/false 下均不可吞掉 |

## 数据流

`ChatCompletionRequest` 先进入 engine active-input predicate。false 时返回
`Disabled`，不创建 routing handle，继续现有 dynamic selection。true 时捕获一个
immutable `RuntimeHandle`，直接从其 snapshot 的 stable model/deployment metadata
与 capability 枚举完整候选 profiles；该枚举不调用 selector、不读取后续 generation，
也不按 health/cooldown/concurrency/RPM/TPM/budget 等瞬时状态过滤。

随后 messages 按顺序进入 bounded fallible fragment builder。每条 message 过滤非
text parts 后，仅在至少一个 text leaf 存在时形成 direct/space/newline 三种去重
projection；全请求普通 text leaves 另形成 legacy newline record。根据 audited
profiles，再分别形成 Anthropic 的 System|Developer exact-newline record、
`GeminiClient` 的 System-only `systemInstruction.parts` 三投影和 Bedrock
Converse/ConverseStream 的 System-only `system[]` 三投影；Developer、独立
VertexAI、Bedrock Invoke/prompt ARN 等 nonmatching transforms 不生成相应 record。
独立结构化字段形成 typed records；每个实际 outgoing Bedrock ToolResult block 的
ordered text sequence 另形成 block-scoped 三投影并局部去重。JSON 同时保留完整表示和
解码 string nodes；document MIME/charset gate 后，正文形成 raw + semantic records。

builder 通过 256/2 MiB 上限后，把 typed batch 一次交给 engine；fatal batch failures
在 `fail_open` 前分类，再由 `enforce` 映射。通过后把同一显式
`Audited(InputGuardrailAudit)` 交给 chat cache、unary/stream/retry execution 与
background task。cache hit pricing 和所有 provider attempts 都只在 audit snapshot
内选择；Background Responses 在 queued response 前完成检查，并把未修改 request 与
audit 一起交给窄作用域 post-input entrypoint。`Disabled` 才继续读取 current router。
现有 `GuardrailError` variants、trait 必需方法、engine 单字符串签名与原请求对象不变。

## 备选方案

- 对整个 `ChatCompletionRequest` 直接 JSON 序列化：拒绝，因为会扫描 image/audio
  base64、混入无关配置，并且 document 正文仍是编码数据。
- 只扫描 JSON/base64 原字符串：拒绝，因为不能识别 document 解码后的自然语言。
- 继续用 `split(';').next()` 剥离 MIME 参数：拒绝，因为会忽略 provider 可执行的
  charset 声明并放行 NUL-interleaved UTF-16 bytes。
- 对不支持 document 类型放行：拒绝，因为输入 guardrail 会继续存在公开 bypass。
- 自动提取 PDF/Office：超出范围且会新增复杂解析/资源消耗面。
- audit 后仍让 execution 或 cache pricing 读取 current router：拒绝，因为 publish
  会在审核与最终选择之间引入未审核 transform。
- 用 selector 当前可选结果推导 profiles：拒绝，因为同 generation 的
  health/cooldown 恢复会让审核时被过滤的 transform 重新成为候选。
- selected provider 改变时再次执行完整 input guardrail：拒绝，因为破坏
  exactly-once/single-batch 契约，background 返回 queued 后也无法同步返回相同错误。

## 风险

- Security: 支持媒体类型列表必须 fail-closed，不能被 MIME 大小写、parser 宽容接受的
  空参数/尾随分号、malformed/重复参数或非 UTF-8 charset 绕过；ToolResult provider
  views 必须覆盖 outgoing block 内实际相邻化，但不能跨 block/message 制造误匹配；
  system profiles 必须覆盖 GeminiClient/Bedrock Converse 的 role filtering，并固定
  audited generation 贯穿 cache pricing、retry 和 dispatch。
- Compatibility: 启用 input guardrail 的二进制 document 将被拒绝；active-input
  request 在 lifecycle 内固定 audited routing generation，并发 publish 只影响后续
  请求；inactive path 保持现有动态 routing。发布说明需明确。
- Public API compatibility: `GuardrailError` variants、
  `GuardrailEngine::check_input` 签名与 `add_guardrail` 调用方式保持不变；
  `Guardrail` 只增加有默认实现的 additive batch method；新增只读 record 与 batch
  error 从首次发布即 `#[non_exhaustive]`。旧 custom implementation、custom output
  override 与手工注册 built-in 必须有 compile/runtime regression fixture。
- Performance: 文档解码、message/ToolResult projections、request legacy view 与
  system-container views 增加 owned 载荷；空 text-leaf message 不分配 projection，
  局部去重避免重复计数，request-scoped handle 只持有一个 immutable snapshot `Arc`。
- Availability/Cost: records 有 256/2 MiB 硬上限；内置 OpenAI moderation 的
  eligible batch 另有保守 32,768-byte context 上限，必须 batch 为单次远程调用，
  并验证 response count。pinned snapshot 若在 lifecycle 内没有可用/priced candidate
  则返回既有 routing/pricing error，不能切到未审核的新 generation。
- Maintenance: DTO 新增文本载体时应在 exhaustiveness test 中显式分类。

## 测试计划

- [ ] Unit tests: 全 variant、request/message legacy/modern function、JSON raw+semantic、
      record isolation、空 text-leaf/256 image-only、通用三投影、legacy newline、
      Anthropic System/User/Developer 与 System/User/System exact-newline、
      GeminiClient/Bedrock Converse System/User/System direct/space/newline、
      Developer/independent-VertexAI/Bedrock-Invoke/prompt-ARN zero-record、Bedrock
      selected-model transform classification、ToolResult block views、duplicate key/BOM。
- [ ] Document tests: plain/csv、JSON raw+semantic/invalid/duplicate-key/BOM/depth-limit、`+json`、完整 MIME syntax、无参数/大小写 UTF-8/quoted UTF-8、`text/plain;`、`text/plain; charset=utf-8;`、连续/空参数段、重复 charset、UTF-16LE/其他 charset、non-charset 参数 fail-closed、bad base64、bad UTF-8，以及 Markdown numeric/named entity、HTML/XML/`+xml`/其他 `text/*`/PDF fail-closed。
- [ ] Batch tests: 256/2 MiB 边界、checked overflow/越界 zero external calls、local record isolation、legacy custom default adapter、现有公开 error enum exhaustive/旧 custom compile fixture、custom input-allow/output-block + disabled + non-default-priority/name regression、config-created 与 manually-added OpenAI array single-call、32,768/32,769 eligible-byte 边界、mixed/all whitespace eligibility、`Log` 非阻断、action-only `Mask` fail-closed、batch response-integrity/input-limit failure 在 `fail_open` true/false 下 fail-closed。
- [ ] Integration tests: blocked 发生在 provider 前，400 error envelope 稳定；engine disabled、`check_input: false` 与仅有 disabled custom guardrail 都在 builder 前不增加 guardrail-specific 拒绝并保持 DTO，malformed base64 仍由前置 request validator 按既有行为拒绝。
- [ ] Provider-boundary tests: Bedrock 的 space-join、Ollama 的 newline-join 与
      Gemini/Vertex 的 empty-join 转换结果分别与 builder 对应 adjacency view 相等；
      image-separated 及 split-message `ignore` + `all previous instructions` 都在
      provider 调用前被拒绝；Anthropic fixture 过滤中间 User 后生成 exact-newline；
      `GeminiClient`（Google AI/自身 Vertex endpoint）与 Bedrock
      Converse/ConverseStream 过滤中间 User 后生成 System-only 三投影。各自
      Developer、独立 VertexAI、Bedrock Invoke/prompt ARN 与其他 nonmatching route
      不产生对应 system record；Bedrock ToolResult blocks 继续保持既定隔离。
- [ ] Routing concurrency tests: 同步 barrier 在 audit 完成、最终 selection 前发布新
      snapshot，分别覆盖 cache-miss unary、chat cache-hit pricing、stream、retry 与
      background；新 deployment zero-call，所有 selected generation 等于 audited
      generation，input guardrail exactly-once。inactive 对照继续选择 current dynamic
      generation。另一个不发布 snapshot 的 fixture 令唯一 profile deployment 在 audit
      时 unhealthy/in cooldown、barrier 后恢复并被 retry/final selector 选中，证明
      stable profile enumeration 已预先覆盖它且不执行第二次 input check。
- [ ] Background Responses tests: normalization limit/invalid document/guardrail block 在
      queued persist/task spawn/200 前返回；失败时 store/task/provider zero-call，
      通过时未修改 request 与 audited handle 同时入 task、input guardrail
      exactly-once、task 不读取 current router，post-input entrypoint caller 仅 lifecycle。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试、`cargo test`。

## 回滚方案

回滚规范化函数与测试即可，无持久化迁移。若二进制 document 兼容性需要恢复，
必须另行设计可审核的解析器或按端点禁用 input guardrail；不得重新静默放行。
