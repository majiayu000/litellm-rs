# Task Plan

## Linked Issue

GH-1064 / #1064

## Spec Packet

- Product: `specs/GH1064/product.md`
- Tech: `specs/GH1064/tech.md`

## Implementation Tasks

- [x] `SP1064-T1` — Owner: coordinator. Dependencies: merged roadmap PR #1068. Done when: the product and technical specs define #1064 as a planning umbrella, keep child implementation out of scope, and give parent closure an explicit meaning. Verify: SpecRail packet checks and manual spec review.
- [x] `SP1064-T2` — Owner: coordinator. Dependencies: `SP1064-T1`. Done when: the roadmap records delivery, focused ownership, and closure semantics without changing a child spec, issue, or production file. Verify: `git diff --check` and changed-path inspection.
- [ ] `SP1064-T3` — Owner: coordinator and independent reviewer. Dependencies: `SP1064-T1`, `SP1064-T2`. Done when: deterministic checks pass, the closing PR uses `Fixes #1064`, an independent exact-head review is clean, current-head CI and PR gates are green, the PR is merged, and remote closure confirms only #1064 closed. Verify: local command evidence, reviewer artifact, GitHub PR evidence, offline PR gate, merge confirmation, and closure audit.

## Parallelization

The documentation edits are one coordinator-owned serial lane. The independent
reviewer is read-only and starts after the PR head is stable. Cargo and full
repository verification, if required by CI or repository gates, run only in
this worktree under the coordinator.

## Verification

Run these commands during `SP1064-T3`:

```bash
python3 checks/check_workflow.py --repo .
python3 checks/check_workflow.py --repo . --spec-dir specs/GH1064
python3 checks/route_gate.py --repo . --route implement --issue 1064 --state ready_to_implement --artifact product_spec=specs/GH1064/product.md --artifact tech_spec=specs/GH1064/tech.md --artifact task_plan=specs/GH1064/tasks.md --duplicate-evidence .specrail/runtime/evidence/duplicate-1064.json --json
git diff --check
```

Then collect exact-head CI, independent review, resolved review threads, clean
merge state, and an allowed PR gate.

## Handoff Notes

PR #1068 already delivered the gap analysis. This tranche only makes the
planning umbrella auditable and closable; all linked implementation remains
owned by its focused issue.
