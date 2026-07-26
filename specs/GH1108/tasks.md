# Task Plan

## Linked Issue

GH-1108 / #1108

## Spec Packet

- Product: `specs/GH1108/product.md`
- Tech: `specs/GH1108/tech.md`
- PR tier: `heavy`
- Spec status: `pending_maintainer`; `implx auto` 只授权起草，不等于 spec approval

## 当前 Gate

本 task plan 可随 spec PR 合并，但 implementation 必须等待 GH1112 production neutral
Google catalog 合并并解除其 `parked` dependency。implementation owner 不得在等待期间把
catalog delta 写入旧 `src/core/providers/gemini/models/**`。此外，maintainer 必须明确
批准本 spec 的最终 commit head；draft、PR open、route gate `allowed` 或旧 head 的批准
均不能满足该 gate。

## 实现任务

- [ ] `SP1108-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017. Owner: spec/coordinator. Dependencies: linked issue; `write_spec` route gate. Done when: detailed completion evidence below is satisfied. Verify: detailed commands below pass.
      Files: `specs/GH1108/product.md`, `specs/GH1108/tech.md`,
      `specs/GH1108/tasks.md`. Done when: product invariants are contiguous
      `B-001..B-017`; tech Product-to-Test Mapping and task `Covers:` union both cover the
      full set; planned-changes manifest is issue=1108/complete=true; official Developer
      sources are recorded；manifest 包含 versioned coverage checker/test，tech 已定义
      五类 exact selector/span 和 fail-closed negative fixtures；GH1112 implementation
      dependency and no-Vertex-inference boundary are explicit；最终 spec head 已获得
      maintainer 明确批准，且批准证据绑定该 exact head。Verify:
      `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108`、
      `python3 checks/check_workflow.py --repo .`、`git diff --check`.

- [ ] `SP1108-T2` Covers: B-001, B-002, B-003, B-004, B-008, B-009, B-013, B-015, B-016, B-017. Owner: neutral catalog implementation owner. Dependencies: SP1108-T1 spec PR and GH1112 implementation merged. Done when: detailed catalog evidence below is satisfied. Verify: detailed commands below pass.
      Fresh `origin/main`,
      duplicate evidence and `implement` route gate allowed; merged GH1112 paths/API
      match the tech manifest or an amendment is merged first. Files:
      `src/core/providers/google/models/registry.rs`,
      `src/core/providers/google/models/catalog/{mod.rs,gemini35.rs,gemini36.rs}`,
      `src/core/providers/google/models/tests.rs`; all Gemini/Vertex consumers read-only.
      Done when: both GA exact IDs、limits、exact closed capability/feature sets、Gemini
      Developer API paid Standard per-million pricing and official evidence are present
      in the Developer overlay；Batch/Flex/Priority 不复用或宣称该定价；every
      pre-refresh Developer chat ID
      has exactly one disposition; retired/shutdown/unverified/other-product entries are
      not advertised; Developer pre/post snapshot is stable, sorted and duplicate-free;
      Vertex overlay and production paths are byte-for-byte unchanged. Verify:
      `cargo test --locked google_model_catalog_2026_07`、
      `cargo test --locked gemini_2026_07_cost`、
      `test "$(git rev-parse HEAD)" = "$IMPLEMENTATION_HEAD_SHA" &&
       git diff --name-only "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA"`
      has no Vertex production path、
      `cargo fmt --all -- --check`、`cargo check --locked`.

- [ ] `SP1108-T3` Covers: B-005, B-006, B-007, B-015. Owner: shared contract + Gemini consumer owner. Dependencies: SP1108-T2 stable committed head. Done when: detailed contract evidence below is satisfied. Verify: detailed commands below pass.
      No other
      writable neutral-catalog owner. Files:
      `src/core/providers/google/models/request_contract.rs`,
      `src/core/providers/gemini/provider.rs`,
      `src/core/providers/gemini/provider_tests.rs`,
      `src/core/providers/gemini/client.rs`,
      `tests/gemini_router_fallback_routes.rs`; T2 catalog records read-only.
      Done when: exact new-model contract removes `temperature`/`top_p`/`top_k` from
      supported params；typed temperature/top_p omitted/JSON-null 按 `Option` absent；
      flattened extra_body/extra_params 的 `top_k: Value::Null` 由 shared normalizer
      消费并删除，任何 non-null 值（包括默认数值）在 auth/network 前拒绝，final body
      无 temperature/topP/top_k/topK；messages 只执行一次 trailing semantically-empty
      strip，retained sequence 同时供 prefill gate 与 serializer，且二者不读取原序列；
      meaningful user/tool + empty assistant 最终 body 不以 model 结尾，non-empty model
      + trailing empties 与 all-empty 均 pre-network error；既有 Gemini
      ToolResult/ToolUse 序列化与完整 callability 归 GH1111、非本任务 acceptance，且
      GH1108 implementation 不依赖 GH1111；no family-substring inheritance or silent
      drop remains for this contract. Verify:
      `cargo test --locked gemini_2026_07`、
      `cargo test --locked gemini_router_fallback`、network-counter negatives、
      `cargo fmt --all -- --check`、`cargo check --locked`.

- [ ] `SP1108-T4` Covers: B-010, B-011, B-012, B-014, B-016. Owner: live-smoke test/documentation owner. Dependencies: SP1108-T3 stable committed head. Done when: detailed smoke/redaction evidence below is satisfied. Verify: detailed commands below pass.
      Developer
      credential path unchanged. Files: `tests/live_gemini.rs`,
      `docs/providers/gemini.md`, `docs/providers/README.md`; production catalog/provider
      files read-only. Done when: ignored live tests require exactly
      `LITELLM_RS_LIVE_GEMINI=1`; absent opt-in/key makes zero network calls; static/list/
      get/minimal-call steps aggregate to a typed result; auth/quota/not_found/protocol/
      network 是且仅是五个 closed error classes；cancellation/timeout 产生带 termination
      reason 的 incomplete、不是第六错误类；sentinel key is absent from
      all captured outputs/artifacts; docs give the exact opt-in command, migration,
      disposition and Developer/Vertex boundary；分类与脱敏分别由 exact symbols
      `classify_live_failure`、`redact_live_artifact` 所有。Verify:
      `cargo test --locked live_gemini`、
      `cargo test --locked --test live_gemini -- --ignored` with no opt-in proves skip/
      zero-network、manual
      `LITELLM_RS_LIVE_GEMINI=1 cargo test --locked --test live_gemini -- --ignored`
      only when a human supplies credentials、documentation diff review.

- [ ] `SP1108-T6` Covers: B-003, B-005, B-006, B-011, B-012, B-016. Owner: coverage-checker coordinator. Dependencies: SP1108-T2 through T4 stable committed head; tech exact selector contract. Done when: versioned checker and negative fixtures below are implemented before SP1108-T5 read-only review. Verify: checker unit tests and exact-head invocation below pass.
      Files: `checks/gh1108_coverage_gate.py`,
      `checks/test_gh1108_coverage_gate.py`; production/test Rust files read-only.
      Done when: checker enforces full SHA/head/ancestor/tracked-clean/LCOV guards、non-empty
      changed-production denominator、all changed production sources in LCOV、changed-line
      ≥80%；五个 mandatory categories 均绑定 tech 表中 exact path + Rust symbol + marker
      span，prefill 的两个 selectors 都满足，classification/redaction 独立满足；missing/
      malformed selector、DA/BRDA、span 或 uncovered branch 全部 fail closed。negative
      fixtures 证明 same-path other-symbol 与 same-symbol outside-marker 的 covered branch
      不能满足 category，并分别证明 classification-only/redaction-only 失败。Verify:
      `python3 checks/test_gh1108_coverage_gate.py`；生成 LCOV 后执行
      `python3 checks/gh1108_coverage_gate.py --repo . --base "$IMPLEMENTATION_BASE_SHA"
       --head "$IMPLEMENTATION_HEAD_SHA" --lcov artifacts/coverage/GH1108/lcov.info
       --output artifacts/coverage/GH1108/gate.json`.

- [ ] `SP1108-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017. Owner: coordinator + independent security reviewer. Dependencies: SP1108-T2 through T4 and SP1108-T6 complete. Done when: detailed exact-head evidence below is satisfied. Verify: every tech-spec Test Plan command plus runtime/remote gates passes.
      Files: read-only
      verification; findings return to owning task. Done when: focused tests, full
      fmt/check/strict Clippy/test, SpecRail gates, exact-head coverage, local independent
      review, hosted CI, GraphQL review threads and `pr_gate.py` are current and green;
      changed production Rust line coverage is at least 80%；versioned checker 证明
      catalog evidence validation、deprecated rejection、prefill rejection、live
      classification、live redaction 的 mandatory symbol/marker spans 各自存在 changed
      branch records 且 100% covered；coverage gate 验证完整
      `IMPLEMENTATION_BASE_SHA`/`IMPLEMENTATION_HEAD_SHA`、exact HEAD、tracked clean、
      LCOV 存在，并对 missing changed production source、empty denominator、missing/
      uncovered category branch fail closed；no raw credential-bearing live output is
      published; final PR uses `Fixes #1108`. Verify: every command in the tech-spec Test
      Plan plus `python3 checks/runtime_ledger_gate.py --checkpoint
      .specrail/runtime/current.json` and fresh GitHub evidence.

## 并行拆分

- Spec PR 与 implementation PR 分离；这是 heavy tier 的两阶段流程。
- T2 → T3 严格串行：T2 owns neutral catalog records，T3 owns shared contract 和 Gemini
  consumers；T3 只能在 T2 head、focused verification 和 clean state 记录后开始。
- T4 理论上文件独立，但依赖 T3 的最终 contract 和 credential/error shape；为避免 smoke
  固化中间 API，按 `serial_after_dependency` 执行，不与 T3 并发写。
- T6 在 T2-T4 stable head 后由 coordinator 独占 `checks/gh1108_coverage_gate.py` 与
  `checks/test_gh1108_coverage_gate.py`；不得与其他 checks writer 并发。T6 完成后 T5
  reviewer 才进入只读审查；full suite/coverage 只有 coordinator 一个 owner。
- GH1111/GH1113/GH1112 或其他 Google/Gemini writable lane 与本 implementation 禁止并发；
  overlapping neutral/consumer paths 必须串行。

## 验证

- Product invariant set:
  `B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010,
  B-011, B-012, B-013, B-014, B-015, B-016, B-017`.
- Task `Covers:` union: 同一完整集合；无 orphan invariant。
- Planned manifest: issue=1108、complete=true、包含 neutral catalog、shared contract、
  Gemini consumers、live smoke、versioned coverage checker/tests 与 provider docs。
- Spec phase:
  `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108`、
  `python3 checks/check_workflow.py --repo .`、`git diff --check`.
- Implementation phase: focused tests during iteration；exact final head 只执行一次 full
  fmt/check/strict Clippy/test/coverage，并把 raw output 保存到 artifact。

## Handoff Notes

- 当前 implementation blockers 是 GH1112 production dependency 与 maintainer 对最终
  spec head 的明确批准；`route_gate allowed` 不等于批准。
- 2026-07-26 verified official facts：
  - `gemini-3.6-flash`: 1,048,576 / 65,536，Gemini Developer API paid Standard
    1.50 / 7.50 USD per million。
  - `gemini-3.5-flash-lite`: 1,048,576 / 65,536，Gemini Developer API paid Standard
    0.30 / 2.50 USD per million。
- Google Developer release/lifecycle/pricing 证据不等于 Vertex availability。
- deprecated sampling fields 对 omitted/JSON-null 按 absent 处理；non-null 使用 typed
  pre-network rejection；其中 flattened top_k null 必须被 shared normalizer 消费并从
  extra_params 删除，禁止 silent drop/透传。
- trailing semantically-empty turns 只 strip 一次；同一 retained sequence 供 gate 与
  serializer，all-empty 与 retained model 结尾 fail closed。
- `checks/gh1108_coverage_gate.py`/test 是 implementation manifest 的必交付物；必须在
  T5 read-only review 前由 coordinator 实现，不能以本 spec 里的临时 heredoc 替代。
- live smoke 仍处于 manual validation 阶段；不得直接做 cron/CI automation。
- 若 merged GH1112 API/path 与本 tech manifest 不同，先修订 spec，不猜 alias/wrapper。
- 不修改 GH1111 tool loop、GH1113 pricing authority 或 unknown-cost semantics；GH1108
  不依赖 GH1111，也不声称 tool-loop 完整 callability。
