# Agent First Review

Agent review is advisory. It must produce findings and evidence, not final
approval.

## Check

- Scope matches linked issue and spec.
- Implementation satisfies acceptance criteria.
- Tests cover changed behavior.
- User-visible behavior is not silently degraded.
- Security-sensitive areas are identified.
- PR template is complete.
- Release note need is explicit.

## Output

Return advisory structured findings. For simple chat review, use:

```json
{
  "verdict": "findings|no_findings",
  "blocking_findings": [],
  "non_blocking_findings": [],
  "missing_evidence": [],
  "recommended_human_reviewers": []
}
```

For a file artifact that should be checked before posting or handoff, use
`schemas/review_result.schema.json` and validate it against the diff:

```sh
python3 checks/review_json_gate.py --repo . --review artifacts/review/pr-<pr-number>.json --diff <patch> --json
```

Inline comments must reference real diff `path`, `line`, and `side` values.
Severity must be `critical`, `important`, `suggestion`, or `nit`.

Top-level `body` must include `## Summary` and `## Verdict` headings. A
multi-line inline comment may add `start_line` and `start_side`, but those fields
must appear together and every line in the inclusive range must exist in the
diff. Suggested changes may use a non-empty `suggestion` field, a non-empty
fenced `suggestion` block in `body`, or both; suggestions are only valid on
RIGHT-side comments.

## Boundary

Do not approve, merge, close issues, or mark security findings public.
