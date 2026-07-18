---
name: specrail-plan-tasks
description: Use when turning an approved SpecRail product and technical spec into the numbered `tasks.md` plan. Creates stable task IDs, owners, done-when conditions, verification commands, dependencies, and handoff notes without implementing the tasks.
---

# SpecRail Plan Tasks

Use this skill to create or update the task plan before implementation.

## Steps

1. Read `specs/GH<issue-number>/product.md` and
   `specs/GH<issue-number>/tech.md`.
2. Read `templates/<locale>/tasks.md` or `templates/tasks.md`.
3. Run the implementation route gate when available:

```sh
python3 checks/route_gate.py --repo . --route implement --issue <issue-number> --state ready_to_implement --json
```

4. Write `specs/GH<issue-number>/tasks.md`.
5. Use stable task IDs such as `SP<issue-number>-T1`.
6. For every task, include owner, dependencies, done-when evidence, and verify
   commands.
7. Separate implementation tasks from verification and handoff notes.

## Boundaries

- Do not implement while planning tasks.
- Do not remove human gates for readiness, spec approval, final review, merge,
  release, or security decisions.
- Keep the plan small enough for one agent or a clearly partitioned thread lane.
