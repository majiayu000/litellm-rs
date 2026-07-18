#!/usr/bin/env python3
"""Evaluate whether a SpecRail action may proceed from local evidence."""

from __future__ import annotations

import argparse
import json
import shlex
import sys
from pathlib import Path, PurePosixPath
from typing import Any

from specrail_lib import (
    TERMINAL_BLOCKING_STATES,
    SpecRailError,
    action_policy,
    infer_state,
    load_pack,
    render_artifact_path,
    resolve_path,
    resolve_repo_path,
    resolve_spec_packet_root,
    spec_packet_artifact_paths,
    state_map,
    validate_action_policy,
    validate_labels,
    validate_state_graph,
)
from duplicate_work_gate import evaluate_duplicate_work_gate_path


ROUTE_ALIASES = {
    "action": "triage_issue",
    "triage": "triage_issue",
    "spec": "write_spec",
    "write-spec": "write_spec",
    "write_spec": "write_spec",
    "implement": "implement",
    "review": "review_pr",
    "review-pr": "review_pr",
    "review_pr": "review_pr",
    "fix-ci": "fix_ci",
    "fix_ci": "fix_ci",
    "release-note": "draft_release_note",
    "draft-release-note": "draft_release_note",
    "draft_release_note": "draft_release_note",
}

ARTIFACT_FILES = {
    "product_spec",
    "tech_spec",
    "task_plan",
}
READINESS_GATED_ROUTES = {"write_spec", "implement"}
DECISION_RANK = {
    "allowed": 0,
    "warn": 1,
    "needs_human": 2,
    "blocked": 3,
}


def normalize_route(raw: str) -> str:
    route = ROUTE_ALIASES.get(raw, raw)
    return route.replace("-", "_")


def parse_artifact_value(raw: str) -> tuple[str, str]:
    name, sep, value = raw.partition("=")
    if not sep or not name.strip() or not value.strip():
        raise SpecRailError(f"invalid --artifact {raw!r}; expected name=value")
    return name.strip(), value.strip()


def load_evidence(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise SpecRailError(f"cannot read evidence file {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise SpecRailError(f"invalid evidence JSON {path}: {exc.msg}") from exc
    if not isinstance(data, dict):
        raise SpecRailError("evidence JSON must be an object")
    return data


def artifact_exists(repo: Path, artifact_path: str | None) -> bool:
    if not artifact_path:
        return False
    return resolve_repo_path(
        repo,
        artifact_path,
        label="artifact path",
    ).is_file()


def stricter_decision(current: str, candidate: str) -> str:
    if DECISION_RANK[candidate] > DECISION_RANK[current]:
        return candidate
    return current


def required_artifact_path(config: Any, artifact: str, issue: int | None) -> str | None:
    if artifact == "linked_issue":
        return None
    if artifact == "linked_pr":
        return None
    if artifact == "verification":
        return None
    return render_artifact_path(config, artifact, issue)


def evaluate_route(args: argparse.Namespace) -> dict[str, Any]:
    repo = resolve_path(Path(args.repo), label="repository")
    config = load_pack(repo)
    config_errors: list[str] = []
    config_errors.extend(validate_state_graph(config))
    config_errors.extend(validate_labels(config))
    config_errors.extend(validate_action_policy(config))
    try:
        configured_spec_paths = spec_packet_artifact_paths(config, 1)
        configured_spec_root = PurePosixPath(
            configured_spec_paths["spec_packet"]
        ).parent
        resolve_spec_packet_root(repo, configured_spec_root)
    except SpecRailError as exc:
        config_errors.append(str(exc))

    route = normalize_route(args.route)
    policies = action_policy(config)
    policy = policies.get(route)
    if policy is None:
        config_errors.append(f"unknown route: {route}")

    evidence = load_evidence(Path(args.evidence) if args.evidence else None)
    labels = list(args.label or [])
    labels.extend(str(label) for label in evidence.get("labels", []) if str(label).strip())
    evidence_state = evidence.get("state")
    explicit_state = args.state or evidence_state
    github_state = str(evidence.get("github_state") or "").upper()
    state_from_cli = args.state is not None
    state_from_evidence = not state_from_cli and evidence_state is not None
    state_source = str(evidence.get("state_source") or "none")
    state_trusted = state_source == "label" and evidence.get("state_trusted") is True

    reasons: list[str] = []
    satisfied: list[str] = []
    missing: list[str] = []
    blocked_actions: list[str] = []
    allowed_actions: list[str] = []
    required_artifacts: list[str] = []
    human_gates: list[str] = []
    duplicate_work_result: dict[str, Any] | None = None

    if config_errors:
        return {
            "decision": "blocked",
            "route": route,
            "current_state": explicit_state,
            "issue": args.issue,
            "pr": args.pr,
            "reasons": config_errors,
            "satisfied": [],
            "missing": [],
            "required_artifacts": [],
            "human_gates": [],
            "allowed_actions": [],
            "blocked_actions": [route],
            "verification_commands": ["python3 checks/check_workflow.py --repo ."],
        }

    if github_state and github_state != "OPEN":
        return blocked_result(
            route,
            explicit_state,
            args,
            [f"GitHub issue state must be OPEN; got {github_state}"],
        )

    current_state, state_evidence = infer_state(config, explicit_state, labels)
    if state_from_evidence and current_state == evidence_state:
        state_evidence = [f"state provided by evidence: {current_state} ({state_source})"]
    satisfied.extend(state_evidence)

    states = state_map(config)
    if current_state and current_state not in states:
        return blocked_result(
            route,
            current_state,
            args,
            [f"unknown current state: {current_state}"],
        )

    if current_state in TERMINAL_BLOCKING_STATES:
        return blocked_result(
            route,
            current_state,
            args,
            [f"state {current_state} is terminal or maintainer-reserved"],
        )

    assert policy is not None
    allowed_from = [str(state) for state in policy.get("allowed_from", [])]
    required = [str(artifact) for artifact in policy.get("required_artifacts", [])]
    creates = [str(artifact) for artifact in policy.get("creates_artifacts", [])]
    human_gates = [str(gate) for gate in policy.get("human_gates", [])]

    if current_state is None:
        missing.append("current_state")
        reasons.append("no state or matching readiness label was provided")
    elif current_state in allowed_from:
        satisfied.append(f"state {current_state} allows {route}")
    else:
        missing.append(f"allowed_state:{'|'.join(allowed_from)}")
        reasons.append(
            f"route {route} requires one of {', '.join(allowed_from)}; got {current_state}"
        )

    if (
        route in READINESS_GATED_ROUTES
        and "readiness_label" in human_gates
        and state_from_evidence
        and current_state in allowed_from
        and not state_trusted
    ):
        missing.append("trusted_state")
        reasons.append(
            f"state {current_state} came from untrusted {state_source} evidence; "
            "maintainer readiness label required"
        )

    provided_artifacts = dict(evidence.get("artifacts", {})) if isinstance(evidence.get("artifacts"), dict) else {}
    for raw_artifact in args.artifact or []:
        name, value = parse_artifact_value(raw_artifact)
        provided_artifacts[name] = value

    for artifact in required:
        if artifact == "linked_issue":
            if args.issue is None:
                missing.append("linked_issue")
            else:
                satisfied.append(f"linked_issue: GH-{args.issue}")
            continue
        if artifact == "linked_pr":
            if args.pr is None:
                missing.append("linked_pr")
            else:
                satisfied.append(f"linked_pr: PR-{args.pr}")
            continue
        if artifact == "verification":
            verification = evidence.get("verification") or provided_artifacts.get("verification")
            if verification:
                satisfied.append("verification evidence provided")
            else:
                missing.append("verification")
            continue
        provided = provided_artifacts.get(artifact)
        if artifact in ARTIFACT_FILES and args.issue is None:
            required_artifacts.append(str(provided) if provided else artifact)
            if provided and artifact_exists(repo, str(provided)):
                satisfied.append(f"{artifact}: {provided}")
            elif provided:
                missing.append(f"{artifact}:{provided}")
            else:
                missing.append(artifact)
            continue
        path = required_artifact_path(config, artifact, args.issue)
        required_artifacts.append(path or artifact)
        if provided:
            if artifact in ARTIFACT_FILES and str(provided) != path:
                missing.append(f"{artifact}:{path}")
                reasons.append(
                    f"{artifact} provided at {provided} does not match "
                    f"configured path {path}"
                )
            elif artifact in ARTIFACT_FILES and not artifact_exists(
                repo,
                str(provided),
            ):
                missing.append(f"{artifact}:{provided}")
            else:
                satisfied.append(f"{artifact}: {provided}")
            continue
        if artifact in ARTIFACT_FILES:
            if artifact_exists(repo, path):
                satisfied.append(f"{artifact}: {path}")
            else:
                missing.append(f"{artifact}:{path}")
        elif path:
            required_artifacts.append(path)

    if route == "implement":
        if args.issue is None:
            duplicate_work_result = {
                "decision": "needs_human",
                "issue": None,
                "reasons": [
                    "duplicate work evidence cannot be evaluated until a linked issue is provided"
                ],
                "satisfied": [],
                "missing": ["duplicate_evidence"],
                "blocked_actions": ["implement"],
                "verification_commands": [
                    "python3 checks/github_duplicate_evidence.py "
                    "--github-repo OWNER/REPO --issue <issue> --json"
                ],
            }
        else:
            duplicate_work_result = evaluate_duplicate_work_gate_path(
                repo,
                args.issue,
                Path(args.duplicate_evidence) if args.duplicate_evidence else None,
            )
        for item in duplicate_work_result.get("satisfied", []):
            satisfied.append(f"duplicate_work: {item}")
        for item in duplicate_work_result.get("missing", []):
            missing.append(f"duplicate_work:{item}")
        for reason in duplicate_work_result.get("reasons", []):
            reasons.append(f"duplicate_work: {reason}")

    for action, action_body in policies.items():
        allowed_states = [str(state) for state in action_body.get("allowed_from", [])]
        if current_state and current_state in allowed_states:
            allowed_actions.append(action)

    if route in {"review_pr", "draft_release_note"}:
        blocked_actions.extend(["final_approval", "merge"])
    else:
        blocked_actions.extend(["final_approval", "merge", "force_push"])

    if missing:
        if (
            current_state is None
            or any(item.startswith("allowed_state:") for item in missing)
            or "trusted_state" in missing
        ):
            decision = "needs_human" if human_gates else "blocked"
        else:
            decision = "warn" if args.mode in {"dry_run", "advisory"} else "blocked"
    else:
        decision = "allowed"
        reasons.append(f"route {route} passed local SpecRail gates")

    if duplicate_work_result is not None:
        decision = stricter_decision(decision, str(duplicate_work_result["decision"]))

    for artifact in creates:
        if args.issue is None:
            required_artifacts.append(artifact)
            continue
        path = render_artifact_path(config, artifact, args.issue)
        if path:
            required_artifacts.append(path)

    verification_commands = ["python3 checks/check_workflow.py --repo ."]
    if args.issue:
        spec_dir = spec_packet_artifact_paths(config, args.issue, repo=repo)["spec_packet"]
        verification_commands.append(
            "python3 checks/check_workflow.py --repo . --spec-dir="
            + shlex.quote(spec_dir)
        )

    return {
        "decision": decision,
        "route": route,
        "mode": args.mode,
        "current_state": current_state,
        "issue": args.issue,
        "pr": args.pr,
        "reasons": reasons,
        "satisfied": sorted(set(satisfied)),
        "missing": sorted(set(missing)),
        "required_artifacts": sorted(set(required_artifacts)),
        "human_gates": human_gates,
        "allowed_actions": sorted(set(allowed_actions)),
        "blocked_actions": sorted(set(blocked_actions)),
        "duplicate_work_gate": duplicate_work_result,
        "verification_commands": verification_commands,
    }


def blocked_result(
    route: str,
    current_state: str | None,
    args: argparse.Namespace,
    reasons: list[str],
) -> dict[str, Any]:
    return {
        "decision": "blocked",
        "route": route,
        "mode": args.mode,
        "current_state": current_state,
        "issue": args.issue,
        "pr": args.pr,
        "reasons": reasons,
        "satisfied": [],
        "missing": [],
        "required_artifacts": [],
        "human_gates": [],
        "allowed_actions": [],
        "blocked_actions": [route],
        "verification_commands": ["python3 checks/check_workflow.py --repo ."],
    }


def print_human(result: dict[str, Any]) -> None:
    print(f"decision: {result['decision']}")
    print(f"route: {result['route']}")
    if result.get("current_state"):
        print(f"current_state: {result['current_state']}")
    if result.get("issue"):
        print(f"issue: GH-{result['issue']}")
    if result.get("pr"):
        print(f"pr: PR-{result['pr']}")
    if result.get("reasons"):
        print("reasons:")
        for reason in result["reasons"]:
            print(f"- {reason}")
    if result.get("missing"):
        print("missing:")
        for item in result["missing"]:
            print(f"- {item}")
    if result.get("required_artifacts"):
        print("required_artifacts:")
        for item in result["required_artifacts"]:
            print(f"- {item}")
    if result.get("verification_commands"):
        print("verification_commands:")
        for command in result["verification_commands"]:
            print(f"- {command}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Evaluate a SpecRail route from local evidence."
    )
    parser.add_argument("--repo", default=".", help="SpecRail pack or adopted repo root")
    parser.add_argument("--route", "--action", required=True, help="Route/action to evaluate")
    parser.add_argument("--issue", type=int, help="Linked GitHub issue number")
    parser.add_argument("--pr", type=int, help="Linked pull request number")
    parser.add_argument("--state", help="Canonical SpecRail state")
    parser.add_argument("--label", action="append", default=[], help="Issue/PR label evidence")
    parser.add_argument(
        "--artifact",
        action="append",
        default=[],
        help="Artifact evidence in name=path form",
    )
    parser.add_argument("--evidence", help="Optional JSON evidence file")
    parser.add_argument("--duplicate-evidence", help="Optional duplicate work evidence JSON file")
    parser.add_argument(
        "--mode",
        default="dry_run",
        choices=["dry_run", "advisory", "required"],
        help="Evaluation enforcement mode",
    )
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    args = parser.parse_args()

    try:
        result = evaluate_route(args)
    except SpecRailError as exc:
        result = {
            "decision": "blocked",
            "route": normalize_route(args.route),
            "mode": args.mode,
            "current_state": args.state,
            "issue": args.issue,
            "pr": args.pr,
            "reasons": [str(exc)],
            "satisfied": [],
            "missing": [],
            "required_artifacts": [],
            "human_gates": [],
            "allowed_actions": [],
            "blocked_actions": [normalize_route(args.route)],
            "verification_commands": ["python3 checks/check_workflow.py --repo ."],
        }

    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print_human(result)

    if result["decision"] == "blocked":
        return 1
    if result["decision"] == "needs_human" and args.mode == "required":
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
