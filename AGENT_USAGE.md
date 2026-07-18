# Agent Usage

## SpecRail

This repository carries SpecRail as a repo-local workflow contract. Use it for
GitHub issue triage, spec writing, task planning, implementation, PR review, CI
diagnosis, PR gates, and release-note drafting when the work is issue-backed or
changes workflow policy, public behavior, cross-module architecture, hooks,
plugins, APIs, or migrations.

Start with `skills/specrail-workflow/SKILL.md`. After the route is known, load
exactly one focused skill from `skills/specrail-*/SKILL.md`; do not load every
SpecRail skill up front.

## Artifacts

- Workflow config: `workflow.yaml`
- State graph: `states.yaml`
- Label groups: `labels.yaml`
- Repo-local skills: `skills/specrail-*/SKILL.md`
- Skill lock: `skills-lock.json`
- SpecRail issue packet: `specs/GH<issue-number>/product.md`,
  `specs/GH<issue-number>/tech.md`, and
  `specs/GH<issue-number>/tasks.md`
- Existing remem contracts and historical implementation specs:
  `docs/specs/`

Do not replace the existing `docs/specs/` index with SpecRail packets. Use
`docs/specs/` for current remem contracts and history, and use
`specs/GH<issue-number>/` for new issue-first SpecRail work.

## Gates

Agents may draft, implement, diagnose, and review. Agents must not provide
final approval, merge, force-push, publish private security details, or bypass
readiness, spec approval, final review, merge, security, or release gates.

Run route gates from the repo root when enough evidence exists:

```sh
python3 checks/route_gate.py --repo . --route <route> --issue <issue-number> --state <state> --json
```

For workflow asset changes, run:

```sh
python3 checks/check_workflow.py --repo .
```
