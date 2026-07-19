# Tech Spec

## Linked Issue

GH-1064 / #1064

## Product Spec

See `specs/GH1064/product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Roadmap | `docs/plan/2026-07-18-best-gateway-gap-analysis.md` | PR #1068 delivered the verified gap analysis and split recommendations, but the document does not define when the umbrella issue can close. | This is the only direct deliverable of #1064. |
| SpecRail packet | `specs/GH1064/` | No GH1064 packet exists. Focused child packets exist separately. | A complete packet prevents the umbrella from absorbing child implementation scope. |
| Focused work | `specs/GH1065/`, `specs/GH1066/`, `specs/GH1067/` and existing issue-owned specs | Implementation requirements are already owned outside #1064. | The closure must preserve those ownership boundaries and states. |

## Proposed Design

Add a small GH1064 SpecRail packet that treats the issue as a planning
umbrella. Add a closure-status section to the existing roadmap that records:

- the roadmap delivery PR (#1068),
- the focused issue ownership model,
- the meaning of closing #1064, and
- the prohibition on interpreting parent closure as child completion.

No production source, configuration, schema, API, workflow policy, or child
spec packet changes.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1: durable fact/inference/advice separation | Existing roadmap plus closure section | Review the rendered Markdown and run SpecRail checks. |
| P2: focused implementation ownership | GH1064 packet and roadmap closure section | Confirm linked ownership is descriptive and no child files/issues are changed. |
| P3: parent closure does not imply child completion | Explicit closure semantics | Independent review of the diff and PR body. |
| P4: future gaps use focused issues | Product invariant and roadmap guidance | Independent review. |
| P5: documentation-only behavior | Git diff scope | `git diff --name-only origin/main...HEAD` contains only `docs/plan/2026-07-18-best-gateway-gap-analysis.md` and the three files under `specs/GH1064/`. |

## Data Flow

There is no runtime data flow. The GitHub issue points to the repository
roadmap; the GH1064 packet defines its acceptance and closure contract; the
closing PR links both and closes only #1064.

## Alternatives Considered

- Keep #1064 open until every linked issue completes: rejected because it
  conflates roadmap planning with independently scoped implementation and
  creates no stable terminal for the planning deliverable.
- Implement one roadmap gap under #1064: rejected because each gap requires a
  focused issue/spec and several already have explicit owners.
- Close #1064 without repository changes: rejected because the missing closure
  semantics would remain implicit and the SpecRail coverage gap would persist.

## Risks

- Security: None; no runtime or security behavior changes.
- Compatibility: Readers could misread parent closure as product completion;
  explicit wording and independent review mitigate this.
- Performance: None.
- Maintenance: Ownership links can age; focused issues remain authoritative.

## Test Plan

- [ ] `python3 checks/check_workflow.py --repo .`
- [ ] `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1064`
- [ ] `python3 checks/route_gate.py --repo . --route implement --issue 1064 --state ready_to_implement --artifact product_spec=specs/GH1064/product.md --artifact tech_spec=specs/GH1064/tech.md --artifact task_plan=specs/GH1064/tasks.md --duplicate-evidence .specrail/runtime/evidence/duplicate-1064.json --json`
- [ ] `git diff --check`
- [ ] Independent exact-head documentation/spec review.
- [ ] Fresh PR CI, review-thread, merge-state, and PR-gate evidence.

## Rollback Plan

Revert the closing PR and reopen #1064 if the parent must again serve as the
active planning umbrella. Reverting does not affect any child issue or runtime
behavior.
