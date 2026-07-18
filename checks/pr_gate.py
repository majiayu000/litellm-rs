#!/usr/bin/env python3
"""Evaluate deterministic PR merge-readiness evidence.

The gate is intentionally offline. GitHub or threads adapters may collect the
evidence JSON, but this script only evaluates it and never writes remote state.
"""

from __future__ import annotations

import argparse
from datetime import datetime
import json
import sys
from pathlib import Path
from typing import Any


CHECK_PASS_CONCLUSIONS = {"SUCCESS"}
CLEAN_MERGE_STATES = {"CLEAN"}
ACTIVE_CHANGE_REQUESTS = {"CHANGES_REQUESTED"}
ALLOWED_RESOLVER_ROLES = {"reviewer_lane", "human"}
INDEPENDENT_REVIEW_SOURCES = {"independent_lane"}
MERGE_PATHS = {"gh_pr_merge", "api_fallback", "merged_by_other"}
KNOWN_REVIEW_SOURCES = {"independent_lane", "self_review"}
BLOCKED_RESOLVER_ROLES = {"implementer", "orchestrator", "coordinator", "unknown"}
REVIEW_SOURCES = {"independent_lane", "self_review"}
LANE_FAILURE_KINDS = {"usage_limit", "crash", "zero_output", "closed", "other"}


def _as_bool(value: Any) -> bool:
    return value is True


def _non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _positive_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def _parse_timestamp(value: str, field: str) -> datetime | None:
    normalized = value.replace("Z", "+00:00")
    try:
        parsed = datetime.fromisoformat(normalized)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        # Naive timestamps cannot be safely compared with the timezone-aware
        # values the evidence collector emits; treat them as unparseable.
        return None
    return parsed


def _load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise ValueError(f"cannot read evidence file {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid evidence JSON {path}: {exc.msg}") from exc
    if not isinstance(data, dict):
        raise ValueError("evidence JSON must be an object")
    return data


def _check_items(evidence: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    checks = evidence.get("checks")
    if not isinstance(checks, list) or not checks:
        missing.append("checks")
        reasons.append("CI/check evidence is missing")
        return satisfied, missing, reasons

    for index, item in enumerate(checks, start=1):
        if not isinstance(item, dict):
            reasons.append(f"check #{index} is not an object")
            continue
        name = str(item.get("name") or f"check #{index}")
        status = str(item.get("status") or "").upper()
        conclusion = str(item.get("conclusion") or "").upper()
        if status != "COMPLETED":
            reasons.append(f"{name} is not completed: {status or 'missing status'}")
            continue
        if conclusion not in CHECK_PASS_CONCLUSIONS:
            reasons.append(f"{name} did not pass: {conclusion or 'missing conclusion'}")
            continue
        satisfied.append(f"check passed: {name}")
    return satisfied, missing, reasons


def _review_items(evidence: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    reviews = evidence.get("reviews", [])
    if reviews is None:
        reviews = []
    if not isinstance(reviews, list):
        reasons.append("reviews must be a list")
        return satisfied, missing, reasons

    for index, review in enumerate(reviews, start=1):
        if not isinstance(review, dict):
            reasons.append(f"review #{index} is not an object")
            continue
        state = str(review.get("state") or "").upper()
        author = review.get("author") or f"review #{index}"
        if state in ACTIVE_CHANGE_REQUESTS:
            reasons.append(f"changes requested by {author}")
    if not any(reason.startswith("changes requested") for reason in reasons):
        satisfied.append("no active changes-requested review evidence")
    return satisfied, missing, reasons


def _thread_items(evidence: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    threads = evidence.get("review_threads")
    if not isinstance(threads, list):
        missing.append("review_threads")
        reasons.append("review thread evidence is missing")
        return satisfied, missing, reasons

    unresolved = []
    for index, thread in enumerate(threads, start=1):
        if not isinstance(thread, dict):
            unresolved.append(f"thread #{index}")
            continue
        is_resolved = _as_bool(thread.get("is_resolved"))
        identifier = str(thread.get("url") or thread.get("id") or f"thread #{index}")
        if not is_resolved:
            unresolved.append(identifier)
            continue

        resolved_by = thread.get("resolved_by")
        resolver_role = thread.get("resolver_role")
        if not _non_empty_string(resolved_by):
            missing.append(f"review_threads[{index}].resolved_by")
        if not _non_empty_string(resolver_role):
            missing.append(f"review_threads[{index}].resolver_role")
            continue

        role = str(resolver_role).strip()
        if role in ALLOWED_RESOLVER_ROLES:
            satisfied.append(f"review thread resolved by {role}: {identifier}")
        elif role in BLOCKED_RESOLVER_ROLES:
            reasons.append(f"review thread resolved by forbidden {role}: {identifier}")
        else:
            reasons.append(f"review thread resolver_role is unsupported: {role}")

    if unresolved:
        reasons.append("unresolved review threads: " + ", ".join(unresolved))
    else:
        satisfied.append("no unresolved active review threads")
    return satisfied, missing, reasons


def _review_source_items(evidence: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    review_source = evidence.get("review_source")
    if not _non_empty_string(review_source):
        missing.append("review_source")
    elif review_source in INDEPENDENT_REVIEW_SOURCES:
        satisfied.append(f"review_source: {review_source}")
    elif review_source in KNOWN_REVIEW_SOURCES:
        reasons.append(
            "review_source self_review does not satisfy the independent-review requirement"
        )
    else:
        allowed = ", ".join(sorted(KNOWN_REVIEW_SOURCES))
        reasons.append(f"review_source must be one of: {allowed}")

    return satisfied, missing, reasons


def _issue_reference_items(
    evidence: dict[str, Any],
) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    if "issue_reference" not in evidence:
        return satisfied, missing, reasons
    relation = evidence["issue_reference"]
    if not isinstance(relation, dict):
        reasons.append("issue_reference must be an object")
        return satisfied, missing, reasons

    allowed_fields = {
        "number",
        "kind",
        "source",
        "verified",
        "state",
        "url",
        "closing_issue_numbers",
    }
    unknown_fields = sorted(set(relation) - allowed_fields)
    if unknown_fields:
        reasons.append(
            "issue_reference contains unsupported fields: " + ", ".join(unknown_fields)
        )

    number = relation.get("number")
    if not _positive_int(number):
        missing.append("issue_reference.number")
    elif number != evidence.get("linked_issue"):
        reasons.append("issue_reference.number must match linked_issue")

    if relation.get("verified") is not True:
        reasons.append("issue_reference.verified must be true")

    closing_issue_numbers = relation.get("closing_issue_numbers")
    valid_closing_numbers = isinstance(closing_issue_numbers, list) and all(
        _positive_int(item) for item in closing_issue_numbers
    )
    if not valid_closing_numbers:
        reasons.append("issue_reference.closing_issue_numbers must be a list of positive integers")
        closing_issue_numbers = []
    elif len(set(closing_issue_numbers)) != len(closing_issue_numbers):
        reasons.append("issue_reference.closing_issue_numbers must not contain duplicates")

    kind = relation.get("kind")
    source = relation.get("source")
    if kind == "partial":
        if source != "pr_body":
            reasons.append("issue_reference partial source must be pr_body")
        if relation.get("state") != "OPEN":
            reasons.append("issue_reference partial state must be OPEN")
        if _positive_int(number) and number in closing_issue_numbers:
            reasons.append("issue_reference partial target must not be closing")
    elif kind == "closing":
        if source != "closingIssuesReferences":
            reasons.append(
                "issue_reference closing source must be closingIssuesReferences"
            )
        if _positive_int(number) and number not in closing_issue_numbers:
            reasons.append(
                "issue_reference closing target must appear in closing_issue_numbers"
            )
    else:
        reasons.append("issue_reference.kind must be one of: closing, partial")

    if not missing and not reasons:
        satisfied.append(f"issue_reference: verified {kind} GH-{number}")
    return satisfied, missing, reasons


def _merge_record_items(evidence: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    record = evidence.get("merge_record")
    if record is None:
        return satisfied, missing, reasons
    if not isinstance(record, dict):
        reasons.append("merge_record must be an object")
        return satisfied, missing, reasons

    merge_path = record.get("merge_path")
    if not _non_empty_string(merge_path):
        missing.append("merge_record.merge_path")
    elif merge_path not in MERGE_PATHS:
        allowed = ", ".join(sorted(MERGE_PATHS))
        reasons.append(f"merge_record.merge_path must be one of: {allowed}")
    else:
        satisfied.append(f"merge_path: {merge_path}")

    if record.get("remote_confirmed") is not True:
        reasons.append(
            "merge_record.remote_confirmed must be true: confirm the merge "
            "outcome via a remote query (gh pr view --json merged,mergeCommit) "
            "before recording success or failure"
        )
    elif not _non_empty_string(record.get("merge_commit_sha")):
        missing.append("merge_record.merge_commit_sha")
    else:
        satisfied.append(f"merge remotely confirmed: {record['merge_commit_sha']}")

    outcome = record.get("branch_deletion_outcome")
    if outcome is not None and not _non_empty_string(outcome):
        reasons.append("merge_record.branch_deletion_outcome must be a non-empty string or null")

    return satisfied, missing, reasons


def _authorization_item(evidence: dict[str, Any]) -> tuple[list[str], list[str]]:
    authorization = evidence.get("human_authorization")
    if not isinstance(authorization, dict):
        return [], ["human_authorization"]
    missing = []
    for key in ["actor", "source"]:
        if not _non_empty_string(authorization.get(key)):
            missing.append(f"human_authorization.{key}")
    if missing:
        return [], missing
    return [f"human authorization from {authorization['actor']} via {authorization['source']}"], []


def _self_review_authorization_item(evidence: dict[str, Any]) -> tuple[list[str], list[str]]:
    authorization = evidence.get("self_review_authorization")
    if not isinstance(authorization, dict):
        return [], ["self_review_authorization"]
    missing = []
    for key in ["actor", "source", "scope"]:
        if not _non_empty_string(authorization.get(key)):
            missing.append(f"self_review_authorization.{key}")
    if missing:
        return [], missing
    return [
        "self-review authorization from "
        f"{authorization['actor']} via {authorization['source']}"
    ], []


def _review_source_items(evidence: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    source = evidence.get("review_source")
    if not _non_empty_string(source):
        missing.append("review_source")
        reasons.append("review_source evidence is missing")
        return satisfied, missing, reasons

    normalized = str(source).strip()
    if normalized not in REVIEW_SOURCES:
        allowed = ", ".join(sorted(REVIEW_SOURCES))
        reasons.append(f"review_source must be one of: {allowed}")
        return satisfied, missing, reasons

    if normalized == "independent_lane":
        satisfied.append("review_source: independent_lane")
        return satisfied, missing, reasons

    failures = evidence.get("lane_failures")
    if not isinstance(failures, list) or not failures:
        reasons.append("self_review requires recorded lane_failures from a reported reviewer lane failure")

    auth_satisfied, auth_missing = _self_review_authorization_item(evidence)
    satisfied.append("review_source: self_review")
    satisfied.extend(auth_satisfied)
    missing.extend(auth_missing)
    if auth_missing:
        reasons.append("self_review requires explicit self_review_authorization")
    return satisfied, missing, reasons


def _lane_failure_items(evidence: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    failures = evidence.get("lane_failures")
    if not isinstance(failures, list):
        missing.append("lane_failures")
        reasons.append("lane_failures must be a list")
        return satisfied, missing, reasons

    for index, failure in enumerate(failures, start=1):
        if not isinstance(failure, dict):
            reasons.append(f"lane_failures[{index}] must be an object")
            continue
        for key in ["lane_id", "failure_kind", "observed_marker"]:
            if not _non_empty_string(failure.get(key)):
                missing.append(f"lane_failures[{index}].{key}")
        kind = str(failure.get("failure_kind") or "").strip()
        if kind and kind not in LANE_FAILURE_KINDS:
            reasons.append(f"lane_failures[{index}].failure_kind is unsupported: {kind}")

    if failures:
        satisfied.append(f"lane failures recorded: {len(failures)}")
    else:
        satisfied.append("no lane failures recorded")
    return satisfied, missing, reasons


def _ordering_items(evidence: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    satisfied: list[str] = []
    missing: list[str] = []
    reasons: list[str] = []

    completed_at = evidence.get("gate_query_completed_at")
    gate_head_sha = evidence.get("gate_query_head_sha")
    head_sha = evidence.get("head_sha")

    if _non_empty_string(completed_at):
        completed_time = _parse_timestamp(completed_at, "gate_query_completed_at")
        if completed_time is None:
            reasons.append(
                "gate_query_completed_at must be a timezone-aware ISO-8601 timestamp"
            )
        else:
            satisfied.append(f"gate query completed at {completed_at}")
    else:
        missing.append("gate_query_completed_at")

    if _non_empty_string(gate_head_sha):
        if gate_head_sha == head_sha:
            satisfied.append("gate_query_head_sha matches head_sha")
        else:
            reasons.append("gate_query_head_sha must match head_sha")
    else:
        missing.append("gate_query_head_sha")

    merge_dispatched_at = evidence.get("merge_dispatched_at")
    merge_head_sha = evidence.get("merge_head_sha")
    has_merge_time = merge_dispatched_at is not None
    has_merge_head = merge_head_sha is not None
    if has_merge_time != has_merge_head:
        missing.append("merge_ordering_pair")
        reasons.append("merge_dispatched_at and merge_head_sha must be provided together")
        return satisfied, missing, reasons

    if has_merge_time and has_merge_head:
        if not _non_empty_string(merge_dispatched_at):
            missing.append("merge_dispatched_at")
            return satisfied, missing, reasons
        if not _non_empty_string(merge_head_sha):
            missing.append("merge_head_sha")
            return satisfied, missing, reasons
        merge_time = _parse_timestamp(merge_dispatched_at, "merge_dispatched_at")
        completed_time = (
            _parse_timestamp(completed_at, "gate_query_completed_at")
            if _non_empty_string(completed_at)
            else None
        )
        if merge_time is None:
            reasons.append(
                "merge_dispatched_at must be a timezone-aware ISO-8601 timestamp"
            )
        elif completed_time is not None and completed_time >= merge_time:
            reasons.append("gate query must complete before merge dispatch")
        else:
            satisfied.append(f"merge dispatch ordered after gate query at {merge_dispatched_at}")
        if merge_head_sha != gate_head_sha:
            reasons.append("merge_head_sha must match gate_query_head_sha")
        else:
            satisfied.append("merge_head_sha matches gate_query_head_sha")

    return satisfied, missing, reasons


def evaluate_pr_gate(evidence: dict[str, Any]) -> dict[str, Any]:
    """Evaluate merge-readiness evidence and return a stable decision object."""

    reasons: list[str] = []
    satisfied: list[str] = []
    missing: list[str] = []

    if _positive_int(evidence.get("pr")):
        satisfied.append(f"pr: {evidence['pr']}")
    else:
        missing.append("pr")

    state = str(evidence.get("state") or "").upper()
    if state == "OPEN":
        satisfied.append("PR state is OPEN")
    elif state:
        reasons.append(f"PR state must be OPEN; got {state}")
    else:
        missing.append("state")

    if evidence.get("is_draft") is False:
        satisfied.append("PR is not draft")
    elif "is_draft" not in evidence:
        missing.append("is_draft")
    else:
        reasons.append("draft PR cannot merge")

    if _non_empty_string(evidence.get("head_sha")):
        satisfied.append(f"head_sha: {evidence['head_sha']}")
    else:
        missing.append("head_sha")

    if _positive_int(evidence.get("linked_issue")):
        satisfied.append(f"linked_issue: {evidence['linked_issue']}")
    else:
        missing.append("linked_issue")

    merge_state = str(evidence.get("merge_state") or "").upper()
    if merge_state in CLEAN_MERGE_STATES:
        satisfied.append(f"merge_state: {merge_state}")
    elif merge_state:
        reasons.append(f"merge_state must be CLEAN; got {merge_state}")
    else:
        missing.append("merge_state")

    for checker in [
        _check_items,
        _review_items,
        _thread_items,
        _review_source_items,
        _issue_reference_items,
        _merge_record_items,
    ]:
        checker_satisfied, checker_missing, checker_reasons = checker(evidence)
        satisfied.extend(checker_satisfied)
        missing.extend(checker_missing)
        reasons.extend(checker_reasons)

    for checker in [_review_source_items, _lane_failure_items]:
        checker_satisfied, checker_missing, checker_reasons = checker(evidence)
        satisfied.extend(checker_satisfied)
        missing.extend(checker_missing)
        reasons.extend(checker_reasons)

    ordering_satisfied, ordering_missing, ordering_reasons = _ordering_items(evidence)
    satisfied.extend(ordering_satisfied)
    missing.extend(ordering_missing)
    reasons.extend(ordering_reasons)

    auth_satisfied, auth_missing = _authorization_item(evidence)
    satisfied.extend(auth_satisfied)
    missing.extend(auth_missing)

    deterministic_missing = [item for item in missing if not item.startswith("human_authorization")]
    if reasons or deterministic_missing:
        decision = "blocked"
    elif auth_missing:
        decision = "needs_human"
    else:
        decision = "allowed"

    blocked_actions = []
    if decision in {"blocked", "needs_human"}:
        blocked_actions.append("merge")
    if decision == "blocked":
        blocked_actions.append("final_approval")

    return {
        "decision": decision,
        "pr": evidence.get("pr"),
        "linked_issue": evidence.get("linked_issue"),
        "issue_reference": evidence.get("issue_reference"),
        "head_sha": evidence.get("head_sha"),
        "review_source": evidence.get("review_source"),
        "gate_query_completed_at": evidence.get("gate_query_completed_at"),
        "gate_query_head_sha": evidence.get("gate_query_head_sha"),
        "reasons": sorted(set(reasons)),
        "satisfied": sorted(set(satisfied)),
        "missing": sorted(set(missing)),
        "blocked_actions": blocked_actions,
        "verification_commands": [
            "python3 checks/pr_gate.py --repo . --evidence <evidence.json>",
            "python3 checks/check_workflow.py --repo .",
        ],
    }


def print_gate_human(result: dict[str, Any]) -> None:
    print(f"decision: {result['decision']}")
    if result.get("pr"):
        print(f"pr: {result['pr']}")
    if result.get("linked_issue"):
        print(f"linked_issue: GH-{result['linked_issue']}")
    if result.get("head_sha"):
        print(f"head_sha: {result['head_sha']}")
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
        description="Evaluate SpecRail PR merge-readiness evidence."
    )
    parser.add_argument("--repo", default=".", help="Repository root, kept for CLI symmetry")
    parser.add_argument("--evidence", required=True, help="PR evidence JSON file")
    parser.add_argument(
        "--mode",
        default="dry_run",
        choices=["dry_run", "advisory", "required"],
        help="Evaluation enforcement mode",
    )
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    args = parser.parse_args()

    try:
        evidence = _load_json(Path(args.evidence))
        result = evaluate_pr_gate(evidence)
    except ValueError as exc:
        result = {
            "decision": "blocked",
            "pr": None,
            "linked_issue": None,
            "head_sha": None,
            "reasons": [str(exc)],
            "satisfied": [],
            "missing": [],
            "blocked_actions": ["merge", "final_approval"],
            "verification_commands": ["python3 checks/pr_gate.py --repo . --evidence <evidence.json>"],
        }

    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print_gate_human(result)

    if result["decision"] == "blocked":
        return 1
    if result["decision"] == "needs_human" and args.mode == "required":
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
