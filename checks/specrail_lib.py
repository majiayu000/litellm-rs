"""Shared deterministic helpers for SpecRail checks.

This module intentionally avoids third-party dependencies so the default pack
can validate in a fresh repository checkout.
"""

from __future__ import annotations

import json
import re
import hashlib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any

SCHEMA_ANNOTATION_KEYS = {"$id", "$schema", "description", "title"}
SUPPORTED_SCHEMA_KEYS = SCHEMA_ANNOTATION_KEYS | {
    "additionalProperties",
    "const",
    "enum",
    "exclusiveMaximum",
    "exclusiveMinimum",
    "items",
    "minItems",
    "minLength",
    "minimum",
    "properties",
    "required",
    "type",
}
DECISIONS = {"allowed", "warn", "needs_human", "blocked"}
SPEC_STATUSES = frozenset(
    {
        "complete",
        "needs_tasks",
        "needs_spec",
        "umbrella_covered",
        "exception_allowed",
    }
)
RUNTIME_ONLY_STATE = "runtime_only"
RUNTIME_STATE_MAPPING = {
    "blocked": RUNTIME_ONLY_STATE,
    "closed": RUNTIME_ONLY_STATE,
    "complete": RUNTIME_ONLY_STATE,
    "deferred": RUNTIME_ONLY_STATE,
    "eligible_impl": ("ready_to_implement",),
    "handoff": RUNTIME_ONLY_STATE,
    "merge_ready": ("merge_ready",),
    "merged": ("merged",),
    "needs_ci": ("human_review",),
    "needs_human": RUNTIME_ONLY_STATE,
    "needs_review": ("impl_pr_open", "agent_review"),
    "needs_spec": ("ready_to_spec",),
    "needs_tasks": ("spec_approved",),
    "open": RUNTIME_ONLY_STATE,
    "planning": RUNTIME_ONLY_STATE,
    "ready_to_merge": ("merge_ready",),
    "review_required": ("human_review",),
    "running": RUNTIME_ONLY_STATE,
    "waiting_ci": ("human_review", "ci_green"),
}
TERMINAL_BLOCKING_STATES = {
    "abandoned",
    "duplicate",
    "reserved_internal",
    "security_private",
}


class SpecRailError(ValueError):
    """Raised when SpecRail configuration or evidence is malformed."""


@dataclass(frozen=True)
class PackConfig:
    repo: Path
    workflow: dict[str, Any]
    states: dict[str, Any]
    labels: dict[str, Any]


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as exc:
        raise SpecRailError(f"cannot read {path}: {exc}") from exc


def parse_scalar(value: str) -> Any:
    value = value.strip()
    if value in {"true", "True"}:
        return True
    if value in {"false", "False"}:
        return False
    if value in {"null", "None", "~"}:
        return None
    if value == "[]":
        return []
    if value.startswith("[") and value.endswith("]"):
        inner = value[1:-1].strip()
        if not inner:
            return []
        return [parse_scalar(item.strip()) for item in inner.split(",")]
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        return value[1:-1]
    if re.fullmatch(r"-?[0-9]+", value):
        return int(value)
    return value


def _significant_lines(text: str) -> list[tuple[int, str]]:
    lines: list[tuple[int, str]] = []
    for raw in text.splitlines():
        if "\t" in raw:
            raise SpecRailError("tabs are not supported in SpecRail YAML")
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        indent = len(raw) - len(raw.lstrip(" "))
        if indent % 2 != 0:
            raise SpecRailError(f"indent must use multiples of two spaces near: {raw.strip()}")
        lines.append((indent, raw.strip()))
    return lines


def parse_yaml_subset(text: str) -> Any:
    """Parse the small YAML subset used by SpecRail config files.

    Supported constructs: nested mappings, lists of scalars, booleans, nulls,
    and integers. This is not a general YAML parser.
    """

    lines = _significant_lines(text)

    def parse_block(index: int, indent: int) -> tuple[Any, int]:
        if index >= len(lines):
            return {}, index
        container: Any = [] if lines[index][1].startswith("- ") else {}

        while index < len(lines):
            line_indent, content = lines[index]
            if line_indent < indent:
                break
            if line_indent > indent:
                raise SpecRailError(f"unexpected indent near: {content}")

            if isinstance(container, list):
                if not content.startswith("- "):
                    break
                item = content[2:].strip()
                if not item:
                    child, index = parse_block(index + 1, indent + 2)
                    container.append(child)
                else:
                    if re.match(r"[^'\"\s]+:\s+", item) and not (
                        len(item) >= 2 and item[0] == item[-1] and item[0] in {"'", '"'}
                    ):
                        raise SpecRailError(f"unsupported list mapping near: {content}")
                    container.append(parse_scalar(item))
                    index += 1
                continue

            if content.startswith("- "):
                break
            key, sep, value = content.partition(":")
            if not sep:
                raise SpecRailError(f"expected key/value near: {content}")
            key = key.strip()
            value = value.strip()
            if key in container:
                raise SpecRailError(f"duplicate key near: {content}")
            if value:
                if value.startswith("{") and value.endswith("}"):
                    raise SpecRailError(f"inline mappings are not supported near: {content}")
                container[key] = parse_scalar(value)
                index += 1
                continue
            if index + 1 < len(lines) and lines[index + 1][0] > line_indent:
                child, index = parse_block(index + 1, lines[index + 1][0])
                container[key] = child
            else:
                container[key] = {}
                index += 1
        return container, index

    parsed, end = parse_block(0, lines[0][0] if lines else 0)
    if end != len(lines):
        raise SpecRailError(f"could not parse YAML near: {lines[end][1]}")
    return parsed


def load_yaml_file(path: Path) -> Any:
    return parse_yaml_subset(read_text(path))


def _json_type_matches(data: Any, expected_type: str) -> bool:
    if expected_type == "object":
        return isinstance(data, dict)
    if expected_type == "array":
        return isinstance(data, list)
    if expected_type == "string":
        return isinstance(data, str)
    if expected_type == "integer":
        return isinstance(data, int) and not isinstance(data, bool)
    if expected_type == "number":
        return isinstance(data, (int, float)) and not isinstance(data, bool)
    if expected_type == "boolean":
        return isinstance(data, bool)
    if expected_type == "null":
        return data is None
    raise SpecRailError(f"unsupported JSON Schema type {expected_type!r}")


def _schema_path(path: str, key: str) -> str:
    return f"{path}.{key}" if path else key


def _data_path(path: str, key: str) -> str:
    return f"{path}.{key}" if path else key


def validate_instance(schema: dict[str, Any], data: Any, path: str = "$") -> None:
    """Validate data against the JSON Schema subset used by SpecRail.

    This intentionally implements only the local schema subset. If a schema
    starts using a new keyword, validation fails until this checker is extended.
    """

    unsupported = sorted(set(schema) - SUPPORTED_SCHEMA_KEYS)
    if unsupported:
        raise SpecRailError(
            f"{path}: unsupported JSON Schema keyword {unsupported[0]!r}"
        )

    if "type" in schema:
        expected = schema["type"]
        expected_types = expected if isinstance(expected, list) else [expected]
        if not all(isinstance(item, str) for item in expected_types):
            raise SpecRailError(f"{path}: type must be a string or list of strings")
        if not any(_json_type_matches(data, item) for item in expected_types):
            joined = ", ".join(expected_types)
            raise SpecRailError(f"{path}: expected type {joined}")

    if "const" in schema and data != schema["const"]:
        raise SpecRailError(f"{path}: expected const {schema['const']!r}")

    if "enum" in schema:
        enum = schema["enum"]
        if not isinstance(enum, list):
            raise SpecRailError(f"{path}: enum must be a list")
        if data not in enum:
            raise SpecRailError(f"{path}: value {data!r} is not in enum")

    if "minLength" in schema:
        if not isinstance(data, str):
            raise SpecRailError(f"{path}: minLength requires a string instance")
        if len(data) < int(schema["minLength"]):
            raise SpecRailError(f"{path}: string is shorter than minLength")

    if "minItems" in schema:
        if not isinstance(data, list):
            raise SpecRailError(f"{path}: minItems requires an array instance")
        if len(data) < int(schema["minItems"]):
            raise SpecRailError(f"{path}: array is shorter than minItems")

    if "minimum" in schema:
        if not _json_type_matches(data, "number"):
            raise SpecRailError(f"{path}: minimum requires a number instance")
        if data < schema["minimum"]:
            raise SpecRailError(f"{path}: value is below minimum")

    if "exclusiveMinimum" in schema:
        if not _json_type_matches(data, "number"):
            raise SpecRailError(f"{path}: exclusiveMinimum requires a number instance")
        if data <= schema["exclusiveMinimum"]:
            raise SpecRailError(f"{path}: value is not above exclusiveMinimum")

    if "exclusiveMaximum" in schema:
        if not _json_type_matches(data, "number"):
            raise SpecRailError(f"{path}: exclusiveMaximum requires a number instance")
        if data >= schema["exclusiveMaximum"]:
            raise SpecRailError(f"{path}: value is not below exclusiveMaximum")

    if "required" in schema:
        if not isinstance(data, dict):
            raise SpecRailError(f"{path}: required fields need an object instance")
        required = schema["required"]
        if not isinstance(required, list) or not all(isinstance(item, str) for item in required):
            raise SpecRailError(f"{path}: required must be a list of strings")
        for key in required:
            if key not in data:
                raise SpecRailError(f"{_data_path(path, key)}: missing required field")

    properties = schema.get("properties", {})
    if properties is not None and not isinstance(properties, dict):
        raise SpecRailError(f"{path}: properties must be an object")
    if isinstance(data, dict) and isinstance(properties, dict):
        for key, child_schema in properties.items():
            if key not in data:
                continue
            if not isinstance(child_schema, dict):
                raise SpecRailError(f"{_schema_path(path, key)}: property schema must be an object")
            validate_instance(child_schema, data[key], _data_path(path, key))

        additional = schema.get("additionalProperties", True)
        if additional is False:
            extra_keys = sorted(set(data) - set(properties))
            if extra_keys:
                raise SpecRailError(
                    f"{_data_path(path, extra_keys[0])}: additional property is not allowed"
                )
        elif isinstance(additional, dict):
            for key in sorted(set(data) - set(properties)):
                validate_instance(additional, data[key], _data_path(path, key))
        elif additional is not True:
            raise SpecRailError(f"{path}: additionalProperties must be boolean or object")

    if "items" in schema:
        if not isinstance(data, list):
            raise SpecRailError(f"{path}: items requires an array instance")
        item_schema = schema["items"]
        if not isinstance(item_schema, dict):
            raise SpecRailError(f"{path}: items must be an object")
        for index, item in enumerate(data):
            validate_instance(item_schema, item, f"{path}[{index}]")


def load_pack(repo: Path) -> PackConfig:
    repo = repo.resolve()
    return PackConfig(
        repo=repo,
        workflow=load_yaml_file(repo / "workflow.yaml"),
        states=load_yaml_file(repo / "states.yaml"),
        labels=load_yaml_file(repo / "labels.yaml"),
    )


def state_map(config: PackConfig) -> dict[str, Any]:
    states = config.states.get("states")
    if not isinstance(states, dict):
        raise SpecRailError("states.yaml must contain a states mapping")
    return states


def label_groups(config: PackConfig) -> dict[str, list[str]]:
    labels = config.labels.get("labels")
    if not isinstance(labels, dict):
        raise SpecRailError("labels.yaml must contain a labels mapping")
    groups: dict[str, list[str]] = {}
    for group, values in labels.items():
        groups[group] = [str(value) for value in values] if isinstance(values, list) else []
    return groups


def action_policy(config: PackConfig) -> dict[str, Any]:
    policy = config.workflow.get("action_policy", {})
    actions = policy.get("actions", {}) if isinstance(policy, dict) else {}
    if not isinstance(actions, dict):
        raise SpecRailError("workflow.yaml action_policy.actions must be a mapping")
    return actions


def artifact_templates(config: PackConfig) -> dict[str, str]:
    artifacts = config.workflow.get("artifacts", {})
    if not isinstance(artifacts, dict):
        raise SpecRailError("workflow.yaml artifacts must be a mapping")
    return {str(key): str(value) for key, value in artifacts.items()}


def work_id_for_issue(issue: int | None) -> str | None:
    if issue is None:
        return None
    return f"GH{issue}"


def render_artifact_path(config: PackConfig, artifact: str, issue: int | None) -> str | None:
    template = artifact_templates(config).get(artifact)
    if not template:
        return None
    if issue is None:
        return template
    return (
        template.replace("{issue_number}", str(issue))
        .replace("{work_id}", work_id_for_issue(issue) or "")
    )


def validated_artifact_path(
    config: PackConfig,
    artifact: str,
    issue: int,
) -> PurePosixPath:
    rendered = render_artifact_path(config, artifact, issue)
    label = f"workflow.yaml: artifacts.{artifact}"
    if not rendered:
        raise SpecRailError(f"{label} is required")
    return validated_repo_relative_path(rendered, label=label)


def validated_repo_relative_path(raw: str, *, label: str) -> PurePosixPath:
    if "\\" in raw:
        raise SpecRailError(f"{label} must use repo-relative POSIX paths")
    candidate = PurePosixPath(raw.rstrip("/"))
    windows_candidate = PureWindowsPath(raw.rstrip("/"))
    has_windows_drive_component = any(":" in part for part in candidate.parts)
    if (
        candidate.is_absolute()
        or windows_candidate.drive
        or has_windows_drive_component
        or ".." in candidate.parts
    ):
        raise SpecRailError(f"{label} must stay within the repository")
    if "{" in raw or "}" in raw:
        raise SpecRailError(f"{label} contains an unsupported placeholder")
    return candidate


def spec_packet_root(config: PackConfig) -> PurePosixPath:
    template = artifact_templates(config).get("spec_packet")
    if not template:
        raise SpecRailError("workflow.yaml: artifacts.spec_packet is required")
    if "{issue_number}" not in template and "{work_id}" not in template:
        raise SpecRailError(
            "workflow.yaml: artifacts.spec_packet must contain "
            "{issue_number} or {work_id}"
        )
    first = validated_artifact_path(config, "spec_packet", 1)
    second = validated_artifact_path(config, "spec_packet", 2)
    if first.name != "GH1" or second.name != "GH2":
        raise SpecRailError(
            "workflow.yaml: artifacts.spec_packet must render its final directory "
            "as GH<number>"
        )
    if first.parent != second.parent:
        raise SpecRailError(
            "workflow.yaml: artifacts.spec_packet parent directory must not depend "
            "on the issue number"
        )
    return first.parent


def _spec_packet_artifact_paths(config: PackConfig, issue: int) -> dict[str, str]:
    packet = validated_artifact_path(config, "spec_packet", issue)
    paths = {"spec_packet": packet.as_posix()}
    expected_files = {
        "product_spec": "product.md",
        "tech_spec": "tech.md",
        "task_plan": "tasks.md",
    }
    for artifact, filename in expected_files.items():
        path = validated_artifact_path(config, artifact, issue)
        expected = packet / filename
        if path != expected:
            raise SpecRailError(
                f"workflow.yaml: artifacts.{artifact} must render inside "
                f"artifacts.spec_packet as {filename}"
            )
        paths[artifact] = path.as_posix()
    return paths


def spec_packet_artifact_paths(
    config: PackConfig,
    issue: int,
    *,
    repo: Path | None = None,
) -> dict[str, str]:
    configured_root = spec_packet_root(config)
    probe_paths = {
        probe_issue: _spec_packet_artifact_paths(config, probe_issue)
        for probe_issue in (1, 2)
    }
    paths = probe_paths.get(issue) or _spec_packet_artifact_paths(config, issue)
    if repo is not None:
        _validate_resolved_spec_packet_paths(repo, configured_root, paths)
    return paths


def resolve_path(path: Path, *, label: str) -> Path:
    missing_parts: list[str] = []
    try:
        candidate = path.absolute()
        while True:
            try:
                candidate.lstat()
            except FileNotFoundError:
                parent = candidate.parent
                if parent == candidate:
                    raise
                missing_parts.append(candidate.name)
                candidate = parent
                continue
            break
        resolved_path = candidate.resolve(strict=True)
    except (OSError, RuntimeError) as exc:
        raise SpecRailError(f"{label} could not be resolved: {exc}") from exc
    return resolved_path.joinpath(*reversed(missing_parts))


def resolve_repo_path(repo: Path, raw: str | PurePosixPath, *, label: str) -> Path:
    relative_path = validated_repo_relative_path(str(raw), label=label)
    resolved_repo = resolve_path(repo, label="repository")
    resolved_path = resolve_path(
        resolved_repo.joinpath(*relative_path.parts),
        label=label,
    )
    try:
        resolved_path.relative_to(resolved_repo)
    except ValueError as exc:
        raise SpecRailError(f"{label} resolves outside the repository") from exc
    return resolved_path


def resolve_spec_packet_root(repo: Path, configured_root: PurePosixPath) -> Path:
    resolved_repo = resolve_path(repo, label="repository")
    resolved_root = resolve_repo_path(
        repo,
        configured_root,
        label="workflow.yaml: configured spec packet root",
    )
    expected_root = resolved_repo.joinpath(*configured_root.parts)
    if resolved_root != expected_root:
        raise SpecRailError(
            "workflow.yaml: configured spec packet root must preserve its "
            "configured identity after resolution"
        )
    return resolved_root


def _validate_resolved_spec_packet_paths(
    repo: Path,
    configured_root: PurePosixPath,
    paths: dict[str, str],
) -> None:
    resolved_root = resolve_spec_packet_root(repo, configured_root)
    resolved_packet = resolve_repo_path(
        repo,
        paths["spec_packet"],
        label="workflow.yaml: artifacts.spec_packet",
    )
    try:
        resolved_packet.relative_to(resolved_root)
    except ValueError as exc:
        raise SpecRailError(
            "workflow.yaml: artifacts.spec_packet resolves outside the "
            "configured spec packet root"
        ) from exc

    expected_packet = resolved_root / PurePosixPath(paths["spec_packet"]).name
    if resolved_packet != expected_packet:
        raise SpecRailError(
            "workflow.yaml: artifacts.spec_packet does not preserve its "
            "configured packet identity after resolution"
        )

    expected_files = {
        "product_spec": "product.md",
        "tech_spec": "tech.md",
        "task_plan": "tasks.md",
    }
    for artifact, filename in expected_files.items():
        resolved_artifact = resolve_repo_path(
            repo,
            paths[artifact],
            label=f"workflow.yaml: artifacts.{artifact}",
        )
        try:
            resolved_artifact.relative_to(resolved_packet)
        except ValueError as exc:
            raise SpecRailError(
                f"workflow.yaml: artifacts.{artifact} resolves outside the "
                "configured spec packet"
            ) from exc
        if resolved_artifact != resolved_packet / filename:
            raise SpecRailError(
                f"workflow.yaml: artifacts.{artifact} does not preserve its "
                "configured artifact identity after resolution"
            )


def infer_state(config: PackConfig, state: str | None, labels: list[str]) -> tuple[str | None, list[str]]:
    if state:
        return state, [f"state provided explicitly: {state}"]

    known_states = set(state_map(config))
    label_set = {label.strip() for label in labels if label.strip()}
    matches = sorted(label_set & known_states)
    if len(matches) == 1:
        return matches[0], [f"state inferred from label: {matches[0]}"]
    if len(matches) > 1:
        raise SpecRailError(f"conflicting state labels: {', '.join(matches)}")
    return None, []


def validate_state_graph(config: PackConfig) -> list[str]:
    errors: list[str] = []
    states = state_map(config)
    for name, body in states.items():
        if not isinstance(body, dict):
            errors.append(f"states.yaml: state {name} must be a mapping")
            continue
        if "owner" not in body:
            errors.append(f"states.yaml: state {name} missing owner")
        next_states = body.get("next", [])
        if body.get("terminal") is True and next_states:
            errors.append(f"states.yaml: terminal state {name} must not define next")
        if next_states and not isinstance(next_states, list):
            errors.append(f"states.yaml: state {name} next must be a list")
            continue
        for next_state in next_states:
            if str(next_state) not in states:
                errors.append(f"states.yaml: state {name} references unknown next state {next_state}")
    return errors


def validate_labels(config: PackConfig) -> list[str]:
    errors: list[str] = []
    states = set(state_map(config))
    groups = label_groups(config)
    for required_group in ["readiness", "outcome", "review"]:
        if required_group not in groups:
            errors.append(f"labels.yaml: missing label group {required_group}")
    for state in ["needs_info", "triaged", "ready_to_spec", "ready_to_implement"]:
        if state not in groups.get("readiness", []):
            errors.append(f"labels.yaml: readiness labels missing {state}")
    for label in groups.get("readiness", []) + groups.get("outcome", []):
        if label not in states and label not in {"merged"}:
            errors.append(f"labels.yaml: label {label} is not a known state or allowed outcome")
    return errors


def validate_action_policy(config: PackConfig) -> list[str]:
    errors: list[str] = []
    states = set(state_map(config))
    actions = action_policy(config)
    for route in ["triage_issue", "write_spec", "implement", "review_pr", "fix_ci", "draft_release_note"]:
        if route not in actions:
            errors.append(f"workflow.yaml: action_policy missing route {route}")
    for route, body in actions.items():
        if not isinstance(body, dict):
            errors.append(f"workflow.yaml: action {route} must be a mapping")
            continue
        allowed_from = body.get("allowed_from", [])
        if not isinstance(allowed_from, list):
            errors.append(f"workflow.yaml: action {route} allowed_from must be a list")
            continue
        for state in allowed_from:
            if str(state) not in states:
                errors.append(f"workflow.yaml: action {route} references unknown state {state}")
    return errors


def _frontmatter(text: str) -> dict[str, str] | None:
    if not text.startswith("---\n"):
        return None
    end = text.find("\n---\n", 4)
    if end < 0:
        return None
    raw = text[4:end]
    metadata: dict[str, str] = {}
    for line in raw.splitlines():
        key, sep, value = line.partition(":")
        if not sep:
            return None
        metadata[key.strip()] = value.strip()
    return metadata


def _sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_skills_lock(repo: Path) -> list[str]:
    """Validate repo-distributed SpecRail skills and their lockfile."""

    errors: list[str] = []
    lock_path = repo / "skills-lock.json"
    if not lock_path.is_file():
        return ["missing required file: skills-lock.json"]
    try:
        lock = json.loads(read_text(lock_path))
    except json.JSONDecodeError as exc:
        return [f"skills-lock.json: invalid JSON: {exc.msg}"]
    if not isinstance(lock, dict):
        return ["skills-lock.json: top-level value must be an object"]

    if lock.get("version") != 1:
        errors.append("skills-lock.json: version must be 1")
    if lock.get("algorithm") != "sha256":
        errors.append("skills-lock.json: algorithm must be sha256")

    skills = lock.get("skills")
    if not isinstance(skills, list) or not skills:
        errors.append("skills-lock.json: skills must be a non-empty list")
        return errors

    seen_names: set[str] = set()
    seen_paths: set[str] = set()
    paths: list[str] = []
    for index, item in enumerate(skills, start=1):
        if not isinstance(item, dict):
            errors.append(f"skills-lock.json: skill #{index} must be an object")
            continue
        name = item.get("name")
        rel_path = item.get("path")
        digest = item.get("computedHash")
        if not isinstance(name, str) or not name.strip():
            errors.append(f"skills-lock.json: skill #{index} missing name")
            continue
        if not isinstance(rel_path, str) or not rel_path.strip():
            errors.append(f"skills-lock.json: skill {name} missing path")
            continue
        if name in seen_names:
            errors.append(f"skills-lock.json: duplicate skill name {name}")
        if rel_path in seen_paths:
            errors.append(f"skills-lock.json: duplicate skill path {rel_path}")
        seen_names.add(name)
        seen_paths.add(rel_path)
        paths.append(rel_path)

        path = Path(rel_path)
        if path.is_absolute() or ".." in path.parts:
            errors.append(f"skills-lock.json: skill {name} path must be repo-relative")
            continue
        if path.parts[:1] != ("skills",) or path.name != "SKILL.md":
            errors.append(f"skills-lock.json: skill {name} path must be skills/<name>/SKILL.md")
            continue
        if len(path.parts) != 3 or path.parts[1] != name:
            errors.append(f"skills-lock.json: skill {name} path must match its name")

        skill_path = repo / path
        if not skill_path.is_file():
            errors.append(f"skills-lock.json: skill file does not exist: {rel_path}")
            continue
        text = read_text(skill_path)
        metadata = _frontmatter(text)
        if metadata is None:
            errors.append(f"{rel_path}: missing YAML frontmatter")
        else:
            if set(metadata) != {"name", "description"}:
                errors.append(f"{rel_path}: frontmatter must contain only name and description")
            if metadata.get("name") != name:
                errors.append(f"{rel_path}: frontmatter name must be {name}")
            if not metadata.get("description"):
                errors.append(f"{rel_path}: description must not be empty")
        if not isinstance(digest, str) or not digest.startswith("sha256:"):
            errors.append(f"skills-lock.json: skill {name} computedHash must start with sha256:")
        elif digest.removeprefix("sha256:") != _sha256_file(skill_path):
            errors.append(f"skills-lock.json: skill {name} computedHash mismatch")

    if paths != sorted(paths):
        errors.append("skills-lock.json: skills must be sorted by path")

    skill_files = {
        str(path.relative_to(repo))
        for path in sorted((repo / "skills").glob("specrail-*/SKILL.md"))
    }
    missing_from_lock = sorted(skill_files - seen_paths)
    extra_in_lock = sorted(
        path for path in seen_paths if path.startswith("skills/specrail-") and path not in skill_files
    )
    for rel_path in missing_from_lock:
        errors.append(f"skills-lock.json: missing skill file {rel_path}")
    for rel_path in extra_in_lock:
        errors.append(f"skills-lock.json: locked skill file missing from repo {rel_path}")
    return errors
