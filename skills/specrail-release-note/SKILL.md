---
name: specrail-release-note
description: Use when drafting a SpecRail release note after a linked PR has merged. Summarizes user-visible changes, verification, linked issues, risks, and rollout notes while preserving release and security human gates.
---

# SpecRail Release Note

Use this skill for the `draft_release_note` route.

## Steps

1. Confirm the PR is merged and identify the linked issue, commits, specs, and
   verification evidence.
2. Run the release-note route gate when available:

```sh
python3 checks/route_gate.py --repo . --route draft_release_note --issue <issue-number> --pr <pr-number> --state merged --json
```

3. Draft a concise release note in the selected locale.
4. Include user-visible change, linked work, verification, migration or rollback
   notes, and any known limitations.
5. Keep stable machine-facing IDs, paths, commands, and JSON keys in English.

## Boundaries

- Do not publish a release.
- Do not mark the release human gate complete.
- Do not include private security details in public notes.
- Do not claim closure for unverified issues or PRs.
