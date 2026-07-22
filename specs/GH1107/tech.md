# Tech Spec

## Linked Issue

GH-1107 / #1107

## Product Spec

见 `specs/GH1107/product.md`。

## Codebase Context

以下锚点已在 `origin/main@3921cad0f1bf8f4a20ec60d37d6e9b484d91ef97`
核验。

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Responses request DTO | `src/core/models/openai/responses_api.rs:13-73,75-127` | 请求支持普通字段；`ResponseInputItem` 只有 `Message`。 | B-002/B-006 的入口根因。 |
| Responses tools/output/events | `src/core/models/openai/responses_api.rs:131-239,283-378,420-514` | tool DTO 有 function 与部分 hosted tools；output/SSE 有 function call，无 custom call events。 | Tier 1 wire contract 与 B-009/B-010。 |
| Responses ingress | `src/server/routes/ai/responses.rs:26-111,139-275` | storage resolution 后立即构造 `ChatCompletionRequest`；structured item loop 只能解构 message；非 function tool 被拒绝。 | 需要在压平前建立 Codex turn plan。 |
| Responses non-stream output | `src/server/routes/ai/responses.rs:279-390` | Chat response 转 message/function call；无 tool-kind projector；空输出会伪造空 assistant item。 | B-004/B-010/B-015。 |
| Responses stream | `src/server/routes/ai/responses_stream.rs:44-185,256-724` | 以 `ToolCallAccum` 聚合 function calls，选择 `ChatCompletionStream`，终止时拼装 output。 | 扩展 custom calls 与 event state machine。 |
| Stream support | `src/server/routes/ai/responses_stream_support.rs:39-132` | output 排序、shell、emit 与 error classify 分散，尚无序列 validator。 | B-009/B-011/B-012。 |
| Lifecycle context | `src/server/routes/ai/responses/lifecycle.rs:19-29,181-238,380-505` | 存原始 input；previous response 合并时只把 message output 恢复为 assistant text，function calls 被过滤。 | B-013/B-014。 |
| Canonical chat | `src/core/types/chat.rs:18-179` | 已能表达 assistant tool calls 与 tool-role output，provider 都消费该类型。 | Tier 1 function loop 可复用，不建立第二执行模型。 |
| Canonical tools | `src/core/types/tools.rs:5-74` | 只表达 function tool/call。 | custom/freeform 需在 Codex adapter 中做可逆 projection，不能污染通用 provider wire。 |
| Provider capability | `src/core/types/model.rs:7-49`、`src/core/providers/capability_dispatch.rs:5-35` | provider/model 已有 Chat/Stream/ToolCalling capability 查询。 | 复用能力事实做 preflight，不新增猜测式 provider allowlist。 |
| Provider surface matrix | `src/core/providers/registry/support_matrix.rs:1-67,100-266` | 公共 surface matrix 未列 Responses/Codex。 | 文档与 conformance 防止支持声明漂移。 |
| Error envelope | `src/server/routes/ai/openai_errors.rs:12-58,78-166` | OpenAI-compatible 4xx/5xx envelope 和 redaction 已集中。 | 新 error code 必须复用单一出口。 |
| Codex upstream contract | `openai/codex@6e5a2d6b8d148a5554fdceb6f399ca45bd1c78d9` 的 `codex-rs/protocol/src/models.rs:797-1024` 与 `codex-rs/tools/src/tool_spec.rs:1-70` | 当前协议含 function/custom/tool-search/compaction items 与 function/namespace/tool-search/custom tools。 | fixture 与版本漂移基线。 |

## Planned Changes

```specrail-planned-changes
{
  "issue": 1107,
  "complete": true,
  "paths": [
    "README.md",
    "docs/codex-compatibility.md",
    "src/core/models/openai/responses_api.rs",
    "src/core/providers/registry/support_matrix.rs",
    "src/core/providers/registry/support_matrix_tests.rs",
    "src/core/types/codex.rs",
    "src/core/types/mod.rs",
    "src/server/routes/ai/chat.rs",
    "src/server/routes/ai/openai_errors.rs",
    "src/server/routes/ai/responses.rs",
    "src/server/routes/ai/responses/codex_compat.rs",
    "src/server/routes/ai/responses/codex_compat_tests.rs",
    "src/server/routes/ai/responses/lifecycle.rs",
    "src/server/routes/ai/responses/lifecycle_tests.rs",
    "src/server/routes/ai/responses_stream.rs",
    "src/server/routes/ai/responses_stream_support.rs",
    "src/server/routes/ai/responses_stream_tests.rs",
    "tests/integration/codex_responses_compat_tests.rs"
  ],
  "spec_refs": [
    "B-001",
    "B-002",
    "B-003",
    "B-004",
    "B-005",
    "B-006",
    "B-007",
    "B-008",
    "B-009",
    "B-010",
    "B-011",
    "B-012",
    "B-013",
    "B-014",
    "B-015",
    "B-016",
    "B-017",
    "B-018",
    "B-019",
    "B-020"
  ]
}
```

清单是 GH-1107 当前设计的完整候选路径。任一 implementation tranche 发现必须修改
清单外文件，或任一 tranche 超过 10 个非文档文件 / 500 changed lines，先提交 spec
amendment；不得边实现边扩大 allowlist。

## 设计方案

### 1. Wire DTO：闭集类型 + 可诊断 unknown

扩展 `ResponseInputItem`、`ResponseTool`、`ResponseOutputItem` 和
`ResponseStreamEvent`：

- Tier 1 使用强类型 variant，所有 machine-facing 字段保持 snake_case。
- Tier 2 使用强类型“recognized but unsupported” variant，不能直接丢弃。
- 真正未知类型由自定义反序列化保留 `type` 和受限元数据；不保留/记录完整敏感
  payload。route 返回 `unsupported_codex_feature`。
- `function_call_output.output` 与 `custom_tool_call_output.output` 支持 Codex
  当前 wire 的 string 或 structured content items，规范化时保留原始 form。

这里不使用 `serde(other)`，因为它无法携带原始 type，也会把 schema 合法的未来
类型伪装成空 variant。

### 2. Canonical Codex turn

新增 crate-private `src/core/types/codex.rs`，包含：

- `CodexTurn`：有序 input items、tools、request flags 与来源 contract version。
- `CodexCallKind`：`Function` / `Custom` 闭集。
- `CodexCallLedger`：记录 call_id、kind、name、namespace、状态
  `declared -> output_received`。
- `CodexExecutionRequirements`：是否需要 streaming/tool/custom projection。
- `CodexExecutionPlan`：现有 `ChatCompletionRequest` 加 tool projection map。

`TryFrom<&ResponsesApiRequest>` 完成协议级验证；`build_chat_request` 改为消费
plan，不再直接 pattern-match 单一 message variant。Call ledger 在 provider 调用前
检查重复/未知/type mismatch，并且只在同一已授权 response context 中解析
`previous_response_id`。

### 3. Reversible projection

Tier 1 function items直接映射到现有 assistant `tool_calls` 与 tool-role message。
custom/freeform tool 使用显式 projection envelope：

- provider-facing tool 名保持可追踪但不泄露 namespace；
- JSON schema 只含一个必需的 raw-input 字段；
- plan 中保存 tool name/namespace/kind 映射；
- provider tool call 返回后，projector 验证 name 与 payload，恢复
  `custom_tool_call` 的原始 input string；
- 无法可逆恢复时返回 error，不降级成 function 或普通文本。

Projection envelope 是 adapter 内部格式，不进入 public provider config、cache key
之外的日志或持久化 API。Cache canonicalization 必须包含原始 Codex item/tool kind；
若现有 Responses 已绕过 Chat cache，则保持绕过。

### 4. Provider/model capability preflight

不增加第二套 provider allowlist。执行 plan 时：

1. 仍由 canonical router 按 requested model 选择 deployment。
2. 在 selected provider 的上游调用闭包内，使用既有
   `supports_capability_for_model` 验证 ChatCompletion/Stream 与 ToolCalling。
3. custom projection 只有在 function tool round-trip conformance 已通过时可用。
4. preflight 失败返回 non-retryable typed compatibility error，发生在 HTTP send、
   budget reservation 和 success callback 之前。
5. support matrix 增加 Responses/Codex sync/stream 列，只声明有 executable
   conformance 的 surface。

`chat.rs` 提取一个 crate-private capability-aware执行入口；现有 Chat Completions
入口继续传原有 capability，保持 B-001。Responses stream 的现有直接
`run_stream` 路径在每个 selected attempt 上执行同一 preflight。

### 5. Output projector 与 SSE state machine

非流式和流式共用 `CodexOutputProjector`：

- 文本/reasoning 沿用当前 item；
- function/custom calls 根据 projection map 生成正确 output variant；
- `call_id` 采用 provider 返回值；缺失时不得随机生成一个无法关联的 call id；
- 零 output 是 error/incomplete，不再补空 assistant message。

在 `responses_stream_support.rs` 增加闭集状态机：
`created -> item_added -> deltas -> item_done -> terminal`。每次 emit 前验证 index、
item state 与 terminal uniqueness；serialization/client disconnect 返回 typed
中断。失败路径发一个 `response.failed`（若连接仍可写）后终止，不能再发
completed。现有 `[DONE]` 行为由 fixture 锁定。

### 6. Lifecycle context

`output_items_as_input_context` 扩展为：

- message -> assistant message；
- function/custom call -> 同类型 input item；
- reasoning 只有 provider contract 允许继续传递时保留，否则按已定义策略忽略，
  不影响 call ledger；
- Tier 1 output 绝不丢弃。

存储仍使用当前 owner、TTL、limit 和 cancel machinery。`store=false` 完全绕过；
跨 owner 的 previous response 仍表现为 not found。Background + stream 继续拒绝。

### 7. Error、审计与隐私

新增 `codex_compatibility_error` 构造器，通过现有 OpenAI envelope 返回：

- status: 400 或 422（由现有仓库错误政策确定一种并 fixture 固定）；
- type: `invalid_request_error`；
- code: `unsupported_codex_feature`、`invalid_codex_call_graph` 或
  `codex_stream_state_error`；
- message: 仅 feature/provider/model/call-id digest，不含 credential、tool output
  全文或 upstream body。

日志只记录 request id、feature、provider/model 和 call 数量。SEC-11 要求
`openai_errors.rs`、payload redaction 与任何 header/URL 修改接受人工安全审查。

### 8. Conformance fixtures

`tests/integration/codex_responses_compat_tests.rs` 使用 loopback mock upstream：

- 同一输入分别驱动 Anthropic、Gemini、OpenAI-compatible provider config；
- 记录上游 request count 与转换后的 tool schema/messages；
- 返回 function/custom calls、文本、reasoning、并行 calls 与错误；
- 同步与 streaming 聚合结果比较；
- 所有 fixture 标注 Codex commit
  `6e5a2d6b8d148a5554fdceb6f399ca45bd1c78d9`；
- 无真实 API key、OAuth token 或互联网依赖。

单元测试覆盖 DTO、call ledger、projection、event state machine、lifecycle 与 error
redaction。负例必须 schema 合法且到达业务 gate，不能只测试 serde reject。

### 9. 文档与配置边界

`docs/codex-compatibility.md` 提供：

- litellm-rs provider/model 配置前置条件；
- Codex `model_providers.<id>`、`base_url`、`wire_api="responses"`、
  `requires_openai_auth=false` 与环境变量示例；
- 启动、健康检查、文本 smoke、tool-loop smoke；
- Tier 1/Tier 2 matrix；
- 删除自定义 profile 或切回原 provider 的恢复步骤。

文档不提供自动写文件命令；若未来要增加 `--print` 或 installer，另开 issue。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | existing Chat/Responses delegate paths | `cargo test --locked responses_api chat_completion` 加 snapshot regression；现有 request fixture 输出不变。 |
| B-002 | Responses DTO + `CodexTurn` | unit fixtures round-trip 每个 Tier 1 字段，并分别覆盖 missing/null/empty。 |
| B-003 | `CodexCallLedger` | negative fixtures: unknown、duplicate、missing、kind mismatch call_id 均在 upstream request count=0 时 4xx。 |
| B-004 | custom projection/projector | round-trip property test：任意合法 name/namespace/input 经 provider envelope 后恢复相同 custom item。 |
| B-005 | ledger + output ordering | parallel two-call fixture 逆序 provider chunks 后仍按稳定 output_index 且 call_id 不串线。 |
| B-006 | custom DTO deserialize + compatibility gate | 每个 Tier 2/unknown schema-valid fixture 返回 `unsupported_codex_feature`，mock upstream requests=0。 |
| B-007 | capability preflight + support matrix | provider/model capability table test；缺 ToolCalling 或 stream capability 的 deployment 被拒绝。 |
| B-008 | selected-attempt preflight | mock budget/callback/upstream counters 全为零的 unsupported fixture。 |
| B-009 | event state machine | table-driven allowed/illegal transitions；terminal count=1；index-before-added 失败。 |
| B-010 | shared projector | 同一 mock provider result 的 sync JSON 与 SSE aggregate deep equality（忽略 id/time）。 |
| B-011 | stream/lifecycle error paths | timeout、disconnect、cancel fixtures 断言无 completed，保留 failed/interrupted evidence。 |
| B-012 | run_stream + settlement | partial-delta failure fixture 断言无 retry/duplicate call/duplicate settlement/terminal。 |
| B-013 | lifecycle context merge | previous response 的 function/custom call+output 保留；cross-owner 返回 not found。 |
| B-014 | lifecycle store | store=false 不可 retrieve；default/true 可继续 tool output；TTL/limit regression 保持。 |
| B-015 | ingress/projector validation | empty/invalid fixtures 4xx；zero provider output 不生成空 assistant success。 |
| B-016 | existing middleware/execution path | auth/model/token/guardrail/rate/budget/cache/callback focused integration suites。 |
| B-017 | OpenAI error helper + logs | adversarial secret/tool-output fixture 断言 response/debug/display/log capture 均无原值。 |
| B-018 | fixture metadata + negative suite | `rg -n "6e5a2d6b8d148a5554fdceb6f399ca45bd1c78d9" tests/integration/codex_responses_compat_tests.rs`；positive/negative case count gate。 |
| B-019 | documentation | docs source guard：含 `wire_api = "responses"`、env placeholder、restore；不含 token/key literal 与写 `~/.codex` 命令。 |
| B-020 | diff/source boundary | `rg -n "UnifiedRouter|ProviderRegistry|run_(unary|stream)"` 人工核对仅复用现有 runtime；无新 binary/service/config writer。 |

## 数据流

```text
Codex /v1/responses JSON
  -> ResponsesApiRequest wire DTO
  -> resolve_previous_response_context(owner checked)
  -> CodexTurn validation + call ledger
  -> CodexExecutionPlan
       - ChatCompletionRequest
       - reversible tool projection map
       - required capabilities
  -> canonical router selects provider/model
  -> capability preflight (before upstream/budget side effects)
  -> existing provider chat/tool adapter
  -> shared CodexOutputProjector
       -> sync ResponsesApiResponse
       -> or validated Responses SSE state machine
  -> existing storage, budget settlement, callbacks and redacted errors
```

没有新的数据库或后台服务。持久化仍是当前 response store；新存储内容只扩展已有
`ResponseInputItem` / `ResponseOutputItem` variant。

## 备选方案

1. **复制 OpenCodex，做独立 Node/Bun daemon**：拒绝。litellm-rs 已拥有 routing、
   provider、预算和运维面，会形成重复控制面。
2. **把所有未知 item 转成文本**：拒绝。call correlation、custom input 和 encrypted
   compaction 会丢语义，违反 B-006。
3. **直接把所有 provider 标成 Codex-compatible**：拒绝。provider tool/streaming
   能力不对称，必须由 executable conformance 声明。
4. **只支持 OpenAI-compatible upstream**：拒绝。无法发挥 litellm-rs 的 native
   Anthropic/Gemini adapter 价值，也不能证明模型无关。
5. **首版自动修改 `~/.codex/config.toml`**：拒绝。高上下文配置写入需要独立设计、
   backup/restore 与跨平台验证；B-019 保持手动。
6. **新增 provider-native Responses trait**：Tier 1 暂不采用。先用可逆 tool
   projection 复用现有 ChatRequest；将来原生 passthrough 需另开 spec，不能和当前
   chat path 静默混用。

## 风险

- **Security**：tool output、headers 与 provider body 可能含凭证；只记录受限元数据，
  error path 强制 redaction，安全相关 diff 必须人工 review。
- **Compatibility**：Codex wire 会变；fixture 固定 commit，未知 type fail closed。
- **Performance**：ledger/projection 是 O(items + tools)，不得 clone 完整 payload
  多次；大 tool output 沿用当前 request body 限制。
- **Streaming**：状态机错误会让 Codex hang；terminal uniqueness 与 disconnect
  fixtures 是 merge gate。
- **Provider drift**：仅 capability 声明不足；support matrix 必须由 conformance
  fixture 支撑。
- **Maintenance**：route 可能继续膨胀；新兼容逻辑放独立文件，所有文件遵守 800 行
  hard ceiling。

## 实施分段

- **D1 Wire + ledger**：DTO、CodexTurn、验证、错误；不接 provider。
- **D2 Sync projection**：function/custom sync loop、capability preflight、lifecycle。
- **D3 Streaming**：shared projector、SSE state machine、disconnect/settlement。
- **D4 Provider conformance**：三类 adapter loopback matrix 与 support surface。
- **D5 Docs + closure**：文档、全量 regression、人工安全 review。

D1→D2→D3 严格串行；D4 只能在 D3 语义稳定后写 conformance；D5 最后。每个 tranche
使用独立 implementation PR、`Refs #1107`，最终 closure PR 才使用
`Fixes #1107`。实现前必须由维护者把 issue 转为 `ready_to_implement`。

## 测试计划

- [ ] DTO/ledger/projector/event-state/lifecycle/error 单元测试。
- [ ] `cargo test --locked codex_compat`
- [ ] `cargo test --locked responses_api`
- [ ] `cargo test --locked --test integration codex_responses_compat`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --all-targets --all-features --locked`
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`
- [ ] `cargo test --all-features --locked -- --test-threads=1`
- [ ] 手动：临时 Codex profile 完成文本 + function/custom tool loop，再恢复原 profile。
- [ ] 安全：人工复核 error/log/header/tool-output redaction。

## 回滚方案

按 tranche 逆序 revert。D4/D5 可单独回滚测试/文档；D3 回滚后禁止在 support matrix
声明 streaming Codex；D2 回滚后恢复 Tier 1 明确 4xx，不得恢复 silent text
fallback；D1 回滚后当前通用 Responses 行为保持。回滚不修改用户
`~/.codex/config.toml`，因为本功能从不自动写入该文件。
