# AGENTS.md

Agent instructions for litellm-rs.

## Workflow

- This repository is SpecRail-governed. Read `AGENT_USAGE.md` before creating
  issues, specs, PRs, reviews, or handoffs.
- Spec packets live under `docs/specs/GH<issue>/` (PRODUCT.md, TECH.md,
  tasks.md). See `docs/specs/README.md`.
- Queue work routes through the implx / specrail-implement-queue skills;
  verification gates live in `checks/`.

## Commands

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run focused tests during iteration; run the full suite plus clippy once
before claiming PR-ready.

## Rules

- One issue per implementation PR by default; PR tier lanes decide whether
  spec content ships in the same PR (see the implement-queue skill).
- Do not merge without green CI and resolved review threads.
- Builds and tests run only inside the session's own worktree.
