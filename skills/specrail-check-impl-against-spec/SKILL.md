---
name: specrail-check-impl-against-spec
description: Use when comparing a SpecRail implementation, diff, or PR against its linked issue, product spec, technical spec, and task plan. Reports acceptance coverage, mismatches, omitted tasks, extra scope, and verification gaps without approving or merging.
---

# SpecRail Check Implementation Against Spec

Use this skill when the question is whether implementation matches the spec.

## Steps

1. Read the linked issue, `product.md`, `tech.md`, `tasks.md`, and the diff or
   PR under review.
2. Map every acceptance criterion and task ID to implementation evidence,
   verification evidence, or a missing item.
3. Identify extra behavior not requested by the spec.
4. Check that stable IDs, paths, JSON keys, states, and commands remain in
   English.
5. Report results as:
   - covered
   - missing
   - mismatched
   - extra scope
   - needs human decision
6. Recommend the smallest corrective action for each gap.

## Boundaries

- Do not treat partial coverage as approval.
- Do not rewrite the spec to match an implementation unless the user asks for a
  spec revision.
- Do not merge or provide final approval.
