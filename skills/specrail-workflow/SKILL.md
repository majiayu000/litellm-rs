---
name: specrail-workflow
description: Use as the router/startup skill when working in a repository that adopts SpecRail for issue-first, spec-first, AI-assisted development. Routes triage, spec writing, task planning, implementation, PR review, CI diagnosis, PR gates, release notes, and spec-vs-implementation checks to focused SpecRail skills while preserving locale and human-gate boundaries.
---

# SpecRail Workflow

Use this skill as the entrypoint for SpecRail-governed repository work. Load a
focused SpecRail skill after the route is known.

## Startup

1. Search before creating a new issue, spec, template, policy, schema, or workflow.
2. Read applicable `AGENTS.md`, then `AGENT_USAGE.md` and `PLAN.md` when present.
3. Read `workflow.yaml`, `states.yaml`, `labels.yaml`, and relevant templates.
4. Identify the route:
   - `triage_issue`
   - `write_spec`
   - `implement`
   - `review_pr`
   - `fix_ci`
   - `draft_release_note`
5. Run `checks/route_gate.py` for the selected route when the repository includes
   it. Treat `allowed` as permission to proceed, `warn` as proceed-with-caution,
   `needs_human` as a maintainer gate, and `blocked` as a stop condition.
6. When GitHub issue evidence is needed and the repository includes the adapter,
   collect it read-only:

```sh
python3 checks/github_issue_evidence.py --github-repo <owner/repo> --issue <issue-number> --json > issue-evidence.json
```

## Route To Focused Skills

- Use `skills/specrail-triage-issue/SKILL.md` for issue classification, duplicate
  searches, label proposals, and triage handoffs.
- Use `skills/specrail-write-product-spec/SKILL.md` for
  `specs/GH<issue-number>/product.md`.
- Use `skills/specrail-write-tech-spec/SKILL.md` for
  `specs/GH<issue-number>/tech.md`.
- Use `skills/specrail-plan-tasks/SKILL.md` for
  `specs/GH<issue-number>/tasks.md`.
- Use `skills/specrail-implement/SKILL.md` for code or workflow-asset changes
  after the implementation gate.
- Use `skills/specrail-check-impl-against-spec/SKILL.md` to compare a diff or PR
  with the linked product spec, tech spec, and task plan.
- Use `skills/specrail-review-pr/SKILL.md` for advisory PR review.
- Use `skills/specrail-diagnose-ci/SKILL.md` for CI failure investigation and
  focused fixes.
- Use `skills/specrail-pr-gate/SKILL.md` before reporting merge readiness.
- Use `skills/specrail-release-note/SKILL.md` after merge when drafting release
  notes.

Default to `write_spec` before `implement` for product-facing, architecture,
cross-module, public API, workflow-policy, or ambiguous behavior changes.
Choose direct `implement` only when the change is already covered by an
approved spec, is a small mechanical fix, is a test-only/doc-only correction, is
a focused CI fix, or the user explicitly asks to skip spec creation.

If `write_spec` is selected and no GitHub issue number is available, search for
an existing issue first. If none exists and GitHub workflow is in scope, create
or request a linked issue before writing `specs/GH<issue-number>/product.md` and
`tech.md`. Do not treat a missing issue number as permission to skip the spec.

## Locale

Choose the language for human-facing text in this order:

1. Explicit user request.
2. User's current language.
3. `presentation.default_locale` in `workflow.yaml`.
4. `presentation.fallback_locale`.

When the user writes Chinese or the selected locale is `zh-CN`, write human-facing artifacts in Chinese:

- issue bodies
- `product.md`
- `tech.md`
- PR bodies
- review summaries
- handoffs
- error explanations

Do not translate stable machine-facing identifiers:

- action IDs such as `write_spec`
- state IDs such as `ready_to_spec`
- decision values such as `needs_human`
- artifact IDs such as `product_spec`
- file paths such as `specs/GH1/product.md`
- command names and CLI flags
- JSON keys and schema field names

## Optional Threads Integration

If the task is a GitHub issue or PR queue, needs disjoint parallel lanes, or
requires review-thread, CI, merge-gate, or closure-audit handling, read
`integrations/threads.md` after this startup flow and use an available threads
skill for orchestration.

Keep the boundary clear:

- SpecRail owns policy, locale, required artifacts, human gates, and
  deterministic verification.
- Threads owns lane maps, queue gates, remote truth refresh, review-thread
  handling, and closure audit.
- If no threads skill or native subagent capability is available, continue with
  the single-agent SpecRail flow and report that no native threads were
  launched.

## Agent Boundaries

Agents may draft, review, diagnose, and propose labels.

Agents must not:

- provide final approval
- merge without explicit user authorization
- force push without explicit user authorization
- publish secrets or private security details
- change repository permissions
- bypass human gates

Do not install repo-distributed SpecRail skills into `$HOME` unless a human
explicitly requests installation. Treat `skills-lock.json`, when present, as the
declared repo skill set. If local Codex skill installation is explicitly
requested, run `python3 tools/install_codex_skills.py --repo .` first and use
`--apply` only for the requested write.

## Output

When reporting completion, include:

- issue or PR link, if created
- spec paths
- selected locale
- stable IDs kept in English
- verification commands and results
- PR gate decision when merge readiness was evaluated
