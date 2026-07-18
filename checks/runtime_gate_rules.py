#!/usr/bin/env python3
"""Field-level validation rules for SpecRail runtime checkpoints.

Split out of runtime_ledger_gate.py to keep both files within the size
guard. These rules cover the GH-59/GH-60/GH-62 contracts: reviewer-lane
failures, session budgets, tranche spec/impl mix, and goal candidates.
"""

from __future__ import annotations

from typing import Any


REVIEW_SOURCES = {"independent_lane", "self_review"}
LANE_FAILURE_KINDS = {"usage_limit", "crash", "zero_output", "closed", "other"}
LANE_FAILURE_BLOCKED_REASON = "reviewer_lane_failure"
LANE_FAILURE_DOWNGRADE_STATES = {"blocked", "needs_human"}
PR_KINDS = {"spec", "impl", "mixed_impl"}
IMPL_PR_KINDS = {"impl", "mixed_impl"}
SPEC_ONLY_STREAK_CAP = 3
BUDGET_BASES = {"compaction", "item_cap", "both"}
BUDGET_STOP_REASONS = {"budget_exhausted", "queue_empty", "user_interrupt", "blocked"}


def _require_positive_int(
    data: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> None:
    value = data.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        errors.append(f"{label}.{key} must be a positive integer")


def _require_nonempty_string(
    data: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label}.{key} must be a non-empty string")
        return ""
    return value


def _nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _item_review(raw_item: dict[str, Any]) -> dict[str, Any]:
    review = raw_item.get("review")
    return review if isinstance(review, dict) else {}


def _item_review_source(raw_item: dict[str, Any]) -> str:
    review = _item_review(raw_item)
    for value in [review.get("review_source"), raw_item.get("review_source")]:
        if isinstance(value, str) and value.strip():
            return value.strip()
    return ""


def _validate_lane_failures(
    raw_item: dict[str, Any],
    label: str,
    errors: list[str],
) -> list[str]:
    lane_failures = raw_item.get("lane_failures")
    if lane_failures is None:
        return []
    if not isinstance(lane_failures, list):
        errors.append(f"{label}: lane_failures must be a list")
        return []

    failed_lane_ids: list[str] = []
    for index, failure in enumerate(lane_failures, start=1):
        if not isinstance(failure, dict):
            errors.append(f"{label}: lane_failures[{index}] must be an object")
            continue
        lane_id = failure.get("lane_id")
        if not isinstance(lane_id, str) or not lane_id.strip():
            errors.append(f"{label}: lane_failures[{index}].lane_id is required")
        else:
            failed_lane_ids.append(lane_id.strip())
        failure_kind = failure.get("failure_kind")
        if not isinstance(failure_kind, str) or failure_kind not in LANE_FAILURE_KINDS:
            allowed = ", ".join(sorted(LANE_FAILURE_KINDS))
            errors.append(
                f"{label}: lane_failures[{index}].failure_kind must be one of: {allowed}"
            )
        elif failure_kind == "other" and not failure.get("detail"):
            errors.append(
                f"{label}: lane_failures[{index}].failure_kind other requires detail"
            )
        if not _nonempty_string(failure.get("observed_marker")):
            errors.append(f"{label}: lane_failures[{index}].observed_marker is required")
    return failed_lane_ids


def _has_successful_retry_lane(
    raw_item: dict[str, Any],
    failed_lane_ids: list[str],
) -> bool:
    review = raw_item.get("review")
    if not isinstance(review, dict):
        return False
    if str(review.get("status") or "").lower() not in {"passed", "approved", "clean"}:
        return False
    if str(review.get("review_source") or "") == "self_review":
        return False
    reviewer_lane = review.get("reviewer_lane")
    if not isinstance(reviewer_lane, str) or not reviewer_lane.strip():
        return False
    return reviewer_lane.strip() not in failed_lane_ids


def _has_self_review_authorization(raw_item: dict[str, Any]) -> bool:
    authorization = raw_item.get("self_review_authorization")
    return (
        isinstance(authorization, dict)
        and _nonempty_string(authorization.get("scope"))
        and _nonempty_string(authorization.get("conversation_marker"))
    )


def _validate_lane_failure_outcome(
    raw_item: dict[str, Any],
    state: str,
    label: str,
    errors: list[str],
) -> None:
    failed_lane_ids = _validate_lane_failures(raw_item, label, errors)
    lane_failures = raw_item.get("lane_failures")
    if not isinstance(lane_failures, list) or not lane_failures:
        return

    state_key = state.lower()
    if state_key in LANE_FAILURE_DOWNGRADE_STATES:
        if raw_item.get("blocked_reason") != LANE_FAILURE_BLOCKED_REASON:
            errors.append(
                f"{label}: lane failure downgrade requires blocked_reason "
                f"{LANE_FAILURE_BLOCKED_REASON}"
            )
        return

    if _item_review_source(raw_item) == "self_review" and _has_self_review_authorization(raw_item):
        return

    if not _has_successful_retry_lane(raw_item, failed_lane_ids):
        errors.append(
            f"{label}: reviewer lane failure requires state downgrade to "
            "blocked/needs_human or a successful independent retry lane"
        )


def _validate_self_review_authorization(
    raw_item: dict[str, Any],
    label: str,
    errors: list[str],
) -> None:
    lane_failures = raw_item.get("lane_failures")
    if not isinstance(lane_failures, list) or not lane_failures:
        errors.append(f"{label}: self_review requires recorded lane_failures")
    value = raw_item.get("self_review_authorization")
    if not isinstance(value, dict):
        errors.append(f"{label}: self_review requires self_review_authorization")
        return
    for key in ["scope", "conversation_marker"]:
        if not _nonempty_string(value.get(key)):
            errors.append(f"{label}: self_review_authorization.{key} is required")


def _validate_declaration(
    value: Any,
    label: str,
    errors: list[str],
) -> bool:
    if not isinstance(value, dict):
        errors.append(f"{label} must be an object with scope and conversation_marker")
        return False
    valid = True
    for key in ["scope", "conversation_marker"]:
        entry = value.get(key)
        if not isinstance(entry, str) or not entry.strip():
            errors.append(f"{label}.{key} must be a non-empty string")
            valid = False
    return valid


def _validate_budget(
    data: dict[str, Any],
    errors: list[str],
    satisfied: list[str],
) -> None:
    budget = data.get("budget")
    if budget is None:
        if (
            data.get("checkpoint_version") == 2
            and data.get("queue_mode") == "full_queue_drain"
        ):
            errors.append(
                "full_queue_drain checkpoint requires a declared budget "
                "(basis, compaction_budget and/or item_cap)"
            )
        return
    if not isinstance(budget, dict):
        errors.append("budget must be an object")
        return

    basis = budget.get("basis")
    if not isinstance(basis, str) or basis not in BUDGET_BASES:
        allowed = ", ".join(sorted(BUDGET_BASES))
        errors.append(f"budget.basis must be one of: {allowed}")
        basis = ""

    compaction_budget = budget.get("compaction_budget")
    if basis in {"compaction", "both"}:
        _require_positive_int(budget, "compaction_budget", "budget", errors)
    if basis in {"item_cap", "both"}:
        _require_positive_int(budget, "item_cap", "budget", errors)

    compaction_count = budget.get("compaction_count")
    if compaction_count is not None and (
        isinstance(compaction_count, bool)
        or not isinstance(compaction_count, int)
        or compaction_count < 0
    ):
        errors.append("budget.compaction_count must be a non-negative integer")
        compaction_count = None
    if basis in {"compaction", "both"} and compaction_count is None:
        errors.append(
            "budget.compaction_count must record the observed compaction events"
        )

    stop_reason = budget.get("stop_reason")
    if stop_reason is not None and (
        not isinstance(stop_reason, str) or stop_reason not in BUDGET_STOP_REASONS
    ):
        allowed = ", ".join(sorted(BUDGET_STOP_REASONS))
        errors.append(f"budget.stop_reason must be one of: {allowed}")

    override = budget.get("budget_override")
    override_valid = False
    if override is not None:
        if not isinstance(override, dict):
            errors.append("budget.budget_override must be an object")
        else:
            override_valid = True
            for key in ["scope", "conversation_marker"]:
                value = override.get(key)
                if not isinstance(value, str) or not value.strip():
                    errors.append(
                        f"budget.budget_override.{key} must be a non-empty string"
                    )
                    override_valid = False

    if (
        isinstance(compaction_count, int)
        and isinstance(compaction_budget, int)
        and not isinstance(compaction_budget, bool)
        and compaction_count > compaction_budget
    ):
        if override_valid:
            satisfied.append(
                "compaction budget exceeded under a recorded budget_override"
            )
        else:
            errors.append(
                f"budget exceeded: compaction_count {compaction_count} > "
                f"compaction_budget {compaction_budget} without a recorded "
                "budget_override; stop and hand off via checkpoint instead"
            )
    elif stop_reason == "budget_exhausted":
        satisfied.append("budget-exhausted stop is a passing terminal with handoff")


def _validate_tranche_mix(data: dict[str, Any], errors: list[str], satisfied: list[str]) -> None:
    items = data.get("items")
    item_list = [item for item in items if isinstance(item, dict)] if isinstance(items, list) else []
    kinds: list[str | None] = []
    has_kind_contract = "tranche_mix" in data and data.get("tranche_mix") is not None

    for index, item in enumerate(item_list, start=1):
        kind = item.get("pr_kind")
        if kind is None:
            kinds.append(None)
            continue
        has_kind_contract = True
        if not isinstance(kind, str) or kind not in PR_KINDS:
            allowed = ", ".join(sorted(PR_KINDS))
            errors.append(f"item #{index}: pr_kind must be one of: {allowed}")
            kinds.append(None)
            continue
        kinds.append(kind)

    if not has_kind_contract:
        return

    declaration = data.get("spec_only_declaration")
    if declaration is None and isinstance(data.get("tranche_mix"), dict):
        declaration = data["tranche_mix"].get("spec_only_declaration")
    declaration_valid = False
    if declaration is not None:
        declaration_valid = _validate_declaration(
            declaration, "spec_only_declaration", errors
        )

    streak = 0
    max_streak = 0
    for kind in kinds:
        if kind == "spec":
            streak += 1
            max_streak = max(max_streak, streak)
        elif kind in IMPL_PR_KINDS:
            streak = 0
        # items without pr_kind (non-PR work, blocked items) keep the streak

    if max_streak > SPEC_ONLY_STREAK_CAP:
        if declaration_valid:
            satisfied.append(
                f"spec-only streak {max_streak} covered by spec_only_declaration"
            )
        else:
            errors.append(
                f"{max_streak} consecutive spec-only PRs exceed the cap of "
                f"{SPEC_ONLY_STREAK_CAP} without a spec_only_declaration; "
                "interleave implementation PRs or record the quoted user "
                "confirmation"
            )

    tranche_mix = data.get("tranche_mix")
    if tranche_mix is None:
        return
    if not isinstance(tranche_mix, dict):
        errors.append("tranche_mix must be an object")
        return

    actual_spec = sum(1 for kind in kinds if kind == "spec")
    actual_impl = sum(1 for kind in kinds if kind in IMPL_PR_KINDS)
    checks = [
        ("spec_pr_count", actual_spec),
        ("impl_pr_count", actual_impl),
        ("consecutive_spec_only", max_streak),
    ]
    for key, actual in checks:
        declared = tranche_mix.get(key)
        if declared is None:
            errors.append(f"tranche_mix.{key} is required")
            continue
        if isinstance(declared, bool) or not isinstance(declared, int) or declared < 0:
            errors.append(f"tranche_mix.{key} must be a non-negative integer")
            continue
        if declared != actual:
            errors.append(
                f"tranche_mix.{key} is {declared} but item records show {actual}; "
                "counters must derive from item pr_kind records"
            )
    if not any(error.startswith("tranche_mix") for error in errors):
        satisfied.append("tranche_mix counters match item records")


def _validate_goal_candidate(data: dict[str, Any], errors: list[str]) -> None:
    if "goal_candidate" not in data or data.get("goal_candidate") is None:
        return
    candidate = data.get("goal_candidate")
    if not isinstance(candidate, dict):
        errors.append("goal_candidate must be an object when present")
        return
    _require_nonempty_string(candidate, "objective", "goal_candidate", errors)
    _require_nonempty_string(
        candidate,
        "blocked_stop_condition",
        "goal_candidate",
        errors,
    )
    done_when = candidate.get("done_when")
    if not isinstance(done_when, list) or not done_when:
        errors.append("goal_candidate.done_when must be a non-empty list")
    elif any(not isinstance(item, str) or not item.strip() for item in done_when):
        errors.append("goal_candidate.done_when entries must be non-empty strings")
    constraints = candidate.get("constraints")
    if constraints is not None:
        if not isinstance(constraints, list):
            errors.append("goal_candidate.constraints must be a list when present")
        elif any(not isinstance(item, str) or not item.strip() for item in constraints):
            errors.append("goal_candidate.constraints entries must be non-empty strings")
