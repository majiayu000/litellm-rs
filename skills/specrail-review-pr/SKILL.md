---
name: specrail-review-pr
description: Use when performing an advisory SpecRail PR review. Checks linked issue/spec evidence, route gates, verification evidence, review-thread state, human-gate preservation, and implementation quality without granting final approval or merging.
---

# SpecRail Review PR

Use this skill for the `review_pr` route.

## Steps

1. Read the PR, linked issue, product spec, tech spec, task plan, and local diff.
2. Confirm the PR has current evidence for linked work, verification, CI, review
   state, and review threads when available.
3. Run the review route gate when available:

```sh
python3 checks/route_gate.py --repo . --route review_pr --issue <issue-number> --pr <pr-number> --state impl_pr_open --json
```

4. Inspect for behavioral regressions, missing acceptance coverage, test gaps,
   silent degradation, security risk, and human-gate bypasses.
5. Lead with findings ordered by severity and cite exact files or lines.
6. When producing a review artifact, use a top-level body with `## Summary` and
   `## Verdict`, keep inline comments bound to real diff `path` / `line` /
   `side` values, and only add `start_line` / `start_side` together for an
   inclusive diff range. Suggested changes must be non-empty and appear only on
   RIGHT-side comments, either through a `suggestion` field, a fenced
   `suggestion` block, or both.
7. Validate review artifacts against the diff when the gate exists:

```sh
python3 checks/review_json_gate.py --repo . --review artifacts/review/pr-<pr-number>.json --diff <patch> --json
```

8. If merge readiness is requested, route to
   `skills/specrail-pr-gate/SKILL.md`.

## Boundaries

- Treat the review as advisory.
- Do not grant final approval.
- Do not merge or mark human gates complete.
- Do not disclose private security details publicly.
