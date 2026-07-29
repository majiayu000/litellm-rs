# Task Plan

## Linked Issue

GH-1128 / #1128

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1128-T0` Covers: B-001 ～ B-009. Owner: maintainer/spec owner.
  Dependencies: none. Done when: 原规格已在 `user-2026-07-27-approve-all-specs`
  获批；本 amendment 的 JSON/document semantic view、request-level function call、
  record-aware batch、不可吞掉的 response-integrity error、256/2 MiB 上限、稳定
  400、provider system-container projections、immutable audited routing
  generation、background/cache/retry handoff 与 zero-text-leaf suppression 契约获得
  exact-head 内容绑定批准并合并。Verify: GitHub exact merged SHA、native CI 与安全
  人工 review。

## 实现任务

- [ ] `SP1128-T1` Covers: B-001, B-002, B-003, B-006, B-009. Owner:
  guardrail/routing implementation owner. Dependencies: SP1128-T0. Done when:
  `content_text` 被 bounded fallible owned record builder 替代，覆盖
  request/message legacy function、modern function、message name、普通 content、
  tool result/use；active input check 先捕获 immutable `RuntimeHandle`/generation，
  并通过 router snapshot 的 stable-candidate API 按 model group/capability 枚举全部
  lifecycle-eligible deployments，禁止用 health/cooldown/concurrency/rate/budget
  selector 缩窄 profiles；每条 message 仅在普通 text leaves 非空时生成
  direct/space/newline projections，全请求另有 legacy newline view；provider profiles
  精确生成：

  - Anthropic System+Developer exact-newline；
  - `GeminiClient`（Google AI/自身 Vertex endpoint）System-only
    `systemInstruction.parts` direct/space/newline，排除 Developer 与独立
    `Provider::VertexAI`；
  - Bedrock 按 selected model 的实际 `Converse`/`ConverseStream` transform 生成
    System-only outgoing `system[]` direct/space/newline，排除 Developer、Invoke 系列
    与 prompt-management ARN，禁止只按 provider enum 分类。

  builder 还镜像 Bedrock ToolResult outgoing-block boundaries；其他字段保持 typed
  isolation；JSON 做 raw+semantic 扫描并拒绝 duplicate key/BOM/structured parse
  failure；全部 records 纳入 256/2 MiB accounting。Verify: focused stable-candidate/
  transient-state/profile-classification/System-role-boundary/non-target-zero-record/
  empty-leaf/256-image-only/ToolResult/record-isolation/JSON/limit tests。
- [ ] `SP1128-T2` Covers: B-003, B-006, B-007. Owner: same implementation owner. Dependencies: SP1128-T1. Done when: `Cargo.toml` 直接声明 `mime = "0.3"`；在 parser 前用 quote-aware 状态机拒绝空参数段、连续/尾随分号（精确覆盖 `text/plain;` 与 `text/plain; charset=utf-8;`），再完整解析 media_type；essence allowlist 后只接受无参数或唯一、大小写不敏感的 `charset=utf-8`，重复/非 UTF-8 charset、任意 non-charset 参数与其他 malformed MIME 稳定 400；通过时原 DTO 不改写；plain/csv document base64 解码为 UTF-8 正文；JSON/`+json` document 生成 raw+semantic records 且 syntax/duplicate-key/BOM/depth/resource failure 稳定 400；Markdown（含 numeric/named entity）、HTML/XML/`+xml`/其他 `text/*`、bad base64/UTF-8 与其他 MIME 在 active-input predicate 为 true 时 fail-closed；无转码、网络、entity 或二进制解析. Verify: document MIME raw-segment/syntax/charset/base64/JSON/duplicate-key/BOM/depth/entity table tests.
- [ ] `SP1128-T3` Covers: B-004, B-005, B-006, B-008, B-009. Owner:
  engine/execution/test owner. Dependencies: SP1128-T2. Done when: engine 保留现有
  `Vec<BoxedGuardrail>` 与公开 API；新增 active-input predicate、source-compatible
  record batch contract、不可被 `fail_open` 吞掉的 batch fatal errors、32,768-byte
  moderation 上限和 action-only Mask fail-closed。`check_chat_input` 返回显式
  `Disabled`/`Audited(InputGuardrailAudit)` outcome：Disabled 不创建 handle 并保留
  动态 routing；Audited outcome 必须完整传给 chat cache pricing、
  cache-miss unary、chat/Responses stream、所有 retry 与 background task。
  `budgeted.rs`/`execution.rs` 在 Audited 路径的每次 selection 都使用同一
  `RuntimeHandle` snapshot；cache pricing 不读取 `AppState.unified_router`；
  background 在 queue/store/spawn/200 前审核并把未修改 request 与 audit 一起传给
  sole-caller post-input entrypoint。Verify: active/disabled predicates、legacy custom
  API、batch/fail-open、route-swap barriers（cache hit/miss、unary、stream、retry、
  background）、same-generation transient-state、selected-generation equality、
  exactly-once、source-boundary 与 external-call-count tests。
- [ ] `SP1128-T4` Covers: B-001 ～ B-009. Owner: verification owner. Dependencies:
  SP1128-T3. Done when: 实现 diff 已按最终 amendment 审计/修正，native Rust CI 与安全
  人工 review 通过；feature-gated Gemini 与独立 VertexAI fixtures 必须实际执行。
  Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo test`; `cargo test --all-features`; `git diff --check`.

## 并行拆分

不并行。实现由同一 owner 串行覆盖：

- `Cargo.toml`、`Cargo.lock`；
- `src/server/guardrails.rs`；
- `src/server/routes/ai/{chat.rs,chat_streaming.rs,budgeted.rs,execution.rs,response_cache.rs}`;
- `src/server/routes/ai/{responses.rs,responses_stream.rs,responses/lifecycle.rs,responses/lifecycle_tests.rs}`;
- `src/core/router/{mod.rs,selection.rs}` 的 stable-candidate 与 pinned selection surface；
- `src/core/providers/bedrock/chat/mod.rs` 的 selected-model transform classifier；
- core guardrail 的 `traits.rs`、`types.rs`、`mod.rs`、`engine.rs`,
  `openai_moderation.rs`、`pii.rs`、`prompt_injection.rs` 及对应 tests。

Anthropic/Gemini/VertexAI/Bedrock transform 文件是 provider-boundary fixtures 的
read-only oracle；如实现必须改动其 runtime transform，则先修订本 task ownership，
禁止在未声明路径静默扩张。任何既有 Claude worktree 仅作为待审 diff，不视为已验证
实现；验证只在本 session 最终实现 worktree 串行执行。

## 验证

- Product invariant 与 tasks `Covers:` 的并集必须都是 B-001 ～ B-009。
- 对每个结构化载体至少有 allow/block 两条 fixture；独立 records 还必须证明正则
  不能跨字段边界匹配，typed provenance 不进入扫描文本。
- provider-boundary fixtures 必须证明 Bedrock space、Ollama newline、Gemini/Vertex
  empty join 都被对应 projection 覆盖；image-separated 与 split-message
  `ignore` + `all previous instructions` 在 provider 转换/调用前命中，alternative
  views 不互相跨 record 匹配；system-container fixtures 必须分别证明：

  - Anthropic System+Developer 精确映射为 newline view；
  - `GeminiClient` 的 Google AI/自身 Vertex endpoint 都把 System-only outgoing
    parts 映射为 direct/space/newline，Developer 与独立 `Provider::VertexAI`
    zero-record；
  - Bedrock 由 selected model 的 `Converse`/`ConverseStream` transform 决定
    System-only outgoing sequence，Invoke 系列与 prompt-management ARN
    zero-record；同一 provider enum 下的正反例都要覆盖。

  Bedrock ToolResult array、tool/function-role 的 Text+ToolResult 与多个 ToolResult
  内相邻 text blocks 的 split-pattern 被同一 outgoing-block views 命中，而普通
  user/assistant sibling blocks 与不同 message 不跨边界。
- 恰好 256 条 baseline 可接受的 image-only message 不得产生 per-message empty
  projection、不得触发 record-limit 400，且 validator/provider 行为保持现有兼容。
- background Responses 必须在 queued persist/task spawn/200 前返回 normalization/
  guardrail 错误；失败 store/task/provider zero-call，通过时未修改 request 与完整
  audited outcome 一起入 task、input check exactly-once、selected generation 等于
  audited generation，post-input entrypoint 的 source-boundary 只允许 lifecycle 调用。
- route-swap barrier fixtures 必须覆盖 chat cache-hit pricing、cache-miss unary、
  chat/Responses streaming、retry 与 background：audit 后发布的新 deployment
  zero-call，所有最终 selection generation 等于 audited generation；inactive 对照
  继续使用 current dynamic generation。same-generation fixture 必须证明 audit 时
  unhealthy/in-cooldown 的 stable candidate 仍被 profile 覆盖，恢复后可由 retry/final
  selector 选中而无需二次 input check。
- moderation mock 必须证明 N 条 trim 后非空 records 只产生 1 次远程请求；
  mixed whitespace 只提交 eligible values，全 whitespace zero-call；records/派生
  字节超限时 engine 与 provider 调用计数均为 0；N-1/N+1 结果在 `fail_open`
  true/false 下都不可放行，`Log` action 仍允许 downstream model provider 调用，
  action-only `Mask` 则固定阻止该调用；32,768 eligible UTF-8 bytes 可提交，
  32,769 bytes 在 `fail_open` true/false 下 moderation/model provider 均 zero-call。
- custom guardrail fixture 只实现既有必需方法 `check_input(&str)`，证明无需新增 trait
  实现即可经默认 batch method 按顺序收到所有 records，且 block/Log/error 语义不变；
  另有下游式 compile fixture 对现有五个 `GuardrailError` variants 穷举匹配，并验证
  旧 custom implementation 无需实现新方法；runtime fixture 证明同一 custom 的
  input-allow/output-block、disabled、非默认 priority 与 name 行为保持。
- `OpenAIModerationGuardrail` 分别通过 config 与公开 `add_guardrail` 注册；两种路径都
  必须对多 records 只调用一次 moderation，并执行 32,768-byte 总上限与 response-count
  完整性检查。
- 测试必须扫描解码后的 document 文本，不能仅断言 base64/JSON 字符串被拼接。
- MIME table 必须覆盖无参数/quoted/case-varied UTF-8 charset、`text/plain;`、
  `text/plain; charset=utf-8;`、连续/空参数段、重复 charset、UTF-16LE/其他 charset、
  其他 malformed syntax 与 non-charset 参数 fail-closed；通过时原 DTO/wire 值不改写。
- 仅注册 disabled custom guardrail 时必须在 builder 前返回，合法 base64 携带
  unsupported MIME、非 UTF-8 charset 或 invalid JSON 不得触发 guardrail-specific
  400；独立 request validator 的既有错误仍保留。
- PR 在最终 slice 使用 `Fixes #1128`，需要 security 人工 review。

## Handoff Notes

- Claude 原 diff 未处理 document base64 的语义性解码，不能直接提交。
- 不支持的二进制 document 在启用 input guardrail 时按规格 fail-closed。
- 保留现有 mask/modified 非可变边界的显式错误。
- 不自动合并、不 force-push。
