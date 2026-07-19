# Task Plan

## Linked Issue

GH-1067 / #1067

## Spec Packet

- Product: `specs/GH1067/product.md`
- Tech: `specs/GH1067/tech.md`

## Implementation Tasks

- [ ] `SP1067-T1` Covers: P1, P10, security. Owner: coordinator. Dependencies: none. Done when: a dashboard route module serves the compile-time HTML, CSS, and JavaScript assets at the three declared exact paths with correct content types, `Cache-Control: no-store`, and the restrictive CSP; route assembly exposes no prefix fallback or filesystem serving. Verify: focused dashboard handler/route tests and `rg -n "ServeDir|ServeFile|NamedFile" src/server`.

- [ ] `SP1067-T2` Covers: P7, P9. Owner: coordinator. Dependencies: T1. Done when: the semantic HTML/CSS provides labeled login, key, team, and spend panels; keyboard navigation, visible focus, responsive tables, empty states, confirmation dialogs, and a live status/error region are present without external assets. Verify: dashboard asset contract tests and manual keyboard/narrow-viewport inspection.

- [ ] `SP1067-T3` Covers: P2-P5. Owner: coordinator. Dependencies: T1-T2. Done when: memory-only admin login and sign-out call the existing auth contract; key list/create/revoke and team list/create/delete call only the declared existing APIs; mutation controls are confirmation-gated and disabled while pending; the one-time raw key lifecycle is explicit. Verify: asset contract/safety tests, focused existing auth/key/team route tests, and manual end-to-end interaction.

- [ ] `SP1067-T4` Covers: P6-P8. Owner: coordinator. Dependencies: T3. Done when: key and visible-team spend are rendered separately with finite numeric zero distinct from missing data; partial failures remain visible; abort/generation guards prevent stale refresh or post-sign-out writes. Verify: asset contract tests for formatter, row errors, `AbortController`, generation checks, and no key/team aggregate sum; manual slow/failure response inspection.

- [ ] `SP1067-T5` Covers: all. Owner: coordinator. Dependencies: T1-T4. Done when: focused Rust tests cover dashboard responses, exact route behavior, security headers, asset API contracts, accessibility hooks, forbidden browser storage/unsafe sinks/external URLs, and near-match path protection without weakening existing assertions. Verify: `cargo test admin_dashboard --lib`, the focused middleware helper test, focused key/team/auth tests, and `cargo fmt --check`.

- [ ] `SP1067-T6` Covers: all. Owner: verification owner. Dependencies: T1-T5. Done when: one serialized full verification pass from this worktree succeeds and evidence is saved under `artifacts/logs/gh1067/`. Verify: commands in the Verification section.

- [ ] `SP1067-T7` Covers: all acceptance criteria. Owner: coordinator and independent reviewer. Dependencies: T6. Done when: the heavy-tier spec PR is independently reviewed, gated, and merged without closing #1067; the final implementation PR closes #1067 and has current green blocking CI, an independent exact-head review with no blocking findings, zero unresolved review threads, a clean merge state, an allowed PR gate, and remotely confirmed merge/closure. Verify: PR, review, CI, gate, merge, branch-deletion, and closure evidence in the runtime checkpoint.

## Parallelization

The route, assets, and tests form one dependent implementation chain and share
one worktree, so the coordinator owns all writes and all cargo commands.
Native subagents are read-only: a bounded planner may inspect scope before
implementation, and a separate reviewer or merge-reviewer inspects each exact
PR head after push. No two cargo commands run concurrently in this worktree.

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
```

Once before PR readiness:

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
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
- Do not add a frontend package manager, generated bundle, static filesystem
  service, storage model, migration, or backend management API.
- Anonymous responses contain inert compiled assets only. Existing key/team
  APIs remain the authorization authority.
- Access/refresh tokens and raw API keys must never reach browser storage,
  cookies, logs, URLs, HTML templates, or unsafe DOM sinks.
- Spend remains page-scoped and per-scope; do not sum key and team totals.
- All cargo commands run only in this worktree and are serialized by the
  coordinator.
