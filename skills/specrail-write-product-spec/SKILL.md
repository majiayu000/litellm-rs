---
name: specrail-write-product-spec
description: Use when writing or updating a SpecRail product spec for a linked issue. Produces the numbered `product.md` spec from the locale-appropriate template, focusing on user-facing behavior, goals, non-goals, and acceptance criteria without implementation detail.
---

# SpecRail Write Product Spec

Use this skill for the product half of the `write_spec` route.

## Steps

1. Confirm the linked issue number. Search first if no issue is provided.
2. Read `workflow.yaml`, `states.yaml`, `labels.yaml`, and the relevant product
   spec template from `templates/<locale>/product_spec.md` or
   `templates/product_spec.md`.
3. Run the local gate when available:

```sh
python3 checks/route_gate.py --repo . --route write_spec --issue <issue-number> --state ready_to_spec --json
```

4. Write `specs/GH<issue-number>/product.md`.
5. Keep product content about observable behavior: goals, non-goals, behavior
   invariants, acceptance criteria, edge cases, and open questions.
6. Write behavior as numbered, testable invariants without implementation
   detail.
7. Keep implementation approach, file ownership, test commands, and rollout
   mechanics for the tech spec or task plan.

## Boundaries

- Do not write a numbered spec without a linked issue unless a human explicitly
  chooses a non-GitHub workflow.
- Do not translate stable IDs, paths, commands, JSON keys, states, or route
  names.
- Keep human-facing product text in the selected locale.
