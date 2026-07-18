#!/usr/bin/env python3
"""Collect read-only duplicate work evidence for SpecRail implement routes."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from typing import Any

from github_evidence_common import EvidenceError


PR_LIST_FIELDS = ["number", "headRefName", "title", "body", "state"]
REPO_PATTERN = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")


def parse_github_repo(raw: str) -> str:
    value = raw.strip()
    if not REPO_PATTERN.fullmatch(value):
        raise EvidenceError("GitHub repository must use OWNER/REPO format")
    owner, name = value.split("/", 1)
    if owner in {".", ".."} or name in {".", ".."}:
        raise EvidenceError("GitHub repository owner and name must be explicit")
    return value


def parse_issue_number(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("issue number must be a positive integer") from exc
    if value <= 0:
        raise argparse.ArgumentTypeError("issue number must be a positive integer")
    return value


def _run_command(command: list[str]) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(command, check=False, capture_output=True, text=True)
    except FileNotFoundError as exc:
        raise EvidenceError(f"{command[0]} executable was not found in PATH") from exc


def run_gh_list(args: list[str]) -> list[Any]:
    command = ["gh", *args]
    completed = _run_command(command)
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no output"
        raise EvidenceError(f"gh command failed: {' '.join(command[:4])}: {detail}")
    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise EvidenceError(f"gh command returned invalid JSON: {exc.msg}") from exc
    if not isinstance(payload, list):
        raise EvidenceError("gh command JSON output must be a list")
    return payload


def collect_open_prs(github_repo: str, limit: int) -> list[Any]:
    return run_gh_list(
        [
            "pr",
            "list",
            "--repo",
            github_repo,
            "--state",
            "open",
            "--limit",
            str(limit),
            "--json",
            ",".join(PR_LIST_FIELDS),
        ]
    )


def collect_remote_branches(remote: str) -> list[str]:
    completed = _run_command(["git", "ls-remote", "--heads", remote])
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no output"
        raise EvidenceError(f"git ls-remote failed for {remote}: {detail}")

    branches: list[str] = []
    for line in completed.stdout.splitlines():
        _sha, sep, ref = line.partition("\t")
        if sep and ref.startswith("refs/heads/"):
            branches.append(ref.removeprefix("refs/heads/"))
    return sorted(branches)


def references_issue_text(text: str, issue: int) -> bool:
    patterns = [
        # bare tokens: #664, GH-664, GH664
        rf"(?<![A-Za-z0-9])(?:GH-?{issue}|#{issue})(?![A-Za-z0-9])",
        # cross-repo shorthand: owner/repo#664
        rf"[\w.-]+/[\w.-]+#{issue}(?![0-9])",
        # copied links: https://github.com/owner/repo/issues/664
        rf"https?://\S+/issues/{issue}(?![0-9])",
    ]
    return any(
        re.search(pattern, text, re.IGNORECASE) is not None for pattern in patterns
    )


def _require_positive_int(payload: dict[str, Any], field: str) -> int:
    value = payload.get(field)
    if not isinstance(value, int) or value <= 0:
        raise EvidenceError(f"{field} must be a positive integer")
    return value


def _require_string(payload: dict[str, Any], field: str) -> str:
    value = payload.get(field)
    if not isinstance(value, str) or not value.strip():
        raise EvidenceError(f"{field} must be a non-empty string")
    return value.strip()


def normalize_open_pr(item: Any, issue: int) -> dict[str, Any]:
    if not isinstance(item, dict):
        raise EvidenceError("PR list items must be objects")
    number = _require_positive_int(item, "number")
    head_ref = _require_string(item, "headRefName")
    haystack = "\n".join(
        str(item.get(field) or "")
        for field in ["headRefName", "title", "body"]
    )
    return {
        "number": number,
        "head_ref": head_ref,
        "references_issue": references_issue_text(haystack, issue),
    }


def build_evidence(
    issue: int,
    open_pr_payload: list[Any],
    remote_branches: list[str],
    pr_limit: int,
) -> dict[str, Any]:
    if issue <= 0:
        raise EvidenceError("issue number must be a positive integer")
    if pr_limit <= 0:
        raise EvidenceError("PR limit must be a positive integer")
    if not all(isinstance(branch, str) and branch.strip() for branch in remote_branches):
        raise EvidenceError("remote branch names must be non-empty strings")
    return {
        "issue": issue,
        "collected_at": datetime.now(timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "open_prs_complete": len(open_pr_payload) < pr_limit,
        "open_pr_limit": pr_limit,
        "open_prs": [normalize_open_pr(item, issue) for item in open_pr_payload],
        "remote_branches": sorted(branch.strip() for branch in remote_branches),
    }


def collect_duplicate_evidence(
    github_repo: str,
    issue: int,
    remote: str,
    pr_limit: int,
) -> dict[str, Any]:
    repo = parse_github_repo(github_repo)
    open_prs = collect_open_prs(repo, pr_limit)
    branches = collect_remote_branches(remote)
    return build_evidence(issue, open_prs, branches, pr_limit)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Collect read-only duplicate work evidence for SpecRail implement routes."
    )
    parser.add_argument("--github-repo", required=True, help="GitHub repository as OWNER/REPO")
    parser.add_argument("--issue", required=True, type=parse_issue_number, help="Issue number")
    parser.add_argument("--remote", default="origin", help="Git remote to inspect")
    parser.add_argument("--pr-limit", type=int, default=100, help="Maximum open PRs to inspect")
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    args = parser.parse_args()

    try:
        evidence = collect_duplicate_evidence(
            args.github_repo,
            args.issue,
            args.remote,
            args.pr_limit,
        )
    except EvidenceError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    print(json.dumps(evidence, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
