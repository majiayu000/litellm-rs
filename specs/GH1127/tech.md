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
| Guardrail config | `src/core/guardrails/config.rs`, `src/config/models/guardrails.rs` | `check_output` and `fail_open` exist, but there is no bounded streaming-output buffer policy. Gateway deserialization rejects unknown fields. | `full_response` buffering requires an explicit, validated byte ceiling and secure default. |
| Chat SSE | `src/server/routes/ai/chat_streaming.rs`, `src/server/routes/ai/chat_sse.rs` | Each converted `ChatCompletionChunk` is serialized and sent immediately; errors currently include `[DONE]`. | Must hold model events until the full output passes, while retaining OpenAI error shape. |
| Completion SSE | `src/server/routes/ai/completions_streaming.rs`, `src/server/routes/ai/completions_sse.rs` | Each text completion event, including `echo`, is sent immediately. | Must scan exactly the client-visible completion text and preserve echo/event order. |
| Responses SSE | `src/server/routes/ai/responses_stream.rs`, `src/server/routes/ai/responses_stream_support.rs` | Text, reasoning and function arguments are emitted while `full_text`/tool state is still accumulating; success is persisted after `response.completed`. | All model-bearing events must be held, checked together, and only then emitted/persisted. |
| Stream lifecycle | `src/server/routes/ai/budgeted.rs`, `src/server/routes/ai/callbacks.rs` | Streaming workers own reservation settlement, callback terminal state and provider lease. | Guardrail rejection is a gateway-policy failure after chargeable provider success, not a free or retryable provider failure. |
| Existing coverage | `tests/integration/guardrail_route_tests.rs`, `tests/integration/completions_route_tests.rs`, `tests/integration/completions_route_tests/tests/streaming_and_budget_tests.rs`, `src/server/routes/ai/responses_stream_tests.rs` | Unary guardrails and normal streaming/error/settlement are covered separately; no streaming-output guardrail fixture exists. | Cross-route behavior and lifecycle assertions require deterministic SSE integration tests. |

## 设计方案

### 1. 配置契约

在 `GuardrailConfig` 增加 `streaming_output.max_buffer_bytes`，gateway 默认值为
`8_388_608` bytes。配置必须使用 snake_case，值必须大于 0；未知字段、0 或无法
表示为 `usize` 的值在启动验证阶段失败。该配置只在
`enabled && check_output && guardrail_count > 0` 时生效。

审核粒度不开放不安全别名或 windowed 模式；稳定 ID 固定为 `full_response`。
`fail_open` 继续由 `GuardrailEngine::check_output` 决定，streaming 层不得增加
第二套 fallback。

### 2. 共享缓冲器

新增 route-private `StreamOutputGuardrail`：

- 创建时记录是否需要输出检查和 `max_buffer_bytes`；
- 按到达顺序保存待回放的完整 SSE `Bytes`，并单独累积带字段边界的扫描文本；
- 对 buffered SSE bytes 与 scan text 使用 checked/saturating accounting，在追加前
  检查上限，超限返回 typed `BufferLimitExceeded`；
- active=false 时不保存历史 event，调用方继续原有 immediate-send 分支；
- active=true 时 upstream EOF 后只调用一次 `check_output_text`；
- 检查通过后按原顺序 drain events；拒绝或错误时丢弃全部 buffered events。

扫描文本采用稳定的 choice/surface 边界，不能依赖 provider chunk。Chat 扫描
`delta.content`；Completion 扫描最终 client-visible `text`（包含一次 `echo`）；
Responses 扫描 output text、reasoning summary 和 function-call arguments。role、
usage、finish reason、IDs 与 transport markers 只缓冲、不进入扫描文本。

### 3. 三条 route 的状态机

每条 worker 明确经过：

`upstream_reading -> guardrail_check -> replaying -> terminal`

- `upstream_reading`：继续执行现有 idle timeout、usage 累积、provider error 与
  `tx.closed()` cancellation；active 时 event 进入缓冲器。
- `guardrail_check`：upstream EOF 后执行一次完整检查，并在 future 等待期间同时
  监听 `tx.closed()`；取消时不回放。
- `replaying`：只回放已通过的 event；任一 send 失败立即进入 client-disconnect
  terminal path，不重试。
- `terminal`：成功才发送原成功 event/`[DONE]` 并调用 success callback；guardrail
  失败发送 endpoint-compatible error + 一个 `[DONE]`，不发送
  `response.completed`。

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
- provider 已成功完成时，无论 guardrail pass/block/error，均按
  `record_completion(final_usage, saw_upstream_output)` 结算；callback 在 block/error
  使用 `guardrail_output`，lease 记录 provider success。provider/idle/disconnect
  仍走既有分支。
- Responses persistence 只能发生在 pass + replay success 后。

## Planned Change Manifest

| File | Planned change |
| --- | --- |
| `src/core/guardrails/config.rs` | 定义、默认化并验证 `streaming_output.max_buffer_bytes`。 |
| `src/config/models/guardrails.rs` | 将新字段接入 deny-unknown gateway wire、merge/default 与配置测试。 |
| `src/server/guardrails.rs` | 增加 crate-private `check_output_text`，复用既有 `enforce`，并测试 pass/block/error。 |
| `src/server/routes/ai/mod.rs` | 声明共享 `stream_output_guardrail` 模块。 |
| `src/server/routes/ai/stream_output_guardrail.rs` | 新增 bounded full-response buffer、surface accumulator、typed errors 与 replay API。 |
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
| B-001, B-002 | 三条 streaming route + shared buffer | 三端点 safe/block 集成测试；断言 guardrail 只调用一次且发生在 EOF 后。 |
| B-003, B-013 | surface accumulator | split-pattern、UTF-8、reasoning、function args fixtures。 |
| B-004, B-007 | buffer + SSE helpers | 断言 blocked body 不含任一模型片段，只含 error 与一个 `[DONE]`。 |
| B-005 | config + buffer | 0 配置启动失败、边界成功、超一 byte fail-closed。 |
| B-006, B-012 | replay | safe fixture 逐 event byte/order 对比，usage/empty/done 不丢失。 |
| B-008, B-009 | engine + route error branches | fail-open/fail-closed、provider error、idle timeout 测试。 |
| B-010, B-011 | settlement/callback/lease | block/error/disconnect 每种场景断言一次 terminal callback 与一次结算。 |
| B-014 | disabled fast path | 延迟上游 fixture 证明首 chunk 在 EOF 前可见，buffer history 为 0。 |
| B-015 | timing contract | controlled clock 测试证明 active 模式在 EOF/check 前无模型 event。 |
| B-016 | Responses persistence | block/error 不可 GET，pass 才可持久化。 |

## 数据流

provider `ChatChunk` 先完成现有 conversion 和 usage 累积。输出检查关闭时 event 直接
进入 `mpsc::Sender<Bytes>`。输出检查开启时，event bytes 与对应模型文本进入 bounded
buffer；upstream EOF 后完整文本交给 `GuardrailEngine::check_output`。pass 后按序
drain 至客户端并完成结算/callback/persistence；block/error/overflow 丢弃 buffer，
发送安全 terminal error，但仍结算已经发生的 provider usage。无新增数据库状态或
外部调用；外部 moderation 调用仍由现有 guardrail engine 管理。

## 备选方案

- Per-chunk 检查：拒绝。provider chunk 可切开 regex、PII 或 Unicode 序列。
- Sliding window：拒绝作为本 Issue 默认。custom rule 与外部 moderation 没有可证明
  的有限 look-behind，窗口会保留绕过面。
- 检测后撤回：拒绝。SSE 已发送 bytes 不可撤回。
- 禁止所有 guarded streaming：安全但破坏面更大，且不能保留 SSE compatibility。

## 风险

- Security: buffer overflow、错误消息泄露、跨 surface 拼接和 `fail_open` 配置可能
  重新形成绕过；必须 bounded、稳定分隔、默认 fail-closed 且不记录被拒正文。
- Compatibility: guarded streaming 的首内容延迟变为完整响应延迟；错误仍保留
  endpoint envelope 与 `[DONE]`，但 Responses 不再发送成功 `response.completed`。
- Performance: active 请求额外持有最多 `max_buffer_bytes` SSE bytes 和扫描文本，并
  执行一次完整检查；disabled fast path 不得复制整个流。
- Maintenance: 三条 route 的 lifecycle 容易漂移；共享 buffer/error 类型和映射测试
  必须作为单一契约。

## 测试计划

- [ ] Unit tests: config validation、buffer accounting、surface ordering、UTF-8、error envelope。
- [ ] Integration tests: Chat/Completion/Responses safe、block、error、overflow、disabled。
- [ ] Lifecycle tests: usage、预算、provider lease、callback、idle timeout、三阶段断连。
- [ ] Deterministic checks: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; focused guardrail/stream tests; `cargo test`。
- [ ] Manual verification: 使用慢速 SSE fixture 比较 guardrail on/off 的首内容时间与最终 event 顺序；不得依赖真实 provider 或密钥。

## 回滚方案

回滚本 Issue 的实现 commit 即恢复原 streaming 行为；新增配置具有默认值，回滚前先从
部署配置移除 `guardrails.streaming_output`，避免旧版本 deny-unknown 启动失败。
不得通过把 `fail_open` 改为 true 作为回滚。若上线后出现内存或延迟风险，应暂停
guarded streaming 请求并回滚，而不是绕过输出检查。
