#!/usr/bin/env python3
"""Collect read-only GitHub issue evidence for the offline SpecRail route gate."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

from github_pr_evidence import (
    EvidenceError,
    _require_positive_int,
    _require_string,
    parse_github_repo,
    run_gh_json,
)
from specrail_lib import (
    TERMINAL_BLOCKING_STATES,
    SpecRailError,
    load_pack,
    resolve_path,
    spec_packet_artifact_paths,
)


ISSUE_VIEW_FIELDS = [
    "number",
    "title",
    "state",
    "labels",
    "url",
    "body",
]

STATE_HINT_PATTERN = re.compile(
    r"^\s*(?:[-*]\s*)?state\s*:\s*[`\"']?([A-Za-z0-9_]+)[`\"']?\s*$",
    re.IGNORECASE,
)
READINESS_STATES = {
    "needs_info",
    "triaged",
    "ready_to_spec",
    "ready_to_implement",
    "reserved_internal",
}
KNOWN_STATES = READINESS_STATES | TERMINAL_BLOCKING_STATES


def parse_issue_number(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("issue number must be a positive integer") from exc
    if value <= 0:
        raise argparse.ArgumentTypeError("issue number must be a positive integer")
    return value


def collect_issue_view(github_repo: str, issue_number: int) -> dict[str, Any]:
    return run_gh_json(
        [
            "issue",
            "view",
            str(issue_number),
            "--repo",
            github_repo,
            "--json",
            ",".join(ISSUE_VIEW_FIELDS),
        ]
    )


def _optional_body(payload: dict[str, Any]) -> str:
    value = payload.get("body")
    if value is None:
        return ""
    if not isinstance(value, str):
        raise EvidenceError("body must be a string or null")
    return value


def normalize_labels(value: Any) -> list[str]:
    if not isinstance(value, list):
        raise EvidenceError("labels must be a list")
    labels: list[str] = []
    for index, item in enumerate(value, start=1):
        if isinstance(item, str):
            label = item.strip()
        elif isinstance(item, dict) and isinstance(item.get("name"), str):
            label = item["name"].strip()
        else:
            raise EvidenceError(f"label item #{index} must be a string or object with name")
        if label:
            labels.append(label)
    return labels


def infer_state_from_labels(labels: list[str]) -> str | None:
    terminal_matches = sorted({label for label in labels if label in TERMINAL_BLOCKING_STATES})
    if len(terminal_matches) == 1:
        return terminal_matches[0]
    if len(terminal_matches) > 1:
        raise EvidenceError(f"conflicting terminal labels: {', '.join(terminal_matches)}")

    readiness_matches = sorted({label for label in labels if label in READINESS_STATES})
    if len(readiness_matches) == 1:
        return readiness_matches[0]
    if len(readiness_matches) > 1:
        raise EvidenceError(f"conflicting readiness labels: {', '.join(readiness_matches)}")
    return None


def infer_state_from_body(body: str) -> str | None:
    matches: list[str] = []
    for line in body.splitlines():
        match = STATE_HINT_PATTERN.fullmatch(line)
        if match is None:
            continue
        state = match.group(1)
        if state in KNOWN_STATES:
            matches.append(state)
    unique_matches = sorted(set(matches))
    if len(unique_matches) == 1:
        return unique_matches[0]
    if len(unique_matches) > 1:
        raise EvidenceError(f"conflicting state hints: {', '.join(unique_matches)}")
    return None


def infer_state_with_source(labels: list[str], body: str) -> tuple[str | None, str, bool]:
    state = infer_state_from_labels(labels)
    if state is not None:
        return state, "label", True

    state = infer_state_from_body(body)
    if state is not None:
        return state, "body_hint", False

    return None, "none", False


def default_artifacts(issue_number: int) -> dict[str, str]:
    return {
        "product_spec": f"specs/GH{issue_number}/product.md",
        "tech_spec": f"specs/GH{issue_number}/tech.md",
        "task_plan": f"specs/GH{issue_number}/tasks.md",
    }


def configured_artifacts(repo: Path, issue_number: int) -> dict[str, str]:
    config = load_pack(repo)
    paths = spec_packet_artifact_paths(config, issue_number, repo=repo)
    return {
        name: paths[name]
        for name in ["product_spec", "tech_spec", "task_plan"]
    }


def build_issue_evidence(
    issue_payload: dict[str, Any],
    artifacts: dict[str, str] | None = None,
) -> dict[str, Any]:
    issue_number = _require_positive_int(issue_payload, "number")
    title = _require_string(issue_payload, "title")
    github_state = _require_string(issue_payload, "state").upper()
    url = _require_string(issue_payload, "url")
    labels = normalize_labels(issue_payload.get("labels"))
    body = _optional_body(issue_payload)
    state, state_source, state_trusted = infer_state_with_source(labels, body)

    return {
        "issue": issue_number,
        "github_state": github_state,
        "state": state,
        "state_source": state_source,
        "state_trusted": state_trusted,
        "labels": labels,
        "url": url,
        "title": title,
        "artifacts": default_artifacts(issue_number) if artifacts is None else artifacts,
    }


def collect_issue_evidence(
    github_repo: str,
    issue_number: int,
    repo: Path,
) -> dict[str, Any]:
    parse_github_repo(github_repo)
    artifacts = configured_artifacts(repo, issue_number)
    issue_payload = collect_issue_view(github_repo, issue_number)
    payload_issue_number = _require_positive_int(issue_payload, "number")
    if payload_issue_number != issue_number:
        raise EvidenceError(
            f"issue number mismatch: expected {issue_number}, got {payload_issue_number}"
        )
    return build_issue_evidence(
        issue_payload,
        artifacts,
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Collect read-only GitHub issue evidence for SpecRail route_gate.py."
    )
    parser.add_argument("--repo", default=".", help="SpecRail pack or adopted repo root")
    parser.add_argument("--github-repo", required=True, help="GitHub repository as OWNER/REPO")
    parser.add_argument("--issue", required=True, type=parse_issue_number, help="Issue number")
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    args = parser.parse_args()

    try:
        evidence = collect_issue_evidence(
            args.github_repo,
            args.issue,
            resolve_path(Path(args.repo), label="repository"),
        )
    except (EvidenceError, SpecRailError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    print(json.dumps(evidence, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
