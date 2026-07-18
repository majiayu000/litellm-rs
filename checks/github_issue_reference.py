"""Normalize auditable GitHub issue relations from PR metadata."""

from __future__ import annotations

import re
from typing import Any

from github_evidence_common import EvidenceError


PARTIAL_ISSUE_REFERENCE_PATTERN = re.compile(
    r"^[ ]{0,3}(?:[-*+][ \t]+)?refs[ \t]+#(?P<number>[1-9][0-9]*)"
    r"[ \t]*[.!]?[ \t]*\r?$",
    re.IGNORECASE | re.MULTILINE,
)
MARKDOWN_FENCE_PATTERN = re.compile(
    r"^[ ]{0,3}(?:[-*+][ \t]+)?(?P<fence>`{3,}|~{3,})"
)
BACKTICK_RUN_PATTERN = re.compile(r"`+")
INDENTED_CODE_PATTERN = re.compile(r"^(?: {4}|\t)")


def _positive_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def normalize_closing_issue_numbers(value: Any) -> list[int]:
    if not isinstance(value, list):
        raise EvidenceError("closingIssuesReferences must be a list")
    numbers: list[int] = []
    for index, item in enumerate(value, start=1):
        if not isinstance(item, dict):
            raise EvidenceError(
                f"closingIssuesReferences item #{index} must be an object"
            )
        number = item.get("number")
        if not _positive_int(number):
            raise EvidenceError(
                f"closingIssuesReferences item #{index}.number must be a positive integer"
            )
        if number in numbers:
            raise EvidenceError(
                f"closingIssuesReferences contains duplicate issue number {number}"
            )
        numbers.append(number)
    return numbers


def normalize_linked_issue(value: Any) -> int | None:
    numbers = normalize_closing_issue_numbers(value)
    return numbers[0] if numbers else None


def _matching_backtick_run(
    raw_line: str,
    cursor: int,
    delimiter_length: int,
) -> re.Match[str] | None:
    return next(
        (
            match
            for match in BACKTICK_RUN_PATTERN.finditer(raw_line, cursor)
            if len(match.group(0)) == delimiter_length
        ),
        None,
    )


def _visible_markdown_line(
    raw_line: str,
    in_comment: bool,
    inline_delimiter_length: int | None,
) -> tuple[str, bool, int | None]:
    visible: list[str] = []
    cursor = 0
    while cursor < len(raw_line):
        if inline_delimiter_length is not None:
            closing_run = _matching_backtick_run(
                raw_line,
                cursor,
                inline_delimiter_length,
            )
            if closing_run is None:
                break
            visible.append(closing_run.group(0))
            cursor = closing_run.end()
            inline_delimiter_length = None
            continue
        if in_comment:
            comment_end = raw_line.find("-->", cursor)
            if comment_end < 0:
                break
            cursor = comment_end + 3
            in_comment = False
            continue
        comment_start = raw_line.find("<!--", cursor)
        opening_run = BACKTICK_RUN_PATTERN.search(raw_line, cursor)
        if opening_run is not None and (
            comment_start < 0 or opening_run.start() < comment_start
        ):
            visible.append(raw_line[cursor : opening_run.end()])
            cursor = opening_run.end()
            delimiter_length = len(opening_run.group(0))
            closing_run = _matching_backtick_run(raw_line, cursor, delimiter_length)
            if closing_run is None:
                inline_delimiter_length = delimiter_length
                break
            visible.append(raw_line[cursor : closing_run.end()])
            cursor = closing_run.end()
            continue
        if comment_start < 0:
            visible.append(raw_line[cursor:])
            break
        visible.append(raw_line[cursor:comment_start])
        cursor = comment_start + 4
        in_comment = True
    return "".join(visible), in_comment, inline_delimiter_length


def references_partial_issue(body: str, issue_number: int) -> bool:
    if not isinstance(body, str):
        raise EvidenceError("PR body must be a string")
    if not _positive_int(issue_number):
        raise EvidenceError("expected issue must be a positive integer")

    in_comment = False
    fence: str | None = None
    inline_delimiter_length: int | None = None
    for raw_line in body.splitlines():
        if fence is not None:
            fence_match = MARKDOWN_FENCE_PATTERN.match(raw_line)
            if fence_match:
                token = fence_match.group("fence")
                suffix = raw_line[fence_match.end() :].strip()
                if token[0] == fence[0] and len(token) >= len(fence) and not suffix:
                    fence = None
            continue
        if not in_comment and inline_delimiter_length is None:
            fence_match = MARKDOWN_FENCE_PATTERN.match(raw_line)
            if fence_match:
                fence = fence_match.group("fence")
                continue
            if INDENTED_CODE_PATTERN.match(raw_line):
                continue
        line, in_comment, inline_delimiter_length = _visible_markdown_line(
            raw_line,
            in_comment,
            inline_delimiter_length,
        )
        match = PARTIAL_ISSUE_REFERENCE_PATTERN.fullmatch(line)
        if match and int(match.group("number")) == issue_number:
            return True
    return False


def normalize_issue_reference(
    pr_payload: dict[str, Any],
    expected_issue: int | None = None,
    issue_payload: dict[str, Any] | None = None,
) -> tuple[int | None, dict[str, Any] | None]:
    closing_issue_numbers = normalize_closing_issue_numbers(
        pr_payload.get("closingIssuesReferences")
    )
    if expected_issue is None:
        if not closing_issue_numbers:
            return None, None
        linked_issue = closing_issue_numbers[0]
        return linked_issue, {
            "number": linked_issue,
            "kind": "closing",
            "source": "closingIssuesReferences",
            "verified": True,
            "closing_issue_numbers": closing_issue_numbers,
        }

    if not _positive_int(expected_issue):
        raise EvidenceError("expected issue must be a positive integer")
    if expected_issue in closing_issue_numbers:
        return expected_issue, {
            "number": expected_issue,
            "kind": "closing",
            "source": "closingIssuesReferences",
            "verified": True,
            "closing_issue_numbers": closing_issue_numbers,
        }

    body = pr_payload.get("body")
    if not isinstance(body, str) or not references_partial_issue(body, expected_issue):
        raise EvidenceError(
            f"PR body must contain a standalone Refs #{expected_issue} directive"
        )
    if not isinstance(issue_payload, dict):
        raise EvidenceError("live issue evidence is required for a partial reference")
    live_number = issue_payload.get("number")
    if not _positive_int(live_number):
        raise EvidenceError("number must be a positive integer")
    if live_number != expected_issue:
        raise EvidenceError(
            f"live issue number {live_number} does not match expected issue {expected_issue}"
        )
    raw_state = issue_payload.get("state")
    if not isinstance(raw_state, str) or not raw_state.strip():
        raise EvidenceError("state must be a non-empty string")
    state = raw_state.strip().upper()
    if state != "OPEN":
        raise EvidenceError(
            f"partial reference target GH-{expected_issue} must be OPEN; got {state}"
        )
    raw_url = issue_payload.get("url")
    if not isinstance(raw_url, str) or not raw_url.strip():
        raise EvidenceError("url must be a non-empty string")
    return expected_issue, {
        "number": expected_issue,
        "kind": "partial",
        "source": "pr_body",
        "verified": True,
        "state": state,
        "url": raw_url.strip(),
        "closing_issue_numbers": closing_issue_numbers,
    }


def relation_snapshot(pr_payload: dict[str, Any]) -> tuple[str, tuple[int, ...]]:
    body = pr_payload.get("body")
    if not isinstance(body, str):
        raise EvidenceError("PR body must be a string")
    closing_issue_numbers = normalize_closing_issue_numbers(
        pr_payload.get("closingIssuesReferences")
    )
    return body, tuple(closing_issue_numbers)
