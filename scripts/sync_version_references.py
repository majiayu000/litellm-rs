#!/usr/bin/env python3
"""Keep README release pins and CHANGELOG comparison links on the package version.

README.md names one exact release in its container image tag, release links,
archive name and `cargo install --version`; CHANGELOG.md ends with one
comparison link per version. `--from OLD --to NEW` moves both to a new release
(Version Bump runs it after `cargo set-version`); `--check` verifies them
against Cargo.toml and exits 1 when any reference lags behind.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys


REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = r"\d+\.\d+\.\d+"
# Text that precedes a version where README.md pins one exact release. Prose
# such as "removed in 0.8.0" is history and stays as written.
PINNED_CONTEXTS = (
    "ghcr.io/majiayu000/litellm-rs:",  # container image tag
    "litellm-rs/releases/tag/v",  # GitHub Release page
    "litellm-rs/releases/download/v",  # release asset
    "raw.githubusercontent.com/majiayu000/litellm-rs/v",  # file at the release tag
    "crates.io/crates/litellm-rs/",  # versioned crate page
    "rust-litellm-gateway-v",  # release archive name
    "cargo install litellm-rs --version ",
    "[v",  # "[vX.Y.Z release]" link text
    "Install version ",
)
PINNED = re.compile(
    "(?P<context>" + "|".join(map(re.escape, PINNED_CONTEXTS)) + rf")(?P<version>{VERSION})"
)
UNRELEASED_LINK = re.compile(
    rf"^\[Unreleased\]: (?P<base>\S+)/compare/v(?P<version>{VERSION})\.\.\.HEAD$",
    re.MULTILINE,
)


def package_version(root: pathlib.Path = REPO_ROOT) -> str:
    manifest = (root / "Cargo.toml").read_text(encoding="utf-8")
    package = manifest.partition("[package]")[2].partition("\n[")[0]
    version = re.search(r'^version = "([^"]+)"$', package, re.MULTILINE)
    if version is None:
        raise SystemExit("Cargo.toml has no [package] version")
    return version[1]


def sync_readme(text: str, old: str, new: str) -> str:
    """Move every pinned reference to `old` onto `new`."""

    def move(match: re.Match[str]) -> str:
        return match["context"] + new if match["version"] == old else match[0]

    return PINNED.sub(move, text)


def sync_changelog(text: str, old: str, new: str) -> str:
    """Compare [Unreleased] from `new` and link `new` to its comparison with `old`."""

    def move(match: re.Match[str]) -> str:
        if match["version"] != old:
            return match[0]
        base = match["base"]
        return (
            f"[Unreleased]: {base}/compare/v{new}...HEAD\n"
            f"[{new}]: {base}/compare/v{old}...v{new}"
        )

    return UNRELEASED_LINK.sub(move, text, count=1)


def stale_references(readme: str, changelog: str, version: str) -> list[str]:
    """Describe every release reference that does not name `version`."""
    problems = [
        f"README.md:{number}: {match[0]!r} should name {version}"
        for number, line in enumerate(readme.splitlines(), 1)
        for match in PINNED.finditer(line)
        if match["version"] != version
    ]
    unreleased = UNRELEASED_LINK.search(changelog)
    if unreleased is None or unreleased["version"] != version:
        problems.append(f"CHANGELOG.md: the [Unreleased] link should compare v{version}...HEAD")
    if not re.search(rf"^\[{re.escape(version)}\]: \S+$", changelog, re.MULTILINE):
        problems.append(f"CHANGELOG.md: no [{version}] comparison link")
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--from", dest="old", metavar="X.Y.Z", help="release being superseded")
    parser.add_argument("--to", dest="new", metavar="X.Y.Z", help="release being cut")
    parser.add_argument(
        "--check", action="store_true", help="verify against Cargo.toml without writing"
    )
    args = parser.parse_args(argv)
    versions = [args.old, args.new]
    if args.check == any(versions) or any(versions) != all(versions):
        parser.error("pass either --check or both --from and --to")
    for version in filter(None, versions):
        if not re.fullmatch(VERSION, version):
            parser.error(f"{version!r} is not an X.Y.Z version")
    readme_path, changelog_path = REPO_ROOT / "README.md", REPO_ROOT / "CHANGELOG.md"
    readme = readme_path.read_bytes().decode("utf-8")
    changelog = changelog_path.read_bytes().decode("utf-8")

    if args.check:
        version = package_version()
        problems = stale_references(readme, changelog, version)
        for problem in problems:
            print(problem, file=sys.stderr)
        if not problems:
            print(f"release references name {version}")
        return 1 if problems else 0

    readme_path.write_bytes(sync_readme(readme, args.old, args.new).encode("utf-8"))
    changelog_path.write_bytes(sync_changelog(changelog, args.old, args.new).encode("utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
