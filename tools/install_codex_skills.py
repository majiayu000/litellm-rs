#!/usr/bin/env python3
"""Install repo-distributed SpecRail skills into a local Codex skill directory.

The command is dry-run by default. Use --apply only after a human explicitly
requests local Codex skill installation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "checks"))

from specrail_lib import read_text, validate_skills_lock  # noqa: E402


class InstallError(ValueError):
    """Raised when the local skill install plan is unsafe or invalid."""


@dataclass(frozen=True)
class LockedSkill:
    name: str
    source_dir: Path
    expected_hash: str


def default_codex_skills_dir() -> Path:
    codex_home = os.environ.get("CODEX_HOME")
    base = Path(codex_home).expanduser() if codex_home else Path.home() / ".codex"
    return base / "skills"


def load_locked_skills(repo: Path) -> list[LockedSkill]:
    errors = validate_skills_lock(repo)
    if errors:
        details = "\n".join(f"- {error}" for error in errors)
        raise InstallError(f"invalid skills-lock.json:\n{details}")

    lock = json.loads(read_text(repo / "skills-lock.json"))
    skills: list[LockedSkill] = []
    for item in lock["skills"]:
        rel_path = Path(item["path"])
        skills.append(
            LockedSkill(
                name=item["name"],
                source_dir=repo / rel_path.parent,
                expected_hash=item["computedHash"],
            )
        )
    return skills


def ensure_safe_destination(source_dir: Path, destination_dir: Path) -> None:
    source = source_dir.resolve()
    destination = destination_dir.resolve()
    if source == destination:
        raise InstallError(f"refusing to install over source skill directory: {source}")
    if source in destination.parents:
        raise InstallError(f"refusing to install inside source skill directory: {destination}")
    if destination in source.parents:
        raise InstallError(f"refusing to install into source parent directory: {destination}")


def install_skills(repo: Path, target_dir: Path, apply: bool) -> list[str]:
    repo = repo.resolve()
    target_dir = target_dir.expanduser()
    skills = load_locked_skills(repo)
    messages: list[str] = []

    for skill in skills:
        destination = target_dir / skill.name
        ensure_safe_destination(skill.source_dir, destination)
        messages.append(f"{skill.name}: {skill.source_dir} -> {destination}")
        if not apply:
            continue
        if destination.exists():
            shutil.rmtree(destination)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(skill.source_dir, destination)

    if apply:
        installed_repo = target_dir
        for skill in skills:
            installed_file = installed_repo / skill.name / "SKILL.md"
            if not installed_file.is_file():
                raise InstallError(f"installed skill missing SKILL.md: {installed_file}")
            digest = "sha256:" + hashlib.sha256(installed_file.read_bytes()).hexdigest()
            if digest != skill.expected_hash:
                raise InstallError(
                    f"installed skill hash mismatch for {skill.name}: "
                    f"expected {skill.expected_hash}, got {digest}"
                )
    return messages


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Install SpecRail repo-distributed skills into local Codex skills.",
    )
    parser.add_argument(
        "--repo",
        default=".",
        help="SpecRail repository root. Defaults to current directory.",
    )
    parser.add_argument(
        "--target-dir",
        default=str(default_codex_skills_dir()),
        help="Codex skills directory. Defaults to $CODEX_HOME/skills or ~/.codex/skills.",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Write files. Without this flag the command is a dry-run.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    repo = Path(args.repo).expanduser()
    target_dir = Path(args.target_dir).expanduser()

    try:
        messages = install_skills(repo, target_dir, args.apply)
    except InstallError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    mode = "apply" if args.apply else "dry-run"
    print(f"mode: {mode}")
    print(f"target: {target_dir}")
    for message in messages:
        print(message)
    if args.apply:
        print(f"installed {len(messages)} skills")
    else:
        print("no files written; rerun with --apply to install")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
