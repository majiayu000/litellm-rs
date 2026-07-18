#!/usr/bin/env python3
"""Collect read-only GitHub PR evidence for the offline SpecRail PR gate."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from typing import Any

from github_evidence_common import EvidenceError
from github_issue_reference import (
    normalize_closing_issue_numbers,
    normalize_issue_reference,
    normalize_linked_issue,
    references_partial_issue,
    relation_snapshot,
)


PR_VIEW_FIELDS = [
    "number",
    "state",
    "isDraft",
    "headRefOid",
    "mergeStateStatus",
    "body",
    "closingIssuesReferences",
    "statusCheckRollup",
    "reviews",
]

REVIEW_THREADS_QUERY = """
query SpecRailReviewThreads($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviewThreads(first: 100) {
        nodes {
          id
          isResolved
          isOutdated
          resolvedBy {
            login
          }
          comments(first: 5) {
            nodes {
              url
              author {
                login
              }
            }
          }
        }
      }
    }
  }
}
""".strip()

REPO_PATTERN = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
STATUS_CONTEXT_STATES = {"SUCCESS", "FAILURE", "ERROR", "PENDING", "EXPECTED"}
REVIEW_SOURCES = {"independent_lane", "self_review"}
LANE_FAILURE_KINDS = {"usage_limit", "crash", "zero_output", "closed", "other"}


def parse_github_repo(raw: str) -> tuple[str, str]:
    value = raw.strip()
    if not REPO_PATTERN.fullmatch(value):
        raise EvidenceError("GitHub repository must use OWNER/REPO format")
    owner, name = value.split("/", 1)
    if owner in {".", ".."} or name in {".", ".."}:
        raise EvidenceError("GitHub repository owner and name must be explicit")
    return owner, name


def parse_pr_number(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("PR number must be a positive integer") from exc
    if value <= 0:
        raise argparse.ArgumentTypeError("PR number must be a positive integer")
    return value


def parse_issue_number(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("issue number must be a positive integer") from exc
    if value <= 0:
        raise argparse.ArgumentTypeError("issue number must be a positive integer")
    return value


def run_gh_json(args: list[str]) -> dict[str, Any]:
    command = ["gh", *args]
    try:
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as exc:
        raise EvidenceError("gh executable was not found in PATH") from exc

    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no output"
        raise EvidenceError(f"gh command failed: {' '.join(command[:4])}: {detail}")

    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise EvidenceError(f"gh command returned invalid JSON: {exc.msg}") from exc
    if not isinstance(payload, dict):
        raise EvidenceError("gh command JSON output must be an object")
    return payload


def collect_pr_view(github_repo: str, pr_number: int) -> dict[str, Any]:
    return run_gh_json(
        [
            "pr",
            "view",
            str(pr_number),
            "--repo",
            github_repo,
            "--json",
            ",".join(PR_VIEW_FIELDS),
        ]
    )


def collect_issue_view(github_repo: str, issue_number: int) -> dict[str, Any]:
    return run_gh_json(
        [
            "issue",
            "view",
            str(issue_number),
            "--repo",
            github_repo,
            "--json",
            "number,state,url",
        ]
    )


def collect_review_threads(owner: str, name: str, pr_number: int) -> dict[str, Any]:
    return run_gh_json(
        [
            "api",
            "graphql",
            "-F",
            f"owner={owner}",
            "-F",
            f"name={name}",
            "-F",
            f"number={pr_number}",
            "-f",
            f"query={REVIEW_THREADS_QUERY}",
        ]
    )


def _require_mapping(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{field} must be an object")
    return value


def _require_list(value: Any, field: str) -> list[Any]:
    if not isinstance(value, list):
        raise EvidenceError(f"{field} must be a list")
    return value


def _require_positive_int(payload: dict[str, Any], field: str) -> int:
    value = payload.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise EvidenceError(f"{field} must be a positive integer")
    return value


def _require_string(payload: dict[str, Any], field: str) -> str:
    value = payload.get(field)
    if not isinstance(value, str) or not value.strip():
        raise EvidenceError(f"{field} must be a non-empty string")
    return value.strip()


def _require_bool(payload: dict[str, Any], field: str) -> bool:
    value = payload.get(field)
    if not isinstance(value, bool):
        raise EvidenceError(f"{field} must be a boolean")
    return value


def _read_json_file(path: str, field: str) -> Any:
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    except OSError as exc:
        raise EvidenceError(f"cannot read {field} file {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise EvidenceError(f"{field} file is not valid JSON: {exc.msg}") from exc


def _first_comment_url(thread: dict[str, Any]) -> str | None:
    comments = thread.get("comments")
    if not isinstance(comments, dict):
        return None
    nodes = comments.get("nodes")
    if not isinstance(nodes, list):
        return None
    for node in nodes:
        if isinstance(node, dict) and isinstance(node.get("url"), str) and node["url"].strip():
            return node["url"].strip()
    return None


def _resolver_login(thread: dict[str, Any]) -> str | None:
    for key in ["resolvedBy", "resolved_by"]:
        value = thread.get(key)
        if isinstance(value, dict) and isinstance(value.get("login"), str) and value["login"].strip():
            return value["login"].strip()
        if isinstance(value, str) and value.strip():
            return value.strip()
    return None


def _resolver_role(thread: dict[str, Any]) -> str | None:
    for key in ["resolverRole", "resolver_role"]:
        value = thread.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return None


def _resolver_role_map(payload: Any) -> dict[str, str]:
    source = payload
    if isinstance(payload, dict):
        if isinstance(payload.get("resolver_roles"), dict):
            source = payload["resolver_roles"]
        elif isinstance(payload.get("lane_roster"), list):
            source = payload["lane_roster"]
        elif isinstance(payload.get("lanes"), list):
            source = payload["lanes"]

    roles: dict[str, str] = {}
    if isinstance(source, dict):
        for login, role in source.items():
            if isinstance(login, str) and isinstance(role, str) and login.strip() and role.strip():
                roles[login.strip()] = role.strip()
        return roles

    if isinstance(source, list):
        for index, lane in enumerate(source, start=1):
            if not isinstance(lane, dict):
                raise EvidenceError(f"resolver role lane_roster item #{index} must be an object")
            role = lane.get("resolver_role") or lane.get("role")
            login = (
                lane.get("login")
                or lane.get("github_login")
                or lane.get("actor")
                or lane.get("resolved_by")
            )
            if isinstance(login, str) and isinstance(role, str) and login.strip() and role.strip():
                roles[login.strip()] = role.strip()
        return roles

    raise EvidenceError("resolver role map must be an object or lane roster list")


def load_resolver_role_map(path: str | None) -> dict[str, str]:
    if path is None:
        return {}
    return _resolver_role_map(_read_json_file(path, "resolver role map"))


def _author_login(value: Any, fallback: str) -> str:
    if isinstance(value, dict) and isinstance(value.get("login"), str) and value["login"].strip():
        return value["login"].strip()
    if isinstance(value, str) and value.strip():
        return value.strip()
    return fallback


def _normalize_status_context(item: dict[str, Any]) -> tuple[str, str]:
    state = str(item.get("state") or "").upper()
    if state not in STATUS_CONTEXT_STATES:
        return "", ""
    if state == "SUCCESS":
        return "COMPLETED", "SUCCESS"
    if state in {"PENDING", "EXPECTED"}:
        return "IN_PROGRESS", ""
    return "COMPLETED", state


def _rollup_items(value: Any) -> list[Any]:
    if isinstance(value, list):
        return value
    if isinstance(value, dict):
        nodes = value.get("nodes")
        if isinstance(nodes, list):
            return nodes
    raise EvidenceError("statusCheckRollup must be a list or nodes object")


def normalize_checks(value: Any) -> list[dict[str, str]]:
    checks: list[dict[str, str]] = []
    for index, item in enumerate(_rollup_items(value), start=1):
        if not isinstance(item, dict):
            raise EvidenceError(f"statusCheckRollup item #{index} must be an object")
        name = str(item.get("name") or item.get("context") or item.get("workflowName") or f"check #{index}")
        status = str(item.get("status") or "").upper()
        conclusion = str(item.get("conclusion") or "").upper()
        if not status and not conclusion:
            status, conclusion = _normalize_status_context(item)
        if not status and conclusion == "SUCCESS":
            status = "COMPLETED"
        check = {
            "name": name,
            "status": status,
            "conclusion": conclusion,
        }
        url = item.get("detailsUrl") or item.get("targetUrl")
        if isinstance(url, str) and url.strip():
            check["url"] = url.strip()
        checks.append(check)
    return checks


def normalize_reviews(value: Any) -> list[dict[str, str]]:
    reviews = _require_list(value, "reviews")
    latest_by_author: dict[str, dict[str, str]] = {}
    author_order: list[str] = []
    for index, item in enumerate(reviews, start=1):
        if not isinstance(item, dict):
            raise EvidenceError(f"review item #{index} must be an object")
        state = str(item.get("state") or "").upper()
        if not state:
            continue
        author = _author_login(item.get("author"), f"review #{index}")
        if author not in latest_by_author:
            author_order.append(author)
        latest_by_author[author] = {"author": author, "state": state}
    return [latest_by_author[author] for author in author_order]


def normalize_review_threads(
    graphql_payload: dict[str, Any],
    resolver_roles: dict[str, str] | None = None,
) -> list[dict[str, Any]]:
    data = _require_mapping(graphql_payload.get("data"), "data")
    repository = _require_mapping(data.get("repository"), "data.repository")
    pull_request = _require_mapping(
        repository.get("pullRequest"), "data.repository.pullRequest"
    )
    review_threads = _require_mapping(
        pull_request.get("reviewThreads"), "data.repository.pullRequest.reviewThreads"
    )
    nodes = _require_list(
        review_threads.get("nodes"), "data.repository.pullRequest.reviewThreads.nodes"
    )

    normalized: list[dict[str, Any]] = []
    for index, item in enumerate(nodes, start=1):
        if not isinstance(item, dict):
            raise EvidenceError(f"review thread item #{index} must be an object")
        thread: dict[str, Any] = {
            "is_resolved": item.get("isResolved") is True,
            "is_outdated": item.get("isOutdated") is True,
        }
        thread_id = item.get("id")
        if isinstance(thread_id, str) and thread_id.strip():
            thread["id"] = thread_id.strip()
        url = _first_comment_url(item)
        if url:
            thread["url"] = url
        resolver = _resolver_login(item)
        if resolver:
            thread["resolved_by"] = resolver
        role = _resolver_role(item)
        if not role and resolver and resolver_roles:
            role = resolver_roles.get(resolver)
        if role:
            thread["resolver_role"] = role
        normalized.append(thread)
    return normalized


def build_human_authorization(
    actor: str | None,
    source: str | None,
    summary: str | None,
) -> dict[str, str] | None:
    provided = [value for value in [actor, source, summary] if value is not None and value.strip()]
    if not provided:
        return None
    if not actor or not actor.strip() or not source or not source.strip():
        raise EvidenceError(
            "--authorization-actor and --authorization-source must be provided together"
        )
    authorization = {
        "actor": actor.strip(),
        "source": source.strip(),
    }
    if summary and summary.strip():
        authorization["summary"] = summary.strip()
    return authorization


def build_self_review_authorization(
    actor: str | None,
    source: str | None,
    scope: str | None,
    summary: str | None,
) -> dict[str, str] | None:
    provided = [
        value
        for value in [actor, source, scope, summary]
        if value is not None and value.strip()
    ]
    if not provided:
        return None
    if not actor or not actor.strip() or not source or not source.strip() or not scope or not scope.strip():
        raise EvidenceError(
            "--self-review-authorization-actor, --self-review-authorization-source, "
            "and --self-review-authorization-scope must be provided together"
        )
    authorization = {
        "actor": actor.strip(),
        "source": source.strip(),
        "scope": scope.strip(),
    }
    if summary and summary.strip():
        authorization["summary"] = summary.strip()
    return authorization


def _normalize_lane_failure(item: Any, index: int) -> dict[str, str]:
    if not isinstance(item, dict):
        raise EvidenceError(f"lane_failures item #{index} must be an object")
    normalized: dict[str, str] = {}
    for key in ["lane_id", "failure_kind", "observed_marker"]:
        value = item.get(key)
        if not isinstance(value, str) or not value.strip():
            raise EvidenceError(f"lane_failures[{index}].{key} must be a non-empty string")
        normalized[key] = value.strip()
    if normalized["failure_kind"] not in LANE_FAILURE_KINDS:
        raise EvidenceError(
            f"lane_failures[{index}].failure_kind is unsupported: {normalized['failure_kind']}"
        )
    detail = item.get("detail")
    if isinstance(detail, str) and detail.strip():
        normalized["detail"] = detail.strip()
    return normalized


def load_lane_failures(path: str | None) -> list[dict[str, str]]:
    if path is None:
        return []
    payload = _read_json_file(path, "lane failures")
    if isinstance(payload, dict):
        payload = payload.get("lane_failures")
    if not isinstance(payload, list):
        raise EvidenceError("lane failures file must contain a list or lane_failures list")
    return [_normalize_lane_failure(item, index) for index, item in enumerate(payload, start=1)]


def build_evidence(
    pr_payload: dict[str, Any],
    threads_payload: dict[str, Any],
    authorization: dict[str, str] | None = None,
    merge_dispatched_at: str | None = None,
    merge_head_sha: str | None = None,
    review_source: str | None = None,
    lane_failures: list[dict[str, str]] | None = None,
    self_review_authorization: dict[str, str] | None = None,
    resolver_roles: dict[str, str] | None = None,
    expected_issue: int | None = None,
    issue_payload: dict[str, Any] | None = None,
) -> dict[str, Any]:
    head_sha = _require_string(pr_payload, "headRefOid")
    linked_issue, issue_reference = normalize_issue_reference(
        pr_payload,
        expected_issue,
        issue_payload,
    )
    evidence: dict[str, Any] = {
        "pr": _require_positive_int(pr_payload, "number"),
        "state": _require_string(pr_payload, "state").upper(),
        "is_draft": _require_bool(pr_payload, "isDraft"),
        "head_sha": head_sha,
        "gate_query_completed_at": datetime.now(timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "gate_query_head_sha": head_sha,
        "merge_state": _require_string(pr_payload, "mergeStateStatus").upper(),
        "linked_issue": linked_issue,
        "checks": normalize_checks(pr_payload.get("statusCheckRollup")),
        "reviews": normalize_reviews(pr_payload.get("reviews")),
        "review_threads": normalize_review_threads(threads_payload, resolver_roles),
        "lane_failures": lane_failures or [],
    }
    if issue_reference is not None:
        evidence["issue_reference"] = issue_reference
    if review_source is not None:
        source = review_source.strip()
        if source not in REVIEW_SOURCES:
            raise EvidenceError(f"review_source must be one of {sorted(REVIEW_SOURCES)}")
        evidence["review_source"] = source
    if authorization is not None:
        evidence["human_authorization"] = authorization
    if self_review_authorization is not None:
        evidence["self_review_authorization"] = self_review_authorization
    provided_merge = [value for value in [merge_dispatched_at, merge_head_sha] if value is not None]
    if provided_merge:
        if not merge_dispatched_at or not merge_dispatched_at.strip() or not merge_head_sha or not merge_head_sha.strip():
            raise EvidenceError(
                "--merge-dispatched-at and --merge-head-sha must be provided together"
            )
        evidence["merge_dispatched_at"] = merge_dispatched_at.strip()
        evidence["merge_head_sha"] = merge_head_sha.strip()
    return evidence


def collect_evidence(
    github_repo: str,
    pr_number: int,
    authorization: dict[str, str] | None,
    merge_dispatched_at: str | None = None,
    merge_head_sha: str | None = None,
    review_source: str | None = None,
    lane_failures: list[dict[str, str]] | None = None,
    self_review_authorization: dict[str, str] | None = None,
    resolver_roles: dict[str, str] | None = None,
    expected_issue: int | None = None,
) -> dict[str, Any]:
    if expected_issue is not None and (
        not isinstance(expected_issue, int)
        or isinstance(expected_issue, bool)
        or expected_issue <= 0
    ):
        raise EvidenceError("expected issue must be a positive integer")
    owner, name = parse_github_repo(github_repo)
    pr_payload_before = collect_pr_view(github_repo, pr_number)
    head_sha_before = _require_string(pr_payload_before, "headRefOid")
    relation_snapshot_before = relation_snapshot(pr_payload_before)
    threads_payload = collect_review_threads(owner, name, pr_number)

    issue_payload = None
    closing_issue_numbers = list(relation_snapshot_before[1])
    if expected_issue is not None and expected_issue not in closing_issue_numbers:
        body = relation_snapshot_before[0]
        if not references_partial_issue(body, expected_issue):
            raise EvidenceError(
                f"PR body must contain a standalone Refs #{expected_issue} directive"
            )
        issue_payload = collect_issue_view(github_repo, expected_issue)

    pr_payload_after = collect_pr_view(github_repo, pr_number)
    head_sha_after = _require_string(pr_payload_after, "headRefOid")
    if head_sha_before != head_sha_after:
        raise EvidenceError(
            "PR head changed while collecting gate evidence; rerun PR evidence collection"
        )
    relation_snapshot_after = relation_snapshot(pr_payload_after)
    if relation_snapshot_before != relation_snapshot_after:
        raise EvidenceError(
            "PR issue relation changed while collecting gate evidence; rerun PR evidence collection"
        )

    return build_evidence(
        pr_payload_after,
        threads_payload,
        authorization,
        merge_dispatched_at,
        merge_head_sha,
        review_source,
        lane_failures,
        self_review_authorization,
        resolver_roles,
        expected_issue,
        issue_payload,
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Collect read-only GitHub PR evidence for SpecRail pr_gate.py."
    )
    parser.add_argument("--github-repo", required=True, help="GitHub repository as OWNER/REPO")
    parser.add_argument("--pr", required=True, type=parse_pr_number, help="Pull request number")
    parser.add_argument(
        "--issue",
        type=parse_issue_number,
        help="Expected linked issue; required to verify a non-closing Refs directive",
    )
    parser.add_argument("--authorization-actor", help="Human authorizing merge")
    parser.add_argument("--authorization-source", help="Where authorization was recorded")
    parser.add_argument("--authorization-summary", help="Short authorization summary")
    parser.add_argument(
        "--review-source",
        choices=sorted(REVIEW_SOURCES),
        help="Review source for the PR gate evidence",
    )
    parser.add_argument(
        "--lane-failures-json",
        help="JSON file containing lane_failures evidence",
    )
    parser.add_argument(
        "--resolver-role-map",
        help="JSON map or lane roster used to map resolver login to resolver_role",
    )
    parser.add_argument("--self-review-authorization-actor", help="Human authorizing self-review")
    parser.add_argument("--self-review-authorization-source", help="Where self-review authorization was recorded")
    parser.add_argument("--self-review-authorization-scope", help="Scope of self-review authorization")
    parser.add_argument("--self-review-authorization-summary", help="Short self-review authorization summary")
    parser.add_argument("--merge-dispatched-at", help="Optional merge dispatch timestamp for audit records")
    parser.add_argument("--merge-head-sha", help="Optional merge target head SHA for audit records")
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    args = parser.parse_args()

    try:
        authorization = build_human_authorization(
            args.authorization_actor,
            args.authorization_source,
            args.authorization_summary,
        )
        self_review_authorization = build_self_review_authorization(
            args.self_review_authorization_actor,
            args.self_review_authorization_source,
            args.self_review_authorization_scope,
            args.self_review_authorization_summary,
        )
        lane_failures = load_lane_failures(args.lane_failures_json)
        resolver_roles = load_resolver_role_map(args.resolver_role_map)
        evidence = collect_evidence(
            args.github_repo,
            args.pr,
            authorization,
            args.merge_dispatched_at,
            args.merge_head_sha,
            args.review_source,
            lane_failures,
            self_review_authorization,
            resolver_roles,
            args.issue,
        )
    except EvidenceError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    print(json.dumps(evidence, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
