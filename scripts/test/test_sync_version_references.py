#!/usr/bin/env python3
"""Regression tests for the release references Version Bump keeps in step."""

from __future__ import annotations

import fnmatch
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS = REPO_ROOT / ".github" / "workflows"
SCRIPT = pathlib.Path("scripts") / "sync_version_references.py"
SYNC_STEP = "Sync pinned release references"
DISCOVERY_PATTERN = re.compile(r"unittest discover -s scripts/test -p '([^']+)'")
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import sync_version_references as refs  # noqa: E402


RELEASES = "https://github.com/majiayu000/litellm-rs"
README = f"""\
docker run --rm ghcr.io/majiayu000/litellm-rs:0.10.1 gateway
attached to the [v0.10.1 release]({RELEASES}/releases/tag/v0.10.1)
as `rust-litellm-gateway-v0.10.1-<platform>.tar.gz` or
{RELEASES}/releases/download/v0.10.1/rust-litellm-gateway-v0.10.1-linux-x86_64.tar.gz
cargo install litellm-rs --version 0.10.1 --locked --bin gateway
https://raw.githubusercontent.com/majiayu000/litellm-rs/v0.10.1/config/gateway.dev.yaml.example
Install version 0.10.1 from the [versioned crate](https://crates.io/crates/litellm-rs/0.10.1),
"""
HISTORY = """\
litellm-rs at commit `93f4e090`, `litellm[proxy]==1.103.2`, oha 1.16.0
The `webhooks` feature was removed in 0.8.0; key budgets persist since 0.10.1.
The [0.8 verification record](docs/gateway/release-0.8.0-verification.md) is history.
`ghcr.io/majiayu000/litellm-rs:0.10.10` only shares a prefix with the release.
"""
CHANGELOG = f"""\
## [Unreleased]

## [0.10.1] - 2026-11-07

[Unreleased]: {RELEASES}/compare/v0.10.1...HEAD
[0.10.1]: {RELEASES}/compare/v0.10.0...v0.10.1
[0.10.0]: {RELEASES}/compare/v0.9.0...v0.10.0
"""


def workflow_steps(workflow: str) -> dict[str, str]:
    """Map each named step of a workflow to its `run: |` script."""
    steps: dict[str, str] = {}
    name = None
    lines = (WORKFLOWS / workflow).read_text(encoding="utf-8").splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        index += 1
        if line.startswith("      - name: "):
            name = line.removeprefix("      - name: ")
            steps[name] = ""
        elif name is not None and line == "        run: |":
            start = index
            while index < len(lines) and (
                not lines[index].strip() or lines[index].startswith(" " * 10)
            ):
                index += 1
            steps[name] = "\n".join(body[10:] for body in lines[start:index])
    return steps


class ReleaseReferenceTests(unittest.TestCase):
    def test_bump_moves_every_pinned_readme_reference(self) -> None:
        synced = refs.sync_readme(README, "0.10.1", "0.11.0")
        self.assertEqual(synced, README.replace("0.10.1", "0.11.0"))
        self.assertEqual(synced.count("0.11.0"), 10)

    def test_bump_leaves_history_and_other_versions_alone(self) -> None:
        self.assertEqual(refs.sync_readme(HISTORY, "0.10.1", "0.11.0"), HISTORY)

    def test_bump_moves_the_changelog_comparison_links(self) -> None:
        synced = refs.sync_changelog(CHANGELOG, "0.10.1", "0.11.0")
        self.assertEqual(
            synced.splitlines()[-4:],
            [
                f"[Unreleased]: {RELEASES}/compare/v0.11.0...HEAD",
                f"[0.11.0]: {RELEASES}/compare/v0.10.1...v0.11.0",
                f"[0.10.1]: {RELEASES}/compare/v0.10.0...v0.10.1",
                f"[0.10.0]: {RELEASES}/compare/v0.9.0...v0.10.0",
            ],
        )
        self.assertEqual(refs.sync_changelog(synced, "0.10.1", "0.11.0"), synced)

    def test_check_names_each_reference_left_on_another_version(self) -> None:
        self.assertEqual(refs.stale_references(README, CHANGELOG, "0.10.1"), [])
        problems = refs.stale_references(README, CHANGELOG, "0.11.0")
        self.assertEqual(len(problems), 12, problems)
        self.assertIn(
            "README.md:5: 'cargo install litellm-rs --version 0.10.1' should name 0.11.0", problems
        )
        self.assertIn(
            "CHANGELOG.md: the [Unreleased] link should compare v0.11.0...HEAD", problems
        )
        self.assertIn("CHANGELOG.md: no [0.11.0] comparison link", problems)

    def test_repository_references_name_the_package_version(self) -> None:
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertEqual(refs.stale_references(readme, changelog, refs.package_version()), [])

    def test_version_bump_leaves_no_reference_on_the_previous_version(self) -> None:
        steps = workflow_steps("version-bump.yml")
        names = list(steps)
        self.assertIn(SYNC_STEP, names)
        # `cargo set-version` and the CHANGELOG entry come first; the commit
        # that the release tag points at comes after.
        self.assertLess(names.index("Bump version"), names.index(SYNC_STEP))
        self.assertLess(names.index(SYNC_STEP), names.index("Commit and push version bump"))
        for path in ("README.md", "CHANGELOG.md"):
            self.assertRegex(steps["Commit and push version bump"], rf"git add .*\b{path}\b")

        old = refs.package_version()
        major, minor, _patch = old.split(".")
        new = f"{major}.{int(minor) + 1}.0"
        script = (
            steps[SYNC_STEP]
            .replace("${{ steps.version.outputs.current_version }}", old)
            .replace("${{ steps.version.outputs.new_version }}", new)
        )
        self.assertNotIn("${{", script)
        before = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory() as directory:
            tree = pathlib.Path(directory)
            (tree / SCRIPT).parent.mkdir()
            for path in (SCRIPT, "README.md", "CHANGELOG.md"):
                shutil.copy(REPO_ROOT / path, tree / path)
            manifest = (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
            (tree / "Cargo.toml").write_text(
                manifest.replace(f'version = "{old}"', f'version = "{new}"', 1), encoding="utf-8"
            )
            result = subprocess.run(
                ["bash", "-eo", "pipefail", "-c", script],
                cwd=tree,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            readme = (tree / "README.md").read_text(encoding="utf-8")
            changelog = (tree / "CHANGELOG.md").read_text(encoding="utf-8")

        pinned_before = [match[0] for match in refs.PINNED.finditer(before)]
        self.assertTrue(pinned_before, "README.md no longer pins a release")
        self.assertEqual(
            [match[0] for match in refs.PINNED.finditer(readme)],
            [reference.replace(old, new) for reference in pinned_before],
        )
        self.assertIn(
            f"\n[Unreleased]: {RELEASES}/compare/v{new}...HEAD\n"
            f"[{new}]: {RELEASES}/compare/v{old}...v{new}\n"
            f"[{old}]: ",
            changelog,
        )

    def test_version_bump_stops_before_the_commit_on_a_stale_reference(self) -> None:
        script = workflow_steps("version-bump.yml").get(SYNC_STEP, "")
        self.assertRegex(script, rf"(?m)^python3 {re.escape(str(SCRIPT))} --check$")
        with tempfile.TemporaryDirectory() as directory:
            tree = pathlib.Path(directory)
            (tree / SCRIPT).parent.mkdir()
            shutil.copy(REPO_ROOT / SCRIPT, tree / SCRIPT)
            (tree / "README.md").write_text(README, encoding="utf-8")
            (tree / "CHANGELOG.md").write_text(CHANGELOG, encoding="utf-8")
            (tree / "Cargo.toml").write_text(
                '[package]\nname = "litellm-rs"\nversion = "0.11.0"\n', encoding="utf-8"
            )
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "--check"], cwd=tree, capture_output=True, text=True
            )
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("README.md:1: 'ghcr.io/majiayu000/litellm-rs:0.10.1'", result.stderr)

    def test_ci_discovers_this_test_file(self) -> None:
        name = pathlib.Path(__file__).name
        patterns = DISCOVERY_PATTERN.findall((WORKFLOWS / "ci.yml").read_text(encoding="utf-8"))
        self.assertTrue(
            any(fnmatch.fnmatch(name, pattern) for pattern in patterns),
            f"ci.yml does not run {name}: {patterns}",
        )


if __name__ == "__main__":
    unittest.main()
