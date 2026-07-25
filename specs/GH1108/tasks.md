# Task Plan

## Linked Issue

GH-1108 / #1108

## Spec Packet

- Product: `specs/GH1108/product.md`
- Tech: `specs/GH1108/tech.md`
- PR tier: `heavy`
- Spec status: `auto_draft` under current `implx auto` run

## 当前 Gate

本 task plan 可随 spec PR 合并，但 implementation 必须等待 GH1112 production neutral
Google catalog 合并并解除其 `parked` dependency。implementation owner 不得在等待期间把
catalog delta 写入旧 `src/core/providers/gemini/models/**`。

## 实现任务

- [ ] `SP1108-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016. Owner: spec/coordinator. Dependencies: linked issue; `write_spec` route gate. Done when: detailed completion evidence below is satisfied. Verify: detailed commands below pass.
      Files: `specs/GH1108/product.md`, `specs/GH1108/tech.md`,
      `specs/GH1108/tasks.md`. Done when: product invariants are contiguous
      `B-001..B-016`; tech Product-to-Test Mapping and task `Covers:` union both cover the
      full set; planned-changes manifest is issue=1108/complete=true; official Developer
      sources are recorded; GH1112 implementation dependency and no-Vertex-inference
      boundary are explicit. Verify:
      `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108`、
      `python3 checks/check_workflow.py --repo .`、`git diff --check`.

- [ ] `SP1108-T2` Covers: B-001, B-002, B-003, B-004, B-008, B-009, B-013, B-015, B-016. Owner: neutral catalog implementation owner. Dependencies: SP1108-T1 spec PR and GH1112 implementation merged. Done when: detailed catalog evidence below is satisfied. Verify: detailed commands below pass.
      Fresh `origin/main`,
      duplicate evidence and `implement` route gate allowed; merged GH1112 paths/API
      match the tech manifest or an amendment is merged first. Files:
      `src/core/providers/google/models/registry.rs`,
      `src/core/providers/google/models/catalog/{mod.rs,gemini35.rs,gemini36.rs}`,
      `src/core/providers/google/models/tests.rs`; all Gemini/Vertex consumers read-only.
      Done when: both GA exact IDs, limits, capabilities, per-million pricing and official
      evidence are present in the Developer overlay; every pre-refresh Developer chat ID
      has exactly one disposition; retired/shutdown/unverified/other-product entries are
      not advertised; Developer pre/post snapshot is stable, sorted and duplicate-free;
      Vertex overlay and production paths are byte-for-byte unchanged. Verify:
      `cargo test --locked google_model_catalog_2026_07`、
      `cargo test --locked gemini_2026_07_cost`、
      `git diff --name-only <base>...HEAD` has no Vertex production path、
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
      supported params and rejects any explicit occurrence before auth/network; last
      non-empty model turn fails before network; provider mapping, direct-client
      preflight and final JSON share the same contract; user/tool-ending requests remain
      compatible; no family-substring inheritance or silent drop remains for this
      contract. Verify: `cargo test --locked gemini_2026_07`、
      `cargo test --locked gemini_router_fallback`、network-counter negatives、
      `cargo fmt --all -- --check`、`cargo check --locked`.

- [ ] `SP1108-T4` Covers: B-010, B-011, B-012, B-014, B-016. Owner: live-smoke test/documentation owner. Dependencies: SP1108-T3 stable committed head. Done when: detailed smoke/redaction evidence below is satisfied. Verify: detailed commands below pass.
      Developer
      credential path unchanged. Files: `tests/live_gemini.rs`,
      `docs/providers/gemini.md`, `docs/providers/README.md`; production catalog/provider
      files read-only. Done when: ignored live tests require exactly
      `LITELLM_RS_LIVE_GEMINI=1`; absent opt-in/key makes zero network calls; static/list/
      get/minimal-call steps aggregate to a typed result; auth/quota/not_found/protocol/
      network and cancellation are closed classifications; sentinel key is absent from
      all captured outputs/artifacts; docs give the exact opt-in command, migration,
      disposition and Developer/Vertex boundary. Verify:
      `cargo test --locked live_gemini`、
      `cargo test --locked --test live_gemini -- --ignored` with no opt-in proves skip/
      zero-network、manual
      `LITELLM_RS_LIVE_GEMINI=1 cargo test --locked --test live_gemini -- --ignored`
      only when a human supplies credentials、documentation diff review.

- [ ] `SP1108-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016. Owner: coordinator + independent security reviewer. Dependencies: SP1108-T2 through T4 complete. Done when: detailed exact-head evidence below is satisfied. Verify: every tech-spec Test Plan command plus runtime/remote gates passes.
      Files: read-only
      verification; findings return to owning task. Done when: focused tests, full
      fmt/check/strict Clippy/test, SpecRail gates, exact-head coverage, local independent
      review, hosted CI, GraphQL review threads and `pr_gate.py` are current and green;
      changed production Rust line coverage is at least 80%; catalog evidence/validation,
      deprecated-param rejection, prefill rejection, live classification and redaction
      branch records exist and are 100% covered; no raw credential-bearing live output is
      published; final PR uses `Fixes #1108`. Verify: every command in the tech-spec Test
      Plan plus `python3 checks/runtime_ledger_gate.py --checkpoint
      .specrail/runtime/current.json` and fresh GitHub evidence.

## 并行拆分

- Spec PR 与 implementation PR 分离；这是 heavy tier 的两阶段流程。
- T2 → T3 严格串行：T2 owns neutral catalog records，T3 owns shared contract 和 Gemini
  consumers；T3 只能在 T2 head、focused verification 和 clean state 记录后开始。
- T4 理论上文件独立，但依赖 T3 的最终 contract 和 credential/error shape；为避免 smoke
  固化中间 API，按 `serial_after_dependency` 执行，不与 T3 并发写。
- T5 reviewer 只读；full suite/coverage 只有 coordinator 一个 owner。
- GH1111/GH1113/GH1112 或其他 Google/Gemini writable lane 与本 implementation 禁止并发；
  overlapping neutral/consumer paths 必须串行。

## 验证

- Product invariant set:
  `B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010,
  B-011, B-012, B-013, B-014, B-015, B-016`.
- Task `Covers:` union: 同一完整集合；无 orphan invariant。
- Planned manifest: issue=1108、complete=true、包含 neutral catalog、shared contract、
  Gemini consumers、live smoke 与 provider docs。
- Spec phase:
  `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108`、
  `python3 checks/check_workflow.py --repo .`、`git diff --check`.
- Implementation phase: focused tests during iteration；exact final head 只执行一次 full
  fmt/check/strict Clippy/test/coverage，并把 raw output 保存到 artifact。

## Handoff Notes

- 当前唯一 blocker 是 GH1112 production dependency，不是缺少 GH1108 产品 done-when。
- 2026-07-26 verified official facts：
  - `gemini-3.6-flash`: 1,048,576 / 65,536，1.50 / 7.50 USD per million。
  - `gemini-3.5-flash-lite`: 1,048,576 / 65,536，0.30 / 2.50 USD per million。
- Google Developer release/lifecycle/pricing 证据不等于 Vertex availability。
- deprecated sampling fields 使用 typed pre-network rejection，禁止 silent drop。
- live smoke 仍处于 manual validation 阶段；不得直接做 cron/CI automation。
- 若 merged GH1112 API/path 与本 tech manifest 不同，先修订 spec，不猜 alias/wrapper。
- 不修改 GH1111 tool loop、GH1113 pricing authority 或 unknown-cost semantics。
