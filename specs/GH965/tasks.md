# Task Plan

## Linked Issue

GH-965 / #965

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP965-T001` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: maintainer decision owner + spec coordinator. Dependencies: current packet merged. Done when: `HD-001` 至 `HD-004` 各自记录选择、理由、迁移版本、exact API/error mapping、rollback 与更新后的 tranche file scope，spec amendment 通过独立 review 并合并；任一 unresolved 时后续 implementation 明确 blocked. Verify: `python3 checks/route_gate.py --repo <specrail> --route write_spec --issue 965 --state ready_to_spec --json`；人工核对四个 HD 均无 `unresolved`；SpecRail packet validation。
- [ ] `SP965-T002` Covers: B-001, B-003, B-007, B-008, B-009, B-010. Owner: runtime-contract owner. Dependencies: SP965-T001 merged. Done when: D1 在批准 API 下使 provider construction、deployment selection/execution、retry/fallback、lease/state 与 immutable generation 属于同一 runtime contract，且无 `Any`/字符串 dispatch；PR 限 tech D1 scope、最多 9 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `cargo test --all-features --locked core::router`; format/check/strict Clippy/full test；scope/overlap；exact-head independent review 与 required PR gate。
- [ ] `SP965-T003` Covers: B-002, B-003, B-004, B-005, B-006, B-011. Owner: completion-unary facade owner. Dependencies: SP965-T002 merged. Done when: D2 让 free functions 与批准保留的 completion facade 通过 canonical runtime 执行 unary 请求，旧 env/registry/static selector 不再是 fallback，公共行为遵循 `HD-003/004`；PR 限 D2 scope、最多 8 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `cargo test --all-features --locked core::completion`; `cargo test --all-features --locked --test lib integration::router_tests`; compile/doc compatibility fixtures；repository gates、scope/overlap、review/gate。
- [ ] `SP965-T004` Covers: B-003, B-004, B-006, B-008, B-010, B-011. Owner: completion-stream-and-override owner. Dependencies: SP965-T003 merged. Done when: D3 按 `HD-002` 迁移或弃用 request overrides，stream 由 canonical selected lease 执行并区分 pre-output failure/post-output failure/cancel/success，legacy dynamic provider/client path 不再可达；PR 限 D3 scope、最多 7 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `cargo test --all-features --locked streaming`; focused override/security negative tests；completion source guard；repository gates、scope/overlap、review/gate。
- [ ] `SP965-T005` Covers: B-001, B-002, B-004, B-005, B-006, B-007, B-008, B-009, B-011. Owner: SDK runtime-binding owner. Dependencies: SP965-T004 merged. Done when: D4 将 `ClientConfig` 归一到 canonical runtime，SDK selection 不再用本地 stats/临时 `RoutingContext` 决策，保留 facade 遵循 `HD-001/003/004`；PR 限 D4 scope、最多 7 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `cargo test --all-features --locked sdk::client`; SDK config/error compatibility fixtures；repository gates、scope/overlap、review/gate。
- [ ] `SP965-T006` Covers: B-002, B-003, B-005, B-006, B-007, B-008, B-009, B-010. Owner: SDK execution owner. Dependencies: SP965-T005 merged. Done when: D5 删除 SDK provider-type sender、local retry/selection state 与双重 stats truth，chat/stream/embeddings 从 canonical selected provider/state 派生；PR 限 D5 scope、最多 7 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: focused SDK chat/stream/embeddings + exactly-once tests；SDK source guard；`cargo test --all-features --locked sdk`; repository gates、scope/overlap、review/gate。
- [ ] `SP965-T007` Covers: B-001, B-003, B-011. Owner: registry-compatibility owner. Dependencies: SP965-T006 merged. Done when: D6 迁移 completion/embedding 等全部 production `ProviderRegistry` 调用方，按 `HD-003` 删除或降级公开 registry/router symbols；任何保留 facade 都无独立 mutable provider map 且不参与执行；PR 限 D6 scope、最多 6 个非文档文件/500 changed lines，使用 `Refs #965`. Verify: `rg -n "ProviderRegistry|DefaultRouter|dyn Router" src`; approved allowlist/source guard；compile/doc fixtures；repository gates、scope/overlap、review/gate。
- [ ] `SP965-T008` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: conformance owner. Dependencies: SP965-T007 merged and #1026 merged/closed. Done when: D7 只在批准 binding/test scope 内完成 HTTP runtime 注入与三入口 deterministic conformance；同一 fixtures 验证 selection identity、errors、retry/fallback、snapshot、stream/cancel、exactly-once，source guard red/green；不修改 #1026 三文件或 `src/server/routes/ai/execution.rs`；PR 最多 5 个非文档文件/500 changed lines，使用 `Fixes #965`. Verify: `cargo test --all-features --locked --test lib integration::router_runtime_conformance`; source guard red/green；format/check/strict Clippy/full test；scope/overlap；exact-head implementation/security review、CI、0 unresolved threads、required gate。
- [ ] `SP965-T009` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: closure auditor. Dependencies: SP965-T008 exact head ready. Done when: final PR exact-head evidence证明全部 invariants、HD 决策与 tranche merge chain，required gate PASS 后合并；#965 closed、分支删除，#519 仅保留 roadmap，open issue/PR/spec closure audit 无遗留双 runtime claim. Verify: GitHub evidence adapter；offline `pr_gate.py --mode required`；merged SHA/issue state/reviewThreads/CI checks；`rg` source guard 在 `origin/main` fresh checkout 重跑。

## 并行拆分

- T001-T008 严格串行：后续任务必须从前一 tranche 的 merged `origin/main` 开始；禁止并行写
  completion、SDK 或 runtime files。
- T009 的只读 closure/review lane 可与 coordinator 的 final exact-head verification 并行；reviewer 不写文件。
- 每个 writable task 使用独立 worktree 和单一 owner。若需要多 agent，只能按 tech 表中的 disjoint file
  ownership 拆只读 reviewer lane；不得让两个 agent 修改同一路径。
- #1026 open 期间，`src/server/routes/ai/gemini.rs`、`src/server/routes/ai/gemini/provider.rs`、
  `tests/gemini_sdk_routes/runtime_provider_tests.rs` 全程只读；T008 依赖其先合并。
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

- 当前 packet 只定义可审查 contract；由于四个 `human_decisions` 均未决，implementation route 必须 blocked。
- canonical runtime 候选是现有 `UnifiedRouter` deployment snapshot；这不是对 `HD-001` lifecycle/API 的预先选择。
- 绝不恢复 config rescan、临时 provider、adapter-owned reqwest client、SDK local routing stats 或
  `DefaultRouter` fallback。
- #725/#728 是已完成前置；#966/#1026、#968 是相邻非重叠边界。GH-965 不修改其 wire、support matrix、
  endpoint policy 或 Gemini selected-provider contract。
- D1-D6 只用 `Refs #965` 并保持 issue open；只有 D7 使用 `Fixes #965`。每次 merge 后删除阶段分支并审计
  下一 tranche base SHA。
- 若 `HD-003` 批准 breaking removal，必须另有明确 release/migration note；auto merge 授权不替代架构与
  semver human decision。
