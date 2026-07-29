# Product Spec

## Linked Issue

GH-1108 / #1108

- `complexity: large`
- `spec_approval: pending_maintainer`
- `draft_source: 2026-07-26 current conversation ("implxauto解决所有的issue和prs")`
- `required_approval: maintainer approval bound to the final spec head`

## 用户问题

Gemini Developer API 的静态模型目录落后于 Google 2026-07-21 的正式发布：
`gemini-3.6-flash` 与 `gemini-3.5-flash-lite` 已 GA，但 provider 仍无法准确公开、
校验和计费这两个 exact model ID。与此同时，目录中还混有已退役、未证实或不属于
Developer API 通用 chat 入口的 ID；只追加两个新 ID 会继续把不可兑现的模型支持暴露给
用户。

新模型还收紧了请求契约：`temperature`、`top_p`、`top_k` 不应再发送，最后一个非空
turn 为 `model` 的 prefill 请求必须被拒绝。若 SDK 仍声明这些参数可用、静默删除参数，
或把错误留给上游通用 `400`，用户会得到互相矛盾且难以定位的行为。

本变更需要把“模型存在、仍可调用、价格正确、请求契约可执行”作为一个可审计的
Developer API 行为面，同时保持 Vertex AI、Interactions API 和其他产品入口独立。

## 官方证据基线

本 spec 于 2026-07-26 依据以下 Google 官方页面起草：

- `gemini-3.6-flash` 与 `gemini-3.5-flash-lite` 于 2026-07-21 GA：
  <https://ai.google.dev/gemini-api/docs/changelog>
- exact IDs、1,048,576 input limit、65,536 output limit、采样参数迁移与 prefill 约束：
  <https://ai.google.dev/gemini-api/docs/latest-model>
- 两个模型的 exact model page 与能力证据：
  <https://ai.google.dev/gemini-api/docs/models/gemini-3.6-flash>、
  <https://ai.google.dev/gemini-api/docs/models/gemini-3.5-flash-lite>
- 当前 lifecycle/shutdown 状态：
  <https://ai.google.dev/gemini-api/docs/deprecations>
- Gemini Developer API 定价（paid Standard tier）：
  <https://ai.google.dev/gemini-api/docs/pricing>
- Google `Content` wire schema：`role` 是 optional，省略时服务默认 `user`，显式值只能是
  `user` 或 `model`：
  <https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rpc/google.cloud.aiplatform.v1>

最后一项仅用于共享 `Content` message 的 wire-shape/default 证据，不证明 Vertex AI
availability；其他证据只证明 Gemini Developer API。

## 目标

- 在 Gemini Developer API chat catalog 中准确公开两个新 GA exact model ID。
- 对两个模型公开可核验的 context、output、能力和价格事实。
- 让 supported params、请求校验和最终请求体遵循同一个新模型请求契约。
- 在网络前拒绝已废弃采样参数和 model-turn prefill，禁止静默删除或透传。
- 对当前公开 chat catalog 的每个 ID 记录 exact 官方 lifecycle disposition。
- 提供显式 opt-in、凭证安全、结果可审计的 Developer API live smoke。

## 非目标

- 不实现或迁移到 Interactions API、Live API、embedding、image、video 或 managed-agent
  协议。
- 不把 Gemini Developer API 的模型、价格或 availability 推断到 Vertex AI。
- 不实现 GH1112 的共享 Google catalog ownership、Vertex overlay 或认证收敛。
- 不实现 GH1111 的完整 `ToolUse` / `ToolResult` 回路。
- 不实现 GH1113 的 pricing authority、unknown-cost 或 spend/budget 语义收敛；但当前
  gateway 的预算预留、结算与 provider-aware pricing endpoint 都以嵌入式
  `model_prices_extended.json` 为运行时 authority，因此为两个 exact Developer IDs 增加
  与 B-002 相等的 runtime rows 和回归测试属于本 issue 的必要正确性，不是新 authority。
- 不把 Gemini Developer API paid Standard 的价格推断为 Batch、Flex 或 Priority tier
  价格，也不在本 issue 增加这些 tier 的计费契约。
- 不自动联网刷新目录，不在正常构建、测试或运行时隐式执行 live smoke。

## Behavior Invariants

1. **B-001** Gemini Developer API chat catalog 必须以区分大小写的 exact model ID
   公开 `gemini-3.6-flash` 与 `gemini-3.5-flash-lite`；前后缀、大小写变体或相似名称
   不得命中这两个模型。
2. **B-002** `gemini-3.6-flash` 必须公开 1,048,576 input token limit、65,536 output
   token limit、Gemini Developer API paid Standard tier 每百万 input tokens 1.50 USD、
   每百万 output tokens 7.50 USD；
   `gemini-3.5-flash-lite` 必须公开相同 token limits、0.30 USD input 与 2.50 USD
   output。价格单位必须明确为每百万 tokens，不能与 per-token 值混用；Batch、Flex、
   Priority 或其他 tier 不得复用这些值并宣称已由本 invariant 验证。
3. **B-003** 一个 Developer API chat model 只有在 exact ID、当前 lifecycle 和通用
   chat 入口均有 Google 官方正证据时才可被公开；retired、shutdown、unverified、
   仅属于其他产品或只有近似名称的条目必须 fail closed 为不公开。
4. **B-004** 每个现有公开 chat model 必须有独立 disposition：
   `available_exact`、`retired`、`shutdown`、`unverified` 或 `other_product`。
   migration 前 17 个 exact ID 的 disposition、官方 URL、`reviewed_at=2026-07-26` 与
   reason 必须与 tech spec 的 “Frozen pre-refresh disposition ledger” 逐行完全相等；
   其中 `gemini-2.0-flash-thinking-exp` 的官方 frozen shutdown date 是
   `2025-12-02`，不得与 `gemini-2.0-flash-exp` 的其他 lifecycle date 混用；
   implementation 不得自行重新分类。缺失、冲突或过期到已越过 shutdown date 的证据不得
   降级为“继续沿用旧目录”；三个 `unverified` ID 即使 live/list 偶然返回，也不能在没有
   spec amendment 的情况下升级为 `available_exact`。
5. **B-005** 对 `gemini-3.6-flash`、`gemini-3.5-flash-lite` 以及显式声明采用同一后续
   契约的模型，supported params 不得包含 `temperature`、`top_p`、`top_k`；
   typed `temperature`/`top_p` 省略或显式 JSON `null` 均按现有 `Option` 语义视为
   absent；flattened `extra_body`/canonical `extra_params` 中的
   `top_k: Value::Null` 必须被 shared normalizer 消费并删除为 absent。三者的任何 non-null
   值（包括看似默认的数值）必须在网络前返回稳定的 OpenAI-compatible
   invalid-request 错误；最终 upstream body 不得包含 `temperature`、`topP`、`top_k`
   或 `topK`，不得静默透传。Gemini-native `generateContent`/
   `streamGenerateContent` 的等价字段是
   `generationConfig.{temperature,topP,topK}`：缺失或 JSON `null` 必须作为 absent
   消费，任一 non-null 值必须在预算预留和网络前以同一稳定错误拒绝。
6. **B-006** 对 B-005 模型，系统必须一次性把原 messages 规范化为实际提交给 Gemini
   的 `contents` 与独立 `systemInstruction`；prefill gate 和 serializer 必须消费这同一
   结果，不能再依据 raw role 列表二次推导。meaningful System 与 Developer 的文本按原
   messages 顺序组成同一个 `systemInstruction.parts`，二者都不进入 `contents`；
   System 或 Developer turn 含 non-text 或其他无法表示为 instruction text 的 meaningful
   payload 时必须在网络前拒绝，不能静默丢弃。trailing semantically-empty turn 不进入
   `contents`，但 instruction extraction 不能遮蔽此前 terminal model：最终 `contents`
   为空或末项 role 为 `model` 时必须在网络前拒绝。因此 developer+user 必须保留
   Developer instruction 且保留 user contents；assistant+developer 仍因最终 contents
   以 model 结尾而拒绝；all-system/developer 和 all-empty 也拒绝，user+system 可通过。
   既有 Gemini `ToolUse`/`ToolResult` 序列化与完整 tool-loop callability 仍归 GH1111，
   不是本 invariant 的通过条件，也不是 GH1108 implementation dependency。
   Gemini-native body 不重新做 OpenAI role conversion；其 shared native preflight 必须
   fail closed 解析 `contents`，跳过 trailing semantic-empty content 后要求至少一个
   meaningful content。对每个 meaningful content，显式 exact `user`/`model` 保留；
   role field 缺失按 Google `Content` schema deterministic normalize 为 `user`，
   因此 terminal omitted-role 与 terminal explicit-user 都是非 prefill，可通过；
   terminal explicit `model` 是 prefill。role 为其他 string、非 string、或 content/
   parts shape 无法确定时必须 fail closed，不能把 unknown/nonrepresentable role 猜成
   user。只有所有 meaningful content 的 role 都能按此闭集唯一归一化时，sequence 才算
   unambiguous。`systemInstruction` 不得
   遮蔽 terminal `model` content。JSON-null/blank-text-only parts 可判定为空；任何其他
   part 均为 meaningful，保持原 body 顺序且不由本 issue 重写 native contents。
7. **B-007** 两个新模型的 OpenAI-compatible positive parameter allowlist 必须恰为闭集
   `{max_tokens, stop, stream}`：`max_tokens`/`stop` 进入既有 Gemini generation body，
   `stream` 只选择 streaming transport、不进入 body。`temperature`、`top_p`、`top_k`
   按 B-005 排除；当前 serializer 未兑现的 `tools`、`tool_choice`、`response_format`
   与 `max_completion_tokens` 同样排除。provider 公布值、preflight、parameter mapping
   与 serializer/transport disposition 必须对该闭集相等，不能仅保持彼此一致却全部
   拒绝，也不能把 map passthrough 冒充实际支持。`stream_options` 不是模型参数，而是
   gateway streaming settlement metadata；它不得加入上述 positive allowlist，也不得
   进入 Gemini upstream body。API boundary 可按闭合 wire shape 对所有请求拒绝 unknown
   字段或非 boolean `include_usage`，但不得借此消费合法 metadata。共享 gateway builder
   继续按既有行为生成只含 `include_usage=true` 的 canonical core `stream_options`；
   “保留”只表示这个 canonical object 在最终 selection 前不被 take/drop，本 issue 不生成、
   保存或暴露额外的 wire-preference usage 状态。等 alias/fallback 已解析且最终
   deployment 已选定后，只有最终选中的 provider 是 Gemini Developer、exact model 是上述
   两个新 ID 时才校验并消费 canonical `include_usage=true`。direct、
   alias 与 fallback 到该契约都必须使用最终身份得到相同行为；最终选中 OpenAI、
   OpenRouter 或其他 provider 时，post-selection hook 的 `stream_options` 输入/输出必须
   相等，selection failure 也不得修改原请求。unknown/non-bool wire shape 在 API
   boundary fail closed；对所选 GH1108
   Gemini 模型，合法 metadata 与非 streaming 请求并存或内部 canonical metadata
   不一致时也必须在网络前 fail closed。Responses unary/stream 的 shared adapter 必须
   无损捕获 `top_k`：omitted/null 为 absent，non-null 保留到 canonical contract 后拒绝；
   `max_output_tokens` 只能 canonicalize 为 `max_tokens`，不得同时生成被本模型排除的
   `max_completion_tokens`，并必须最终映射到 `generationConfig.maxOutputTokens`。
   unary response cache 不得成为绕过点：当前 cache canonical policy 会删除
   `stream_options`，因此任何 non-stream request 只要携带该 metadata，就必须在 key lookup/
   store 前安全 bypass cache，再由最终 selected-model hook 判定；cache 中已有合法同 key
   response 也不得返回。
8. **B-008** 新模型的 neutral catalog cost lookup 与当前 gateway runtime
   `PricingService` 必须都返回 B-002 的确定值并保持单位一致。默认嵌入式 pricing source
   必须以 exact Developer keys `gemini/gemini-3.6-flash` 与
   `gemini/gemini-3.5-flash-lite`、`litellm_provider=gemini` 保存 per-token 值，使
   OpenAI-compatible chat、Responses、legacy completions、Gemini-native
   generateContent/streamGenerateContent 的预算预留和结算，以及 provider-aware
   `/v1/pricing/calculate` 都不会按默认 unpriced reject 策略拒绝这两个 ID。不得新增
   unprefixed 或 Vertex rows；`provider=vertex_ai` 对两个 ID 必须继续 fail closed。
   这些值只代表 Gemini Developer API paid Standard tier；Batch、Flex、Priority 与
   其他 unknown model/tier 的全局 pricing 行为不得由本 issue 改写、复用或伪装为
   已知零成本。
9. **B-009** catalog 列表必须稳定排序、无重复；同一不可变证据输入在重复或并发读取时
   返回相同 ID、metadata、价格和请求契约。
10. **B-010** live smoke 默认关闭，只能由文档声明的单一显式 opt-in 环境变量开启；
    opt-in 与 Developer credential 任一缺失时均不得联网。supported Developer aliases
    的闭集与 production `GeminiConfig::from_env` 一致，仅为 `GOOGLE_API_KEY` 与
    `GEMINI_API_KEY`，且两者同时存在时 `GOOGLE_API_KEY` 优先。closed 13-case offline
    actual-env fixture 必须分别覆盖：全 unset；仅 GOOGLE key；仅 GEMINI key；仅 opt-in；
    opt-in+GOOGLE；opt-in+GEMINI；opt-in+两 key 并证明 GOOGLE precedence；opt-in+
    Vertex-only `GOOGLE_CLOUD_PROJECT`+`GOOGLE_CLOUD_LOCATION` 且无 Developer key；
    opt-in+Vertex pair+`GOOGLE_APPLICATION_CREDENTIALS` 且无 Developer key；以及
    opt-in+单独 project、opt-in+单独 location、opt-in+GOOGLE key+Vertex pair、
    opt-in+GEMINI key+Vertex pair。无 opt-in 的两个 key cases、无 Developer
    key 的 opt-in/partial-Vertex/full-Vertex cases均 network counter=0；只有
    opt-in+任一 Developer key 才可进入 fake/显式真实 Developer transport，Vertex env
    不得满足此 gate，Developer key 必须按 production 顺序先于 Vertex。
    每个 case 必须通过 `env_clear` + 精确 env set 的隔离子进程（或所有 env reader 共用的
    完全注入配置）测试真实 env boundary；普通并行测试不得调用 `set_var`/`remove_var`，
    case 间不得泄漏 opt-in 或 credential。普通单元测试、全量测试和应用启动不得隐式触发。
11. **B-011** opt-in live smoke 必须分别记录静态目录、官方 list-models/get-model
    exact 结果与最小调用结果，并把失败分类为闭集
    `{auth, quota, not_found, protocol, network}`，且 source→class 映射必须闭合、确定：
    exact structured Google reason 优先于 typed provider error，typed provider error 优先于
    HTTP status fallback；禁止按 error message substring 分类。Authentication/401/403/
    `UNAUTHENTICATED`/`PERMISSION_DENIED` → auth；RateLimit/QuotaExceeded/402/429/
    `RESOURCE_EXHAUSTED` → quota；ModelNotFound/404/`NOT_FOUND` → not_found；
    InvalidRequest、serialization/parsing/schema/exact mismatch、其他 4xx 和自然结束后的
    missing step → protocol；Network/Timeout/ProviderUnavailable/streaming transport、
    408/504/5xx/`UNAVAILABLE`/`DEADLINE_EXCEEDED` → network。显式 runner
    execution-terminal event
    优先于以上 response/error facts；deadline 与 external cancellation/interruption
    竞争时，原子记录的首个 execution-terminal event 胜出，后到 execution event 不得
    重分类。artifact finalization 是其后的独立 required commit gate，不参与这次 CAS；
    它不得抹掉已记录的 execution terminal fact。已发出的 transport
    请求达到 deadline timeout 必须是 `status=failed`、`error_class=network`，并记录
    timeout termination；
    无 external terminal event 时，任一必需步骤在自然结束后仍未执行必须是
    `status=failed`、`error_class=protocol`，并记录 missing-step termination，不得报告
    整体通过；external cancellation/interruption 后未执行的步骤按 B-014 记为
    incomplete。每个 record 必须保存 credential-free
    的 closed typed attempt/termination facts；只有真实取得并解析的响应事实才可保存为
    `observation`，不得为 auth、timeout、cancel 或未收到/未解析响应的失败填充空字符串、
    零值或成功形状。`passed` 必须保存完整、step-specific 的 closed typed
    `observation` 与匹配 digest；`failed`/`incomplete` 可没有 response observation，若只
    得到部分事实则只能保存相应的 typed partial observation。静态
    observation 记录 exact ID、catalog/lifecycle、limits、paid Standard pricing、
    capabilities/features 与 evidence；list/get observation 记录请求 exact ID、远端
    exact match/resource/methods/limits；minimal-call observation 记录请求 exact ID、
    response model version、candidate/finish/text 与 usage token facts。passed record 缺少
    对应 step 的必需 observation 字段时必须按 protocol failure 处理。required keys 恰为
    两个 exact ID 各自的 `static_snapshot`、`list_models`、`get_model`、`minimal_call`
    共 8 个 per-model keys，aggregate 另算。official list-models 必须从无 page token 开始，
    遍历每一页直到响应不再含 non-empty next-page token 后才算 complete；重复 token、
    malformed token、任一中间页失败，或 100 页后仍有 next token 都必须 deterministic
    fail closed，不能用已收集的 partial pages 报 passed。一次完整 paginated traversal
    可派生两条独立 per-model observations，但每条的 key/model/`requested_exact_id`/
    case-sensitive exact match 必须与对应 exact ID 一致；后续页命中的模型必须被发现，
    跨页重复 exact match 必须失败，不能以一条 global list record 或另一模型记录替代。
12. **B-012** live smoke、错误、Debug/Display、命令回显和持久化 artifact 均不得包含
    API key 或其他 credential；redaction 负例必须安装 captured tracing subscriber，
    并用 sentinel 凭证证明 tracing、stdout、stderr、error、Debug 与 artifact 所有
    sink 明文命中均为零。
13. **B-013** Developer API 证据不得扩大 Vertex AI availability、endpoint、region、
    auth 或 pricing 声明；任何 Vertex 结果只能作为独立信息记录，不能满足本 spec 的
    Developer API 正证据，也不能由本 spec 改变 Vertex 行为。
14. **B-014** live smoke 被取消、中断或部分完成时必须保留已完成步骤与终止原因，
    typed artifact 每条记录必须包含同一 invocation 的 non-empty `run_id` 和可审计的
    `termination_reason` 与 attempted/response-received facts。`observation` 与
    `observation_sha256` 是同生同灭的可选 pair：无真实可解析 observation 时二者都缺失；
    有完整或 partial typed observation 时必须先脱敏、canonicalize，并保存匹配 digest；
    digest 不能替代 observation。只有外部 cancel/interruption 为
    `status=incomplete` 且 `error_class` 缺失；transport deadline 按 B-011 归
    `failed/network`。runner 必须在开始下一 required step 前等待当前 step 的
    credential-free snapshot 原子持久化完成；真实 external cancellation/interruption
    发生时必须停止后续网络调用、flush 当前 termination/incomplete state，并允许重新读取
    已完成步骤。取消后的当前与尚未开始 keys 归 `incomplete_keys`，不得被后续
    missing-step aggregation 改写为 protocol failure；只有无 external terminal event 的
    自然结束缺项才是 missing-required-step。每次重试必须生成不同 run_id，不得复用旧凭证
    输出或把先前部分成功冒充当前完整通过。manual runner 默认把 credential-free snapshot
    持久化到 `artifacts/live/GH1108/<run_id>.json`，只有显式
    `LITELLM_RS_LIVE_GEMINI_OUTPUT_DIR` 才覆盖目录；temp + atomic replace，支持 POSIX
    permissions 的平台必须把临时/最终文件限制为 `0600`，其他平台按文档 best-effort
    contract。成功与中断 artifact 均不得自动删除，provider docs 必须给出检索与显式清理
    命令；implementation 必须在 repository ignore policy 加入精确 anchored pattern
    `/artifacts/live/GH1108/`，避免 retained manual artifacts 被纳入版本控制，且不得用更宽
    pattern 代替此验收。offline tests 仍注入临时 sink，不写默认目录。aggregate 必须对
    B-011 的 8 个
    per-model keys 逐项结算，一个模型的成功不能替代另一个。任一 observation fact 不同都
    必须产生不同 canonical observation/digest，不能落成相同的成功记录。runner 的闭合
    返回类型必须区分 `Committed(LiveArtifact)` 与
    `UncommittedFinalizationFailure`。execution terminal 使用 first-event CAS 的闭集
    `{transport_timeout, externally_cancelled, externally_interrupted}` 或 none；
    finalization 使用闭集 `{persisted, persistence_failed}`。若 external cancel/
    interruption 后最终 flush 成功，返回 committed artifact，仍为
    incomplete/no-error-class/external reason；若任一逐步 snapshot 或最终 flush 失败，
    不得声称失败 snapshot 已持久化，必须返回 typed
    `UncommittedFinalizationFailure`，其整体事实固定为
    `status=failed`、`error_class=protocol`、
    `termination_reason=artifact_persistence_failed`，同时以独立 typed
    `execution_terminal` 保留已由 CAS 赢得的 cancel/interruption/deadline（若有），并
    指向可重读的 last committed snapshot。last committed snapshot 保持原样，不得伪造
    新的 failed record；发生持久化失败后不得发起后续网络调用。
15. **B-015** 除明确列出的新模型、evidence disposition 和新请求契约外，已有仍受支持
    模型的 exact ID、能力、合法参数、认证、endpoint 与响应转换保持兼容；不因刷新
    意外删除或改变无关模型。
16. **B-016** catalog refresh 必须保留 evidence reviewed-at 与官方 source URL；官方
    页面互相冲突、来源不可访问或模型只存在于非官方二手资料时按 B-003 fail closed，
    不能以 live smoke 单次成功替代 lifecycle/source 记录。
17. **B-017** 两个新模型公开的 `ModelInfo.capabilities` 必须恰为闭合集
    `{ChatCompletion, ChatCompletionStream, GeminiGenerateContent}`，
    `supports_tools=false`；model feature
    flags 必须恰为闭合集
    `{MultimodalSupport, StreamingSupport, SystemInstructions}`。这里
    `MultimodalSupport` 只表示当前 serializer 可提交 inline base64 image，
    `StreamingSupport` 表示 streaming transport；`GeminiGenerateContent` 表示
    gateway 已公开且 provider capability selector 可达的 native unary/stream
    `generateContent`/`streamGenerateContent` transport，
    `SystemInstructions` 表示 `systemInstruction` 序列化；公开 metadata 只能反映
    本 provider 当前可调用面，Google 产品页面声明能力不能替代本 provider serializer/
    transport 证据。由于当前 transformer 不消费 `tools`、`tool_choice` 或
    `response_format`，两个新模型不得广告 ToolCalling、FunctionCalling 或 JsonMode；
    这些能力留给 GH1111 或后续契约完成后再独立启用。任何其他集合外能力也 fail closed
    为不广告，尤其包括 CodeExecution、BatchProcessing、Realtime API/streaming、
    ContextCaching、SearchGrounding、VideoUnderstanding、AudioUnderstanding、Computer
    Use、audio/image generation、Live 与 Interactions。实际 capability route selection
    必须对 Gemini deployment 的最终 exact model 查询 neutral registry 并使用上述闭集；
    不得只修公开 metadata。neutral registry 无 exact record 的既有 Gemini model 继续回落
    provider-wide capability behavior，禁止为实现本 model-specific 收紧而全局移除 Gemini
    ToolCalling。

## 验收标准

- [ ] 两个新 exact model ID 的 Developer API metadata、limits、能力闭合集与 paid
      Standard 价格符合 B-001/B-002/B-017，并有 registry 与 cost 行为测试；Batch/Flex/
      Priority 不在该价格断言范围内；默认 embedded PricingService 的 provider-aware
      lookup、chat/Responses/completions 与 native reservation/settlement 使用相同 exact
      per-token 值，Vertex lookup 保持 fail closed。
- [ ] migration 前 17 个 Developer chat exact IDs 的 disposition/source URLs/
      `reviewed_at=2026-07-26`/reason 与 tech frozen ledger 逐行完全相等；shutdown、
      retired 与 unverified fixture 不被公开，implementation 不自行升级 unverified。
- [ ] 新契约模型的 supported params 与最终请求体均不含三项废弃采样参数；typed
      `temperature/top_p` JSON `null` 与 flattened `top_k: null` 均由 shared normalizer
      消费为 absent，任何 non-null 输入在网络前得到稳定错误；native
      generationConfig 三个等价字段及 native final-model prefill 在预算/网络前得到同一
      fail-closed 结果。
- [ ] one-shot role/content normalization 直接产生 serializer-ready `contents` 与
      `systemInstruction.parts`；meaningful System+Developer text 按原消息顺序组成 parts
      且都不进 contents，System/Developer non-text 或不可表示 payload 均 pre-network 拒绝；
      developer+user 不丢 instruction/user contents，assistant+developer 仍因 final model
      prefill 拒绝；all-system/developer、non-empty model+trailing-empty 和 all-empty 也
      拒绝，user+system 可通过；完整 tool-loop callability 不作为 GH1108 acceptance。
- [ ] 两个新模型的 positive parameter allowlist 精确等于
      `{max_tokens, stop, stream}`；tools/tool_choice/response_format/max_completion_tokens
      与三项 deprecated sampling params 均不在集合，provider/preflight/map/serializer
      fixture 对集合和值去向完全一致；gateway builder 保留 `stream_options` 直到最终
      deployment 选定；这里保留的是既有 builder 生成、只含 `include_usage=true` 的
      canonical core metadata，不生成/保存额外 usage preference state。direct/alias/fallback
      到两个新 Gemini 模型后才校验并消费
      `include_usage=true` 并到达 streaming transport、从不进入 upstream body；
      OpenAI/OpenRouter 在 post-selection hook 前后值相等、selection failure 不修改请求，
      所选 GH1108 Gemini 模型的非法/不一致 metadata 与 non-stream 组合均 pre-network
      拒绝。Responses unary/stream 对 `top_k` non-null 无损拒绝、null/omitted absent；
      `max_output_tokens` 只产生 `max_tokens` 且最终命中 maxOutputTokens，
      `max_completion_tokens=None`。cache regression 必须先填充合法同 key response，再证明
      non-stream + stream_options 在 lookup/store 前 bypass、cache return=0、network=0 和
      stable invalid-request；合法无 metadata 请求仍可命中 cache。
- [ ] 公开请求入口矩阵闭合覆盖：OpenAI chat unary/stream、legacy completions
      unary/stream、Responses unary/stream 都在最终 selected Gemini identity 上进入
      shared chat preflight；`/v1`、`/v1beta`、`/gemini/v1`、
      `/gemini/v1beta` 的 native unary/stream 共 8 个 endpoint shape 都进入 shared native
      preflight；Batch 只代理 OpenAI/OpenAI-compatible batch provider lifecycle，不能路由
      到 Gemini provider，其他 capability route 也不可选择 Gemini chat capability；
      native terminal omitted-role 按 official default=user 与 explicit user 同样通过，
      explicit model 拒绝，explicit null/unknown/non-string role 与不可判定 sequence
      fail closed。
- [ ] catalog 重复/并发读取稳定排序且 metadata/price/contract 一致。
- [ ] capability dispatch 对最终 Gemini exact model 查询 neutral registry：两个新 ID 只可
      由 ChatCompletion/ChatCompletionStream/GeminiGenerateContent route 选择，
      ToolCalling/FunctionCalling route 不可选择；未命中 exact record 的既有 Gemini model
      保持 provider-wide compatibility，Gemini ToolCalling 未被全局移除。
- [ ] live actual-env matrix 对 `GOOGLE_API_KEY` 与 `GEMINI_API_KEY` 分别覆盖 opt-in
      缺失/存在，并覆盖双 key 的 GOOGLE precedence；opt-in-only、Vertex project/location
      pair、pair+service-account、project-only、location-only 且无 Developer key 均零
      network；每个 Developer alias 与 Vertex pair 并存时都锁定 Developer-first。只有
      opt-in+Developer key 可命中 fake/显式真实 Developer transport；每
      case 使用 `env_clear` + exact env 隔离子进程命中 production reader，普通并行测试不
      调用 `set_var`/`remove_var` 且无 case 泄漏。
- [ ] opt-in live smoke 对两个新 exact ID 各自产生 static/list/get/minimal-call 共 8 个
      per-model required records
      含 `run_id`、attempt/termination facts、status-dependent optional typed
      observation/digest pair 的 closed artifact；passed 必须是完整 step-specific
      observation，failed/incomplete 不得伪造 response facts，partial 只保存实际取得的
      typed fields；list-models 必须完整遍历 pagination，覆盖 later-page match，并对
      repeated/malformed token、中间页失败、100-page bound 与 cross-page duplicate
      fail closed；完整 traversal 派生的两条 list observations 仍分别精确绑定 requested
      model。aggregate 以闭合 8-key `(step, model)` set 结算且缺任一项即失败。不同
      observation 不产生相同成功记录；transport timeout 为 failed/network，外部
      cancel/interruption 为无 error_class 的 incomplete；runner-level fixture 必须在真实
      step 阻塞点触发 cancellation，重新读取逐 step 原子 snapshot，证明已完成步骤保留、
      后续网络调用为零且 retry 使用新 run_id；cancel 后 final flush failure 必须返回
      typed uncommitted failed/protocol/artifact_persistence_failed，保留
      execution_terminal=externally_cancelled、last committed snapshot 可重读且不伪造
      final artifact。manual artifact 默认持久保留在
      `artifacts/live/GH1108/<run_id>.json`，显式 output-dir override、atomic replace、
      POSIX `0600`/其他平台 best-effort、检索与清理命令均有测试/文档；repository ignore
      policy 含精确 `/artifacts/live/GH1108/`，offline sink 只写临时目录。
- [ ] captured tracing subscriber 下，sentinel 凭证在 tracing、stdout、stderr、错误、
      Debug/Display 和 artifact 中均无明文命中。
- [ ] Vertex AI、Interactions API、GH1111 tool loop 与 GH1113 pricing authority
      acceptance 均未被本实现顺带修改或宣称完成。
- [ ] 新增关键 catalog/request-validation 分支覆盖 100%，新增代码总体 line coverage
      至少 80%。
- [ ] `cargo fmt --check`、`cargo check`、strict Clippy、`cargo test` 与 SpecRail gates
      全部通过。

## 边界检查

| 边界类别 | 判定 |
| --- | --- |
| Empty / missing input | covered: B-005、B-006、B-010、B-011。typed/flattened null、空 contents、all-system/developer、缺 opt-in/credential、缺 smoke step 都有确定结果。 |
| Error and failure paths | covered: B-003、B-005、B-006、B-011、B-014、B-016。未知证据、非法参数、serialized prefill、timeout/cancel 和 live failures 均 fail closed。 |
| Authorization / permission | covered: B-010、B-012、B-013。Developer credential 只用于 opt-in Developer 请求，不扩大 Vertex 权限。 |
| Concurrency / race / ordering | covered: B-009。目录与契约为稳定不可变快照。 |
| Retry / repetition / idempotency | covered: B-009、B-014。重复目录读取幂等，每次 smoke retry 使用新 run_id 且不复用部分成功。 |
| Illegal state transitions | covered: B-003、B-004、B-011。无证据/部分 smoke 不能变为 advertised/passed。 |
| Compatibility / migration | covered: B-005、B-007、B-015、B-017。明确行为收紧与新模型能力闭合集，其余模型与入口保持兼容。 |
| Degradation / fallback | covered: B-003、B-005、B-016、B-017。旧目录、silent drop、二手资料、近似 ID 和未兑现能力均不能冒充成功。 |
| Evidence and audit integrity | covered: B-004、B-011、B-012、B-014、B-016、B-017。每个公开 ID/能力与 passed smoke 结论绑定完整可复原的 typed observation；失败/中断保存真实 attempt/termination 与可选 partial observation，hash-only 或伪造响应不构成证据。 |
| Cancellation / interruption / partial completion | covered: B-011、B-014。transport timeout 为 failed/network；只有外部 cancel/interruption 为无 error_class incomplete。 |

## 边界情况

- `gemini-3.6-flash-preview`、`Gemini-3.6-Flash` 与
  `foo-gemini-3.6-flash` 均不能命中 stable exact ID。
- typed `temperature/top_p` 省略或 JSON `null` 视为 absent；flattened
  `extra_body`/`extra_params` 的 `top_k: null` 被消费并从 map 删除；默认或非默认的任何
  non-null 数值均按 B-005 拒绝，最终 serializer 对四种 upstream key 均零输出。
- meaningful user/tool turn + trailing empty assistant turns 在 strip 后保留 user/tool
  结尾且 serializer 不产生尾部 model；non-empty model + trailing empties 在 strip 后
  仍拒绝；全部 turns 均 semantically empty 时按空 normalized sequence 拒绝。
- meaningful System+Developer text 按原消息顺序组成 `systemInstruction.parts` 且都不进
  `contents`；developer+user 保留两边内容，System 或 Developer non-text/不可表示
  instruction 都 pre-network 拒绝。assistant+system 与 assistant+developer 的 raw last role 虽非
  assistant，最终 `contents` 仍以 model 结尾，必须拒绝；all-system/developer 产生空
  contents 并拒绝；user+system 产生 user contents + systemInstruction，可通过 prefill gate。
- native terminal explicit `user` 与 omitted role 都 normalize 为 user 并通过；
  terminal explicit `model` 拒绝。explicit null、unknown string、其他 non-string role、
  malformed/无法判定 sequence fail closed；4 prefixes 的 unary/stream fixtures必须逐项
  相同。
- `max_tokens`/`stop` 的 body mapping 与 `stream` transport selection 是新模型仅有的三项
  positive params；tools/tool_choice 的 passthrough、response_format/max_completion_tokens
  的字段存在都不能冒充 serializer support。`stream_options` 是 gateway metadata 而非
  第四个 provider param；共享 builder 生成 canonical core metadata 并在 pre-selection
  阶段保持该 object 不被 take/drop，不承诺 client wire `include_usage=false` 原值透传；
  最终选中两个新 Gemini 模型后才消费。direct/alias/fallback 的 canonical
  include_usage=true 可到达 stream transport 但不得进入 upstream body；OpenAI/OpenRouter
  在 post-selection hook 前后值相等，selection failure 不修改请求，所选新模型的
  non-stream + stream_options 必须拒绝。即使同 messages/model 的合法请求已经填充 cache，
  metadata-bearing non-stream request 也必须在生成/读取 key 前 safe bypass，不能返回旧
  response；key collision fixture 不得用改 prompt 或清 cache 伪造 miss。
- Responses `top_k:null`/omitted 均按 absent，non-null 必须穿过 unary/stream shared
  adapter 到 selected-model contract 后 network=0 拒绝；合法 `max_output_tokens` 的
  canonical request 必须是 `max_tokens=Some(value), max_completion_tokens=None`，最终 body
  只出现 `generationConfig.maxOutputTokens`。
- capability router 以 deployment 的 case-sensitive final exact model 查询 neutral
  registry；两个新 ID 的 ToolCalling/FunctionCalling eligibility 为 false，但没有 exact
  record 的既有 Gemini model 仍使用原 provider-wide能力，不能通过全局删除 ToolCalling
  让负例“通过”。
- opt-in=1 但两个 Developer keys 均 unset（包括 Vertex-only env）与任一 key set 但
  opt-in unset 均必须零网络；双 key 时 GOOGLE_API_KEY 胜出。transport deadline
  timeout 记录 failed/network，新 retry 使用新 run_id；外部 cancel/interruption 才是
  无 error_class incomplete。
- live passed artifact 必须保存 static/list/get/minimal-call 的完整 typed facts；
  auth/timeout/cancel 无响应时只能保存 attempt/termination 且 observation/digest 都缺失，
  partial response 只能保存真实 typed partial facts。只保存 digest、伪造成功 observation、
  用 global static/list 替代 8 个 per-model required keys、让一个模型替代另一个模型，
  或把两个不同 response observations
  canonicalize 成同一成功记录均失败。
- list-models 的 exact match 仅在从空 token 开始、完整走到无 next token 后结算；目标只在
  后续页仍必须命中。repeated/malformed token、任一页失败、超过 100 页或跨页 exact
  duplicate 均不得用 partial results false-pass。
- 403 + structured `RESOURCE_EXHAUSTED` 必须按 quota 而非 auth；deadline 与 external
  cancellation 同时就绪时只采用原子记录的首个 execution-terminal event，后到
  execution signal 不得覆写。
- runner 在 fake transport barrier 上收到 external cancellation 后必须停止后续请求；
  final flush 成功时 reload artifact 仍能看到此前已 await-persist 的 steps 和当前
  incomplete termination；final flush 失败时返回 typed uncommitted
  failed/protocol/artifact_persistence_failed，并保留 external cancellation 与 last
  committed snapshot，不得声称 final artifact 已落盘。
- manual success/interruption artifact 不自动删除；默认目录、显式 output-dir override、
  atomic replace 与权限契约可验证，repository ignore policy 精确忽略
  `/artifacts/live/GH1108/`，offline test sink 不污染默认目录。
- list-models 出现但 lifecycle 页面未提供通用 chat 正证据时保持不公开，并记录冲突。
- live minimal call 成功但静态价格/limits 不匹配时 smoke 整体失败，不能用连通性覆盖
  metadata 漂移。

## 发布说明

这是 Developer API catalog refresh 与请求行为收紧。发布说明必须列出新增的两个 GA
exact IDs、价格/limits、三项采样参数拒绝和 model-turn prefill 拒绝，并列出停止公开的
旧 ID 及 disposition。必须明确 Vertex AI 未随本变更更新，live smoke 仍为显式 opt-in，
且回滚 binary 前应先移除只被新版本识别的模型配置。manual artifacts 是 retained
persistent files；回滚实现或 binary 不会自动删除它们，operator 必须先检索并显式清理
默认目录以及曾配置的 override directories。
