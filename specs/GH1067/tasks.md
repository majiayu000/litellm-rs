# Task Plan

## Linked Issue

GH-1067 / #1067

## Spec Packet

- Product: `specs/GH1067/product.md`
- Tech: `specs/GH1067/tech.md`

## Implementation Tasks

- [x] `SP1067-T1` Covers: P1, P10, security. Owner: coordinator. Dependencies: none. Done when: a dashboard route module serves the compile-time HTML, CSS, and JavaScript assets at the three declared exact paths with correct content types, `Cache-Control: no-store`, and the restrictive CSP; route assembly exposes no prefix fallback or filesystem serving. Verify: focused dashboard handler/route tests and `rg -n "actix_files|Files::new|NamedFile|ServeDir|ServeFile" src/server`.

- [x] `SP1067-T2` Covers: P7, P9. Owner: coordinator. Dependencies: T1. Done when: the semantic HTML/CSS provides labeled login, key, team, and spend panels; keyboard navigation, visible focus, responsive tables, empty states, confirmation dialogs, and a live status/error region are present without external assets. Verify: dashboard asset contract tests and manual keyboard/narrow-viewport inspection.

- [x] `SP1067-T3` Covers: P2-P5. Owner: coordinator. Dependencies: T1-T2. Done when: memory-only admin login and sign-out call the existing auth contract; every authenticated response is generation-checked before state/DOM commit and sign-out aborts all active requests; key list/create/revoke and team list/create/delete call only the declared existing APIs; key creation requires exactly one user/team owner plus non-wildcard model/endpoint permissions with `is_admin=false`; mutation controls are confirmation-gated and disabled while pending; the one-time raw key lifecycle is explicit. Verify: asset contract/safety tests, focused existing auth/key/team route tests, and the repeatable manual checklist.

- [x] `SP1067-T4` Covers: P6-P8. Owner: coordinator. Dependencies: T3. Done when: key and visible-team spend are rendered separately with finite numeric zero distinct from missing data; partial failures remain visible; all-request abort/generation guards prevent stale refresh, mutation, one-time raw-key, or post-sign-out writes. Verify: deterministic asset contract tests for formatter, row errors, active-controller cleanup, session generation checks before commits, and no key/team aggregate sum; repeatable delayed/failing-response manual checklist.

- [x] `SP1067-T5` Covers: P1-P10（仅 source/Rust）. Owner: coordinator. Dependencies: T1-T4. Done when: focused Rust tests and static source assertions cover dashboard responses, exact route behavior, security headers, asset API contracts, safe key ownership/permissions, all-request session guard source structure, accessibility hooks, forbidden browser storage/unsafe sinks/external URLs, and near-match path protection without weakening existing assertions; this task does not claim executable DOM behavior. Verify: `cargo test admin_dashboard --lib`, the focused middleware helper test, focused key/team/auth tests, and `cargo fmt --check`.

- [ ] `SP1067-T6` Covers: all. Owner: verification owner. Dependencies: T1-T5, T8. Done when: one serialized full verification pass from the exact implementation head succeeds, including the executable DOM suite, and SHA-scoped evidence is saved under the ignored `artifacts/logs/gh1067/<HEAD_SHA>/` contract. Verify: commands in the Verification section and the remote Actions run/artifact evidence.

- [ ] `SP1067-T7` Covers: all acceptance criteria. Owner: coordinator and independent reviewer. Dependencies: T6. Done when: the heavy-tier spec PR is independently reviewed, gated, and merged without closing #1067; the final implementation PR closes #1067 and has a current-head check rollup, an independent exact-head review with no blocking findings, zero unresolved review threads, a clean merge state, and an `allowed` SpecRail PR gate before human-authorized merge and remotely confirmed closure. The workflow check is not described as required or blocking because branch protection is external, mutable state. Verify: exact PR head, check rollup, review, review-thread, merge-state, PR-gate, merge, branch-deletion, and closure evidence in the runtime checkpoint.

- [ ] `SP1067-T8` Covers: B1, B2, B3, B4, B5, B6. Owner: executable DOM verification owner. Dependencies: T5. Done when: the complete six-path implementation manifest is implemented exactly; Node `24.14.0`, jsdom `29.1.1`, a committed lockfile, and `npm ci --ignore-scripts` provide an isolated test-only environment; the `node:test` harness executes the real embedded `app.js` and deterministically covers every B invariant; the verification script writes ignored SHA-scoped manifest/log/checksum/`_SUCCESS` evidence; and exact-head Actions uploads that evidence with `actions/upload-artifact@v4`. `app.js` is not changed unless a failing executable test first triggers another spec-only amendment. Verify: dependency install, DOM test, evidence script, manifest/scope checks, and the exact-head Actions run/artifact URL.

## Parallelization

The route, assets, and Rust/source tests through T5 form the completed
implementation chain. T8 is a bounded executable-verification lane whose
exclusive future writes are the six paths in the complete manifest. T6 consumes
both lanes serially. Native subagents remain read-only unless given explicit,
disjoint ownership; a separate reviewer or merge-reviewer inspects each exact
PR head after push. No two verification commands write the evidence directory
concurrently in one worktree.

The public authentication boundary makes this a `heavy` PR tier. Follow the
two-PR flow: first a spec-only PR with `Refs #1067`, then the final
implementation PR with `Fixes #1067`.

## Verification

During implementation:

```bash
cargo test admin_dashboard --lib
cargo test server::middleware --lib
cargo test server::routes::keys --lib
cargo test server::routes::teams --lib
(cd tests/admin_dashboard && npm ci --ignore-scripts)
node --test tests/admin_dashboard/admin_dashboard_dom.test.mjs
bash scripts/verify-gh1067.sh
```

Once before PR readiness:

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
bash scripts/verify-gh1067.sh
python3 checks/check_workflow.py --repo .
python3 checks/check_workflow.py --repo . --spec-dir specs/GH1067
bash scripts/guards/check_pr_scope.sh
bash scripts/guards/check_pr_overlap.sh
```

After each push:

```bash
gh pr checks <pr> --repo majiayu000/litellm-rs --watch --fail-fast
```

Collect current PR/head/check/review-thread evidence, run the repository PR
gate serially, and merge only when its decision is `allowed`.

## Handoff Notes

- Only #1067 is in scope. Do not inspect, edit, comment on, or merge another
  issue or PR.
- Do not add a runtime frontend bundle, runtime package manager, deployed
  frontend toolchain, static filesystem service (`actix_files`, `Files::new`,
  `NamedFile`, `ServeDir`, or `ServeFile`), storage model, migration, or backend
  management API. Only the exact, locked, isolated test-only Node/jsdom harness
  is allowed.
- The complete implementation manifest contains exactly
  `tests/admin_dashboard/package.json`,
  `tests/admin_dashboard/package-lock.json`,
  `tests/admin_dashboard/admin_dashboard_dom.test.mjs`,
  `scripts/verify-gh1067.sh`,
  `.github/workflows/admin-dashboard-verification.yml`, and `.gitignore`.
  `src/server/routes/admin_dashboard/app.js` remains out of scope unless a
  later spec-only amendment adds it after an executable test exposes a defect.
- Anonymous responses contain inert compiled assets only. Existing key/team
  APIs remain the authorization authority.
- Access/refresh tokens and raw API keys must never reach browser storage,
  cookies, logs, URLs, HTML templates, or unsafe DOM sinks.
- Spend remains page-scoped and per-scope; do not sum key and team totals.
- Keyboard flow, narrow-layout behavior, and real-browser rendering remain
  manual checks; executable DOM automation must not be presented as their
  evidence.
- Report the workflow as current-head check evidence/check rollup, not as a
  required or blocking check; branch-protection configuration is external and
  mutable.
- All cargo commands run only in this worktree and are serialized by the
  coordinator.
