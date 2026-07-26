# Tech Spec

## Linked Issue

GH-1108 / #1108

## Product Spec

见 `specs/GH1108/product.md`。

## Implementation Gate

本实现依赖 GH1112 的 production neutral Google catalog API。当前
`origin/main@f09ddb7d4f871e735b9b132db58ae7e2300c7231` 尚无
`src/core/providers/google/**`，GH1112 又因 same-issue circuit breaker 被 `parked`。

因此本 packet 可以独立审查和合并，但 implementation lane 在以下条件全部满足前必须
保持 blocked：

1. GH1112 implementation 已合并到 `origin/main`；
2. merged head 提供 single neutral catalog、Developer availability overlay 和 shared
   request contract；
3. 本 spec 的 planned paths 与 merged API 重新核对；若 API/路径不同，先修订 tech/task，
   不得写回旧 `gemini/models/**` 建立第二套 authority；
4. maintainer 已明确批准本 spec 的最终 commit head；draft、PR open、route gate
   `allowed` 或队列级 `implx auto` 授权均不等于 `spec_approval`，approval 不得从旧 head
   复用；
5. fresh duplicate evidence 与 `implement` route gate 为 `allowed`。

## Codebase Context

以下锚点已在 `origin/main@f09ddb7d4f871e735b9b132db58ae7e2300c7231` 核验。

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Current Gemini registry | `src/core/providers/gemini/models/mod.rs:144-179` | 以 `HashMap` 存 17 个模型，duplicate insert 会覆盖，`list_models()` 直接返回 values。 | GH1112 将迁移为 neutral exact catalog；GH1108 只能在其 merged API 上刷新。 |
| Current 3.5 catalog | `src/core/providers/gemini/models/catalog/gemini35.rs:7-68` | 只登记 `gemini-3.5-flash`，无 3.6 Flash 或 3.5 Flash-Lite。 | 两个新 GA model records 的现状锚点。 |
| Provider list/preflight | `src/core/providers/gemini/provider.rs:51-56,74-112` | 模型列表来自 registry；validation 只有通用温度/top-p 数值范围。 | 需要按 exact model contract 拒绝 deprecated params 与 prefill。 |
| Supported params/mapping | `src/core/providers/gemini/provider.rs:169-212` | 当前表列出 temperature/max_tokens/top_p/stop/stream/tools/tool_choice；map 对 tools/tool_choice 仅 passthrough，unknown 也 passthrough。 | B-005/B-007 要按最终 consumer 证据收敛为 model-specific closed contract。 |
| Canonical extra params | `src/core/types/chat.rs:177-179` | `ChatRequest.extra_params` 使用 `#[serde(flatten)] HashMap<String, Value>`，因此 `top_k: null` 保留为 `Value::Null`，不同于 typed `Option` 字段。 | B-005 必须在 shared normalizer 明确消费 flattened null，并拒绝 non-null。 |
| Final Developer body | `src/core/providers/gemini/client.rs:252-336` | System 移到 systemInstruction、Developer 被跳过；body 只消费 max_tokens/temperature/top_p/stop，未消费 tools/tool_choice/response_format。 | prefill 必须基于最终 contents，positive params 必须按真实 sink 收敛。 |
| OpenAI stream metadata DTO | `src/core/models/openai/requests.rs:19-93,136-141` | transport request 的 exact 字段为 `stream_options.include_usage: Option<bool>`；当前 `StreamOptions` 未声明 unknown-field rejection。 | B-007 必须把 closed wire metadata 与 provider positive params 分开，并让 malformed/unknown metadata fail closed。 |
| Gateway stream routing | `src/server/routes/ai/chat.rs:271-405`、`src/server/routes/ai/chat_streaming.rs:36-50,69-95,138-139`、`src/server/routes/ai/token_policy.rs:80-88`、`src/server/routes/ai/execution.rs:201-311`、`src/core/router/selection.rs:225-243` | shared builder 以既有 `include_usage_override=Some(true)` 生成只含 `include_usage=true` 的 canonical core request；router 解析 alias 并在 fallback/retry 中选择最终 deployment，operation callback 才拿到最终 provider/model，随后 `prepare_chat_request_for_provider` 处理该次 selected request。 | B-007 只要求 canonical metadata 在 pre-selection 阶段不被 take/drop，并在这个 post-selection hook 按最终 Gemini exact identity 校验/消费；不创建额外 usage preference state，其他 provider 与 selection failure 不得被提前修改。 |
| Gemini streaming transport | `src/core/providers/gemini/client.rs:93-118` | unary/stream 分别选择 generateContent/streamGenerateContent；stream 与 stream_options 都不是 generation body 字段，transformer 也未读取 stream_options。 | B-007 可保留 `stream`，但 `stream_options` 只能作为 gateway metadata，不能成为第四个 provider param。 |
| Captured tracing pattern | `src/core/observability/tests.rs:81-101,310-315` | `MakeWriter` + `tracing::subscriber::set_default` 可把 tracing bytes 捕获到测试 buffer。 | B-012 live redaction fixture 必须覆盖 tracing sink。 |
| Pricing storage | `src/core/providers/gemini/models/mod.rs:83-105,127-140` | pricing helper 接收 per-million 值并换算到 per-1k fields，metadata 与 limits 同记录。 | 新价格的单位换算、cost parity 与 GH1113 边界。 |
| Credential config | `src/core/providers/gemini/config.rs:14-15,82-105,134-141` | Developer config 从 env 读 key；当前 type derive `Debug`。 | live smoke 不得格式化/落盘 key；production credential redaction 由 GH1112 T4 所有。 |
| Existing live-test pattern | `tests/live_bedrock.rs:1-27,63-68` | `#[ignore]` + 单一 opt-in env，未开启时不联网。 | 复用成熟手动验证形态，不创建后台自动化。 |
| Dependency contract | `specs/GH1112/tech.md`、`specs/GH1112/tasks.md` | 已声明 neutral `google/models` owner、Developer overlay、shared request contract，T1→T2 串行。 | GH1108 implementation 的唯一合法 owner/base gate。 |

## Planned Changes

```specrail-planned-changes
{
  "issue": 1108,
  "complete": true,
  "paths": [
    "src/core/providers/google/models/registry.rs",
    "src/core/providers/google/models/request_contract.rs",
    "src/core/providers/google/models/catalog/mod.rs",
    "src/core/providers/google/models/catalog/gemini35.rs",
    "src/core/providers/google/models/catalog/gemini36.rs",
    "src/core/providers/google/models/tests.rs",
    "src/core/providers/gemini/provider.rs",
    "src/core/providers/gemini/provider_tests.rs",
    "src/core/providers/gemini/client.rs",
    "src/core/models/openai/requests.rs",
    "src/server/routes/ai/token_policy.rs",
    "src/server/routes/ai/chat_tests.rs",
    "tests/gemini_router_fallback_routes.rs",
    "tests/live_gemini.rs",
    "checks/gh1108_coverage_gate.py",
    "checks/test_gh1108_coverage_gate.py",
    "docs/providers/README.md",
    "docs/providers/gemini.md"
  ],
  "spec_refs": [
    "specs/GH1108/product.md#behavior-invariants",
    "specs/GH1108/product.md#验收标准",
    "specs/GH1108/tech.md#implementation-gate",
    "specs/GH1112/tech.md",
    "specs/GH1112/tasks.md"
  ]
}
```

`src/core/providers/google/**` 是 GH1112 计划并拥有的路径，当前尚不存在。implementation
开始前必须以 merged GH1112 head 重新验证以上清单；任何必要路径差异通过 spec amendment
处理，不能把旧 `src/core/providers/gemini/models/**` 加回 manifest。

`src/server/routes/ai/chat.rs`、`src/server/routes/ai/chat_streaming.rs` 与
`src/core/types/chat.rs` 是本设计已核验的 read-only context，不是 planned writable
paths：现有 builder 已生成 canonical core `StreamOptions`，GH1108 只在最终 selected
deployment 的 `token_policy` hook 条件消费它。“preserve”指 canonical object 在
pre-selection 阶段不被 take/drop，不承诺 client wire `include_usage=false` 原值传到
upstream。implementation exact-head 必须以以下 gate 证明三个 context paths 未变；若实现
发现必须修改其中任一路径，先 amend 本 manifest，不得静默扩 scope：

```bash
test -z "$(git diff --name-only \
  "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" -- \
  src/server/routes/ai/chat.rs \
  src/server/routes/ai/chat_streaming.rs \
  src/core/types/chat.rs)"
```

## 设计方案

### 1. 证据驱动的 Developer catalog delta

在 GH1112 neutral registry 上增加两个 exact records：

- `gemini-3.6-flash`：Developer `available_exact`、GA、1,048,576 input、
  65,536 output、Gemini Developer API paid Standard 1.50/7.50 USD per million；
- `gemini-3.5-flash-lite`：Developer `available_exact`、GA、相同 limits、
  Gemini Developer API paid Standard 0.30/2.50 USD per million。

每个 record 保留 Developer official URL、reviewed-at、lifecycle stage 和 shutdown date
（当前为 none）。`gemini36.rs` 是新 family owner；`gemini35.rs` 只扩展现有 3.5 family。
Developer availability 只写入 Developer overlay；Vertex overlay 保持只读且不得由本 PR
改变。

#### Frozen pre-refresh disposition ledger

以下 17 行是 implementation 的批准输入，不是待实现代码重新推导的分类结果。fixture
必须对 `exact_id`、disposition、完整 source URL set、`reviewed_at` 与 reason 逐字段
exact-equal；增删 URL、改变 reason 或把 `unverified` 升级为 available 都必须先 amend
本 spec。所有来源均为 `ai.google.dev`，统一 `reviewed_at=2026-07-26`：

| Exact ID | Disposition | Official source URL(s) | Reason |
| --- | --- | --- | --- |
| `gemini-2.5-pro` | `available_exact` | `https://ai.google.dev/gemini-api/docs/models`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | current models/deprecations exact row；shutdown date 2026-10-16 尚未到达 |
| `gemini-2.5-flash` | `available_exact` | `https://ai.google.dev/gemini-api/docs/models`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | current models/deprecations exact row；shutdown date 2026-10-16 尚未到达 |
| `gemini-2.5-flash-lite` | `available_exact` | `https://ai.google.dev/gemini-api/docs/models/gemini-2.5-flash-lite`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | exact model page + deprecations row；shutdown date 2026-10-16 尚未到达 |
| `gemini-3-flash-preview` | `available_exact` | `https://ai.google.dev/gemini-api/docs/models`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | current models/deprecations exact preview row；无 announced shutdown |
| `gemini-3.1-pro-preview` | `available_exact` | `https://ai.google.dev/gemini-api/docs/models`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | current models/deprecations exact preview row；无 announced shutdown |
| `gemini-3.1-flash-lite` | `available_exact` | `https://ai.google.dev/gemini-api/docs/models`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | current models/deprecations exact row；shutdown date 2027-05-07 尚未到达 |
| `gemini-3.5-flash` | `available_exact` | `https://ai.google.dev/gemini-api/docs/models/gemini-3.5-flash`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | exact model page + current deprecations row；无 announced shutdown |
| `gemini-3-pro-image-preview` | `shutdown` | `https://ai.google.dev/gemini-api/docs/deprecations` | exact preview row shutdown 2026-06-25 |
| `gemini-2.0-flash-exp` | `shutdown` | `https://ai.google.dev/gemini-api/docs/models/gemini-2.0-flash`<br>`https://ai.google.dev/gemini-api/docs/changelog` | exact model page marks ID shut down；changelog records 2025-11-04 notice and 2025-12-09 shutdown |
| `gemini-2.0-flash-thinking-exp` | `shutdown` | `https://ai.google.dev/gemini-api/docs/changelog` | changelog records 2025-11-04 notice and 2025-12-09 shutdown for exact experimental ID |
| `gemini-1.5-pro` | `shutdown` | `https://ai.google.dev/gemini-api/docs/changelog` | changelog 2025-09-29 records exact 1.5 model shut down |
| `gemini-1.5-flash` | `shutdown` | `https://ai.google.dev/gemini-api/docs/changelog` | changelog 2025-09-29 records exact 1.5 model shut down |
| `gemini-1.5-flash-8b` | `shutdown` | `https://ai.google.dev/gemini-api/docs/changelog` | changelog 2025-09-29 records exact 1.5 model shut down |
| `gemini-1.0-pro` | `retired` | `https://ai.google.dev/gemini-api/docs/changelog` | changelog 2025-02-18 records Gemini 1.0 Pro no longer supported |
| `gemini-3-pro` | `unverified` | `https://ai.google.dev/gemini-api/docs/models`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | exact ID absent from both current models and deprecations at reviewed_at |
| `gemini-3-pro-deep-think` | `unverified` | `https://ai.google.dev/gemini-api/docs/models`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | exact ID absent from both current models and deprecations at reviewed_at |
| `gemini-3.1-flash` | `unverified` | `https://ai.google.dev/gemini-api/docs/models`<br>`https://ai.google.dev/gemini-api/docs/deprecations` | exact ID absent from both current models and deprecations at reviewed_at |

`available_exact` 七行继续公开；`retired`/`shutdown`/`unverified` 十行停止或保持不公开。
unverified 的证据事实是 exact ID 同时缺席 current models 与 deprecations，不是近似
family 缺席。live list/get 的偶然结果不能覆盖 frozen ledger。registry 初始化继续由
GH1112 规则拒绝 duplicate、missing evidence、missing contract 和非法 lifecycle。

### 2. 新模型请求契约

在 shared `request_contract.rs` 中为两个 exact IDs 声明 closed allowed-param set 和
illegal-state policy：

- `temperature`、`top_p`、`top_k` 不在 allowlist；
- typed `temperature`/`top_p` 的字段省略和显式 JSON `null` 均反序列化为 `None`、视为
  absent；任何 `Some(non-null value)`（包括看似默认的数值）即 typed
  invalid-request；
- OpenAI wire `extra_body` 转为 canonical `ChatRequest.extra_params` 后，shared
  normalizer 必须移除 `top_k` 且检查其 `Value`：`Value::Null` 消费为 absent，任何
  non-null value 即 typed invalid-request；normalizer 返回的 map 不再含 `top_k`；
- final serializer 只消费 normalized request，且生成的 upstream body 不含
  `temperature`、`topP`、`top_k` 或 `topK`；
- shared contract 只执行一次 `normalize_gemini_contents`：它按最终 transport 规则同时
  完成 semantic-empty stripping、role mapping 和 content conversion，直接返回
  serializer-ready `{contents, system_instruction}`；validator 与 serializer 都只消费该
  结果，禁止重新读取 raw messages 或再次映射 roles；
- semantically-empty turn 定义为：`content` 缺失、blank text 或仅含 blank text parts，
  且无 thinking、audio、tool calls、function call、tool result/call ID 等 meaningful
  payload；任一 non-text content part 或 tool payload 均使 turn meaningful；
- System 与 Developer 的 meaningful text 都不进入 `contents`，而是按原 messages 索引
  顺序逐 part 追加到同一个 `system_instruction.parts`；不能先按 role 分组后拼接。
  Developer turn 的 meaningful non-text part、tool payload 或任何无法无损表示为
  instruction text 的 payload 必须 typed invalid-request、network counter=0，不能丢弃；
  User→`user`、Assistant→`model`；Tool/Function 的现有 mapping 不在 GH1108 扩展；
- `validate_no_model_prefill` 必须在最终 `contents` 上运行：contents 为空或最后一项 role
  为 `model` 即 typed invalid-request。fixture 必须锁定 interleaved
  system/developer parts 的原序、developer+user 同时保留 instruction/user、
  Developer non-text/不可表示 payload pre-network 拒绝、assistant+system 与
  assistant+developer 因 final contents=model 拒绝、all-system/developer 因 contents
  为空拒绝、non-empty model+trailing-empty/all-empty 拒绝，以及 user+system 通过；
- 既有 Gemini `ToolUse`/`ToolResult` wire 序列化与完整 callability 由 GH1111 所有，
  不是 GH1108 acceptance，也不构成 GH1108 implementation dependency；
- 只有官方明确声明相同契约的未来 model record 才能复用，禁止 family substring 推断。

Gemini provider 的 supported params、`validate_request`、`map_openai_params` 和
`GeminiClient::transform_chat_request` 都查询同一 contract。最终 body serializer 只消费
已经校验的 provider-neutral fields；direct client entry point 也先执行同一 preflight。
网络计数 fixture 证明所有负例在 auth/HTTP 之前终止。

两个新模型的 positive OpenAI-compatible allowlist 恰为：

| Parameter | Disposition | Evidence at spec time |
| --- | --- | --- |
| `max_tokens` | allowed；映射到 `generationConfig.maxOutputTokens` | `provider.rs:172,194-196`；`client.rs:295-297` |
| `stop` | allowed；非空时映射到 `generationConfig.stopSequences` | `provider.rs:174,191-193`；`client.rs:307-312` |
| `stream` | allowed；只选择 stream transport/endpoint，不写 body | `provider.rs:175,191-193`；`client.rs:93-118` |
| `temperature` / `top_p` / `top_k` | excluded；按 B-005 null/absent 与 non-null rejection 处理 | B-005 shared contract |
| `tools` / `tool_choice` | excluded；当前 provider 只广告/passthrough，`transform_chat_request` 未消费 | `provider.rs:176-177,198-200`；`client.rs:252-336` |
| `response_format` / `max_completion_tokens` | excluded；canonical fields 存在但 Gemini transformer 未消费 | `src/core/types/chat.rs:98-101,129-131`；`client.rs:252-336` |

`get_supported_openai_params`、preflight allowlist、map disposition 与
serializer/transport disposition 必须以集合相等断言锁定 `{max_tokens, stop, stream}`；
对每个 allowed param 断言精确 sink，对每个 excluded param 断言 pre-network typed error
或 B-005 absent 规则。不得以 catalog 产品能力自动添加 tools/tool_choice；
完整 tool serialization 仍归 GH1111。

`stream_options` 走独立的 gateway metadata lane，不进入上表。shared builder 保持
provider-neutral，并继续执行既有 `include_usage_override=Some(true)` settlement
normalization；输出且由本 issue 观察的 canonical core metadata 只含
`include_usage=true`，必须保留到 final selection。本 issue 不生成、保存或暴露额外的
wire-preference usage state。alias resolution、budget/upstream
fallback 与 capability selection 完成后，selected operation 才调用
`token_policy::prepare_chat_request_for_provider`：

- wire DTO 的闭合 shape 只有 `stream_options.include_usage: bool`；在
  `src/core/models/openai/requests.rs` 以 `deny_unknown_fields` 和 typed bool 让非 boolean
  或未知字段在 API boundary typed fail closed。该 structural validation 不消费合法
  object，也不执行任何 provider/model-specific normalization；
- 只有最终 selected provider 是 Gemini Developer 且 selected exact model 属于
  `{gemini-3.6-flash, gemini-3.5-flash-lite}` 时，
  `prepare_chat_request_for_provider` 内的 `normalize_selected_gemini_stream_metadata`
  才执行模型特定校验并消费：`stream != true`、internal `include_usage != true` 或消费后
  metadata 残留均为 OpenAI-compatible invalid-request，network counter=0；有效输入只
  消费 canonical `include_usage=true`，provider-bound request 不再含 `stream_options`，
  不创建额外 usage state；
- direct exact ID、alias 到新 ID、以及 budget/upstream fallback 最终落到新 ID 都以 final
  selected identity 触发同一 contract；不能按原始 request model/provider substring
  判断。最终选中 OpenAI、OpenRouter 或其他 provider 时，post-selection hook 的
  canonical `stream_options` input/output 必须逐字段相等并传给其既有请求路径；
  selection failure 不调用
  post-selection hook，原始 immutable/core request 保持不变；
- selected-new-Gemini streaming fixture 使用 canonical `include_usage=true`，spy 必须观测
  `ProviderCapability::ChatCompletionStream`/`chat_completion_stream` 被调用，并断言
  Gemini upstream body 同时不含 `stream_options` 与 `include_usage`；selected-new-Gemini
  non-stream/invalid fixture 证明 metadata 不能绕过 contract，OpenAI/OpenRouter fixture
  证明 preserved equality，selection-failure fixture 证明 no mutation。user-visible
  positive allowlist 仍精确为 `{max_tokens, stop, stream}`。

### 3. Pricing 与能力边界

价格仍作为 neutral model metadata 的现有字段写入，保持 GH1112 已定义的 access API。
本 spec 的数值只表示 Gemini Developer API paid Standard tier；测试从“官方
per-million 数值 → stored per-1k 数值 → cost for fixed tokens”逐层断言单位，避免
1000 倍换算错误。Batch、Flex、Priority 或其他 tier 不得写入相同 fixture、复用这些
数值或由测试宣称通过。

本 issue 只为两个新模型提供确定 pricing facts：

- 不更改 pricing authority、fallback、unknown-cost 或 spend/budget/callback 路径；
- 不新增零成本 fallback；
- pricing consumer 若需要新 accessor，必须先走 GH1113 spec amendment，而不是在本 PR
  建第二套价格表。

两个新模型必须使用相同的 exact、闭合能力 disposition：

- public `ModelInfo.capabilities` 恰为
  `{ProviderCapability::ChatCompletion, ProviderCapability::ChatCompletionStream}`，
  `supports_streaming=true`、`supports_tools=false`；
- model feature flags 恰为
  `{ModelFeature::MultimodalSupport, ModelFeature::StreamingSupport,
  ModelFeature::SystemInstructions}`。三项分别绑定当前 callable source：
  `client.rs:361-395` 的 inline base64 image→`inlineData`、
  `client.rs:93-118` 的 `streamGenerateContent` endpoint，以及
  `client.rs:287-290` 的 `systemInstruction` serializer；`MultimodalSupport` 不扩张为
  audio/video/document support。

测试必须比较集合相等而非只做 `contains`。任何不在闭合集中的能力都 fail closed 为不
广告。当前 Gemini transformer 不消费 tools/tool_choice/response_format，所以
`ProviderCapability::{ToolCalling, FunctionCalling}`、
`ModelFeature::{ToolCalling, FunctionCalling, JsonMode}` 必须有显式 negative assertions；
`ModelFeature::{ContextCaching, SearchGrounding, VideoUnderstanding, AudioUnderstanding}`
也必须有显式 negative assertions；其中 `client.rs:382-385` 明确拒绝 Audio，content
type 无 video serializer。
Google 产品支持不能代替本 provider callability，相关广告只可在 GH1111 或后续契约真正
实现 serializer/transport 后单独启用。其余不得加入的能力尤其包括
`ProviderCapability::CodeExecution`、
`ProviderCapability::BatchProcessing`、`ProviderCapability::RealtimeApi`、
`ProviderCapability::ImageGeneration`、任何 audio generation capability，或
`ModelFeature::CodeExecution`、`ModelFeature::BatchProcessing`、
`ModelFeature::RealtimeStreaming`。Computer Use、audio/image generation、Live 与
Interactions 即使官方产品页面存在，也因当前公开 API 无对应可兑现契约而不得用 metadata
宣称支持。

### 4. Opt-in live smoke

新增 `tests/live_gemini.rs`，复用现有 live Bedrock pattern：

- `#[ignore]`；
- 只有 `LITELLM_RS_LIVE_GEMINI=1` 才允许联网；
- key 只从 `GEMINI_API_KEY`/既有 Developer credential path 读取，不写入命令、Debug、
  error 或 artifact；
- 依次执行两个 exact ID 各自的 static snapshot、一次 official list（派生两个独立
  per-model records）、两个 exact ID 各自的 get 与最小 generate-content call；
- offline gate fixture 覆盖 opt-in/key 完整 2×2 matrix，但普通并行 test process 禁止
  调用 `std::env::set_var`/`remove_var`。每个 case 启动隔离子进程，对 child
  `Command` 先 `env_clear()`，再精确设置该 case 允许的
  `LITELLM_RS_LIVE_GEMINI`、`GEMINI_API_KEY`/Developer aliases、fake transport/counter
  配置与必要的非 credential test bootstrap；child 必须经过 production actual env
  reader boundary。双 unset；仅 sentinel `GEMINI_API_KEY`；仅
  `LITELLM_RS_LIVE_GEMINI=1` 且所有 Developer credential aliases unset，三者均
  network counter=0；双满足只命中 fake transport。parent 并行运行所有 cases 后断言
  child env/counter/artifact 无交叉泄漏；真实 endpoint 仍只由手工 ignored test 使用；
- 每次 invocation 生成新的 non-empty opaque `run_id`；同一 run 的所有 step 共用该值；
- 每条结果使用 closed typed schema：
  `{run_id, model, step, status, error_class, http_status, observed_at,
  termination_reason, attempt, observation?, observation_sha256?}`。`attempt` 是
  deny-unknown typed facts
  `{started, network_attempted, response_received, response_parsed}`，只能按实际阶段单调从
  false 变 true；`model`/`http_status` 按 step/实际响应可为 none；`step` 闭集为
  `{static_snapshot, list_models, get_model, minimal_call, aggregate}`；`status` 闭集为
  `{passed, failed, incomplete}`；`termination_reason` 闭集为
  `{step_failed, transport_timeout, externally_cancelled, externally_interrupted,
  missing_required_step}` 或 none，不含 request URL query/header/body credential。
  只有 aggregate 的 `model=None`；static/list/get/minimal-call 的 `model` 必须是两个新
  exact ID 之一，且必须与 observation/request key 一致；
- `observation` 是 optional
  `#[serde(tag = "kind", content = "facts", deny_unknown_fields)]` closed union；每个
  step kind 的 `facts` 又是
  `#[serde(tag = "completeness", deny_unknown_fields)] complete | partial` typed union。
  partial 字段自身使用 `Option`/非空 collection 表示“确实观察到”，禁止用 `""`、`0`、
  空集合或 success defaults 填补未取得事实：
  - `static_metadata`：
    `{exact_id, catalog_present, lifecycle_disposition, input_token_limit,
    output_token_limit, paid_standard_input_usd_per_million,
    paid_standard_output_usd_per_million, supports_streaming, supports_tools,
    supports_multimodal, capabilities, features,
    evidence_source_urls, evidence_reviewed_at}`；
  - `list_models`：
    `{requested_exact_id, returned_model_count, exact_matches}`，其中每个 exact match 保存
    `{resource_name, exact_id, supported_generation_methods, input_token_limit,
    output_token_limit}`；
  - `get_model`：
    `{requested_exact_id, returned_resource_name, returned_exact_id, exact_id_match,
    supported_generation_methods, input_token_limit, output_token_limit}`；
  - `minimal_call`：
    `{requested_exact_id, returned_model_version, candidate_count, finish_reasons,
    response_text, prompt_token_count, candidates_token_count, total_token_count}`；
  - `aggregate`：
    `{required_keys, passed_keys, failed_keys, incomplete_keys}`，每个 key 为
    deny-unknown `{step, model}`；
- passed static record 要求 catalog_present 与所有 metadata/evidence facts；一次 official
  list response 可派生两个独立 per-model list records，但每条的 `(step, model)`、
  `requested_exact_id`、exact_matches 中唯一 case-sensitive match 必须共同指向该 exact
  ID，且每条独立 canonicalize/hash/persist；passed get 要求 returned exact ID 匹配；
  passed minimal call 要求 non-empty returned_model_version、candidate_count>0、
  non-blank redacted response_text 与完整 usage token facts。list/get 的 `exact_id` 是从
  保留原值的 `resource_name` 严格移除单一 `models/` 前缀所得；前缀缺失/重复或结果与
  requested_exact_id 不同均不得通过。`status=passed` 必须同时满足
  `attempt.started=true`、正确 step/model 的 complete observation、digest pair 一致、
  `error_class=None` 与 `termination_reason=None`；step-kind/model 错配、必需字段缺失、
  partial/none observation 或 only-hash record 均为 failed/protocol，不能只凭 status
  通过；
- `failed`/`incomplete` 必须保留 step、model、attempt、termination 与分类，但 response
  observation 按实际结果可缺失：auth preflight、未收到响应的 timeout/cancel 为
  `observation=None`、`observation_sha256=None`；收到并只解析出部分事实时只能保存对应
  kind 的 typed partial observation。auth HTTP response 可保留真实 `http_status`，只有
  实际解析出 whitelist 内事实才附 partial observation。完整响应已解析但后续
  exact/protocol validation 失败时可保存 complete observation，但 complete 不代表
  passed。任何 failure path 都不得构造“成功响应形状”；
- optional pair consistency 是闭合 invariant：observation none 当且仅当 digest none；
  observation some 当且仅当 digest some，且 digest 必须精确等于 redacted canonical
  observation 的 SHA-256。status 不允许改变此 pair rule；
- aggregate 的 required key 闭集精确为两个 model 各四项、共 8 项：
  `{(static_snapshot,Some("gemini-3.6-flash")),
  (list_models,Some("gemini-3.6-flash")),
  (get_model,Some("gemini-3.6-flash")),
  (minimal_call,Some("gemini-3.6-flash")),
  (static_snapshot,Some("gemini-3.5-flash-lite")),
  (list_models,Some("gemini-3.5-flash-lite")),
  (get_model,Some("gemini-3.5-flash-lite")),
  (minimal_call,Some("gemini-3.5-flash-lite"))}`。aggregate record 自身使用
  `(aggregate,None)`，不加入 required keys。passed/failed/incomplete key sets 必须
  pairwise disjoint、无重复/未知 key，union 精确等于 required；aggregate 仅在
  `passed_keys == required_keys` 且 failed/incomplete 为空时 passed。任一 model 的
  static/list/get/minimal-call 缺失都产生 failed/protocol/missing_required_step，一个
  model 或 global static/list record 不能替代另一个；
- canonicalization 在 redaction 后执行：ID/resource name/response text 保留原始大小写与
  内容，不 trim/case-fold；capabilities/features/methods/source URLs 以 UTF-8 lexical
  order 排序并去重，aggregate keys 按 step 再按 exact model 排序（None 先于 Some）并
  拒绝重复，exact_matches 按 resource_name lexical
  排序但不折叠重复（重复本身是失败事实），candidate/finish/text 顺序保持上游顺序；
  token/count 使用整数，价格使用固定小数字符串；JSON object keys 递归 lexical sort、
  UTF-8、无多余 whitespace。`observation_sha256` 只对 canonical observation 求 SHA-256，
  不能代替同条记录中的 observation；partial 与 complete tag 都进入 canonical bytes；
- typed whitelist 不保存 request URL、query、headers、credential config 或 raw error
  body；先经 `redact_live_artifact`，再 canonicalize/hash，序列化后仍命中已知 credential
  sentinel 时拒绝写 artifact。fixture 改变任一 observation fact 后必须同时得到不同的
  canonical observation 与 digest；不同 facts 不得生成同一 passed record；
- 五个且仅五个 error classes 为 `{auth, quota, not_found, protocol, network}`。
  `classify_live_failure` 必须使用以下 closed decision table；输入是 typed terminal event、
  exact structured Google reason、typed `ProviderError` variant 与 numeric HTTP status，
  禁止读取 error message substring：

| Source facts | `status` / `error_class` | `termination_reason` |
| --- | --- | --- |
| runner 原子记录的首个 event 是 external cancellation/interruption | `incomplete` / none | `externally_cancelled` / `externally_interrupted` |
| runner 原子记录的首个 event 是 transport deadline | `failed` / `network` | `transport_timeout` |
| Google exact reason `UNAUTHENTICATED` / `PERMISSION_DENIED` | `failed` / `auth` | `step_failed` |
| Google exact reason `RESOURCE_EXHAUSTED` | `failed` / `quota` | `step_failed` |
| Google exact reason `NOT_FOUND` | `failed` / `not_found` | `step_failed` |
| Google exact reason `UNAVAILABLE` / `DEADLINE_EXCEEDED` | `failed` / `network` | `step_failed` |
| Google exact reason `INVALID_ARGUMENT` / `FAILED_PRECONDITION` / `UNIMPLEMENTED` 或其他已解析非 transport reason | `failed` / `protocol` | `step_failed` |
| `ProviderError::Authentication` | `failed` / `auth` | `step_failed` |
| `ProviderError::RateLimit` / `QuotaExceeded` | `failed` / `quota` | `step_failed` |
| `ProviderError::ModelNotFound` / `DeploymentError` | `failed` / `not_found` | `step_failed` |
| `ProviderError::Network` / `Timeout` / `ProviderUnavailable` / `RoutingError` / `Streaming` | `failed` / `network` | `step_failed` |
| `ProviderError::ApiError` | 依其 numeric status 使用下方 HTTP fallback | `step_failed` |
| 其余 typed provider variants（含 InvalidRequest、Configuration、Serialization、ResponseParsing、TransformationError、ContextLengthExceeded、ContentFiltered、TokenLimitExceeded、NotSupported、NotImplemented、FeatureDisabled、无 verified external event 的 Cancelled、Other） | `failed` / `protocol` | `step_failed` |
| HTTP fallback 401/403；402/429；404；408/504/5xx | 分别 `auth`；`quota`；`not_found`；`network` | `step_failed` |
| 其他 HTTP status、local schema/exact/observation validation、artifact persistence failure | `failed` / `protocol` | `step_failed` |
| 无 external terminal event 的自然聚合缺 required key | `failed` / `protocol` | `missing_required_step` |

classification precedence 固定为：

1. runner 以 compare-and-set 原子记录的首个 terminal event；
2. response 中已成功解析的 exact structured Google reason；
3. typed `ProviderError` variant；
4. numeric HTTP status fallback；
5. local protocol validation。

同一层冲突或未知值 fail closed 为 protocol。deadline/cancellation 同时就绪时
first-terminal-event-wins，后到 signal 不得覆写 status/class/termination。403 +
`RESOURCE_EXHAUSTED` 因 structured reason 优先而归 quota；remote
`DEADLINE_EXCEEDED` 是 network/step_failed，只有 runner 自身 deadline 才使用
`transport_timeout`。passed record 的 error_class/termination_reason 均为 none。

`run_live_gemini_smoke` 必须接收可注入 cancellation token、可在指定 step barrier 阻塞的
transport，以及 `LiveArtifactSink`。runner 对每个 step 执行：

1. 产生并 redaction/canonicalization 当前 typed record；
2. 调用 sink 以 temp-write + atomic replace 保存整个当前 run snapshot，并 await 成功；
3. 只有 persistence 成功后才开始下一个 required step/network call。

收到 external cancellation/interruption 后，runner 停止调度新网络调用，为 in-flight 与
尚未开始的 required keys 写入 `incomplete_keys`，保存真实 attempt/termination 与可选
partial observation，await 最终 atomic flush 后返回；不得把这些 keys 再合成为
missing-required protocol failure。若 flush 自身失败，runner 返回 protocol failure，不能
报告 passed；此前已成功原子写入的 snapshot 仍须可 reload。retry 必须生成不同 run_id，
旧 run steps 保持只读，不能聚合到新 run。

manual invocation 的 production `LiveArtifactSink` 默认目录是
`artifacts/live/GH1108`，final path 精确为
`artifacts/live/GH1108/<run_id>.json`；只有非空
`LITELLM_RS_LIVE_GEMINI_OUTPUT_DIR` 才覆盖目录，filename 仍是 `<run_id>.json`。
sink 在 final file 同目录创建唯一 temp file，写入完整 credential-free snapshot 后
flush，再 atomic replace final path；不得跨文件系统 rename。Unix 平台创建 temp 时使用
`mode(0o600)`，replace 后 best-effort 重申 `0600` 并由 Unix fixture 断言；不提供 POSIX
mode 的平台使用其 private-file creation 能力 best-effort，docs 明示平台权限契约。
success、failed 与 interruption artifacts 都不自动删除。offline fixtures 必须注入
temporary-directory sink，断言默认 durable path 零写入；default/override/path traversal、
atomic replace、reload、retention 与权限分支都必须有测试。

offline unit test 使用 sentinel key 和 loopback/fake response 覆盖分类与 redaction。
redaction fixture 必须复用已核验的 captured tracing pattern，安装局部
`tracing_subscriber`/captured `MakeWriter`，触发含 sentinel 的 upstream URL/error
负例，并断言 tracing bytes、stdout、stderr、Error Display/Debug、config/result Debug
与 serialized artifact 均零命中。普通 `cargo test` 只运行 offline tests，live cases
保持 ignored 且 opt-in 双门禁。runner-level interruption fixture 必须启动真实
`run_live_gemini_smoke`，在确认至少一个 snapshot 已持久化后用 barrier 阻塞下一
transport step，再触发 cancellation token；await runner 后从 sink reload artifact，
断言已完成 records/digests 未变、in-flight/remaining keys 为 incomplete、后续 network
counter=0、final aggregate 非 passed，并以新 invocation 证明新 run_id 不聚合旧 records。

### 5. 文档与发布 snapshot

`docs/providers/gemini.md` 记录：

- 新模型 exact IDs、limits、价格与 sampling/prefill migration；
- live smoke 的 opt-in 命令、所需 env、错误分类和“不向 issue/PR 粘贴原始错误”的安全说明；
- 默认 `artifacts/live/GH1108/<run_id>.json`、可选
  `LITELLM_RS_LIVE_GEMINI_OUTPUT_DIR` override、平台权限/retention 契约；检索命令
  `find artifacts/live/GH1108 -maxdepth 1 -type f -name '*.json' -print` 与显式清理命令
  `find artifacts/live/GH1108 -maxdepth 1 -type f -name '*.json' -delete`，override 时要求
  operator 把命令中的目录替换为已确认的 exact output directory；
- Developer/Vertex 分离；
- 被停止公开的旧 ID disposition。

`docs/providers/README.md` 只增加 provider doc 索引。不得修改高上下文
`AGENTS.md`/`CLAUDE.md` 或用户配置。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | neutral exact records + Developer overlay | `cargo test --locked google_model_catalog_2026_07_exact_ids`；大小写/前后缀负例均不命中。 |
| B-002 | `gemini35.rs`/`gemini36.rs` limits 与 paid Standard pricing | `cargo test --locked google_model_catalog_2026_07_metadata`；断言 Developer paid Standard per-million 与 stored per-1k/cost，并断言 Batch/Flex/Priority 无同值声明。 |
| B-003 | Developer evidence filter | `cargo test --locked google_model_catalog_2026_07_dispositions`；retired/shutdown/unverified/other-product 不公开。 |
| B-004 | frozen 17-ID disposition ledger | `cargo test --locked google_model_catalog_2026_07_dispositions` 对 17 行 exact ID/disposition/full URL set/reviewed_at/reason 逐字段 exact-equal；七个 available 继续公开，六个 shutdown、一个 retired、三个 unverified 不公开，unverified 不得由实现自行升级。 |
| B-005 | shared request allowlist + canonical extra-map normalizer | `cargo test --locked gemini_2026_07_deprecated_sampling_rejected`；typed temperature/top_p omitted/JSON-null 均为 absent，flattened extra_body/extra_params 的 `top_k: Value::Null` 被消费并删除，三者 non-null 均 pre-network error，final JSON 四种 key 均不存在。 |
| B-006 | one-shot final-contents normalization + prefill/serializer parity | `cargo test --locked gemini_2026_07_prefill_rejected`；interleaved meaningful System+Developer text 按原消息顺序组成 systemInstruction.parts 且不进 contents，developer+user 不丢 instruction/user，Developer non-text/不可表示 payload pre-network 拒绝；assistant+system/assistant+developer 仍因 final model 拒绝，all-system/developer/model+trailing-empty/all-empty 拒绝，user+system 通过；spy/source fixture 证明 normalizer 只执行一次并直接产出 serializer-ready contents/systemInstruction；不把 GH1111 tool-loop callability 计为通过条件。 |
| B-007 | exact positive param allowlist + post-selection stream-metadata separation + sink parity | `cargo test --locked gemini_2026_07_request_contract_parity` 与 `cargo test --locked gemini_2026_07_stream_metadata`；provider/preflight/map/serializer param-name 集合精确等于 `{max_tokens, stop, stream}`，逐项断言 maxOutputTokens/stopSequences/stream transport sink，并断言 temperature/top_p/top_k/tools/tool_choice/response_format/max_completion_tokens 不在集合；wire unknown/non-bool 在 DTO boundary 拒绝但不消费合法 object；existing builder 生成、只含 include_usage=true 的 canonical metadata 在 pre-selection 不被 take/drop，且无额外 usage preference state；final selected Gemini exact identity 的 direct/alias/fallback 才消费 canonical `include_usage=true` 并到达 `ChatCompletionStream`，upstream body 无 stream_options/include_usage；OpenAI/OpenRouter canonical hook input/output 逐字段相等，selection failure 不修改原请求，所选新 Gemini 的 inconsistent metadata 与 non-stream 组合均 pre-network fail closed。 |
| B-008 | neutral paid Standard pricing facts only | `cargo test --locked gemini_2026_07_cost`；fixed-token cost 精确，Batch/Flex/Priority 未声明同值，unknown behavior snapshot 不变。 |
| B-009 | immutable stable snapshot | `cargo test --locked google_model_catalog_2026_07_stability`；重复/并发查询结果完全相等、升序、无重复。 |
| B-010 | opt-in/key 2×2 actual-env gate | `cargo test --locked live_gemini_gate_matrix`；四个 `env_clear` + exact-env 子进程经过 production env reader，三种缺门组合 network counter=0、双满足只命中 fake transport；普通 parallel tests 零 `set_var`/`remove_var`，并行 child counter/env/artifact 无交叉泄漏。 |
| B-011 | deterministic failure classification + typed live attempted/partial observation + keyed aggregation | `cargo test --locked live_gemini_failure_mapping_precedence`、`cargo test --locked live_gemini_result_classification` 与 `cargo test --locked live_gemini_observations`；table-driven fixture 穷举 terminal event、Google reason、ProviderError variant、HTTP fallback 与 local protocol sources，证明 precedence/first-terminal-event-wins 且无 substring branch；两个 exact IDs 各有 static/list/get/minimal-call 共 8 个 per-model required records，一次 list response 派生的两条 list observations 分别保持 requested_exact_id/model/exact-match 一致并独立 hash/persist；auth/timeout/cancel 无响应不构造 observation，partial 只含真实 facts；hash-only/缺字段 passed record 被拒；transport deadline 精确为 failed/network/transport_timeout，natural missing `(step, model)` 精确为 failed/protocol/missing_required_step。 |
| B-012 | credential redaction including tracing | `cargo test --locked live_gemini_redaction` 安装 captured tracing subscriber；sentinel 在 tracing/stdout/stderr/error Display+Debug/config+result Debug/artifact 中零命中。 |
| B-013 | Vertex overlay/read paths | `test "$(git rev-parse HEAD)" = "$IMPLEMENTATION_HEAD_SHA" && test -z "$(git diff --name-only "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" -- ':(glob)src/core/providers/vertex_ai/**')"` 必须零输出/零退出；synthetic prohibited-path fixture 证明任一 Vertex production path 使 gate 非零退出，catalog snapshot fixture 独立证明 neutral record 的 Vertex overlay 未变。 |
| B-014 | live runner cancellation、durable incremental persistence、termination/retry/canonical observation state | `cargo test --locked live_gemini_runner_cancellation_persists_incrementally`、`cargo test --locked live_gemini_artifact_sink` 与 `cargo test --locked live_gemini_observation_canonicalization`；真实 runner + barrier + cancellation token + reloadable sink 证明每 step await atomic snapshot、cancel flush、后续 network=0、completed facts 保留、remaining keys incomplete、aggregate 非 passed；default/override sink 证明 `<run_id>.json`、same-dir atomic replace、Unix 0600/其他平台 contract、success/interruption retention、offline temp isolation；deadline/cancel race first-terminal-event-wins，重试生成不同 run_id 且不聚合旧 keys；pair presence/recomputed digest 一致，8-key fixture 逐项移除均不得 aggregate false-pass，任一事实变化导致 canonical observation/digest 都变化。 |
| B-015 | existing model compatibility | `cargo test --locked gemini_provider` 与 migration snapshot；只允许 fixture 声明的 advertised-ID delta。 |
| B-016 | evidence manifest validation | `cargo test --locked google_model_catalog_2026_07_evidence`；missing/conflicting/stale/unofficial evidence 初始化失败。 |
| B-017 | provider-callable public capability + model-feature closed sets | `cargo test --locked google_model_catalog_2026_07_metadata`；分别对两个模型断言 capability=`{ChatCompletion, ChatCompletionStream}`、supports_tools=false、feature=`{MultimodalSupport, StreamingSupport, SystemInstructions}` 集合相等，并以 image inlineData/stream endpoint/systemInstruction source fixtures 证明三项正向 sink；显式断言 ContextCaching/SearchGrounding/VideoUnderstanding/AudioUnderstanding、ToolCalling/FunctionCalling/JsonMode 及 CodeExecution/BatchProcessing/Realtime、Computer Use、generation、Live/Interactions 均未广告。 |

## 数据流

```text
official Developer evidence fixture (offline, immutable)
  -> GH1112 neutral Google catalog validation
  -> Developer availability filter
  -> stable Gemini models()

OpenAI-compatible chat request
  -> provider-neutral gateway builder (preserve stream_options)
  -> alias/fallback resolution + final deployment selection
  -> post-selection token policy
       -> selected GH1108 Gemini exact model: validate/consume stream_options
          + canonical include_usage=true settlement metadata
       -> other provider: preserve stream_options unchanged
  -> shared model request contract / provider allowlist
       -> supported params
       -> preflight validation
       -> final Developer request body
  -> ChatCompletionStream transport (no stream_options in upstream body)
  -> existing Developer endpoint + query API key

explicit live opt-in + Developer credential
  -> per-model static snapshot (2 records)
  -> one official list response -> two independently keyed list observations
  -> per-model get (2 records)
  -> per-model minimal call (2 records)
  -> typed attempt/termination + optional redacted observations/digests
  -> await durable/temp-injected artifact sink after every logical step
  -> aggregate by closed 8-key (step, model) set
  -> result (pass | incomplete | classified failure)
```

正常构建、单元测试和运行时不读取远端模型目录，也不根据 live smoke 修改 catalog。

## 备选方案

1. **直接写回旧 `gemini/models/**`**：拒绝。会绕过 GH1112 single authority，并让后续
   Vertex/pricing/tool work 再次分叉。
2. **先实现两个 ID，GH1112 以后再迁移**：拒绝。当前 GH1112 是明写依赖；短期第二套
   authority 会产生不可审计的中间状态。
3. **静默删除 deprecated sampling fields**：拒绝。调用方会误以为参数生效；按 B-005
   返回 stable error 才能阻止 silent degradation。
4. **只依赖 live list-models**：拒绝。单次连通性不等于 lifecycle、通用 chat、pricing
   或长期可用证据。
5. **把 live smoke 加进默认 CI**：拒绝。凭证、quota、网络波动会破坏确定性，并把手动
   验证过早自动化。

## 风险

- **Security**：live smoke 接触 API key；必须双门禁、sentinel redaction、禁止原始
  URL/header/error artifact。SEC-11 要求 exact-head 人工/独立审查。
- **Compatibility**：deprecated params 从可能 ignored 变为 deterministic error；这是
  有意收紧，发布说明必须明确。停止公开旧 ID 也必须逐项列 disposition。
- **Data correctness**：official pages 可能在 spec 到 implementation 之间变化；实现时
  fresh re-verify reviewed-at/lifecycle/pricing，冲突按 fail closed。
- **Dependency**：GH1112 当前 parked；在其 API 合并前本实现不可开始。若其 API 改名，
  先修 tech manifest，不猜路径。
- **Performance**：catalog 增量和 contract lookup 为 O(1)；排序只在 snapshot 构造时
  发生。live smoke 不进入生产热路径。
- **Maintenance**：未来 model 不能靠 family substring 自动继承 request contract；
  每个 record 显式绑定证据与 contract。

## 测试计划

- [ ] Catalog: `cargo test --locked google_model_catalog_2026_07`
- [ ] Provider contract: `cargo test --locked gemini_2026_07`
- [ ] Router/network negatives: `cargo test --locked gemini_router_fallback`
- [ ] Offline live-smoke fixtures: `cargo test --locked live_gemini`
- [ ] Manual opt-in:
      `LITELLM_RS_LIVE_GEMINI=1 cargo test --locked --test live_gemini -- --ignored`
- [ ] Format/build: `cargo fmt --all -- --check && cargo check --locked`
- [ ] Strict lint: `cargo clippy --locked --all-targets -- -D warnings`
- [ ] Full suite: `cargo test --locked`
- [ ] SpecRail:
      `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108 &&
       python3 checks/check_workflow.py --repo .`
- [ ] Diff integrity: `git diff --check`
- [ ] Read-only routing contexts unchanged:
      `test -z "$(git diff --name-only
       "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" --
       src/server/routes/ai/chat.rs src/server/routes/ai/chat_streaming.rs
       src/core/types/chat.rs)"`
- [ ] Vertex production paths unchanged:
      `test "$(git rev-parse HEAD)" = "$IMPLEMENTATION_HEAD_SHA" &&
       test -z "$(git diff --name-only
       "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" --
       ':(glob)src/core/providers/vertex_ai/**')"`
- [ ] Coverage checker unit tests:
      `python3 checks/test_gh1108_coverage_gate.py`
- [ ] Coverage artifact:
      `mkdir -p artifacts/coverage/GH1108 &&
       cargo llvm-cov --locked --all-features --workspace --branch --lcov
       --output-path artifacts/coverage/GH1108/lcov.info`
- [ ] Exact-head coverage gate:
      `python3 checks/gh1108_coverage_gate.py --repo . --base "$IMPLEMENTATION_BASE_SHA" --head "$IMPLEMENTATION_HEAD_SHA" --lcov artifacts/coverage/GH1108/lcov.info --output artifacts/coverage/GH1108/gate.json`

full suite、strict Clippy、coverage 与 SpecRail gates 在 exact implementation head 各执行
一次；reviewer 默认 inspection/focused，避免重复 full run。

### Versioned exact-head coverage checker contract

implementation PR 必须新增并提交 `checks/gh1108_coverage_gate.py` 与
`checks/test_gh1108_coverage_gate.py`；本 spec PR 不实现 checker，也不需要独立 policy
JSON。checker 必须验证：

- `IMPLEMENTATION_BASE_SHA`/`IMPLEMENTATION_HEAD_SHA` 是不同的完整 40 位小写 commits，
  base 是 head ancestor，当前 `HEAD` 等于 head，tracked worktree clean，LCOV 存在；
- `base...head` changed paths 必须是本 complete planned-changes manifest 的子集；
  `src/server/routes/ai/{chat.rs,chat_streaming.rs}`、`src/core/types/chat.rs` 或
  `src/core/providers/vertex_ai/**` 任一路径变化均 fail closed。Vertex neutral overlay
  是否未变仍由 catalog snapshot fixture 独立证明，不能用 path gate 替代；
- `base...head` changed production Rust executable lines 是非空分母，所有 changed
  production sources 均存在于 LCOV，changed-line coverage 至少 80%；
- LCOV malformed/missing `DA`/`BRDA`、selector source/symbol/span 缺失、selector span
  没有 changed executable line 或没有 branch record、任一 selected branch hit 为零均
  非零退出；
- 成功 artifact 保存 immutable base/head、changed-line manifest、LCOV SHA256，以及每个
  selector 的 path、symbol、marker span、branch 数与 hit 结果。

关键行为不能只按 path 归类。checker 内置以下 mandatory selectors；每个 selector 都必须
定位唯一 exact Rust function symbol，并要求唯一、配对的
`// gh1108-coverage:<marker>:start|end` marker 完全位于该 symbol body 内：

| Required category | Path | Exact symbol(s) and required marker |
| --- | --- | --- |
| `catalog_evidence_validation` | `src/core/providers/google/models/registry.rs` | `validate_developer_catalog_evidence` / `catalog-evidence-validation` |
| `deprecated_param_rejection` | `src/core/providers/google/models/request_contract.rs` | `normalize_deprecated_sampling_params` / `deprecated-param-rejection` |
| `prefill_rejection` | `src/core/providers/google/models/request_contract.rs` | `normalize_gemini_contents` / `final-contents-normalization` **and** `validate_no_model_prefill` / `prefill-rejection`；两个 selector 都必需 |
| `stream_metadata_validation` | `src/server/routes/ai/token_policy.rs` | `prepare_chat_request_for_provider` / `selected-deployment-stream-metadata` |
| `live_classification` | `tests/live_gemini.rs` | `classify_live_failure` / `live-classification` |
| `live_redaction` | `tests/live_gemini.rs` | `redact_live_artifact` / `live-redaction` |
| `live_observation_canonicalization` | `tests/live_gemini.rs` | planned `canonicalize_live_observation` / `live-observation-canonicalization` |
| `live_interruption_persistence` | `tests/live_gemini.rs` | planned `run_live_gemini_smoke` / `live-runner-cancellation-persistence` |

GH1112 merged API 若不能采用这些 exact symbols/paths，必须先 amend spec、manifest 与 checker，
不得让 checker 猜 alias。marker 缺失、重复、反序、跨 symbol、空 span，或仅在同 path/
同 symbol 的 marker 外存在 covered branch，都不得满足 selector。每个 category 的所有
selector 都必须有自身 span 内的 changed `BRDA`，并达到 100%；`live_classification`、
`live_redaction`、`live_observation_canonicalization` 与
`live_interruption_persistence` 分开判定，不能互相替代。

`checks/test_gh1108_coverage_gate.py` 使用 synthetic source/diff/LCOV fixtures，至少覆盖：

- happy path 与 full-SHA/head/ancestor/tracked-clean/LCOV guards；
- changed path 不在 complete manifest、三个 read-only routing context 任一变化、任一
  `src/core/providers/vertex_ai/**` 变化均非零退出；synthetic allowed path 仍通过；
- missing changed source、empty denominator、line coverage <80%、malformed/missing
  `DA`/`BRDA`；
- 每个 required symbol/marker 的 missing/duplicate/out-of-order/outside-symbol/empty
  span；
- unrelated covered branch（同 path 但其他 symbol，以及同 symbol 但 marker span 外）
  不能满足任何 category；
- catalog、deprecated、prefill、stream metadata 的任一 selector 缺失或 uncovered 均失败；
- prefill fixtures 覆盖 interleaved System/Developer 原序 parts、developer+user
  instruction/content 均保留、Developer non-text/不可表示 payload pre-network rejection、
  assistant+developer final-model rejection，且证明 normalizer/serializer 不二次读 raw roles；
- DTO unit fixtures 独立覆盖 unknown/non-bool wire rejection，证明合法 object 未被消费；
  stream selector fixtures 覆盖 selected Gemini direct/alias/fallback 的 canonical
  include_usage=true happy path、selected Gemini internal-inconsistent metadata 与 non-stream
  + stream_options、OpenAI/OpenRouter post-selection input/output equality、selection
  failure no mutation、同 path 其他 function 与 marker 外 covered branch；任何
  conditional consume/preserve/fail-closed branch 未命中均失败；
- observation selector fixtures 覆盖 redaction-before-canonicalization、optional pair
  consistency/recomputed digest、auth/timeout/cancel no-response、typed partial facts、
  hash-only/complete-required-for-passed rejection、8 个 per-model required keys 逐项缺失、
  global static/list replacement rejection、单一 list response 派生的两条 observations
  各自 model/requested_exact_id/exact-match 一致，以及 one-fact-different
  canonical/digest branches；
- env-gate fixtures 用四个 `env_clear` + exact-env child 经过 production reader，证明
  parallel parent 零 `set_var`/`remove_var`、network counters 与 artifact paths 无泄漏；
- classification selector fixtures 穷举完整 source→class table、precedence、
  first-terminal-event-wins 与 no-message-substring branches；
- interruption/persistence selector fixtures 必须由真实 runner 驱动 barrier +
  cancellation token + reloadable artifact sink，覆盖每-step awaited atomic replace、
  cancel flush、remaining incomplete、no-later-network、persistence failure 与 new-run-id
  isolation；sink fixtures 另覆盖 default/override `<run_id>.json`、same-dir temp replace、
  Unix 0600/其他平台 contract、success/interruption retention、path traversal rejection 与
  offline temp sink/default-dir-zero-write；只构造 incomplete record 或只覆盖 sink helper
  不能满足该 category；
- classification、redaction、observation canonicalization、interruption persistence 任一
  missing/uncovered 均失败，其他三类 covered 不能替代缺失类。

## 回滚方案

以完整 implementation PR 为单位回滚 catalog delta、request contract 和 live smoke。
不得只恢复 silent sampling drop、保留新 model IDs 但删除 evidence，或把 ID 写回旧
Gemini registry 作为“部分回滚”。回滚 binary 前，operator 应停止配置两个新 ID；live
smoke 无持久状态，只删除/禁用新测试和文档入口。若回滚原因是官方 lifecycle 变化，应
先将受影响 model fail closed，再通过独立 evidence update PR 恢复。
