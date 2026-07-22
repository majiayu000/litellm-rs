# Task Plan

## Linked Issue

GH-1107 / #1107

## Spec Packet

- Product: `specs/GH1107/product.md`
- Tech: `specs/GH1107/tech.md`

## 状态

本文件是 Spec PR 内的实施拆分，不代表实施获批。Issue 仍需维护者完成 spec review 并授予 `ready_to_implement`；在此之前所有任务保持未勾选，禁止修改生产代码。

## 实现任务

- [ ] `SP1107-T1` Covers: B-002, B-006, B-015, B-018. Owner: responses wire owner. Dependencies: spec approved + `ready_to_implement`. Files: `src/core/models/openai/responses_api.rs`, `src/core/types/codex.rs`, `src/core/types/mod.rs`, `src/server/routes/ai/openai_errors.rs`, `src/server/routes/ai/responses.rs`, `src/server/routes/ai/responses/codex_compat_tests.rs`, `src/server/routes/ai/responses/lifecycle.rs`, `src/server/routes/ai/responses/lifecycle_tests.rs`. Done when: Tier 1/Tier 2/unknown wire types 按 tech contract 可解析；`core/types/codex.rs` 在本 tranche 只包含 wire DTO/serde/error helper，不含 `CodexTurn`、ledger 或 projection；Tier 1 的 `id`/`call_id`/`name`/`namespace`/payload 与 string/structured output form round-trip；unknown 只保留 metadata allowlist；非 message item、custom/Tier 2/unknown tool 和 `additional_tools` 在 call ledger 合并前统一返回 400 `unsupported_codex_feature`（message 含 feature/model/`provider=unselected`），且不进入 provider 执行路径。route/lifecycle 改动仅用于 fail-closed 与新增 optional 字段的编译适配，不得加入 call ledger、provider projection 或 previous-response call 恢复。fixtures 固定 Codex commit，正负例均 schema valid. Verify: `cargo test --locked codex_wire`，HTTP error envelope/upstream=0、missing/null/empty、unknown redaction、fixture source/count guard，fmt/check/strict Clippy。

- [ ] `SP1107-T2` Covers: B-002, B-003, B-005, B-006, B-015, B-018, B-020. Owner: canonical turn owner. Dependencies: SP1107-T1 merged. Files: `src/core/types/codex.rs`, `src/core/types/mod.rs`, `src/server/routes/ai/responses/codex_compat.rs`, `src/server/routes/ai/responses/codex_compat_tests.rs`. Done when: ordered CodexTurn、closed call kind、call ledger 和 execution requirements 完成，unknown/duplicate/missing/type mismatch 在 provider 前拒绝，且没有新 router 或 tool executor. Verify: `cargo test --locked codex_turn codex_call_ledger`，property/negative tests、source guard、fmt/check/Clippy。

- [ ] `SP1107-T3` Covers: B-001, B-004, B-006, B-007, B-008, B-010, B-015, B-016, B-017, B-020. Owner: sync Responses owner. Dependencies: SP1107-T2 merged. Files: `src/server/routes/ai/chat.rs`, `src/server/routes/ai/openai_errors.rs`, `src/server/routes/ai/responses.rs`, `src/server/routes/ai/responses/codex_compat.rs`, `src/server/routes/ai/responses/codex_compat_tests.rs`. Done when: function/custom plan 可逆映射到现有 ChatRequest，selected provider/model 缺能力时 upstream/budget/success-callback counters 为零，零输出不伪造成功，现有 Chat/Responses wire 不变. Verify: `cargo test --locked codex_sync responses_api chat_completion`，adversarial redaction tests、fmt/check/strict Clippy。

- [ ] `SP1107-T4` Covers: B-003, B-013, B-014, B-016, B-017. Owner: Responses lifecycle owner. Dependencies: SP1107-T3 merged. Files: `src/server/routes/ai/responses/lifecycle.rs`, `src/server/routes/ai/responses/lifecycle_tests.rs`. Done when: previous response 保留 Tier 1 call/output，store=false 不持久化，owner/TTL/limit/cancel 语义保持，错误不泄露存在性或 payload. Verify: `cargo test --locked responses_lifecycle codex_previous_response`，cross-owner/store/TTL/cancel fixtures、fmt/check/Clippy。

- [ ] `SP1107-T5` Covers: B-004, B-005, B-009, B-010, B-011, B-012, B-015, B-017. Owner: Responses stream owner. Dependencies: SP1107-T3 and SP1107-T4 merged. Files: `src/server/routes/ai/responses_stream.rs`, `src/server/routes/ai/responses_stream_support.rs`, `src/server/routes/ai/responses_stream_tests.rs`, `src/server/routes/ai/responses/codex_compat.rs`. Done when: function/custom calls 共用 projector，added/delta/done 顺序被状态机验证，terminal 恰好一次，并行、timeout、disconnect、cancel、partial failure 无串线、伪完成、retry 或重复结算. Verify: `cargo test --locked codex_stream responses_stream`，event table + disconnect/settlement tests、fmt/check/strict Clippy。

- [ ] `SP1107-T6` Covers: B-001, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-016, B-018, B-020. Owner: conformance owner. Dependencies: SP1107-T5 merged. Files: `src/core/providers/registry/support_matrix.rs`, `src/core/providers/registry/support_matrix_tests.rs`, `tests/integration/codex_responses_compat_tests.rs`. Done when: Anthropic、Gemini、OpenAI-compatible loopback fixtures 完成两回合 function/custom sync+stream，matrix 只声明 executable fixture 证明的 surface，unsupported provider request count=0. Verify: `cargo test --locked --test integration codex_responses_compat`，`cargo test --locked provider_surface_matrix`，无网络/真实密钥，fmt/check/Clippy。

- [ ] `SP1107-T7` Covers: B-006, B-018, B-019, B-020. Owner: docs owner. Dependencies: SP1107-T6 merged. Files: `docs/codex-compatibility.md`, `README.md`. Done when: 文档包含 `wire_api="responses"`、env-key、启动、文本/tool smoke、Tier 1/Tier 2 matrix 与恢复，不含真实 token、自动写 `~/.codex` 或新 daemon. Verify: docs source guard、人工按文档 smoke、`git diff --check`。

- [ ] `SP1107-T8` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017, B-018, B-019, B-020. Owner: coordinator + independent security reviewer. Dependencies: SP1107-T1 through SP1107-T7 merged. Files: read-only verification; defects return to the owning task. Done when: merged `origin/main` 上 full fmt/check/Clippy/test 和 coverage 通过，manual Codex smoke/恢复通过，安全 reviewer 对 exact head 给出 clean verdict，GitHub CI/review threads/required gate 完整，最终 closure PR 才可 `Fixes #1107`. Verify: tech spec 全量命令、fresh GitHub PR evidence + required `pr_gate.py`、人工 merge authorization；agent 不批准、不 merge。

## 并行拆分

- SP1107-T1 → T2 → T3 严格串行，共同定义 wire/canonical/sync 语义。
- T1 与后续 T3/T4 会串行触碰 `responses.rs` / lifecycle；T1 仅做 enum 编译适配和
  pre-provider fail-closed，禁止提前实现后续 owner 的 projection 或 context 行为。
- T4 与 T5 都依赖 T3；T5 还依赖 T4 的 context contract，因此默认串行，避免共享 `codex_compat.rs`。
- T6 在 T5 后只写 matrix + integration fixture。
- T7 在 T6 exact behavior 稳定后由 docs owner 独立写，不与 production owner 共享文件。
- T8 是只读 reviewer/coordinator lane，不写 production/spec 文件。
- 每个 writable task 使用独立 worktree 和单一 owner。若必须多 agent，文件所有权严格按 Files 列表分离；共享文件出现时改为串行。
- 任一 tranche 超过 10 个非文档文件或 500 changed lines，先合并 spec amendment，禁止删除测试、压缩断言或扩大 allowlist 规避。

## 验证

- Product invariant set: `B-001..B-020`。
- Task `Covers:` union: `B-001..B-020`；无 orphan 或 undeclared ID。
- Tech manifest: issue=1107、complete=true、18 个候选路径、spec refs=`B-001..B-020`。
- Spec 阶段：`python3 checks/check_workflow.py --repo . --spec-dir specs/GH1107`、`python3 checks/check_workflow.py --repo .`、`cargo fmt --all -- --check`、`cargo check --locked`、`git diff --check`。
- 实施阶段：按 tech spec Test Plan 执行 focused + full commands，命令必须来自当次 exact-head session。

## Handoff Notes

- 当前优先级是完成 spec review，不是开始编码。
- Root cause 是 Responses ingress 只有 message 且过早 flatten；不要新建控制层。
- Codex protocol baseline 固定到 `openai/codex@6e5a2d6b8d148a5554fdceb6f399ca45bd1c78d9`。
- Tier 1 是 function/custom 双工具闭环；Tier 2 未实现前必须显式 4xx。
- Codex 执行工具，gateway 只翻译；禁止加入 shell/MCP executor。
- 使用现有 canonical router/provider/budget/callback/lifecycle；禁止第二套 registry。
- 用户配置只写文档，不自动改 `~/.codex/config.toml`。
- 实施前维护者必须授予 `ready_to_implement` 并批准 product/tech spec。
