# Tech Spec

## Linked Issue

GH-1108 / #1108

## Product Spec

见 `specs/GH1108/product.md`。

## Implementation Gate

本实现依赖 GH1112 的 production neutral Google catalog API。当前
`origin/main@acb8051c2e203e395d90bc2de6eb8558548d552b` 尚无
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

以下锚点已在 `origin/main@acb8051c2e203e395d90bc2de6eb8558548d552b` 核验。

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
| Responses request/stream/background adapters | `src/core/models/openai/responses_api.rs:20-82`、`src/server/routes/ai/responses.rs:32-129,156-290`、`src/server/routes/ai/responses_stream.rs:53-129`、`src/server/routes/ai/responses/lifecycle.rs:147-190`、`src/server/routes/ai/chat.rs:94-163`、`src/server/routes/ai/token_policy.rs:80-88` | `ResponsesApiRequest` 没有 `top_k`，因此 non-null input 被 Serde 丢弃；shared `build_chat_request` 把 `max_output_tokens` 同时写入 `max_tokens` 与 `max_completion_tokens`。alias/fallback 的最终 provider/model 只在 sync/stream/background chat execution callback 内可用。`RequestContext.metadata` 是 serialized、untyped map，不适合作 trusted provenance carrier。 | B-005/B-007 必须用 route-local typed、non-serialized provenance 无损捕获 Responses wire origin，同时完整保留 provider-neutral canonical adapter；只有 post-selection exact GH1108 Gemini consumer 才拒绝 top_k 并执行 token-field normalization，其他 provider/model 的字段和值与 serialization 保持 B-015 compatibility。 |
| Model-specific capability dispatch | `src/core/providers/capability_dispatch.rs:5-35` | `supports_capability_for_model` 仅对 OpenAI registry 做 model lookup；Gemini 回落到 provider-wide capabilities，因此 provider-wide `ToolCalling` 会让两个 closed-capability 新模型仍可被 ToolCalling route 选择。 | B-017 必须让 Gemini deployment 按 neutral registry 的 exact model capabilities 分派，同时保留其他 Gemini 模型的 provider-wide兼容行为，不能全局删除 ToolCalling。 |
| Unary response-cache return | `src/server/routes/ai/chat.rs:103-163`、`src/server/routes/ai/response_cache.rs:25-79`、`src/core/cache/key_generator.rs:29-73`、`src/core/cache/key_policy.rs:41-70`、`src/core/cache/llm_cache.rs:273-301` | unary cache lookup/return 发生在 selected-provider hook 前；chat key payload包含请求字段，但 canonical policy 删除 `stream_options`，所以合法 non-stream 请求与带 `stream_options` 的非法请求可命中同一缓存项。 | B-007 必须在 cache return 前执行 selected-model contract，或在任何 key lookup/store 前安全 bypass 这类 metadata-bearing 请求；不能让 cache hit 绕过 non-stream + stream_options 拒绝。 |
| Gemini streaming transport | `src/core/providers/gemini/client.rs:93-118` | unary/stream 分别选择 generateContent/streamGenerateContent；stream 与 stream_options 都不是 generation body 字段，transformer 也未读取 stream_options。 | B-007 可保留 `stream`，但 `stream_options` 只能作为 gateway metadata，不能成为第四个 provider param。 |
| Captured tracing pattern | `src/core/observability/tests.rs:81-101,310-315` | `MakeWriter` + `tracing::subscriber::set_default` 可把 tracing bytes 捕获到测试 buffer。 | B-012 live redaction fixture 必须覆盖 tracing sink。 |
| Pricing authorities | `src/core/providers/gemini/models/mod.rs:83-105,127-140`、`src/core/pricing_service/{mod.rs,loader.rs,authority.rs}`、`config/model_prices_extended.json` | neutral catalog pricing helper 供 provider-local cost 使用；gateway 默认运行时 authority 独立加载 embedded `model_prices_extended.json`。provider-aware resolver 只有 Azure/Bedrock/xAI catalog fallback，没有 Gemini neutral-catalog fallback。 | 只加 neutral metadata 会使默认 unpriced reject 在预算预留前拒绝新 ID；必须添加 exact Developer runtime rows 与双路径 parity tests。 |
| Public Gemini-capable entrypoints | `src/server/routes/ai/{mod.rs,chat.rs,chat_streaming.rs,completions.rs,completions_streaming.rs,responses.rs,responses_stream.rs,responses/lifecycle.rs,gemini.rs}` | chat、legacy completions unary/stream 与 Responses sync/stream/background 都转换到 shared chat execution；native `/v1`、`/v1beta`、`/gemini/v1`、`/gemini/v1beta` 的 generateContent/streamGenerateContent 直接转发 JSON `Value`，不经过 chat preflight。 | 必须封闭入口矩阵并给 native 路径增加 exact-model preflight；只测 `/v1/chat/completions` 不能证明协议约束。 |
| Credential config | `src/core/providers/gemini/config.rs:135-161` | `from_env` 依次尝试 `GOOGLE_API_KEY`、`GEMINI_API_KEY`，最后才尝试 Vertex 的 `GOOGLE_CLOUD_PROJECT`+`GOOGLE_CLOUD_LOCATION`；service-account path 只在 Vertex pair 后读取。 | live smoke 必须分别接受两个 Developer aliases、锁定 GOOGLE precedence，并拒绝 Vertex-only env 满足 Developer opt-in gate；不得格式化/落盘 key。 |
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
    "src/core/providers/capability_dispatch.rs",
    "src/server/routes/ai/gemini.rs",
    "src/server/routes/ai/gemini/spend.rs",
    "src/core/models/openai/requests.rs",
    "src/core/models/openai/responses_api.rs",
    "src/server/routes/ai/token_policy.rs",
    "src/server/routes/ai/chat.rs",
    "src/server/routes/ai/chat_streaming.rs",
    "src/server/routes/ai/chat_tests.rs",
    "src/server/routes/ai/response_cache.rs",
    "src/server/routes/ai/responses.rs",
    "src/server/routes/ai/responses_stream.rs",
    "src/server/routes/ai/responses/lifecycle.rs",
    "src/server/routes/ai/responses/lifecycle_tests.rs",
    "src/server/routes/ai/responses_stream_tests.rs",
    "src/core/cache/key_generator.rs",
    "src/core/cache/key_policy.rs",
    "src/core/cache/llm_cache.rs",
    "tests/gemini_router_fallback_routes.rs",
    "tests/responses_routes.rs",
    "config/model_prices_extended.json",
    "src/core/pricing_service/authority_tests.rs",
    "src/server/routes/ai/spend_runtime_pricing_tests.rs",
    "tests/live_gemini.rs",
    "checks/gh1108_coverage_gate.py",
    "checks/test_gh1108_coverage_gate.py",
    ".github/workflows/ci-coverage.yml",
    ".gitignore",
    "docs/providers/README.md",
    "docs/providers/gemini.md"
  ],
  "spec_refs": [
    "specs/GH1108/product.md#behavior-invariants",
    "specs/GH1108/product.md#验收标准",
    "specs/GH1108/tech.md#implementation-gate",
    "specs/GH1108/validation.md#product-to-test-mapping",
    "specs/GH1108/validation.md#test-plan",
    "specs/GH1108/validation.md#versioned-exact-head-coverage-checker-contract",
    "specs/GH1112/tech.md",
    "specs/GH1112/tasks.md"
  ]
}
```

`src/core/providers/google/**` 是 GH1112 计划并拥有的路径，当前尚不存在。implementation
开始前必须以 merged GH1112 head 重新验证以上清单；任何必要路径差异通过 spec amendment
处理，不能把旧 `src/core/providers/gemini/models/**` 加回 manifest。

`src/server/routes/ai/completions.rs`、`src/server/routes/ai/completions_streaming.rs`、
`src/server/routes/ai/mod.rs`、`src/server/routes/ai/batches.rs` 与
`src/core/types/chat.rs` 是本设计已核验的 read-only context，不是 planned writable
paths。`chat.rs`、`chat_streaming.rs`、Responses DTO/sync/stream/background adapters、
model-capability dispatch 与实际 cache policy/key paths 已因 selected-model contract
闭环加入 writable manifest；现有 builder 的 canonical `StreamOptions` 仍只在最终
selected deployment 的 `token_policy` hook 条件消费。“preserve”指 canonical object
在 pre-selection 阶段不被 take/drop，不承诺 client wire `include_usage=false` 原值传到
upstream。implementation exact-head 必须以以下 gate 证明五个 remaining context paths
未变；若实现
发现必须修改其中任一路径，先 amend 本 manifest，不得静默扩 scope：

```bash
test -z "$(git diff --name-only \
  "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" -- \
  src/server/routes/ai/completions.rs \
  src/server/routes/ai/completions_streaming.rs \
  src/server/routes/ai/mod.rs \
  src/server/routes/ai/batches.rs \
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
| `gemini-2.0-flash-thinking-exp` | `shutdown` | `https://ai.google.dev/gemini-api/docs/changelog` | changelog records the exact experimental ID shutdown on 2025-12-02 |
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
  任一 System 或 Developer turn 的 meaningful non-text part、tool payload 或任何无法
  无损表示为 instruction text 的 payload 都必须 typed invalid-request、
  network counter=0，不能丢弃；
  User→`user`、Assistant→`model`；Tool/Function 的现有 mapping 不在 GH1108 扩展；
- `validate_no_model_prefill` 必须在最终 `contents` 上运行：contents 为空或最后一项 role
  为 `model` 即 typed invalid-request。fixture 必须锁定 interleaved
  system/developer parts 的原序、developer+user 同时保留 instruction/user、
  System/Developer non-text 或不可表示 payload pre-network 拒绝、assistant+system 与
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

Responses sync、stream 与 background 使用 two-stage contract，不能全局改变 provider-neutral
adapter：

1. `src/core/models/openai/responses_api.rs` 的 Responses DTO 增加 deserialize-only wire
   capture。它用 typed presence enum 区分 `Missing`、`Null`、`Value(Value)`，字段
   `skip_serializing`；`max_output_tokens` 是否来自 Responses wire 也进入同一个 typed
   `ResponsesRequestProvenance`。该类型只表达可信 API-boundary facts，不是 provider
   request 字段。
2. `src/server/routes/ai/responses.rs` 在调用 `build_chat_request` 前构造 provenance，
   再将其作为独立 route-local sidecar 交给 sync、stream 或 background execution。
   `build_chat_request` 本身保持当前 canonical contract：`max_output_tokens` 继续同时写入
   `ChatCompletionRequest.max_tokens` 与 `max_completion_tokens`，`top_k` 不写入
   `extra_body`。因此任何 final provider selection 之前的 request 字段和值保持不变。
3. sync lane 在 `src/server/routes/ai/chat.rs` 的 selected-operation closure、stream lane
   在 `src/server/routes/ai/responses_stream.rs` 的 selected-operation closure 捕获 sidecar；
   `src/server/routes/ai/responses/lifecycle.rs` 的 background task 也必须把 sidecar 传给
   同一 provenance-aware chat entrypoint。三者只在 alias/fallback 已解析、最终
   provider/model 已知后，把它传给
   `src/server/routes/ai/token_policy.rs` 的
   `normalize_selected_gemini_responses_provenance`。不得把 provenance 存入
   `ChatCompletionRequest`、`CoreChatRequest`、`RequestContext.metadata`、
   `extra_body`、cache key 或 provider serialization。
4. 只有最终 selected provider 是 Gemini Developer 且 exact model 属于
   `{gemini-3.6-flash, gemini-3.5-flash-lite}` 时，consumer 才读取 sidecar：
   `Missing`/`Null` top_k 为 absent，`Value` 在 budget/network 前返回 typed
   invalid-request；若有 `max_output_tokens` origin，则对该 selected request 单次归一化为
   `max_tokens=Some(value), max_completion_tokens=None`。Gemini mapping 随后把同一 limit
   写到 `generationConfig.maxOutputTokens`，预算也使用 normalized limit。background task
   若在此 selected-model preflight 失败，必须在 provider network 前把 `GatewayError`
   确定性映射为 `ResponseApiError { code, message }`，并用单次 store mutation 同时写入
   `status=failed` 与 `error=Some(...)`；只调用现有 `set_background_status(..., "failed")`
   而留下 `error=None` 不满足契约。后续 GET 必须返回该 typed error，cancelled record
   仍不得被晚到的 task 覆盖。
5. 最终 selected OpenAI、OpenAI-like、OpenRouter、其他 provider 或其他 Gemini model
   时，consumer 丢弃 sidecar且不修改 canonical request；provider-bound 字段、值与
   serialization 必须和本变更前逐字段相等。selection failure 不消费 sidecar，也不修改
   immutable/core request。所有 provider 的 upstream body 都不得出现 provenance 或
   `top_k`。

sync/stream/background fixtures 必须分别锁定：OpenAI exact、OpenAI-like alias/fallback、其他
Gemini model 的 canonical request 与 serialized upstream baseline parity；两个 exact
GH1108 Gemini model 的 direct/alias/fallback selected identity 才触发 non-null top_k
network=0 rejection 与 token single-normalization；null/omitted 不拒绝；sidecar 不进入
extra_body、cache identity 或 upstream。Chat Completions 输入不能通过伪造 extra_body
获得 trusted Responses provenance。background negative fixture 必须等待 task terminal，
并断言 stored response 原子进入 `failed`、`error.code`/`error.message` 与 unary
invalid-request 映射一致、network=0；只观察初始 queued 或 opaque failed 不算通过。

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

unary response cache 是同一 contract 的 pre-network boundary。因为当前 cache
canonicalization 明确移除 `stream_options`，实现不得把 cache-key 差异当作安全保证。
本 spec 固定采用 safe-bypass：`response_cache` 的 lookup 与 store policy 在生成 key 或
访问 memory/Redis cache 前，对任何 `stream_options.is_some()` 的 non-stream request 返回
cache miss/no-store；请求随后按正常 alias/fallback selection 进入
`prepare_chat_request_for_provider`。最终选中两个 GH1108 Gemini exact IDs 时，
non-stream + canonical metadata 必须 typed invalid-request、network counter=0；其他 provider
保持原 selected hook 语义，只是本次请求不读写 response cache。`chat.rs` 必须保证 bypass
发生在 cache return 之前；`key_generator.rs`/`key_policy.rs`/`llm_cache.rs` 的 fixture
必须显式证明带/不带 metadata 的请求在旧 key policy 下会碰撞，但 policy bypass 使该碰撞
不可达，不能靠删除 regression 或把 `stream_options` 悄悄加入 response identity 掩盖。

cache-hit regression 先用合法无 metadata 的 non-stream 请求填充两个新 exact model 之一的
response cache，再发出 messages/model 相同但含 canonical `include_usage=true` 的
non-stream 请求；必须观测 cache return=0、provider network=0、稳定 invalid-request。
direct exact ID、alias 与 fallback 最终选择该 exact model 至少各有一例；合法无 metadata
请求仍可命中 cache，证明不是全局禁用缓存。测试不得通过清空 cache 或更换 prompt/model
制造 miss。

#### Closed public-entrypoint matrix

所有可以最终选择 Gemini Developer chat deployment 的公开请求入口必须归入下表。表外
route 若使用其他 `ProviderCapability`，按 capability selector 不能路由到这两个 chat
models；Batch route 只选择 OpenAI/OpenAI-compatible lifecycle proxy config，不执行
Gemini chat model request，二者都必须以 source/static invariant 保持 non-routable。

| Public entry shape | Unary/stream adapter | Required GH1108 preflight | Exact fixture |
| --- | --- | --- | --- |
| `POST /v1/chat/completions` | `chat.rs` / `chat_streaming.rs` | alias/fallback 后 selected Gemini exact identity 调用 shared chat contract；provider direct entry 再防御性校验 | unary + stream；deprecated params、terminal model、direct/alias/fallback network=0 |
| `POST /completions`、`/v1/completions`、`/engines/{id}/completions`、`/v1/engines/{id}/completions`、`/openai/deployments/{id}/completions` | `completions.rs` / `completions_streaming.rs` 先构造 `ChatCompletionRequest` | 与 chat 相同的 selected-provider hook；adapter 不得绕过或静默删除 deprecated typed fields | 每个 handler family 至少一个 unary/stream route fixture，另用 route-table/source invariant 锁定所有 aliases 仍指向同 handler |
| `POST /v1/responses` | `responses.rs::build_chat_request` 保持 sync/stream/background 当前 canonical adapter；typed provenance 由 Responses route-local sidecar 经 `chat.rs`/`responses_stream.rs`/`responses/lifecycle.rs` 到 selected-model `token_policy` | pre-selection 保持 `max_tokens` 与 `max_completion_tokens` 双字段且 top_k 不进 extra_body；只有 final selected exact GH1108 Gemini 才拒绝 provenance 中 non-null top_k 并把 Responses token origin 单次归一化为 `max_tokens`；其他 provider/model 字段和值及 serialization 不变 | sync + stream + background：OpenAI/OpenAI-like alias/fallback baseline parity、其他 Gemini parity、GH1108 direct/alias/fallback normalization、top_k missing/null/value、no upstream leak、terminal output-role、network counter |
| `POST /v1/models/{model}:generateContent`、`/v1beta/models/{model}:generateContent`、`/gemini/v1/models/{model}:generateContent`、`/gemini/v1beta/models/{model}:generateContent` | `gemini.rs` unary handler | path exact ID 上调用 shared native preflight，发生在 router/budget reservation/network 前；`GeminiProvider::gemini_generate_content` 再防御性调用 | 4 prefixes × unary，sampling null/non-null、terminal explicit-user/omitted-role success、explicit-model/unknown/malformed failure、network=0 |
| 上述四个 prefix 的 `:streamGenerateContent` | `gemini.rs` stream handler | 与 native unary 相同，且 validation 必须发生在产生 streaming HTTP response 前 | 4 prefixes × stream，同一 role/sampling/ambiguity 矩阵、network=0 |

OpenAI-compatible 三个 adapter family 共享的生产 preflight 仍位于
`request_contract.rs` + selected-provider `token_policy` + Gemini provider，不为每个 route
复制逻辑。`tests/gemini_router_fallback_routes.rs` 驱动公开 handler/fake provider 证明
每个 family 可达 shared gate；route alias 绑定由 `src/server/routes/ai/mod.rs` 的精确
source invariant 锁定。native lane 在 `request_contract.rs` 增加
`normalize_native_gemini_request(model, body)`：

- 只对两个 exact IDs 生效，其他已支持模型 body byte-for-byte/Value-equal 保持现状；
- body 必须是 object；`generationConfig` 缺失或 JSON null 视为 absent（null key 被
  删除），若 non-null 则必须是 object。删除 object 中 value 为 JSON null 的
  `temperature`/`topP`/`topK`，任一 non-null 值返回 typed invalid request；
- `contents` 必须是 array。由尾向前跳过 semantic-empty content：`parts` 必须存在且为
  array，空 array 或只含 null/blank `{text}` 才为空；任何其他合法 part 为
  meaningful。missing/malformed content/parts 或无法闭合判定的 part shape fail closed；
- 对每个 meaningful content，role normalization 是闭集：exact string `user`/`model`
  保留；field missing 按 Google `Content` schema default normalize 为 `user`；explicit
  null、其他 string、number/bool/object/array 一律 unknown/nonrepresentable 并 fail
  closed。只有全序列均唯一归一化才算 unambiguous；
- 至少一个 meaningful content；terminal normalized role 为 `user`（包括 omitted）
  通过，terminal exact `model` 以稳定 prefill error 拒绝。`systemInstruction` 独立存在，
  不参与 terminal content 判定；
- normalizer 返回 cleaned body，native route 的预算 estimate 与 provider transport 消费
  同一个结果；`GeminiProvider::gemini_generate_content` 对内部直接 caller 重复调用须
  幂等，不能重新解释或改变已 normalized body。

### 3. Pricing 与能力边界

neutral metadata 保持 GH1112 access API；同时当前 gateway 的真实预算/结算 authority
是 embedded `config/model_prices_extended.json`，且 provider catalog fallback 不包含
Gemini。为避免新 ID 在默认 `unpriced_model_policy=reject` 下 pre-network 被拒，本 issue
必须更新既有 runtime table，不新增 accessor、fallback 或第二个 authority。

闭合 runtime pricing rows 只有：

| Runtime JSON key | `litellm_provider` | input/output per token | limits | Source |
| --- | --- | --- | --- | --- |
| `gemini/gemini-3.6-flash` | `gemini` | `0.0000015` / `0.0000075` | input 1,048,576；output 65,536 | Google Developer paid Standard pricing URL |
| `gemini/gemini-3.5-flash-lite` | `gemini` | `0.0000003` / `0.0000025` | input 1,048,576；output 65,536 | Google Developer paid Standard pricing URL |

不得添加 unprefixed key，也不得写 `vertex_ai`/`google` row。provider-aware resolution 对
`provider=gemini, model=<exact ID>` 必须解析到上表对应 prefixed key；对
`provider=vertex_ai` 必须返回 missing/unpriced。价格路径矩阵为：

| Consumer | Actual authority call | Required result |
| --- | --- | --- |
| OpenAI chat、Responses、legacy completions unary/stream reservation | `estimate_loaded_completion_cost_for_provider` | 默认 embedded service 对两个 ID 产生精确正数 reservation，不进入 unpriced policy |
| 同上 response settlement | `calculate_loaded_usage_cost_for_provider` | fixed usage 按 exact per-token 值 settle |
| native generateContent/streamGenerateContent reservation | `gemini::spend::reserve_gemini_budget` → loaded estimate | 两个 ID 均成功；请求 maxOutputTokens 参与 estimate |
| native unary/stream settlement | `gemini::spend::{record_gemini_spend,settle_gemini_stream_spend}` → loaded usage cost | usageMetadata 的 prompt/candidate tokens 产生相同精确成本 |
| `POST /v1/pricing/calculate` with `provider=gemini` | `calculate_loaded_completion_cost_for_provider` | fixed token response 与 neutral catalog cost exact-equal |
| any row with `provider=vertex_ai` | provider-aware lookup | fail closed；Developer evidence 不满足 Vertex |

测试从“官方 per-million → neutral stored unit/cost → runtime per-token row → reservation/
settlement fixed usage”逐层断言，避免 1000/1,000,000 倍误差。Batch、Flex、Priority 或
其他 tier 不得写入相同 fixture、复用这些数值或由测试宣称通过。不得更改
pricing resolver alias、fallback、unpriced policy、budget/callback 语义；发现需要这些
更改时必须先 amend spec 并与 GH1113 串行。

两个新模型必须使用相同的 exact、闭合能力 disposition：

- public `ModelInfo.capabilities` 恰为
  `{ProviderCapability::ChatCompletion, ProviderCapability::ChatCompletionStream,
  ProviderCapability::GeminiGenerateContent}`，
  `supports_streaming=true`、`supports_tools=false`；
- model feature flags 恰为
  `{ModelFeature::MultimodalSupport, ModelFeature::StreamingSupport,
  ModelFeature::SystemInstructions}`。三项分别绑定当前 callable source：
  `client.rs:361-395` 的 inline base64 image→`inlineData`、
  `client.rs:93-118` 的 `generateContent`/`streamGenerateContent` endpoints 与
  `gemini.rs` 的 `ProviderCapability::GeminiGenerateContent` selector，以及
  `client.rs:287-290` 的 `systemInstruction` serializer；`MultimodalSupport` 不扩张为
  audio/video/document support。

| Provider capability | Advertised for both exact IDs | Callable evidence / disposition |
| --- | --- | --- |
| `ChatCompletion` | yes | shared OpenAI-compatible unary chat execution |
| `ChatCompletionStream` | yes | shared OpenAI-compatible streaming execution |
| `GeminiGenerateContent` | yes | native route selector + provider `generateContent`/`streamGenerateContent` transport |
| `ToolCalling` / `FunctionCalling` | no | transformer 没有完整 tool consumer；留给 GH1111 |
| `CodeExecution` / `BatchProcessing` / `RealtimeApi` | no | 无本 provider 可兑现 route/serializer contract |
| `ImageGeneration` / audio generation capabilities | no | 当前只有 multimodal input，不是 generation output |

前三行是且仅是 capability equality set；后三行代表完整 enum 中相关负例，其他未列出的
capability 也默认不广告，不能把表当作开放 allowlist。

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

路由 eligibility 必须消费同一 exact-model capability truth，而不能只修 model-list
metadata。`Provider::supports_capability_for_model` 对 Gemini deployment 必须查询 GH1112
neutral registry 的 exact、区分大小写 model record：找到 record 时以其 closed
`ModelInfo.capabilities` 判定，两个新 ID 因而只允许
ChatCompletion/ChatCompletionStream/GeminiGenerateContent；ToolCalling、FunctionCalling
及其他集合外 route 不得选择它们。找不到 exact neutral record 的既有 Gemini model 必须
回落到当前 provider-wide capability behavior，保持 B-015 compatibility；不得为了两个
新模型从 `GeminiProvider::capabilities` 全局删除 ToolCalling，也不得用 family substring、
prefix、大小写归一化或 alias 猜 record。alias/fallback 必须对 deployment 的最终 exact
model 执行 lookup。

`capability_dispatch.rs` inline tests 与 router fixture 必须同时证明：两个新 exact IDs 的
三项正能力可选择；ToolCalling/FunctionCalling 及随机 enum 负能力不可选择；case/prefix
近似 ID 不命中新 record；一个有既有 provider-wide ToolCalling 行为但 neutral registry
无 exact record 的旧 Gemini model 仍保持原 eligibility。这样 model metadata equality 与
实际 route dispatch 使用同一 registry fact，且 model-specific 收紧不扩大为 provider-wide
breaking change。

### 4. Opt-in live smoke

新增 `tests/live_gemini.rs`，复用现有 live Bedrock pattern：

- `#[ignore]`；
- 只有 `LITELLM_RS_LIVE_GEMINI=1` 才允许联网；
- Developer key aliases 精确为 production `from_env` 支持的
  `GOOGLE_API_KEY`、`GEMINI_API_KEY`，解析优先级也精确为 GOOGLE 后 GEMINI；Vertex env
  永远不能满足此 Developer live gate。key 不写入命令、Debug、error 或 artifact；
- 依次执行两个 exact ID 各自的 static snapshot、一次完整 official list pagination
  traversal（派生两个独立 per-model records）、两个 exact ID 各自的 get 与最小
  generate-content call；
- offline gate fixture 使用以下 closed actual-env matrix；普通并行 test process 禁止调用
  `std::env::set_var`/`remove_var`。每 case 启动隔离子进程，对 child 先
  `env_clear()`，再精确设置表内 env、fake transport/counter 与必要的非 credential
  bootstrap，经过 production actual env reader boundary：

| Case | Opt-in | Exact credential env after `env_clear` | Expected source | Network |
| --- | --- | --- | --- | --- |
| disabled-empty | unset | none | disabled | 0 |
| disabled-google | unset | `GOOGLE_API_KEY=sentinel_google` | disabled | 0 |
| disabled-gemini | unset | `GEMINI_API_KEY=sentinel_gemini` | disabled | 0 |
| enabled-empty | `1` | none | missing Developer key | 0 |
| enabled-google | `1` | GOOGLE only | Developer/GOOGLE | fake Developer only |
| enabled-gemini | `1` | GEMINI only | Developer/GEMINI | fake Developer only |
| enabled-both | `1` | GOOGLE + GEMINI, distinct sentinels | Developer/GOOGLE precedence | fake Developer only |
| enabled-vertex | `1` | `GOOGLE_CLOUD_PROJECT` + `GOOGLE_CLOUD_LOCATION` | Vertex-only rejected | 0 |
| enabled-vertex-sa | `1` | Vertex pair + `GOOGLE_APPLICATION_CREDENTIALS` | Vertex-only rejected | 0 |
| enabled-project-only | `1` | project only | missing Developer key | 0 |
| enabled-location-only | `1` | location only | missing Developer key | 0 |
| enabled-google-vertex | `1` | GOOGLE + Vertex pair | Developer/GOOGLE before Vertex | fake Developer only |
| enabled-gemini-vertex | `1` | GEMINI + Vertex pair | Developer/GEMINI before Vertex | fake Developer only |

每个 fake-positive case 必须证明只使用预期 sentinel/source 且不读取另一 alias/Vertex
credential；zero-network cases 在 gate return 前 counter 保持 0。parent 并行运行所有
cases 后断言 child env/counter/artifact 无交叉泄漏；真实 endpoint 仍只由手工 ignored
test 使用；
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
  `artifact_persistence_failed` 只属于下述 uncommitted runner outcome，不得写进并未成功
  commit 的 artifact record。
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
    `{requested_exact_id, pages_fetched, returned_model_count, exact_matches}`，其中每个
    exact match 保存 `{resource_name, exact_id, supported_generation_methods,
    input_token_limit, output_token_limit}`；
  - `get_model`：
    `{requested_exact_id, returned_resource_name, returned_exact_id, exact_id_match,
    supported_generation_methods, input_token_limit, output_token_limit}`；
  - `minimal_call`：
    `{requested_exact_id, returned_model_version, candidate_count, finish_reasons,
    response_text, prompt_token_count, candidates_token_count, total_token_count}`；
  - `aggregate`：
    `{required_keys, passed_keys, failed_keys, incomplete_keys}`，每个 key 为
    deny-unknown `{step, model}`；
- `collect_all_models_pages` 从 `page_token=None` 开始，请求每一页并聚合完整 models
  response。missing/null/empty `nextPageToken` 是唯一正常终止；non-string/malformed token、
  已见 token 再现、任一中间页 transport/parse failure，或第 100 页仍返回 non-empty next
  token 均以 failed/protocol/step_failed 终止，partial pages 可审计但不得 passed。token
  set 与 page counter 只用于控制流，不写 credential-bearing request data。fixture 必须覆盖
  first-page terminal、目标只在 later page、重复 token cycle、malformed token、page
  failure、100-page bound 与跨页 duplicate exact match；
- passed static record 要求 catalog_present 与所有 metadata/evidence facts；一次完整
  official pagination traversal 可派生两个独立 per-model list records，但必须先到达正常
  terminal token。每条的 `(step, model)`、`requested_exact_id`、`pages_fetched`、
  `returned_model_count` 与 exact_matches 中唯一 case-sensitive match 必须共同对应完整
  traversal 和该 exact ID，且每条独立 canonicalize/hash/persist；passed get 要求 returned
  exact ID 匹配；
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
| 其他 HTTP status、local schema/exact/observation validation | `failed` / `protocol` | `step_failed` |
| 无 external terminal event 的自然聚合缺 required key | `failed` / `protocol` | `missing_required_step` |

classification precedence 固定为：

1. runner 以 compare-and-set 原子记录的首个 execution-terminal event；
2. response 中已成功解析的 exact structured Google reason；
3. typed `ProviderError` variant；
4. numeric HTTP status fallback；
5. local protocol validation。

同一层冲突或未知值 fail closed 为 protocol。deadline/cancellation 同时就绪时
first-execution-terminal-event-wins，后到 execution signal 不得覆写该 execution
fact。403 +
`RESOURCE_EXHAUSTED` 因 structured reason 优先而归 quota；remote
`DEADLINE_EXCEEDED` 是 network/step_failed，只有 runner 自身 deadline 才使用
`transport_timeout`。passed record 的 error_class/termination_reason 均为 none。

artifact commit 是 classification 之后的独立 finalization dimension。闭合优先级矩阵为：

| First execution terminal CAS | Finalization | Returned outcome | Durable claim |
| --- | --- | --- | --- |
| none 或已分类 step/deadline failure | persisted | `Committed(LiveArtifact)`，artifact 保持对应 passed/failed facts | final snapshot 可重读 |
| external cancellation/interruption | persisted | `Committed(LiveArtifact)`，in-flight/remaining 为 incomplete、no error class、external reason | final cancellation snapshot 可重读 |
| none、deadline、cancel 或 interruption 中任一 | persistence_failed | `UncommittedFinalizationFailure {run_id, status=failed, error_class=protocol, termination_reason=artifact_persistence_failed, execution_terminal, last_committed_snapshot}` | 不声称失败 snapshot 已写入；只允许指向此前真实 committed snapshot |

`execution_terminal` 是独立 deny-unknown typed enum
`Option<{transport_timeout, externally_cancelled, externally_interrupted}>`，保留 CAS winner；
持久化失败不能把 cancel 改写为“从未取消”，也不能让 cancel 掩盖 required commit failure。
`last_committed_snapshot` 只含 credential-free exact path/run_id/digest（若尚无成功 commit
则为 none），不嵌入 raw error 或伪造 artifact。调用方/CLI 对
`UncommittedFinalizationFailure` 必须以非成功返回处理。

`run_live_gemini_smoke` 必须接收可注入 cancellation token、可在指定 step barrier 阻塞的
transport，以及 `LiveArtifactSink`。runner 对每个 step 执行：

1. 产生并 redaction/canonicalization 当前 typed record；
2. 调用 sink 以 temp-write + atomic replace 保存整个当前 run snapshot，并 await 成功；
3. 只有 persistence 成功后才开始下一个 required step/network call。

收到 external cancellation/interruption 后，runner 停止调度新网络调用，为 in-flight 与
尚未开始的 required keys 写入 `incomplete_keys`，保存真实 attempt/termination 与可选
partial observation，await 最终 atomic flush 后返回；不得把这些 keys 再合成为
missing-required protocol failure。若逐 step persist 或 cancel final flush 失败，runner
立即停止后续 network，返回上述 typed `UncommittedFinalizationFailure`，保留原
execution-terminal CAS winner，不构造/返回 `Committed` final artifact；此前已成功原子
写入的 last snapshot 仍须可 reload 且 bytes/digest 不变。retry 必须生成不同 run_id，旧
run steps 保持只读，不能聚合到新 run。

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
atomic replace、reload、retention 与权限分支都必须有测试。implementation 同一 PR 必须在
repository `.gitignore` 添加精确 anchored line `/artifacts/live/GH1108/`；该 exact pattern
属于 planned change，不能用未声明的更宽目录 ignore 代替验收。

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
- repository ignore policy 的 exact `/artifacts/live/GH1108/` pattern；
- Developer/Vertex 分离；
- 被停止公开的旧 ID disposition。

`docs/providers/README.md` 只增加 provider doc 索引。不得修改高上下文
`AGENTS.md`/`CLAUDE.md` 或用户配置。

## Validation Contract

完整 B-001..B-017 mapping、verification data flow、test plan 与 exact-head coverage-checker
requirements 已集中到 [`validation.md`](validation.md)。implementation 与 review 必须同时
满足该文件；拆分只控制文档大小，不改变任何验收要求。

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

## Validation Execution

完整命令清单见 [`validation.md#test-plan`](validation.md#test-plan)，versioned checker 的
mandatory function policies、negative fixtures 与 exact-head fail-closed rules 见
[`validation.md#versioned-exact-head-coverage-checker-contract`](validation.md#versioned-exact-head-coverage-checker-contract)。

## 回滚方案

以完整 implementation PR 为单位回滚 catalog delta、request contract 和 live smoke。
不得只恢复 silent sampling drop、保留新 model IDs 但删除 evidence，或把 ID 写回旧
Gemini registry 作为“部分回滚”。回滚 binary 前，operator 应停止配置两个新 ID。
manual smoke 会保留 credential-free artifacts；回滚代码、测试或 `.gitignore` 不会也不得
静默删除这些 operator data。operator 必须先用文档命令检索默认
`artifacts/live/GH1108/` 与所有曾配置的 override directories，按 retention policy 显式
归档或清理，再删除/禁用测试和文档入口；若保留 artifacts，则回滚 `.gitignore` 时仍必须
维持等价 ignore protection。若回滚原因是官方 lifecycle 变化，应先将受影响 model fail
closed，再通过独立 evidence update PR 恢复。
