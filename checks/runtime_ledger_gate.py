#!/usr/bin/env python3
"""Evaluate an optional SpecRail local runtime checkpoint.

The runtime checkpoint is a handoff aid for long agent runs. It does not
replace GitHub issues, pull requests, labels, reviews, branches, or SpecRail
spec packets as canonical workflow state.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from pr_gate import evaluate_pr_gate
from runtime_gate_rules import (
    REVIEW_SOURCES,
    _require_nonempty_string,
    _require_positive_int,
    _validate_budget,
    _validate_goal_candidate,
    _validate_lane_failure_outcome,
    _validate_self_review_authorization,
    _validate_tranche_mix,
)
from specrail_lib import SPEC_STATUSES


MERGE_READY_STATES = {"complete", "merge_ready", "ready_to_merge", "merged"}
PR_MERGE_STATES = {"merge_ready", "ready_to_merge", "merged"}
PASSED_STATUSES = {"passed", "success", "successful", "green"}
PENDING_STATUSES = {"pending", "running", "in_progress", "queued"}
REVIEW_THREAD_CLEAN_STATUSES = {"clean", "resolved", "none", "passed"}
PR_GATE_PASSED_STATUSES = {"passed", "allowed", "clean", "success", "green"}
MERGE_STATE_CLEAN_STATUSES = {"clean", "mergeable"}
CHECKPOINT_STATUSES = {"planning", "running", "blocked", "handoff", "complete"}
FULL_QUEUE_NON_DRAINED_STATES = {
    "needs_spec",
    "needs_tasks",
    "eligible_impl",
    "waiting_ci",
    "needs_ci",
    "needs_review",
    "review_required",
    "open",
    "planning",
    "running",
}
FULL_QUEUE_TERMINAL_REMAINDER_STATES = {
    "blocked",
    "deferred",
    "needs_human",
    "closed",
    "merged",
}
SPEC_PLANNING_STATES = {
    "planning",
    "needs_spec",
    "needs_tasks",
    "blocked",
    "deferred",
    "needs_human",
}
THREAD_YES_VALUES = {"yes", "true", "required", "available"}
THREAD_REQUIRED_VALUES = {"required", "yes", "true"}
THREAD_AVAILABLE_VALUES = {"available", "yes", "true"}
CHECKPOINT_VERSIONS = {1, 2}


def _load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise ValueError(f"cannot read checkpoint: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"checkpoint is not valid JSON: {exc.msg}") from exc
    if not isinstance(data, dict):
        raise ValueError("checkpoint top-level value must be an object")
    return data


def _require_mapping(data: dict[str, Any], key: str, errors: list[str]) -> dict[str, Any]:
    value = data.get(key)
    if isinstance(value, dict):
        return value
    errors.append(f"{key} must be an object")
    return {}


def _item_label(item: dict[str, Any], index: int) -> str:
    if item.get("pr"):
        return f"item #{index} PR #{item['pr']}"
    if item.get("issue"):
        return f"item #{index} issue #{item['issue']}"
    return f"item #{index}"


def _is_yes(value: Any) -> bool:
    if value is True:
        return True
    if isinstance(value, str):
        return value.strip().lower() in THREAD_YES_VALUES
    return False


def _is_required(value: Any) -> bool:
    if value is True:
        return True
    if isinstance(value, str):
        return value.strip().lower() in THREAD_REQUIRED_VALUES
    return False


def _is_available(value: Any) -> bool:
    if value is True:
        return True
    if isinstance(value, str):
        return value.strip().lower() in THREAD_AVAILABLE_VALUES
    return False


def _is_url(value: str) -> bool:
    return value.startswith("https://") or value.startswith("http://")


def _resolve_local_evidence_path(reference: Any) -> Path | None:
    if not isinstance(reference, str) or not reference.strip() or _is_url(reference.strip()):
        return None
    return Path(reference.strip()).expanduser()


def _load_local_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:
    if not path.is_file():
        errors.append(f"{label}: evidence file does not exist: {path}")
        return None
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        errors.append(f"{label}: cannot read evidence file {path}: {exc}")
        return None
    except json.JSONDecodeError as exc:
        errors.append(f"{label}: evidence file is not valid JSON {path}: {exc.msg}")
        return None
    if not isinstance(payload, dict):
        errors.append(f"{label}: evidence file JSON must be an object: {path}")
        return None
    return payload


def _validate_pr_gate_artifact(
    raw_item: dict[str, Any],
    evidence: Any,
    label: str,
    errors: list[str],
) -> None:
    path = _resolve_local_evidence_path(evidence)
    if path is None:
        return
    payload = _load_local_json(path, f"{label}: pr_gate", errors)
    if payload is None:
        return

    if "decision" in payload:
        result = payload
    else:
        result = evaluate_pr_gate(payload)

    if result.get("decision") != "allowed":
        reasons = result.get("reasons")
        detail = f": {reasons}" if reasons else ""
        errors.append(f"{label}: pr_gate evidence decision must be allowed{detail}")

    item_pr = raw_item.get("pr")
    if item_pr and result.get("pr") and result.get("pr") != item_pr:
        errors.append(f"{label}: pr_gate evidence pr must match item pr")

    item_head = raw_item.get("head_sha")
    if item_head and result.get("head_sha") and result.get("head_sha") != item_head:
        errors.append(f"{label}: pr_gate evidence head_sha must match item head_sha")


def _thread_dispatch_gate(data: dict[str, Any]) -> dict[str, Any]:
    gate = data.get("thread_dispatch_gate")
    if isinstance(gate, dict):
        return gate
    return {}


def _spawned_agents(gate: dict[str, Any]) -> list[dict[str, Any]]:
    evidence = gate.get("native_thread_evidence")
    if not isinstance(evidence, dict):
        return []
    agents = evidence.get("spawned_agents")
    if not isinstance(agents, list):
        return []
    return [agent for agent in agents if isinstance(agent, dict)]


def _native_threads_required(gate: dict[str, Any]) -> bool:
    return (
        _is_available(gate.get("native_subagents"))
        and _is_required(gate.get("spawn_requirement"))
    )


def _validate_thread_dispatch_gate(
    data: dict[str, Any],
    errors: list[str],
) -> tuple[dict[str, Any], list[dict[str, Any]], bool]:
    gate = _thread_dispatch_gate(data)
    if not gate:
        return {}, [], False

    spawned_agents = _spawned_agents(gate)
    native_required = _native_threads_required(gate)
    if native_required and not spawned_agents:
        errors.append(
            "thread_dispatch_gate: native subagents are available and required, "
            "but native_thread_evidence.spawned_agents is empty"
        )

    fallback_mode = str(gate.get("fallback_mode") or "").lower()
    if native_required and fallback_mode == "single_agent":
        errors.append(
            "thread_dispatch_gate: single_agent fallback is invalid when native "
            "subagents are available and spawn_requirement is required"
        )

    if _is_yes(gate.get("explicit_thread_request")) and native_required:
        for index, lane in enumerate(gate.get("planned_native_threads") or [], start=1):
            if not isinstance(lane, dict):
                errors.append(
                    f"thread_dispatch_gate: planned_native_threads #{index} "
                    "must be an object"
                )
                continue
            if lane.get("spawn_status") == "skipped" and not lane.get("no_spawn_reason"):
                errors.append(
                    f"thread_dispatch_gate: planned_native_threads #{index} skipped "
                    "without no_spawn_reason"
                )

    return gate, spawned_agents, native_required


def _validate_native_review_evidence(
    review: dict[str, Any],
    spawned_agents: list[dict[str, Any]],
    label: str,
    errors: list[str],
) -> None:
    reviewer_lane = review.get("reviewer_lane")
    native_thread_id = review.get("native_thread_id") or review.get(
        "agent_id_or_thread_id"
    )
    if not isinstance(native_thread_id, str) or not native_thread_id.strip():
        errors.append(f"{label}: native reviewer requires native_thread_id")
        return

    for agent in spawned_agents:
        if (
            agent.get("lane_id") == reviewer_lane
            or agent.get("agent_id_or_thread_id") == native_thread_id
        ):
            if not agent.get("wait_evidence"):
                errors.append(f"{label}: native reviewer thread missing wait_evidence")
            if not agent.get("close_evidence"):
                errors.append(f"{label}: native reviewer thread missing close_evidence")
            if str(agent.get("result_collected") or "").lower() not in {"yes", "true"}:
                errors.append(f"{label}: native reviewer thread result_collected must be yes")
            return

    errors.append(f"{label}: native reviewer thread is not listed in spawned_agents")


def _validate_spec_status(
    value: Any,
    label: str,
    errors: list[str],
    *,
    required: bool,
) -> str:
    if value is None:
        if required:
            errors.append(f"{label}: spec_status is required")
        return ""
    if not isinstance(value, str) or value not in SPEC_STATUSES:
        errors.append(f"{label}: spec_status must be one of {sorted(SPEC_STATUSES)}")
        return ""
    return value


def _validate_full_queue_checkpoint(
    data: dict[str, Any],
    errors: list[str],
    warnings: list[str],
) -> None:
    queue_mode = data.get("queue_mode")
    if queue_mode != "full_queue_drain":
        return

    _require_nonempty_string(data, "overall_objective", "checkpoint", errors)

    spec_coverage = data.get("spec_coverage")
    if not isinstance(spec_coverage, dict):
        errors.append("full_queue_drain requires spec_coverage object")
    else:
        for key in sorted(SPEC_STATUSES):
            value = spec_coverage.get(key)
            if not isinstance(value, list):
                errors.append(f"spec_coverage.{key} must be a list")

    remaining_queue = data.get("remaining_queue")
    if not isinstance(remaining_queue, list):
        errors.append("full_queue_drain requires remaining_queue list")
        return

    checkpoint_status = str(data.get("status") or "").lower()
    for index, raw_item in enumerate(remaining_queue, start=1):
        if not isinstance(raw_item, dict):
            errors.append(f"remaining_queue item #{index} must be an object")
            continue
        label = _item_label(raw_item, index).replace("item", "remaining_queue item", 1)
        state = str(raw_item.get("state") or "").lower()
        if not state:
            errors.append(f"{label}: state is required")
        if not raw_item.get("next_action"):
            errors.append(f"{label}: next_action is required")
        spec_status = _validate_spec_status(
            raw_item.get("spec_status"),
            label,
            errors,
            required=True,
        )
        if spec_status == "exception_allowed" and not raw_item.get("spec_status_reason"):
            warnings.append(f"{label}: exception_allowed should record spec_status_reason")

        if checkpoint_status == "complete":
            if state in FULL_QUEUE_NON_DRAINED_STATES:
                errors.append(
                    f"{label}: state {state!r} means full_queue_drain is not complete"
                )
            elif state not in FULL_QUEUE_TERMINAL_REMAINDER_STATES:
                warnings.append(
                    f"{label}: state {state!r} is not a standard terminal remainder state"
                )


def evaluate_checkpoint(data: dict[str, Any]) -> dict[str, Any]:
    errors: list[str] = []
    warnings: list[str] = []
    satisfied: list[str] = []

    for key in [
        "checkpoint_version",
        "tranche_id",
        "repo",
        "scope",
        "status",
        "context_budget",
        "output_firewall",
        "items",
        "resume_prompt",
    ]:
        if key not in data:
            errors.append(f"missing required field: {key}")

    if data.get("checkpoint_version") not in CHECKPOINT_VERSIONS:
        allowed = ", ".join(str(v) for v in sorted(CHECKPOINT_VERSIONS))
        errors.append(f"checkpoint_version must be one of: {allowed}")

    for key in ["tranche_id", "repo", "scope", "resume_prompt"]:
        if key in data:
            _require_nonempty_string(data, key, "checkpoint", errors)

    if "status" in data:
        status = data.get("status")
        if not isinstance(status, str) or not status.strip():
            errors.append("checkpoint.status must be a non-empty string")
        elif status not in CHECKPOINT_STATUSES:
            allowed = ", ".join(sorted(CHECKPOINT_STATUSES))
            errors.append(f"checkpoint.status must be one of: {allowed}")

    _validate_goal_candidate(data, errors)
    _validate_budget(data, errors, satisfied)
    _validate_tranche_mix(data, errors, satisfied)
    _validate_full_queue_checkpoint(data, errors, warnings)
    queue_mode = data.get("queue_mode")
    thread_gate, spawned_agents, native_required = _validate_thread_dispatch_gate(data, errors)

    context_budget = _require_mapping(data, "context_budget", errors)
    _require_positive_int(context_budget, "window_tokens", "context_budget", errors)
    soft = context_budget.get("soft_stop_ratio")
    hard = context_budget.get("hard_stop_ratio")
    critical = context_budget.get("critical_stop_ratio")
    if not all(isinstance(value, (int, float)) for value in [soft, hard, critical]):
        errors.append("context_budget stop ratios must be numbers")
    elif not (0 < soft < hard < critical < 1):
        errors.append("context_budget ratios must satisfy 0 < soft < hard < critical < 1")
    else:
        satisfied.append("context_budget ratios are ordered")

    output_firewall = _require_mapping(data, "output_firewall", errors)
    raw_log_policy = output_firewall.get("raw_log_policy")
    if raw_log_policy != "file_only":
        errors.append("output_firewall.raw_log_policy must be file_only for long-run checkpoints")
    else:
        satisfied.append("raw logs are file-only")
    _require_positive_int(
        output_firewall,
        "max_parent_stdout_lines",
        "output_firewall",
        errors,
    )
    _require_positive_int(
        output_firewall,
        "max_subagent_final_lines",
        "output_firewall",
        errors,
    )
    _require_nonempty_string(output_firewall, "artifact_root", "output_firewall", errors)

    items = data.get("items")
    if not isinstance(items, list) or not items:
        errors.append("items must be a non-empty list")
        items = []

    for index, raw_item in enumerate(items, start=1):
        if not isinstance(raw_item, dict):
            errors.append(f"item #{index} must be an object")
            continue
        label = _item_label(raw_item, index)
        state = str(raw_item.get("state") or "")
        if not state:
            errors.append(f"{label}: state is required")
        if not raw_item.get("next_action"):
            errors.append(f"{label}: next_action is required")
        if queue_mode == "full_queue_drain" and (raw_item.get("issue") or raw_item.get("pr")):
            spec_status = _validate_spec_status(
                raw_item.get("spec_status"),
                label,
                errors,
                required=True,
            )
            if spec_status == "exception_allowed" and not raw_item.get("spec_status_reason"):
                warnings.append(f"{label}: exception_allowed should record spec_status_reason")
            if state in {"complete", "merge_ready", "merged"} and spec_status in {
                "needs_spec",
                "needs_tasks",
            }:
                errors.append(
                    f"{label}: terminal state {state!r} cannot use spec_status {spec_status!r}"
                )
            if spec_status in {"needs_spec", "needs_tasks"} and state not in SPEC_PLANNING_STATES:
                errors.append(
                    f"{label}: spec_status {spec_status!r} must route to spec or task "
                    "planning before implementation"
                )

        _validate_lane_failure_outcome(raw_item, state, label, errors)

        local_verification = raw_item.get("local_verification", [])
        if not isinstance(local_verification, list):
            errors.append(f"{label}: local_verification must be a list")
            local_verification = []

        for verify_index, verification in enumerate(local_verification, start=1):
            if not isinstance(verification, dict):
                errors.append(f"{label}: local_verification #{verify_index} must be an object")
                continue
            status = str(verification.get("status") or "").lower()
            command = verification.get("command")
            evidence = verification.get("evidence")
            if not command:
                errors.append(f"{label}: local_verification #{verify_index} missing command")
            if status in PENDING_STATUSES and state in {"complete", "merge_ready", "merged"}:
                errors.append(f"{label}: pending verification cannot be marked {state}")
            if status in PASSED_STATUSES | {"failed"} and not evidence:
                errors.append(f"{label}: verification {command!r} status {status} needs evidence")

        merge_evidence_required = state in PR_MERGE_STATES or (
            state == "complete" and bool(raw_item.get("pr"))
        )
        if merge_evidence_required:
            if not thread_gate:
                errors.append(f"{label}: merge-ready PR item requires thread_dispatch_gate")
            if raw_item.get("truth_level") != "A":
                errors.append(f"{label}: merge-ready state requires truth_level A")
            if not raw_item.get("pr"):
                errors.append(f"{label}: merge-ready state requires pr")
            if not raw_item.get("head_sha"):
                errors.append(f"{label}: merge-ready state requires head_sha")
            head_sha = raw_item.get("head_sha")

            ci = raw_item.get("ci") if isinstance(raw_item.get("ci"), dict) else {}
            if str(ci.get("status") or "").lower() not in PASSED_STATUSES:
                errors.append(f"{label}: merge-ready state requires green CI/check evidence")
            if not ci.get("evidence"):
                errors.append(f"{label}: merge-ready state requires CI evidence path or URL")

            review = raw_item.get("review") if isinstance(raw_item.get("review"), dict) else {}
            review_status = str(review.get("status") or "").lower()
            blocking_findings = review.get("blocking_findings", [])
            if blocking_findings:
                errors.append(f"{label}: merge-ready state has blocking review findings")

            review_source = review.get("review_source")
            if not isinstance(review_source, str) or review_source not in REVIEW_SOURCES:
                allowed = ", ".join(sorted(REVIEW_SOURCES))
                errors.append(
                    f"{label}: merge-ready state requires review.review_source "
                    f"(one of: {allowed})"
                )
            elif review_source == "self_review":
                _validate_self_review_authorization(raw_item, label, errors)

            if review_status not in {"passed", "approved", "clean"}:
                if review_source == "self_review":
                    errors.append(f"{label}: self_review merge requires passed self-review evidence")
                else:
                    errors.append(f"{label}: merge-ready state requires passed independent review")
            if not review.get("evidence"):
                errors.append(f"{label}: merge-ready state requires review evidence")
            if review_source != "self_review" and not review.get("reviewer_lane"):
                errors.append(f"{label}: merge-ready state requires reviewer_lane")
            if native_required and review_source != "self_review":
                _validate_native_review_evidence(review, spawned_agents, label, errors)

            review_threads = (
                raw_item.get("review_threads")
                if isinstance(raw_item.get("review_threads"), dict)
                else {}
            )
            review_threads_status = str(review_threads.get("status") or "").lower()
            if review_threads_status not in REVIEW_THREAD_CLEAN_STATUSES:
                errors.append(f"{label}: merge-ready state requires clean review_threads status")
            if review_threads.get("unresolved_count") != 0:
                errors.append(f"{label}: merge-ready state requires zero unresolved review threads")
            if not review_threads.get("evidence"):
                errors.append(f"{label}: merge-ready state requires review_threads evidence")
            if not review_threads.get("checked_at"):
                errors.append(f"{label}: merge-ready state requires review_threads checked_at")

            pr_gate = raw_item.get("pr_gate") if isinstance(raw_item.get("pr_gate"), dict) else {}
            pr_gate_status = str(pr_gate.get("status") or "").lower()
            if pr_gate_status not in PR_GATE_PASSED_STATUSES:
                errors.append(f"{label}: merge-ready state requires passed pr_gate status")
            if not pr_gate.get("evidence"):
                errors.append(f"{label}: merge-ready state requires pr_gate evidence")
            else:
                _validate_pr_gate_artifact(raw_item, pr_gate.get("evidence"), label, errors)
            if not pr_gate.get("checked_at"):
                errors.append(f"{label}: merge-ready state requires pr_gate checked_at")
            if pr_gate.get("head_sha") != head_sha:
                errors.append(f"{label}: pr_gate head_sha must match item head_sha")

            merge_state = str(raw_item.get("merge_state") or "").lower()
            if merge_state not in MERGE_STATE_CLEAN_STATUSES:
                errors.append(f"{label}: merge-ready state requires fresh clean merge_state")

            merge_authorization = raw_item.get("merge_authorization")
            if not merge_authorization:
                errors.append(f"{label}: merge-ready state requires explicit merge_authorization")
            elif not isinstance(merge_authorization, dict):
                errors.append(f"{label}: merge_authorization must be an object")
            else:
                for key in ["actor", "source"]:
                    value = merge_authorization.get(key)
                    if not isinstance(value, str) or not value.strip():
                        errors.append(
                            f"{label}: merge_authorization.{key} must be a non-empty string"
                        )

        blocker = raw_item.get("blocker")
        if blocker and state in {"complete", "merged"}:
            warnings.append(f"{label}: blocker is still recorded on terminal state")

    decision = "blocked" if errors else ("warn" if warnings else "allowed")
    return {
        "decision": decision,
        "errors": errors,
        "warnings": warnings,
        "satisfied": satisfied,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Evaluate a SpecRail runtime checkpoint.")
    parser.add_argument("--checkpoint", required=True, help="Path to runtime checkpoint JSON")
    parser.add_argument("--json", action="store_true", help="Print machine-readable result")
    args = parser.parse_args()

    try:
        data = _load_json(Path(args.checkpoint))
        result = evaluate_checkpoint(data)
    except ValueError as exc:
        result = {
            "decision": "blocked",
            "errors": [str(exc)],
            "warnings": [],
            "satisfied": [],
        }

    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(f"SpecRail runtime checkpoint gate: {result['decision']}")
        for key in ["errors", "warnings", "satisfied"]:
            for item in result[key]:
                print(f"- {key[:-1]}: {item}")

    return 0 if result["decision"] in {"allowed", "warn"} else 1


if __name__ == "__main__":
    sys.exit(main())
