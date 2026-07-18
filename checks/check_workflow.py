#!/usr/bin/env python3
"""Validate a SpecRail workflow pack without network or GitHub writes."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from schema_contract import uses_runtime_profile, validate_schema_node
from specrail_lib import (
    SpecRailError,
    load_pack,
    read_text,
    validate_action_policy,
    validate_labels,
    validate_state_graph,
    validate_skills_lock,
)


REQUIRED_FILES = [
    "AGENT_USAGE.md",
    "AGENTS.md",
    "workflow.yaml",
    "states.yaml",
    "labels.yaml",
    "checks/check_workflow.py",
    "checks/schema_contract.py",
    "checks/github_issue_evidence.py",
    "checks/github_pr_evidence.py",
    "checks/pr_gate.py",
    "checks/route_gate.py",
    "checks/review_json_gate.py",
    "checks/specrail_lib.py",
    "tools/install_codex_skills.py",
    "skills-lock.json",
    "templates/issue_bug.md",
    "templates/issue_feature.md",
    "templates/product_spec.md",
    "templates/tech_spec.md",
    "templates/tasks.md",
    "templates/pull_request.md",
    "templates/zh-CN/issue_bug.md",
    "templates/zh-CN/issue_feature.md",
    "templates/zh-CN/product_spec.md",
    "templates/zh-CN/tech_spec.md",
    "templates/zh-CN/tasks.md",
    "templates/zh-CN/pull_request.md",
    "review/agent_first_review.md",
    "review/human_final_review.md",
    "policies/security_disclosure.md",
    "policies/maintainer_escalation.md",
    "schemas/flow_manifest.schema.json",
    "schemas/issue_triage.schema.json",
    "schemas/issue_evidence.schema.json",
    "schemas/evaluation_result.schema.json",
    "schemas/adoption_matrix.schema.json",
    "schemas/spec_packet.schema.json",
    "schemas/task_plan.schema.json",
    "schemas/pr_review_gate.schema.json",
    "schemas/review_result.schema.json",
    "schemas/workflow_run.schema.json",
]

REQUIRED_TOKENS = {
    "workflow.yaml": [
        "default_mode: dry_run",
        "forbidden_agent_actions:",
        "required_human_gates:",
        "action_policy:",
    ],
    "states.yaml": [
        "ready_to_spec",
        "ready_to_implement",
        "agent_review",
        "human_review",
        "merge_ready",
    ],
    "labels.yaml": [
        "readiness:",
        "ready_to_spec",
        "ready_to_implement",
        "security_private",
    ],
    "templates/product_spec.md": [
        "## Goals",
        "## Non-Goals",
        "## Acceptance Criteria",
    ],
    "templates/tech_spec.md": [
        "## Proposed Design",
        "## Test Plan",
        "## Rollback Plan",
    ],
    "templates/tasks.md": [
        "## Implementation Tasks",
        "## Verification",
        "## Handoff Notes",
    ],
    "templates/pull_request.md": [
        "## Linked Work",
        "## Readiness Gate",
        "## Review Gate",
        "## Merge Gate",
        "## Verification",
    ],
}


def validate_required_files(repo: Path) -> list[str]:
    errors: list[str] = []
    for rel in REQUIRED_FILES:
        path = repo / rel
        if not path.is_file():
            errors.append(f"missing required file: {rel}")
    return errors


def validate_tokens(repo: Path) -> list[str]:
    errors: list[str] = []
    for rel, tokens in REQUIRED_TOKENS.items():
        path = repo / rel
        if not path.is_file():
            continue
        text = read_text(path)
        for token in tokens:
            if token not in text:
                errors.append(f"{rel}: missing token {token!r}")
    return errors


def validate_all_json_schemas(repo: Path) -> list[str]:
    """Validate every JSON schema adopted by remem."""

    errors: list[str] = []
    schema_dir = repo / "schemas"
    if not schema_dir.is_dir():
        return ["missing schemas/ directory"]
    for path in sorted(schema_dir.glob("*.schema.json")):
        relative_path = path.relative_to(repo)
        try:
            data = json.loads(read_text(path))
        except json.JSONDecodeError as exc:
            errors.append(f"{relative_path}: invalid JSON: {exc.msg}")
            continue
        if not isinstance(data, dict):
            errors.append(f"{relative_path}: top-level JSON must be an object")
            continue
        if "$schema" not in data:
            errors.append(f"{relative_path}: missing $schema")
        if "title" not in data:
            errors.append(f"{relative_path}: missing title")
        if data.get("type") != "object":
            errors.append(f"{relative_path}: top-level type must be object")
        errors.extend(
            validate_schema_node(
                data,
                relative_path,
                runtime_profile=uses_runtime_profile(path),
            )
        )
    return errors


def validate_template_parity(repo: Path) -> list[str]:
    """Validate the complete base and zh-CN template surface."""

    errors: list[str] = []
    root = repo / "templates"
    zh = root / "zh-CN"
    base_files = sorted(path.name for path in root.glob("*.md"))
    zh_files = sorted(path.name for path in zh.glob("*.md")) if zh.is_dir() else []
    for name in base_files:
        if name not in zh_files:
            errors.append(f"templates/zh-CN: missing localized template {name}")
    for name in zh_files:
        if name not in base_files:
            errors.append(f"templates/zh-CN/{name}: no matching base template")
    stable_tokens = ["GH-", "ready_to_spec", "ready_to_implement"]
    for name in ["issue_feature.md", "product_spec.md", "tech_spec.md", "pull_request.md"]:
        base_path = repo / "templates" / name
        if not base_path.is_file():
            continue
        base_text = read_text(base_path)
        for rel in [Path("templates") / name, Path("templates/zh-CN") / name]:
            path = repo / rel
            if not path.is_file():
                continue
            text = read_text(path)
            for token in stable_tokens:
                if token in base_text and token not in text:
                    errors.append(f"{rel}: missing stable token {token}")
    return errors


def validate_spec_packet(spec_dir: Path) -> list[str]:
    errors: list[str] = []
    if not spec_dir.exists():
        return [f"spec packet does not exist: {spec_dir}"]
    if not spec_dir.is_dir():
        return [f"spec packet is not a directory: {spec_dir}"]

    match = re.fullmatch(r"GH([0-9]+)", spec_dir.name)
    if not match:
        errors.append(f"{spec_dir}: spec packet directory must be named GH<number>")
        issue_number = None
    else:
        issue_number = match.group(1)

    issue_tokens = []
    if issue_number:
        issue_tokens = [f"GH-{issue_number}", f"GH{issue_number}", f"#{issue_number}"]

    for name in ["product.md", "tech.md"]:
        path = spec_dir / name
        if not path.is_file():
            errors.append(f"{spec_dir}: missing {name}")
            continue
        text = read_text(path)
        if not text.strip():
            errors.append(f"{path}: must not be empty")
        if issue_tokens and not any(token in text for token in issue_tokens):
            errors.append(f"{path}: missing linked issue token {' or '.join(issue_tokens)}")

    task_path = spec_dir / "tasks.md"
    if not task_path.is_file():
        errors.append(f"{spec_dir}: missing tasks.md")
    else:
        errors.extend(validate_task_plan(task_path, issue_number))
    return errors


def spec_packet_sort_key(spec_dir: Path) -> tuple[int, int, str]:
    match = re.fullmatch(r"GH([0-9]+)", spec_dir.name)
    if match:
        return (0, int(match.group(1)), spec_dir.name)
    return (1, 0, str(spec_dir))


def discover_spec_packet_dirs(repo: Path) -> list[Path]:
    specs_dir = repo / "specs"
    if not specs_dir.is_dir():
        return []
    return sorted(
        [
            path.resolve()
            for path in specs_dir.iterdir()
            if path.is_dir() and re.fullmatch(r"GH([0-9]+)", path.name)
        ],
        key=spec_packet_sort_key,
    )


def select_spec_packet_dirs(
    repo: Path,
    raw_spec_dirs: list[str],
    *,
    all_specs: bool,
) -> list[Path]:
    spec_dirs: list[Path] = []
    if all_specs:
        spec_dirs.extend(discover_spec_packet_dirs(repo))
    spec_dirs.extend((repo / raw_spec_dir).resolve() for raw_spec_dir in raw_spec_dirs)

    unique_spec_dirs: list[Path] = []
    seen: set[Path] = set()
    for spec_dir in spec_dirs:
        if spec_dir in seen:
            continue
        seen.add(spec_dir)
        unique_spec_dirs.append(spec_dir)

    if all_specs:
        return sorted(unique_spec_dirs, key=spec_packet_sort_key)
    return unique_spec_dirs


def validate_task_plan(path: Path, issue_number: str | None) -> list[str]:
    errors: list[str] = []
    text = read_text(path)
    if not text.strip():
        return [f"{path}: must not be empty"]
    prefix = f"SP{issue_number}-T" if issue_number else "SP"
    ids: list[str] = []
    for line_number, line in enumerate(text.splitlines(), start=1):
        if "- [" not in line:
            continue
        match = re.search(r"`([^`]+)`", line)
        if not match:
            errors.append(f"{path}:{line_number}: task is missing stable ID")
            continue
        task_id = match.group(1)
        ids.append(task_id)
        if issue_number and not task_id.startswith(prefix):
            errors.append(f"{path}:{line_number}: task ID {task_id} must start with {prefix}")
        for token in ["Owner:", "Done when:", "Verify:"]:
            if token not in line:
                errors.append(f"{path}:{line_number}: task {task_id} missing {token}")
    if not ids:
        errors.append(f"{path}: no task checklist items found")
    duplicates = sorted({task_id for task_id in ids if ids.count(task_id) > 1})
    for duplicate in duplicates:
        errors.append(f"{path}: duplicate task ID {duplicate}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate a SpecRail workflow pack."
    )
    parser.add_argument("--repo", default=".", help="Workflow pack root")
    parser.add_argument(
        "--spec-dir",
        action="append",
        default=[],
        help="Optional specs/GH<number> directory to validate",
    )
    parser.add_argument(
        "--all-specs",
        action="store_true",
        help="Validate every specs/GH<number> directory under the repo",
    )
    args = parser.parse_args()

    repo = Path(args.repo).resolve()
    errors: list[str] = []
    try:
        config = load_pack(repo)
        errors.extend(validate_required_files(repo))
        errors.extend(validate_tokens(repo))
        errors.extend(validate_all_json_schemas(repo))
        errors.extend(validate_state_graph(config))
        errors.extend(validate_labels(config))
        errors.extend(validate_action_policy(config))
        errors.extend(validate_skills_lock(repo))
        errors.extend(validate_template_parity(repo))
        for spec_dir in select_spec_packet_dirs(
            repo,
            args.spec_dir,
            all_specs=args.all_specs,
        ):
            errors.extend(validate_spec_packet(spec_dir))
    except SpecRailError as exc:
        errors.append(str(exc))

    if errors:
        print("SpecRail check failed")
        for error in errors:
            print(f"- {error}")
        return 1

    print("SpecRail check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
