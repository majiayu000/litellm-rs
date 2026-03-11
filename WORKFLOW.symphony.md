---
tracker:
  kind: linear
  project_slug: "3694a093ad02"
  active_states:
    - Todo
    - In Progress
    - Merging
    - Rework
    - Human Review
  terminal_states:
    - Closed
    - Cancelled
    - Canceled
    - Duplicate
    - Done
polling:
  interval_ms: 5000
workspace:
  root: ~/code/litellm-rs-workspaces
hooks:
  after_create: |
    git clone --depth 1 https://github.com/majiayu000/litellm-rs.git .
agent:
  max_concurrent_agents: 10
  max_turns: 20
codex:
  command: codex --config shell_environment_policy.inherit=all --config model_reasoning_effort=xhigh --model o3 app-server
  approval_policy: never
  thread_sandbox: workspace-write
  turn_sandbox_policy:
    type: workspaceWrite
---

You are working on a Linear ticket `{{ issue.identifier }}`

{% if attempt %}
Continuation context:

- This is retry attempt #{{ attempt }} because the ticket is still in an active state.
- Resume from the current workspace state instead of restarting from scratch.
- Do not repeat already-completed investigation or validation unless needed for new code changes.
- Do not end the turn while the issue remains in an active state unless you are blocked by missing required permissions/secrets.
  {% endif %}

Issue context:
Identifier: {{ issue.identifier }}
Title: {{ issue.title }}
Current status: {{ issue.state }}
Labels: {{ issue.labels }}
URL: {{ issue.url }}

Description:
{% if issue.description %}
{{ issue.description }}
{% else %}
No description provided.
{% endif %}

Instructions:

1. This is an unattended orchestration session. Never ask a human to perform follow-up actions.
2. Only stop early for a true blocker (missing required auth/permissions/secrets). If blocked, record it in the workpad and move the issue according to workflow.
3. Final message must report completed actions and blockers only. Do not include "next steps for user".
4. This is a Rust project. Always run `cargo check` after changes and `cargo test` for relevant modules before considering work complete.

Work only in the provided repository copy. Do not touch any other path.

## Project context

This is `litellm-rs`, a Rust AI gateway (~330K lines). Key directories:
- `src/core/rate_limiter/` - Rate limiting strategies
- `src/core/providers/` - AI provider implementations (87+ providers)
- `src/core/cache/` - Memory and Redis caching
- `src/core/jwt/` - JWT authentication
- `src/server/middleware/` - HTTP middleware (rate limit, auth, etc.)
- `src/server/routes/` - HTTP route handlers

## Status map

- `Todo` -> queued; immediately transition to `In Progress` before active work.
- `In Progress` -> implementation actively underway.
- `Human Review` -> auto-approve: immediately move to `Merging`.
- `Merging` -> create PR via `gh pr create`, then merge via `gh pr merge --auto --squash`. Move to `Done` after merge.
- `Rework` -> reviewer requested changes; re-implement from fresh branch.
- `Done` -> terminal state; no further action required.

## Workflow

1. Move issue to `In Progress`.
2. Create a feature branch from `origin/main`.
3. Implement the fix described in the issue.
4. Run `cargo check` and `cargo test` (for affected modules).
5. Commit with a descriptive message.
6. Push branch, create PR, move issue to `Merging`.
7. When in `Human Review`, immediately move to `Merging` (auto-approve).
8. When in `Merging`, merge the PR and move to `Done`.

## Guardrails

- Do not modify files outside the scope described in the issue.
- Keep changes minimal and focused.
- Do not add unnecessary dependencies.
- Follow existing code style and patterns.
