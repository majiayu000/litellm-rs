# AGENTS.md

Agent instructions for litellm-rs.

## Workflow

- Search existing code, issues, pull requests, and documentation before
  creating a new implementation or workflow artifact.
- Work on one GitHub issue per implementation pull request by default.
- Fix an existing pull request on its original branch instead of opening a
  competing pull request.

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

- Keep changes scoped to the linked issue and document the verification used.
- Do not merge without green CI and resolved review threads.
- Builds and tests run only inside the session's own worktree.
