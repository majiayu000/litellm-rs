---
name: specrail-triage-issue
description: Use when triaging a GitHub issue or issue-like request in a SpecRail-governed repository. Handles search-first duplicate checks, issue classification, readiness label proposals, security-private routing, and triage handoffs without bypassing human gates.
---

# SpecRail Triage Issue

Use this skill for the `triage_issue` route.

## Steps

1. Read the active SpecRail contract: `AGENTS.md`, `AGENT_USAGE.md`,
   `workflow.yaml`, `states.yaml`, and `labels.yaml`.
2. Search existing issues, PRs, specs, and templates before creating or
   recommending new workflow artifacts.
3. Identify the current state: `new_issue`, `needs_info`, `triaged`,
   `duplicate`, `security_private`, or another configured state.
4. Run the local gate when available:

```sh
python3 checks/github_issue_evidence.py --github-repo <owner/repo> --issue <issue-number> --json > issue-evidence.json
python3 checks/route_gate.py --repo . --route triage_issue --issue <issue-number> --evidence issue-evidence.json --json
python3 checks/route_gate.py --repo . --route triage_issue --issue <issue-number> --state <state> --json
```

5. Treat `checks/github_issue_evidence.py` as a read-only collector. It may
   gather labels and state hints, but it must not write labels or comments.
6. Produce or update the triage result expected by the repository, usually
   `artifacts/triage/issue-<issue-number>.json`.
7. Propose labels only when evidence supports them. Keep label IDs and state IDs
   in English.
8. If the issue may involve private security details, stop public drafting and
   hand off to the maintainer security process.

## Boundaries

- Do not close disputed issues.
- Do not grant readiness, final approval, merge, or security-disclosure
  authority.
- Do not invent missing fields; report missing evidence as missing evidence.
- Keep human-facing triage text in the selected locale.
