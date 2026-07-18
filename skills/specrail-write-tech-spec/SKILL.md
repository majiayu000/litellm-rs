---
name: specrail-write-tech-spec
description: Use when writing or updating a SpecRail technical spec for a linked issue. Produces the numbered `tech.md` spec from the locale-appropriate template, translating accepted product behavior into design, affected files, risks, verification, and rollback without starting implementation.
---

# SpecRail Write Tech Spec

Use this skill for the technical half of the `write_spec` route.

## Steps

1. Read the linked issue and `specs/GH<issue-number>/product.md`.
2. Read the relevant tech spec template from
   `templates/<locale>/tech_spec.md` or `templates/tech_spec.md`.
3. Run the local gate when available:

```sh
python3 checks/route_gate.py --repo . --route write_spec --issue <issue-number> --state ready_to_spec --json
```

4. Write `specs/GH<issue-number>/tech.md`.
5. Ground the plan in codebase context: relevant files, current behavior, and
   why each area changes or stays unchanged.
6. Map each product behavior invariant to implementation area and verification.
7. Describe the proposed design, data flow, touched components, edge cases,
   migration or compatibility risks, verification plan, and rollback plan.
8. Name deterministic checks that prove the design was implemented.

## Boundaries

- Do not start implementation from the tech spec step.
- Do not claim spec approval; report missing approval as a human gate.
- Preserve dry-run and advisory defaults for automation.
- Keep stable machine-facing IDs in English.
