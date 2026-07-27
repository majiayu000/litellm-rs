# Tech Spec

## Linked Issue

GH-1127 / #1127

complexity: high

## Product Spec

[`product.md`](product.md)

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Canonical guardrail boundary | `src/server/guardrails.rs` | `check_chat_output` only accepts a completed `ChatCompletionResponse` and extracts non-streaming message content. | Streaming needs a text-oriented entrypoint that preserves the same `enforce` semantics. |
| Guardrail config | `src/core/guardrails/config.rs`, `src/config/models/guardrails.rs` | `check_output` and `fail_open` exist, but there is no streaming-output window policy. Gateway deserialization rejects unknown fields. | `windowed_cumulative` requires an explicit, validated character threshold and fixed memory ceiling. |
| Chat SSE | `src/server/routes/ai/chat_streaming.rs`, `src/server/routes/ai/chat_sse.rs` | Each converted `ChatCompletionChunk` is serialized and sent immediately; errors currently include `[DONE]`. | Must hold each pending event window until its cumulative output passes, while retaining OpenAI error shape. |
| Completion SSE | `src/server/routes/ai/completions_streaming.rs`, `src/server/routes/ai/completions_sse.rs` | Each text completion event, including `echo`, is sent immediately. | Must scan exactly the client-visible completion text and preserve echo/event order. |
| Responses SSE | `src/server/routes/ai/responses_stream.rs`, `src/server/routes/ai/responses_stream_support.rs` | Text, reasoning and function arguments are emitted while `full_text`/tool state is still accumulating; success is persisted after `response.completed`. | Each pending model-bearing window must pass a cumulative check before emission; persistence remains final-success-only. |
| Stream lifecycle | `src/server/routes/ai/budgeted.rs`, `src/server/routes/ai/callbacks.rs` | Streaming workers own reservation settlement, callback terminal state and provider lease. | Guardrail rejection is a gateway-policy failure after chargeable provider success, not a free or retryable provider failure. |
| Existing coverage | `tests/integration/guardrail_route_tests.rs`, `tests/integration/completions_route_tests.rs`, `tests/integration/completions_route_tests/tests/streaming_and_budget_tests.rs`, `src/server/routes/ai/responses_stream_tests.rs` | Unary guardrails and normal streaming/error/settlement are covered separately; no streaming-output guardrail fixture exists. | Cross-route behavior and lifecycle assertions require deterministic SSE integration tests. |

## 设计方案

### 1. 配置契约

在 `GuardrailConfig` 增加 `stream_output_check_chars: usize`，gateway 默认值为
`256`，启动验证只接受 `1..=4096`。配置必须使用 snake_case，未知字段、0 或
超过 4096 的值显式失败。该配置只在
`enabled && check_output && guardrail_count > 0` 时生效。

审核模式不暴露运行时 enum；稳定行为固定为 `windowed_cumulative`。pending SSE
bytes 与累计扫描文本分别受内部常量 `8_388_608` bytes 限制，避免引入第二个公开
配置。`fail_open` 继续由 `GuardrailEngine::check_output` 决定，streaming 层不得
增加第二套 fallback。

### 2. 共享缓冲器

新增 route-private `StreamOutputGuardrail`：

- 创建时记录是否需要输出检查和 `stream_output_check_chars`；
- 按到达顺序保存当前 pending 窗口的完整 SSE `Bytes`，并单独累积带字段边界的
  全部历史扫描文本；
- 对 pending SSE bytes 与累计 scan text 使用 checked accounting，在追加前检查
  8 MiB 内部上限，超限返回 typed `BufferLimitExceeded`；
- active=false 时不保存历史 event，调用方继续原有 immediate-send 分支；
- active=true 时自上次通过检查后新增的模型文本达到配置字符数或 upstream EOF 时，
  对截至当前 event 的累计文本调用 `check_output_text`；阈值按 Unicode scalar
  value 计数，是触发下限而非硬切分边界，触发 event 不得拆分；
- 检查通过后按原顺序 drain 当前 pending events 并清空 pending；拒绝或错误时丢弃
  当前 pending events，取消上游并进入 terminal。

扫描文本采用稳定的 choice/surface 边界，不能按 provider chunk 独立判断：

- Chat 从转换后的 `ChatCompletionDelta` 提取 `content`、`thinking.content` 或其
  `reasoning_content` alias（同一语义值只累计一次）、`audio.transcript`、
  legacy `function_call.name/arguments` 与
  `tool_calls[].function.name/arguments`；同时从 choice logprobs 提取
  `content[].token` 与 `top_logprobs[].token`。每个 choice 必须维护跨 event 的
  content 累计值、ordered chosen-token 累计值和不可逆的 alias/materialized 状态，
  不能按单个 event 的相等性跳过 chosen token。只要两个累计值仍完全相等，chosen
  可作为 content 的 alias 而不重复写入 scan payload；一旦两者不再相等，就把完整
  chosen 历史（包括此前 alias 的前缀）物化为一个连续 scan surface，并在后续 event
  始终向该 surface 追加，不能重新折叠。alias bookkeeping 和物化后的 surface 都纳入
  同一个 cumulative 8 MiB checked accounting。chosen token 之间不能插入边界，也
  不能用全局 string set 删除不同位置的同值 token。top candidate 按
  `(content_position, candidate_index)` 加边界；仅当候选 token 在同一位置等于
  chosen token 时视为同一 representation 而跳过。audio base64 data、role、IDs、
  usage、finish reason、logprob bytes/数值与 transport markers 只缓冲、不扫描。
- 当前 canonical `ChatDelta` 没有 refusal，`convert_core_chunk_to_streaming` 也不会
  从 provider pipeline 填充 wire `ChatCompletionDelta.refusal`；本 tranche 不新增
  该 surface，不得用 synthetic post-conversion fixture 代替端到端可达性。未来若
  provider/canonical/conversion 三层接入 refusal，必须先更新 manifest 与测试矩阵。
- Completion 扫描最终 client-visible `text`，包含一次 `echo`；还必须从 route
  实际转发的 `choice.logprobs` 提取 ordered chosen `content[].token`、
  `top_logprobs[].token` 与 `refusal`。chosen/content 的跨 event
  alias/materialized 状态和 chosen/top 的 positional 去重沿用 Chat 契约，不能因
  compatibility request 层拒绝 `logprobs` 参数而忽略 provider/custom adapter
  主动返回的 logprobs。
- Responses 从每个上游 `choice.delta` 提取 output text 与 thinking；function
  name/arguments 必须绑定现有 `tool_states` 的接受/发布分支：携带 call ID 创建
  vacant state 时累计初始 name，state.name 为空时累计一次 late name；arguments
  仅在 `tool_states.get_mut(idx)` 成功且 route 实际 `push_str` 并发布
  `ResponseFunctionCallArgumentsDelta` 时累计。call ID/state 创建前到达而被 route
  丢弃的 arguments 与重复 raw name 都不累计。所有已接受文本在派生
  `.delta` event 前只累计一次；派生的 `.done` events、output items、
  `response.completed` snapshot 只缓冲，不再扫描。

### 3. 三条 route 的状态机

每条 worker 在上游循环内重复：

`upstream_reading -> window_guardrail_check -> window_replay -> upstream_reading`

最终进入 `terminal`。

- `upstream_reading`：继续执行现有 idle timeout、usage 累积、provider error 与
  `tx.closed()` cancellation；active 时 event 进入 pending buffer。
- `window_guardrail_check`：达到字符阈值或 EOF 后执行累计检查，并在 future 等待
  期间同时监听 `tx.closed()`；取消时不回放。
- `window_replay`：只回放当前已通过窗口的 event；任一 send 失败立即进入 client-disconnect
  terminal path，不重试。
- `terminal`：最后一个窗口通过后才发送原成功 event/`[DONE]` 并调用 success callback；guardrail
  失败发送 endpoint-compatible error + 一个 `[DONE]`，不发送
  `response.completed`。

若 provider error 或 idle timeout 发生在 pending 窗口尚未审核时，先丢弃全部
pending events，再发送既有 provider error；不得把 pending 模型文本写入错误、
日志或 callback。此前已通过并释放的窗口不重发、不撤回。

为避免 `responses_stream.rs` 超过 800 行硬上限，缓冲、扫描、错误分类和测试 helper
必须放入新模块，route 文件只保留状态机接线。

### 4. SSE 与 lifecycle 语义

- violation: `type=content_policy_error`, `code=guardrail_violation`,
  message=`Response blocked by output guardrails`。
- engine/buffer error: `type=server_error`, `code=guardrail_error`，消息不得包含模型
  文本、规则命中内容或 provider secret。
- Chat/Completion 使用现有 OpenAI error envelope；Responses 使用
  `ResponseStreamEvent::ResponseFailed`，其 `ResponsesApiResponse.status` 为 `failed`
  且 `error.code` 使用上述稳定 code。两者随后发送一个 `[DONE]` 作为 transport
  sentinel。
- guardrail 在中途 block/error 时取消上游，按已观察 usage 或 reservation fallback
  执行现有 disconnect/partial settlement；callback 使用 `guardrail_output`，lease
  不记录 provider failure。上游已完成后发生的最终窗口结果按 completion settlement。
  provider/idle/disconnect 仍走既有分支。
- Responses persistence 只能发生在 pass + replay success 后。

## Planned Change Manifest

| File | Planned change |
| --- | --- |
| `src/core/guardrails/config.rs` | 定义、默认化并验证 `stream_output_check_chars`。 |
| `src/config/models/guardrails.rs` | 将新字段接入 deny-unknown gateway wire、merge/default 与配置测试。 |
| `config/gateway.yaml.example` | 记录默认 256、合法范围、首窗口延迟和已释放前缀不可撤回风险。 |
| `CHANGELOG.md` | 记录 guarded streaming 的 windowed cumulative 行为与错误终止契约。 |
| `src/server/guardrails.rs` | 增加 crate-private `check_output_text`，复用既有 `enforce`，并测试 pass/block/error。 |
| `src/server/routes/ai/mod.rs` | 声明共享 `stream_output_guardrail` 模块。 |
| `src/server/routes/ai/stream_output_guardrail.rs` | 新增 bounded pending window、cumulative surface accumulator、typed errors 与 replay API。 |
| `src/server/routes/ai/stream_output_guardrail_tests.rs` | 覆盖边界、UTF-8、上限、disabled fast path、drop/replay 顺序。 |
| `src/server/routes/ai/chat_streaming.rs` | 接入 Chat 状态机、完整检查、取消与 terminal lifecycle。 |
| `src/server/routes/ai/chat_sse.rs` | 提供 guardrail error envelope，保证错误后恰好一个 `[DONE]`。 |
| `src/server/routes/ai/completions_streaming.rs` | 接入 Completion 状态机，并把 client-visible echo 纳入扫描。 |
| `src/server/routes/ai/completions_sse.rs` | 复用稳定 guardrail error code，避免重复 terminal marker。 |
| `src/server/routes/ai/responses_stream.rs` | 缓冲 text/reasoning/function arguments，pass 后 replay/persist，失败时完成 lifecycle。 |
| `src/server/routes/ai/responses_stream_support.rs` | 构造 typed `response.failed` 与安全错误消息。 |
| `src/server/routes/ai/responses_stream_tests.rs` | 覆盖 Responses event/error/settlement/persistence 契约。 |
| `tests/integration/guardrail_route_tests.rs` | 增加 Chat SSE safe/block/cross-chunk/disabled fixtures。 |
| `tests/integration/completions_route_tests.rs` | 扩展 mock provider 的 safe、blocked、split-pattern 与 idle scenarios。 |
| `tests/integration/completions_route_tests/tests/streaming_and_budget_tests.rs` | 覆盖 Completion 和 Responses 的回放顺序、error、callback、lease、结算与断连。 |

除上述文件外不得修改源码、配置、workflow 或持久化 schema；若实现发现新增文件，
必须先更新 tech spec manifest 并重新取得 spec approval。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001, B-002 | 三条 streaming route + shared buffer | 三端点多窗口 safe/block 集成测试；断言 guardrail 在阈值/EOF 调用且输入为累计文本。 |
| B-003, B-013 | surface accumulator | split-pattern、UTF-8、触发 event 超过阈值、thinking/reasoning alias 去重、audio transcript、Chat/Completion chosen sequence 等于 content、chosen tokens 跨 token 拼成敏感词、`content/chosen=sec` 后 chosen-only `ret` 的跨 event alias 物化、不同位置同值 token、top candidate position 去重、Completion logprobs refusal、tool/function name+args、Responses initial/late/repeated name、accepted arguments 与 pre-ID dropped arguments state-acceptance，以及 done/snapshot 不重复 fixtures。 |
| B-004, B-007 | buffer + SSE helpers | 断言 blocked body 不含任一模型片段，只含 error 与一个 `[DONE]`。 |
| B-005 | config + buffer | 0/4097 配置启动失败，1/4096 成功，pending/cumulative 超限 fail-closed。 |
| B-006, B-012 | replay | safe fixture 逐 event byte/order 对比，usage/empty/done 不丢失。 |
| B-008, B-009 | engine + route error branches | fail-open/fail-closed、provider error、idle timeout 测试。 |
| B-010, B-011 | settlement/callback/lease | block/error/disconnect 每种场景断言一次 terminal callback 与一次结算。 |
| B-014 | disabled fast path | 延迟上游 fixture 证明首 chunk 在 EOF 前可见，buffer history 为 0。 |
| B-015 | timing contract | controlled clock 测试证明 active 模式在首窗口 check 前无模型 event，之后按窗口释放。 |
| B-016 | Responses persistence | block/error 不可 GET，pass 才可持久化。 |

## 数据流

provider `ChatChunk` 先完成现有 conversion 和 usage 累积。输出检查关闭时 event 直接
进入 `mpsc::Sender<Bytes>`。输出检查开启时，event bytes 进入 pending buffer，模型
文本追加到 cumulative accumulator；自上次通过检查后新增文本达到 256 默认字符
阈值或 EOF 时，把触发 event 的完整文本纳入累计载荷并交给
`GuardrailEngine::check_output`。pass 后按序 drain 当前 event-aligned 窗口并继续上游；
block/error/overflow 丢弃当前 pending、取消上游并发送安全 terminal error，同时
结算已经发生的 provider usage。Responses 只在最终窗口 pass 后持久化。无新增数据库
状态或 guardrail 外部调用类型；调用次数会随窗口数增加。

## 备选方案

- Per-chunk 检查：拒绝。provider chunk 可切开 regex、PII 或 Unicode 序列。
- Full-response buffer：能避免已释放前缀风险，但维护者在
  `user-2026-07-27-approve-all-specs` 明确选择 windowed cumulative，以保留有限
  流式延迟。
- 只检查独立 sliding window：拒绝。custom rule 与外部 moderation 没有可证明的
  有限 look-behind；实现必须每次检查累计文本。
- 检测后撤回：拒绝。SSE 已发送 bytes 不可撤回。
- 禁止所有 guarded streaming：安全但破坏面更大，且不能保留 SSE compatibility。

## 风险

- Security: buffer overflow、错误消息泄露、跨 surface 拼接、已释放前缀不可撤回和
  `fail_open` 配置可能形成风险；当前 pending 必须 bounded、稳定分隔、默认
  fail-closed 且不记录被拒正文。
- Compatibility: guarded streaming 的首内容延迟变为首窗口生成+检查延迟；错误仍
  保留 endpoint envelope 与 `[DONE]`，Responses 失败时不发送成功
  `response.completed`。
- Performance: active 请求额外持有最多 8 MiB pending SSE bytes 和最多 8 MiB
  cumulative scan text，并对随输出增长的累计文本执行多次检查；disabled fast path
  不得复制整个流。
- Maintenance: 三条 route 的 lifecycle 容易漂移；共享 buffer/error 类型和映射测试
  必须作为单一契约。

## 测试计划

- [ ] Unit tests: `stream_output_check_chars` validation、pending/cumulative accounting、所有文本 surface、reasoning/logprobs 语义去重、UTF-8、error envelope。
- [ ] Integration tests: Chat/Completion/Responses 单窗口/多窗口 safe、后续窗口 block、跨阈值完整 event、error、overflow、disabled。
- [ ] Lifecycle tests: usage、预算、provider lease、callback、pending 后 provider error/idle timeout 不泄露、三阶段断连。
- [ ] Deterministic checks: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; focused guardrail/stream tests; `cargo test`。
- [ ] Manual verification: 使用慢速 SSE fixture 比较 guardrail on/off 的首内容时间与最终 event 顺序；不得依赖真实 provider 或密钥。

## 回滚方案

回滚本 Issue 的实现 commit 即恢复原 streaming 行为；新增配置具有默认值，回滚前先从
部署配置移除 `guardrails.stream_output_check_chars`，避免旧版本 deny-unknown 启动失败。
不得通过把 `fail_open` 改为 true 作为回滚。若上线后出现内存或延迟风险，应暂停
guarded streaming 请求并回滚，而不是绕过输出检查。
