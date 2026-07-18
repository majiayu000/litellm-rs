---
name: specrail-diagnose-ci
description: Use when diagnosing or fixing CI failures in a SpecRail-governed repository. Collects fresh CI evidence, reproduces failures locally when possible, identifies root cause before fixing, and reports verification without claiming green CI from stale or missing data.
---

# SpecRail Diagnose CI

Use this skill for the `fix_ci` route.

## Steps

1. Collect the failing workflow, job, step, command, logs, PR head SHA, and base
   branch evidence.
2. Run the CI route gate when available:

```sh
python3 checks/route_gate.py --repo . --route fix_ci --issue <issue-number> --pr <pr-number> --state human_review --json
```

3. Reproduce the failure locally when the command is available.
4. Form one root-cause hypothesis, test it, then fix the smallest responsible
   code or workflow surface.
5. Run the failing command again after the fix.
6. Report fresh command output, remaining CI status, and any remote evidence
   that could not be verified.

## Boundaries

- Do not claim CI is green from stale, absent, or unrelated evidence.
- Do not make unrelated improvements while fixing CI.
- Do not bypass tests, weaken assertions, or hide failures.
- Do not merge without explicit human authorization and PR-gate evidence.
