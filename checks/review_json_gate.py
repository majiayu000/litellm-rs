#!/usr/bin/env python3
"""Validate advisory review JSON artifacts against a pull request diff."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


VERDICTS = {"APPROVE", "REJECT"}
SIDES = {"RIGHT", "LEFT"}
SEVERITIES = {"critical", "important", "suggestion", "nit"}
SPEC_ALIGNMENT_STATUSES = {"matched", "drift", "not_applicable"}
REVIEW_MODES = {"full", "resumed", "diff_only"}
PRIOR_FINDING_STATUSES = {"resolved", "unresolved", "obsolete"}
FULL_REVIEW_ROUND_CAP = 2
FORBIDDEN_FINAL_AUTHORITY = {
    "approved for merge": re.compile(r"\bapproved\s+for\s+merge\b", re.IGNORECASE),
    "I approve this PR": re.compile(r"\bi\s+approve\s+this\s+pr\b", re.IGNORECASE),
    "merge now": re.compile(r"\bmerge\s+now\b", re.IGNORECASE),
    "ready to merge": re.compile(r"\bready\s+to\s+merge\b", re.IGNORECASE),
    "you can merge": re.compile(r"\byou\s+can\s+merge\b", re.IGNORECASE),
    "go ahead and merge": re.compile(r"\bgo\s+ahead\s+and\s+merge\b", re.IGNORECASE),
    "looks good to merge": re.compile(r"\blooks\s+good\s+to\s+merge\b", re.IGNORECASE),
    "safe to merge": re.compile(r"\bsafe\s+to\s+merge\b", re.IGNORECASE),
    "LGTM, merge": re.compile(r"\blgtm\b[^.\\n]{0,40}\bmerge\b", re.IGNORECASE),
    "ship it": re.compile(r"\bship\s+it\b", re.IGNORECASE),
}
HUNK_RE = re.compile(
    r"^@@ -(?P<old_start>[0-9]+)(?:,[0-9]+)? "
    r"\+(?P<new_start>[0-9]+)(?:,[0-9]+)? @@"
)
SUGGESTION_FENCE_RE = re.compile(
    r"(?:^|\n)```suggestion[^\n]*\n(?P<content>.*?)\n```",
    re.DOTALL,
)
SUGGESTION_OPEN_RE = re.compile(r"(?:^|\n)```suggestion[^\n]*\n")
SUMMARY_HEADING_RE = re.compile(r"^## Summary\s*$", re.MULTILINE)
VERDICT_HEADING_RE = re.compile(r"^## Verdict\s*$", re.MULTILINE)


@dataclass(frozen=True)
class DiffIndex:
    left: dict[str, set[int]]
    right: dict[str, set[int]]


def _non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _positive_int(value: Any) -> bool:
    return isinstance(value, int) and value > 0


def _load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise ValueError(f"cannot read review file {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid review JSON {path}: {exc.msg}") from exc
    if not isinstance(data, dict):
        raise ValueError("review JSON must be an object")
    return data


def _load_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ValueError(f"cannot read diff file {path}: {exc}") from exc


def _resolve_path(repo: Path, raw_path: str) -> Path:
    path = Path(raw_path)
    if path.is_absolute():
        return path
    return repo / path


def _clean_diff_path(raw_path: str) -> str | None:
    path = raw_path.strip().split("\t", 1)[0]
    if path == "/dev/null":
        return None
    if path.startswith("a/") or path.startswith("b/"):
        path = path[2:]
    return path or None


def _add_line(lines: dict[str, set[int]], path: str | None, line: int) -> None:
    if path is None or line <= 0:
        return
    lines.setdefault(path, set()).add(line)


def parse_unified_diff(diff_text: str) -> DiffIndex:
    """Index old/new line numbers present in a unified diff."""

    left: dict[str, set[int]] = {}
    right: dict[str, set[int]] = {}
    old_path: str | None = None
    new_path: str | None = None
    old_line = 0
    new_line = 0
    in_hunk = False

    for raw_line in diff_text.splitlines():
        if raw_line.startswith("diff --git "):
            in_hunk = False
            old_path = None
            new_path = None
            continue
        if not in_hunk and raw_line.startswith("--- "):
            in_hunk = False
            old_path = _clean_diff_path(raw_line[4:])
            continue
        if not in_hunk and raw_line.startswith("+++ "):
            in_hunk = False
            new_path = _clean_diff_path(raw_line[4:])
            continue

        hunk = HUNK_RE.match(raw_line)
        if hunk:
            if old_path is None and new_path is None:
                raise ValueError("diff hunk is missing file paths")
            old_line = int(hunk.group("old_start"))
            new_line = int(hunk.group("new_start"))
            in_hunk = True
            continue

        if not in_hunk:
            continue

        if raw_line.startswith(" "):
            _add_line(left, old_path, old_line)
            _add_line(right, new_path, new_line)
            old_line += 1
            new_line += 1
        elif raw_line.startswith("-"):
            _add_line(left, old_path, old_line)
            old_line += 1
        elif raw_line.startswith("+"):
            _add_line(right, new_path, new_line)
            new_line += 1
        elif raw_line.startswith("\\"):
            continue
        else:
            raise ValueError(f"unsupported diff line inside hunk: {raw_line!r}")

    return DiffIndex(left=left, right=right)


def _validate_top_level(review: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []
    allowed_keys = {
        "verdict",
        "body",
        "comments",
        "spec_alignment",
        "pr",
        "head_sha",
        "review_round",
        "review_mode",
        "base_head_sha",
        "human_full_review_request",
        "prior_findings",
    }

    for key in sorted(set(review) - allowed_keys):
        reasons.append(f"unknown top-level field: {key}")

    verdict = review.get("verdict")
    if verdict in VERDICTS:
        satisfied.append(f"verdict: {verdict}")
    elif "verdict" in review:
        reasons.append(f"verdict must be APPROVE or REJECT; got {verdict!r}")
    else:
        missing.append("verdict")

    body = review.get("body")
    if _non_empty_string(body):
        satisfied.append("body present")
        if SUMMARY_HEADING_RE.search(body):
            satisfied.append("body includes ## Summary")
        else:
            reasons.append("body must include ## Summary heading")
        if VERDICT_HEADING_RE.search(body):
            satisfied.append("body includes ## Verdict")
        else:
            reasons.append("body must include ## Verdict heading")
    elif "body" in review:
        reasons.append("body must be a non-empty string")
    else:
        missing.append("body")

    comments = review.get("comments")
    if isinstance(comments, list):
        satisfied.append(f"comments: {len(comments)}")
    elif "comments" in review:
        reasons.append("comments must be a list")
    else:
        missing.append("comments")

    if "spec_alignment" in review:
        _validate_spec_alignment(review["spec_alignment"], satisfied, reasons)

    _validate_review_round(review, satisfied, reasons)

    return satisfied, missing, reasons


def _validate_review_round(
    review: dict[str, Any], satisfied: list[str], reasons: list[str]
) -> None:
    has_round = "review_round" in review
    has_mode = "review_mode" in review
    if not has_round and not has_mode:
        return
    if has_round != has_mode:
        reasons.append("review_round and review_mode must be provided together")
        return

    review_round = review.get("review_round")
    if not _positive_int(review_round):
        reasons.append("review_round must be a positive integer")
        return

    review_mode = review.get("review_mode")
    if review_mode not in REVIEW_MODES:
        allowed = ", ".join(sorted(REVIEW_MODES))
        reasons.append(f"review_mode must be one of: {allowed}")
        return

    if review_mode == "full" and review_round > FULL_REVIEW_ROUND_CAP:
        request = review.get("human_full_review_request")
        if _non_empty_string(request):
            satisfied.append(
                f"round {review_round} full review authorized by human request"
            )
        else:
            reasons.append(
                f"review_round {review_round} with review_mode full exceeds the "
                f"cap of {FULL_REVIEW_ROUND_CAP}; use resumed/diff_only or record "
                "human_full_review_request"
            )

    if review_mode in {"resumed", "diff_only"}:
        if review_round < 2:
            reasons.append(f"review_mode {review_mode} requires review_round >= 2")
        _validate_prior_findings(review, reasons)

    if review_mode == "diff_only" and not _non_empty_string(review.get("base_head_sha")):
        reasons.append("review_mode diff_only requires base_head_sha of the prior round")

    if review_mode in REVIEW_MODES and not reasons:
        satisfied.append(f"review round {review_round} mode {review_mode}")


def _validate_prior_findings(review: dict[str, Any], reasons: list[str]) -> None:
    prior_findings = review.get("prior_findings")
    if not isinstance(prior_findings, list):
        reasons.append(
            "resumed/diff_only rounds require prior_findings[] with per-finding status"
        )
        return
    for index, finding in enumerate(prior_findings, start=1):
        if not isinstance(finding, dict):
            reasons.append(f"prior_findings #{index} must be an object")
            continue
        if not _non_empty_string(finding.get("summary")):
            reasons.append(f"prior_findings #{index} requires summary")
        status = finding.get("status")
        if status not in PRIOR_FINDING_STATUSES:
            allowed = ", ".join(sorted(PRIOR_FINDING_STATUSES))
            reasons.append(f"prior_findings #{index} status must be one of: {allowed}")


def _validate_spec_alignment(
    value: Any, satisfied: list[str], reasons: list[str]
) -> None:
    if not isinstance(value, dict):
        reasons.append("spec_alignment must be an object")
        return

    allowed_keys = {"status", "spec", "details"}
    for key in sorted(set(value) - allowed_keys):
        reasons.append(f"spec_alignment has unknown field: {key}")

    status = value.get("status")
    if status not in SPEC_ALIGNMENT_STATUSES:
        reasons.append(f"spec_alignment.status must be matched, drift, or not_applicable; got {status!r}")
    elif status == "drift":
        reasons.append("spec_alignment reports drift")
    else:
        satisfied.append(f"spec_alignment: {status}")

    spec = value.get("spec")
    if spec is not None and not _non_empty_string(spec):
        reasons.append("spec_alignment.spec must be a non-empty string or null")

    details = value.get("details")
    if details is not None and not _non_empty_string(details):
        reasons.append("spec_alignment.details must be a non-empty string")


def _validate_comment_shape(comment: Any, index: int) -> tuple[dict[str, Any] | None, list[str], list[str]]:
    missing: list[str] = []
    reasons: list[str] = []
    if not isinstance(comment, dict):
        return None, missing, [f"comment #{index} must be an object"]

    allowed_keys = {
        "path",
        "line",
        "side",
        "severity",
        "body",
        "start_line",
        "start_side",
        "suggestion",
    }
    for key in sorted(set(comment) - allowed_keys):
        reasons.append(f"comment #{index} has unknown field: {key}")

    for key in ["path", "line", "side", "severity", "body"]:
        if key not in comment:
            missing.append(f"comments[{index - 1}].{key}")

    path = comment.get("path")
    if "path" in comment and not _non_empty_string(path):
        reasons.append(f"comment #{index} path must be a non-empty string")
    if isinstance(path, str) and (Path(path).is_absolute() or ".." in Path(path).parts):
        reasons.append(f"comment #{index} path must be a relative repository path")

    line = comment.get("line")
    if "line" in comment and not _positive_int(line):
        reasons.append(f"comment #{index} line must be a positive integer")

    side = comment.get("side")
    if "side" in comment and side not in SIDES:
        reasons.append(f"comment #{index} side must be RIGHT or LEFT; got {side!r}")

    has_start_line = "start_line" in comment
    has_start_side = "start_side" in comment
    if has_start_line != has_start_side:
        reasons.append(f"comment #{index} start_line and start_side must appear together")

    start_line = comment.get("start_line")
    if has_start_line and not _positive_int(start_line):
        reasons.append(f"comment #{index} start_line must be a positive integer")

    start_side = comment.get("start_side")
    if has_start_side and start_side not in SIDES:
        reasons.append(f"comment #{index} start_side must be RIGHT or LEFT; got {start_side!r}")

    severity = comment.get("severity")
    if "severity" in comment and severity not in SEVERITIES:
        reasons.append(
            f"comment #{index} severity must be critical, important, suggestion, or nit; got {severity!r}"
        )

    if "body" in comment and not _non_empty_string(comment.get("body")):
        reasons.append(f"comment #{index} body must be a non-empty string")

    if "suggestion" in comment and not _non_empty_string(comment.get("suggestion")):
        reasons.append(f"comment #{index} suggestion must be a non-empty string")

    return comment, missing, reasons


def _check_diff_location(
    comment: dict[str, Any], index: int, diff_index: DiffIndex
) -> str | None:
    return _check_diff_line(
        comment.get("path"),
        comment.get("line"),
        comment.get("side"),
        index,
        diff_index,
    )


def _line_set_for_side(diff_index: DiffIndex, side: str) -> dict[str, set[int]]:
    return diff_index.right if side == "RIGHT" else diff_index.left


def _check_diff_line(
    path: Any, line: Any, side: Any, index: int, diff_index: DiffIndex, label: str = ""
) -> str | None:
    if not (_non_empty_string(path) and _positive_int(line) and side in SIDES):
        return None

    side_lines = _line_set_for_side(diff_index, side)
    if str(path) not in side_lines:
        return f"comment #{index} {side} {path} is not present in the diff"
    if int(line) in side_lines[str(path)]:
        return None
    label_suffix = f" {label}" if label else ""
    return f"comment #{index} {side} {path}:{line}{label_suffix} is not present in the diff"


def _check_diff_range(
    comment: dict[str, Any], index: int, diff_index: DiffIndex
) -> list[str]:
    if "start_line" not in comment and "start_side" not in comment:
        return []
    if "start_line" not in comment or "start_side" not in comment:
        return []

    path = comment.get("path")
    start_line = comment.get("start_line")
    start_side = comment.get("start_side")
    line = comment.get("line")
    side = comment.get("side")
    if not (
        _non_empty_string(path)
        and _positive_int(start_line)
        and start_side in SIDES
        and _positive_int(line)
        and side in SIDES
    ):
        return []

    reasons: list[str] = []
    start_reason = _check_diff_line(
        path, start_line, start_side, index, diff_index, "range start"
    )
    if start_reason:
        reasons.append(start_reason)

    if start_side != side:
        reasons.append(f"comment #{index} start_side must match side for a range")
        return reasons

    if int(start_line) > int(line):
        reasons.append(f"comment #{index} start_line must be <= line for a {side} range")
        return reasons

    side_lines = _line_set_for_side(diff_index, side)
    path_lines = side_lines.get(str(path), set())
    missing = [
        candidate
        for candidate in range(int(start_line), int(line) + 1)
        if candidate not in path_lines
    ]
    if missing:
        preview = ", ".join(str(candidate) for candidate in missing[:5])
        if len(missing) > 5:
            preview = f"{preview}, ..."
        reasons.append(
            f"comment #{index} {side} {path}:{start_line}-{line} includes "
            f"lines not present in the diff: {preview}"
        )
    return reasons


def _suggestion_blocks(body: Any) -> list[str]:
    if not isinstance(body, str):
        return []
    return [match.group("content") for match in SUGGESTION_FENCE_RE.finditer(body)]


def _unterminated_suggestion_count(body: Any) -> int:
    if not isinstance(body, str):
        return 0
    return len(SUGGESTION_OPEN_RE.findall(body)) - len(SUGGESTION_FENCE_RE.findall(body))


def _validate_suggestions(comment: dict[str, Any], index: int) -> list[str]:
    reasons: list[str] = []
    has_suggestion = "suggestion" in comment
    blocks = _suggestion_blocks(comment.get("body"))
    unterminated_count = _unterminated_suggestion_count(comment.get("body"))

    for block_index, content in enumerate(blocks, start=1):
        if not content.strip():
            reasons.append(f"comment #{index} suggestion block #{block_index} must be non-empty")
    if unterminated_count > 0:
        reasons.append(f"comment #{index} has unterminated suggestion block")

    if not has_suggestion and not blocks and unterminated_count == 0:
        return reasons

    if comment.get("side") != "RIGHT" or (
        "start_side" in comment and comment.get("start_side") != "RIGHT"
    ):
        reasons.append(f"comment #{index} suggestions are only allowed on RIGHT-side comments")
    return reasons


def _iter_review_strings(review: dict[str, Any]) -> list[tuple[str, str]]:
    values: list[tuple[str, str]] = []
    if isinstance(review.get("body"), str):
        values.append(("body", review["body"]))

    comments = review.get("comments")
    if isinstance(comments, list):
        for index, comment in enumerate(comments, start=1):
            if isinstance(comment, dict) and isinstance(comment.get("body"), str):
                values.append((f"comments[{index - 1}].body", comment["body"]))

    spec_alignment = review.get("spec_alignment")
    if isinstance(spec_alignment, dict):
        for key, value in spec_alignment.items():
            if isinstance(value, str):
                values.append((f"spec_alignment.{key}", value))
    return values


def _find_forbidden_language(review: dict[str, Any]) -> list[str]:
    reasons: list[str] = []
    for path, value in _iter_review_strings(review):
        for label, pattern in FORBIDDEN_FINAL_AUTHORITY.items():
            if pattern.search(value):
                reasons.append(f"{path} grants final approval or merge authority: {label!r}")
    return reasons


def evaluate_review_gate(review: dict[str, Any], diff_text: str) -> dict[str, Any]:
    """Validate a review artifact and return a stable gate result."""

    reasons: list[str] = []
    satisfied: list[str] = []
    missing: list[str] = []

    top_satisfied, top_missing, top_reasons = _validate_top_level(review)
    satisfied.extend(top_satisfied)
    missing.extend(top_missing)
    reasons.extend(top_reasons)
    reasons.extend(_find_forbidden_language(review))

    try:
        diff_index = parse_unified_diff(diff_text)
    except ValueError as exc:
        diff_index = DiffIndex(left={}, right={})
        reasons.append(str(exc))

    comments = review.get("comments")
    if isinstance(comments, list):
        for index, comment in enumerate(comments, start=1):
            shaped, comment_missing, comment_reasons = _validate_comment_shape(comment, index)
            missing.extend(comment_missing)
            reasons.extend(comment_reasons)
            if shaped is None:
                continue
            location_reason = _check_diff_location(shaped, index, diff_index)
            if location_reason:
                reasons.append(location_reason)
            reasons.extend(_check_diff_range(shaped, index, diff_index))
            reasons.extend(_validate_suggestions(shaped, index))

    decision = "blocked" if reasons or missing else "allowed"
    return {
        "decision": decision,
        "verdict": review.get("verdict"),
        "comment_count": len(comments) if isinstance(comments, list) else 0,
        "advisory_only": True,
        "reasons": sorted(set(reasons)),
        "satisfied": sorted(set(satisfied)),
        "missing": sorted(set(missing)),
        "blocked_actions": ["final_approval", "merge"],
        "verification_commands": [
            "python3 checks/review_json_gate.py --repo . --review <review.json> --diff <patch>",
            "python3 checks/check_workflow.py --repo .",
        ],
    }


def print_review_gate_human(result: dict[str, Any]) -> None:
    print(f"decision: {result['decision']}")
    if result.get("verdict"):
        print(f"verdict: {result['verdict']}")
    print("advisory_only: true")
    if result["reasons"]:
        print("reasons:")
        for reason in result["reasons"]:
            print(f"- {reason}")
    if result["missing"]:
        print("missing:")
        for item in result["missing"]:
            print(f"- {item}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate a SpecRail advisory review JSON artifact."
    )
    parser.add_argument("--repo", default=".", help="Workflow pack root")
    parser.add_argument("--review", required=True, help="Review artifact JSON file")
    parser.add_argument("--diff", required=True, help="Unified diff patch file")
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    args = parser.parse_args()

    repo = Path(args.repo).resolve()
    try:
        review = _load_json(_resolve_path(repo, args.review))
        diff_text = _load_text(_resolve_path(repo, args.diff))
        result = evaluate_review_gate(review, diff_text)
    except ValueError as exc:
        result = {
            "decision": "blocked",
            "verdict": None,
            "comment_count": 0,
            "advisory_only": True,
            "reasons": [str(exc)],
            "satisfied": [],
            "missing": [],
            "blocked_actions": ["final_approval", "merge"],
            "verification_commands": [
                "python3 checks/review_json_gate.py --repo . --review <review.json> --diff <patch>"
            ],
        }

    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print_review_gate_human(result)

    return 1 if result["decision"] == "blocked" else 0


if __name__ == "__main__":
    sys.exit(main())
