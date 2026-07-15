# Task Plan

## Linked Issue

GH-965 / #965

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP965-T001` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: maintainer decision owner + spec coordinator. Dependencies: current packet merged；issue comment `4982855807`. Done when: `HD-001` 至 `HD-004` 均为 resolved，记录 exact runtime/default binding、validated request context、现有 `ErrorCode`/`CanonicalError` 的 exhaustive typed mapping、0.6.0 deprecation → 0.7.0 removal、release-workflow prerequisite、rollback 与更新后的 tranche scope；spec amendment 通过独立 review 并合并，此前后续 implementation 明确 blocked. Verify: 人工核对无 `unresolved`、无新增平行 error taxonomy；`python3 <specrail>/checks/check_workflow.py --repo <specrail> --spec-dir "$PWD/specs/GH965"`；`git diff --check`；independent spec review。
- [ ] `SP965-T002` Covers: B-001, B-003, B-004, B-007, B-008, B-009, B-010. Owner: runtime-contract owner. Dependencies: SP965-T001 merged. Done when: D1 在批准 API 下使 provider construction、deployment selection/execution、retry/fallback、lease/state 与 immutable generation 属于同一 runtime contract；实现字段私有、只能经 `RuntimeRequestContext::validate` 构造的 request context，固定保留 validated `headers`/`timeout` 与 0.6.0-only legacy selector，且无 `Any`/字符串 dispatch；PR 限 tech D1 scope、最多 9 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `cargo test --all-features --locked core::router`; request-context struct-literal compile-fail fixture + header/timeout/selector negative tests；format/check/strict Clippy/full test；scope/overlap；exact-head independent review 与 required PR gate。
- [ ] `SP965-T011` Covers: B-006, B-012. Owner: canonical-error owner. Dependencies: SP965-T002 merged. Done when: D1E 复用现有 `ErrorCode`/`CanonicalError` 与 `ProviderHttpErrorFacts`，增加 cancellation category，令 SDK/Gateway/HTTP/retry/redaction 只消费 typed canonical facts；不得新增平行 error taxonomy，现有 `SDKError::ProviderError(String)` 仅作为 0.6.0 deprecated facade. Verify: exhaustive per-variant table fixture；`cargo test --all-features --locked utils::error`; `cargo test --all-features --locked sdk::errors`; source guard 拒绝 adapter string classification；scope/overlap；exact-head independent review 与 required gate。
- [ ] `SP965-T003` Covers: B-002, B-003, B-004, B-005, B-006, B-011. Owner: completion-unary facade owner. Dependencies: SP965-T011 merged. Done when: D2 让 free functions 与批准保留的 completion facade 通过 canonical runtime 执行 unary 请求，旧 env/registry/static selector 不再是 fallback，公共行为遵循 `HD-003/004`；PR 限 D2 scope、最多 8 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `cargo test --all-features --locked core::completion`; `cargo test --all-features --locked --test lib integration::router_tests`; compile/doc compatibility fixtures；repository gates、scope/overlap、review/gate。
- [ ] `SP965-T004` Covers: B-003, B-004, B-006, B-008, B-010, B-011. Owner: completion-stream-and-override owner. Dependencies: SP965-T003 merged. Done when: D3 按已解决的 `HD-002` 保留 validated `headers`/`timeout`，将 `api_key`/`api_base` 限制为 0.6.0 deprecated legacy selector 并锁定 0.7.0 removal；stream 由 canonical selected lease 执行并区分 pre-output failure/post-output failure/cancel/success，legacy dynamic provider/client path 不再可达；PR 限 D3 scope、最多 7 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `cargo test --all-features --locked streaming`; focused override/security negative tests；completion source guard；repository gates、scope/overlap、review/gate。
- [ ] `SP965-T005` Covers: B-001, B-002, B-004, B-005, B-006, B-007, B-008, B-009, B-011. Owner: SDK runtime-binding owner. Dependencies: SP965-T004 merged. Done when: D4 将 `ClientConfig` 归一到 canonical runtime，SDK selection 不再用本地 stats/临时 `RoutingContext` 决策，保留 facade 遵循 `HD-001/003/004`；PR 限 D4 scope、最多 7 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `cargo test --all-features --locked sdk::client`; SDK config/error compatibility fixtures；repository gates、scope/overlap、review/gate。
- [ ] `SP965-T006` Covers: B-002, B-003, B-005, B-006, B-007, B-008, B-009, B-010. Owner: SDK execution owner. Dependencies: SP965-T005 merged. Done when: D5 删除 SDK provider-type sender、local retry/selection state 与双重 stats truth，chat/stream/embeddings 从 canonical selected provider/state 派生；PR 限 D5 scope、最多 7 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: focused SDK chat/stream/embeddings + exactly-once tests；SDK source guard；`cargo test --all-features --locked sdk`; repository gates、scope/overlap、review/gate。
- [ ] `SP965-T007` Covers: B-001, B-003, B-011. Owner: registry-compatibility owner. Dependencies: SP965-T006 merged. Done when: D6 迁移 completion/embedding 等全部 production `ProviderRegistry` 调用方，按 `HD-003` 将公开 registry/router symbols 降级为 0.6.0 deprecated stateless facade；任何 facade 都无独立 mutable provider map 且不参与执行，0.7.0 removal 留给 T010 follow-up；PR 限 D6 scope、最多 6 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `rg -n "ProviderRegistry|DefaultRouter|dyn Router" src`; approved allowlist/source guard；compile/doc fixtures；repository gates、scope/overlap、review/gate。
- [ ] `SP965-T008` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: conformance owner. Dependencies: SP965-T007 merged; #1026 merged/closed prerequisite is already satisfied. Done when: D7 只在批准 binding/test scope 内完成 HTTP runtime 注入与三入口 deterministic conformance；同一 fixtures 验证 selection identity、errors、retry/fallback、snapshot、stream/cancel、exactly-once，source guard red/green；不修改 #1026 三文件或 `src/server/routes/ai/execution.rs`；PR 最多 5 个非文档文件/500 changed lines，使用 `Refs #965`，不得在 0.7 removal handoff 缺失时提前关闭 issue. Verify: `cargo test --all-features --locked --test lib integration::router_runtime_conformance`; source guard red/green；format/check/strict Clippy/full test；scope/overlap；exact-head implementation/security review、CI、0 unresolved threads、required gate。
- [ ] `SP965-T010` Covers: B-011. Owner: release-policy owner + spec coordinator. Dependencies: SP965-T008 merged；0.6.0 deprecation symbols/fields 与 migration note 已发布或有可验证的 release artifact. Done when: 创建并链接独立 0.7.0 removal issue/spec，逐项列出 `DefaultRouter`、completion `Router` trait、mutable `ProviderRegistry` ownership、request-level `api_key`/`api_base` 与 legacy `SDKError::ProviderError(String)`；该 follow-up 将 `.github/workflows/version-bump.yml` 修订与 deterministic 0.x fixture 设为 removal 前硬依赖，并明确禁止用 non-breaking commit label 绕过；#965 closure comment 链接该 durable work item. Verify: GitHub issue/spec link；0.6.0 release/deprecation evidence；人工核对 removal 表与 B-011/HD-003 完全一致。
- [ ] `SP965-T009` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: closure auditor. Dependencies: SP965-T008 merged；SP965-T010 durable 0.7.0 follow-up 已创建并链接. Done when: merged `origin/main` 证据证明全部 runtime invariants、HD 决策与 tranche merge chain，required gate PASS；重复 runtime state 已删除或降级为 stateless facade，#965 closed、分支删除，#519 仅保留 roadmap，closure audit 无遗留双 runtime claim；0.7.0 public removal 只由 SP965-T010 follow-up 关闭. Verify: GitHub evidence adapter；offline `pr_gate.py --mode required`；merged SHA/issue state/reviewThreads/CI checks；`rg` source guard 在 `origin/main` fresh checkout 重跑。

## 并行拆分

- T001 → T002 → T011 → T003 → T004 → T005 → T006 → T007 → T008 严格串行：后续任务必须从前一
  tranche 的 merged `origin/main` 开始；禁止并行写 completion、SDK、error adapter 或 runtime files。
- T010 在 T008 后建立 durable release handoff；T009 只读 closure/review lane 必须等 T010 完成，可与
  coordinator 的 final exact-head verification 并行；reviewer 不写文件。
- 每个 writable task 使用独立 worktree 和单一 owner。若需要多 agent，只能按 tech 表中的 disjoint file
  ownership 拆只读 reviewer lane；不得让两个 agent 修改同一路径。
- merged PR #1026 是已满足前置；`src/server/routes/ai/gemini.rs`、
  `src/server/routes/ai/gemini/provider.rs`、`tests/gemini_sdk_routes/runtime_provider_tests.rs` 作为非重叠边界保持只读。
- 任一 tranche 超过 10 个非文档文件或 500 changed lines，先提交并合并 spec amendment 拆 tranche；禁止
  删除测试、压缩断言或扩大 allowlist 规避 gate。

## 验证

- Product、tech mapping 与 tasks `Covers:` union 精确为 B-001 至 B-012，无 orphan/undeclared invariant。
- `HD-001` 至 `HD-004` 在 implementation 前全部 resolved；current packet 不把任何 option 当默认。
- 每个 tranche fresh exact-head 通过 `cargo fmt --all -- --check`、
  `cargo check --all-targets --all-features --locked`、strict Clippy、
  `cargo test --all-features --locked -- --test-threads=1`。
- 每个 PR 有 scope/overlap、independent review、CI、0 unresolved review threads 与 required gate 证据。
- final conformance 与 source guard 在 merged `origin/main` 重跑；support matrix 单测不能替代 B-012。

## Handoff Notes

- `HD-001` 至 `HD-004` 已由 issue comment `4982855807` 解决；implementation 只在 T001 amendment review/merge 后开放。
- canonical runtime 是现有 `UnifiedRouter` deployment snapshot；生命周期/API 以本 amendment 的 resolved contract 为准。
- 绝不恢复 config rescan、临时 provider、adapter-owned reqwest client、SDK local routing stats 或
  `DefaultRouter` fallback。
- #725/#728 是已完成前置；#966/#1026、#968 是相邻非重叠边界。GH-965 不修改其 wire、support matrix、
  endpoint policy 或 Gemini selected-provider contract。
- D1-D7 只用 `Refs #965` 并保持 issue open；T010 durable release handoff 完成后才由 T009 closure audit
  关闭 #965。每次 merge 后删除阶段分支并审计下一 tranche base SHA。
- 0.6.0 只做 deprecation；0.7.0 removal 由 T010 durable follow-up 管理。当前 version-bump workflow 会把
  0.x breaking 计算为 1.0.0，未先修订并 fixture 验证时不得执行 removal；不得通过提交命名规避。
